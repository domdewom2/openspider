//! Lese-Pool: alle Datenbankzugriffe der App laufen hier oder in `spawn_blocking`, nie auf dem UI-Thread oder einem
//! Tokio-Worker (`docs/plan/M2.md` 3.6, M2-ui-concept 2.3 Regel 1).
//!
//! Je geöffnetem Crawl hält der Pool bis zu [`DEFAULT_READ_CONNECTIONS`] schreibgeschützte Verbindungen. Jede
//! Verbindung bekommt beim Öffnen die Kollation `OS_TEXT` (`collation::ensure`). Eine Arbeit läuft auf einem
//! Blocking-Thread, ihre Marke ist an das Interrupt-Handle der Verbindung gekoppelt: `op.cancel` unterbricht die
//! laufende Anweisung binnen 100 ms (UI-141). Die Marke stammt aus dem Aufrufkontext (`CallCtx.cancel`, bei
//! Operationen die der Operation), siehe [`ReadPool::run`] und [`ReadPool::run_with`].
//!
//! Dies ist neben `write_queue` die einzige Datei von `os-app`, in der `rusqlite`/`CrawlFile` direkt benannt werden
//! dürfen (`clippy.toml`: `disallowed-types`/`disallowed-methods`).
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use os_analyze::analysis::CancelToken;
use os_store::{CrawlFile, OpenMode, StoreError};
use tokio::sync::{OwnedRwLockWriteGuard, RwLock, Semaphore};

use crate::dto::{CmdError, ErrorCode};
use crate::runtime::error::{internal, no_crawl, panic_text, store_error};
use crate::runtime::interrupt::InterruptHub;

/// Standardzahl gleichzeitiger Leseverbindungen je Crawl.
pub const DEFAULT_READ_CONNECTIONS: usize = 4;

/// Schlüssel eines geöffneten Crawls (Kennung als String, UUID v7).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrawlKey(pub String);

struct Slot {
    path: PathBuf,
    idle: Mutex<Vec<CrawlFile>>,
    permits: Arc<Semaphore>,
    access: Arc<RwLock<()>>,
}

struct Inner {
    crawls: Mutex<HashMap<CrawlKey, Arc<Slot>>>,
    hub: Arc<InterruptHub>,
    capacity: usize,
}

/// Pool schreibgeschützter Verbindungen je Crawl.
#[derive(Clone)]
pub struct ReadPool {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ReadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadPool")
            .field("crawls", &self.inner.lock().len())
            .finish()
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<CrawlKey, Arc<Slot>>> {
        self.crawls.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl ReadPool {
    /// Neuer Pool; `hub` koppelt Abbruchmarken an die Verbindungen.
    pub fn new(hub: Arc<InterruptHub>, capacity: usize) -> ReadPool {
        ReadPool {
            inner: Arc::new(Inner {
                crawls: Mutex::new(HashMap::new()),
                hub,
                capacity: capacity.max(1),
            }),
        }
    }

    /// Sperrt neue Leser und wartet bestehende Leser ab. Unbekannte Schlüssel liefern einen leeren Wächter.
    pub async fn exclusive(&self, key: &CrawlKey) -> ExclusiveGuard {
        let slot = self.inner.lock().get(key).cloned();
        let guard = match slot {
            Some(slot) => {
                let guard = slot.access.clone().write_owned().await;
                slot.idle.lock().unwrap_or_else(|p| p.into_inner()).clear();
                Some(guard)
            }
            None => None,
        };
        ExclusiveGuard { _guard: guard }
    }

    /// Prüft das Schema vor der Registrierung. Alte Dateien werden als eigenständige Kopie migriert.
    pub async fn prepare_open(&self, key: CrawlKey, path: PathBuf) -> Result<OpenReport, CmdError> {
        let report =
            tokio::task::spawn_blocking(move || match CrawlFile::open(&path, OpenMode::ReadOnly) {
                Ok(_) => Ok(OpenReport {
                    path,
                    migrated: false,
                    original: None,
                }),
                Err(StoreError::MigrationRequired { .. }) => {
                    let name = path.file_stem().unwrap_or_default().to_string_lossy();
                    let dest = path.with_file_name(format!("{name}.migrated.ospider"));
                    CrawlFile::migrate_copy(&path, &dest).map_err(|e| store_error(&e))?;
                    Ok(OpenReport {
                        path: dest,
                        migrated: true,
                        original: Some(path),
                    })
                }
                Err(e) => Err(store_error(&e)),
            })
            .await
            .map_err(|e| internal("ReadPool::prepare_open", e.to_string()))??;
        self.register(key, report.path.clone());
        Ok(report)
    }

    /// Macht eine Crawl-Datei unter `key` lesbar. Ein vorhandener Eintrag mit demselben Schlüssel wird ersetzt
    /// (leerlaufende Verbindungen werden geschlossen, laufende Arbeiten beenden ihre Verbindung).
    pub fn register(&self, key: CrawlKey, path: impl Into<PathBuf>) {
        let slot = Arc::new(Slot {
            path: path.into(),
            idle: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(self.inner.capacity)),
            access: Arc::new(RwLock::new(())),
        });
        self.inner.lock().insert(key, slot);
    }

    /// Entfernt den Crawl; laufende Arbeiten dürfen zu Ende laufen.
    pub fn unregister(&self, key: &CrawlKey) -> bool {
        self.inner.lock().remove(key).is_some()
    }

    /// Ist der Schlüssel registriert?
    pub fn is_registered(&self, key: &CrawlKey) -> bool {
        self.inner.lock().contains_key(key)
    }

    /// Pfad der Datei zu einem Schlüssel.
    pub fn path_of(&self, key: &CrawlKey) -> Option<PathBuf> {
        self.inner.lock().get(key).map(|s| s.path.clone())
    }

    /// Anzahl der zurzeit offenen (leerlaufenden) Leseverbindungen zu einem Crawl.
    pub fn idle_connections(&self, key: &CrawlKey) -> usize {
        self.inner.lock().get(key).map_or(0, |s| {
            s.idle.lock().unwrap_or_else(|p| p.into_inner()).len()
        })
    }

    /// Führt `f` auf einer Leseverbindung des Crawls aus (Thread-Pool, abbrechbar über die Marke des laufenden
    /// Aufrufs bzw. der laufenden Operation; ohne Kontext eine neue, nie abgebrochene Marke).
    pub async fn run<T: Send + 'static>(
        &self,
        crawl: CrawlKey,
        f: impl FnOnce(&CrawlFile, &CancelToken) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, CmdError> {
        let token = crate::runtime::current_cancel();
        self.run_with(crawl, token, f).await
    }

    /// Wie [`run`](Self::run) mit ausdrücklicher Marke.
    pub async fn run_with<T: Send + 'static>(
        &self,
        crawl: CrawlKey,
        token: CancelToken,
        f: impl FnOnce(&CrawlFile, &CancelToken) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, CmdError> {
        let slot = self
            .inner
            .lock()
            .get(&crawl)
            .cloned()
            .ok_or_else(|| no_crawl(&crawl.0))?;
        if token.is_cancelled() {
            return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
        }
        let access = slot.access.clone().read_owned().await;
        let permit = Arc::clone(&slot.permits)
            .acquire_owned()
            .await
            .map_err(|_| internal("ReadPool::run", "Semaphore geschlossen"))?;
        let hub = Arc::clone(&self.inner.hub);
        let joined = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _access = access;
            run_blocking(&slot, &hub, &token, f)
        })
        .await;
        match joined {
            Ok(r) => r,
            Err(e) => Err(internal("ReadPool::run", e.to_string())),
        }
    }
}

fn checkout(slot: &Slot) -> Result<CrawlFile, StoreError> {
    if let Some(file) = slot.idle.lock().unwrap_or_else(|p| p.into_inner()).pop() {
        return Ok(file);
    }
    open_read(&slot.path)
}

fn open_read(path: &Path) -> Result<CrawlFile, StoreError> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly)?;
    // je Verbindung (M2.md 3.6); `open` registriert sie bereits, die Wiederholung ist idempotent
    os_store::collation::ensure(file.conn())?;
    Ok(file)
}

fn run_blocking<T>(
    slot: &Slot,
    hub: &InterruptHub,
    token: &CancelToken,
    f: impl FnOnce(&CrawlFile, &CancelToken) -> Result<T, StoreError>,
) -> Result<T, CmdError> {
    let file = checkout(slot).map_err(|e| store_error(&e))?;
    let outcome = {
        let _registration = hub.register(token, file.conn().get_interrupt_handle());
        if token.is_cancelled() {
            Ok(Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled")))
        } else {
            std::panic::catch_unwind(AssertUnwindSafe(|| {
                f(&file, token).map_err(|e| store_error(&e))
            }))
        }
    };
    match outcome {
        Ok(result) => {
            // die Verbindung bleibt nach Fehlern und Interrupts benutzbar
            slot.idle
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(file);
            result
        }
        Err(panic) => Err(internal("ReadPool::run", panic_text(panic.as_ref()))),
    }
}

/// Exklusiver Zugriff; Freigabe beim Verwerfen, auch nach Abbruch.
pub struct ExclusiveGuard {
    _guard: Option<OwnedRwLockWriteGuard<()>>,
}
/// Ergebnis der Versionsprüfung; das Original bleibt bei Migration erhalten.
#[derive(Debug, Clone)]
pub struct OpenReport {
    pub path: PathBuf,
    pub migrated: bool,
    pub original: Option<PathBuf>,
}
