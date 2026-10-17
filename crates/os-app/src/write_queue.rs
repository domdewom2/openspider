//! Serialisierte Schreibwarteschlange der App (K-20): eigener Thread, eigene Verbindung, `busy_timeout` 5 s.
//!
//! Alle Schreibzugriffe der App neben dem Engine-Writer (`config.set`, SERP-Bearbeitungen, Segmente, …) laufen als
//! Aufträge nacheinander auf einem einzigen Thread. Der Thread hält je Crawl eine eigene Lese-/Schreibverbindung
//! (mit `OS_TEXT` und `busy_timeout` 5 s); sperrt der Engine-Writer die Datei kurz, wartet der Auftrag, statt mit
//! `SQLITE_BUSY` zu scheitern. Die Datei wird nie migriert: ein Schema, das nicht dem der App entspricht, ist ein
//! Fehler (die Engine migriert beim Öffnen).
//!
//! Dies ist neben `pool` die einzige Datei von `os-app`, in der `rusqlite` direkt benannt werden darf.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use os_store::schema::{self, APPLICATION_ID, MIGRATIONS};
use os_store::StoreError;
use rusqlite::{Connection, OpenFlags};

use crate::dto::CmdError;
use crate::pool::CrawlKey;
use crate::runtime::error::{internal, no_crawl, panic_text, store_error};

/// Wartezeit auf eine gesperrte Datei, bevor ein Auftrag mit `Busy` scheitert (K-20).
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

type Registry = Arc<Mutex<HashMap<CrawlKey, PathBuf>>>;

enum Msg {
    Job(Box<dyn FnOnce(&mut Worker) + Send>),
    Close,
}

struct Worker {
    paths: Registry,
    conns: HashMap<CrawlKey, Connection>,
    files: HashMap<CrawlKey, os_store::CrawlFile>,
}

impl Worker {
    fn connection(&mut self, key: &CrawlKey) -> Result<&mut Connection, CmdError> {
        if !self.conns.contains_key(key) {
            let path = self
                .paths
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .get(key)
                .cloned()
                .ok_or_else(|| no_crawl(&key.0))?;
            let conn = open_write(&path).map_err(|e| store_error(&e))?;
            self.conns.insert(key.clone(), conn);
        }
        self.conns
            .get_mut(key)
            .ok_or_else(|| internal("WriteQueue", "Verbindung fehlt"))
    }
}

fn open_write(path: &std::path::Path) -> Result<Connection, StoreError> {
    if !path.exists() {
        return Err(StoreError::NotFound(path.to_path_buf()));
    }
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    if schema::application_id(&conn)? != APPLICATION_ID {
        return Err(StoreError::NotACrawlFile {
            path: path.to_path_buf(),
        });
    }
    let found = schema::user_version(&conn)?;
    let latest = schema::latest_version(MIGRATIONS);
    if found != latest {
        // Älter: die Engine migriert beim Öffnen. Neuer: die Datei bleibt unverändert.
        return Err(if found > latest {
            StoreError::NewerVersion {
                path: path.to_path_buf(),
                found,
                supported: latest,
                generator: "einer neueren OpenSpider-Version".to_string(),
            }
        } else {
            StoreError::MigrationRequired {
                path: path.to_path_buf(),
                found,
                supported: latest,
            }
        });
    }
    os_store::collation::ensure(&conn)?;
    Ok(conn)
}

/// Die einzige Schreibstelle der App neben dem Engine-Writer.
pub struct WriteQueue {
    tx: Mutex<Option<mpsc::Sender<Msg>>>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    paths: Registry,
}

impl std::fmt::Debug for WriteQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WriteQueue").finish_non_exhaustive()
    }
}

impl Default for WriteQueue {
    fn default() -> Self {
        WriteQueue::new()
    }
}

impl WriteQueue {
    /// Startet den Schreib-Thread.
    pub fn new() -> WriteQueue {
        let paths: Registry = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = mpsc::channel::<Msg>();
        let mut worker = Worker {
            paths: Arc::clone(&paths),
            conns: HashMap::new(),
            files: HashMap::new(),
        };
        let thread = std::thread::Builder::new()
            .name("os-app-write-queue".into())
            .spawn(move || {
                while let Ok(msg) = rx.recv() {
                    match msg {
                        Msg::Job(job) => job(&mut worker),
                        Msg::Close => break,
                    }
                }
            })
            .ok();
        WriteQueue {
            tx: Mutex::new(Some(tx)),
            thread: Mutex::new(thread),
            paths,
        }
    }

    /// Macht eine Crawl-Datei unter `key` beschreibbar. Eine vorhandene Verbindung zu einem früheren Pfad wird beim
    /// nächsten Auftrag neu geöffnet.
    pub fn register(&self, key: CrawlKey, path: impl Into<PathBuf>) {
        let path = path.into();
        let changed = {
            let mut paths = self.paths.lock().unwrap_or_else(|p| p.into_inner());
            paths
                .insert(key.clone(), path.clone())
                .is_none_or(|old| old != path)
        };
        if changed {
            self.drop_connection(key);
        }
    }

    /// Entfernt den Crawl und schließt die Verbindung (nach den bereits eingereihten Aufträgen).
    pub fn unregister(&self, key: &CrawlKey) {
        self.paths
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(key);
        self.drop_connection(key.clone());
    }

    fn drop_connection(&self, key: CrawlKey) {
        self.send(Msg::Job(Box::new(move |w: &mut Worker| {
            w.conns.remove(&key);
            w.files.remove(&key);
        })));
    }

    fn send(&self, msg: Msg) -> bool {
        let guard = self.tx.lock().unwrap_or_else(|p| p.into_inner());
        guard.as_ref().is_some_and(|tx| tx.send(msg).is_ok())
    }

    /// Führt `f` mit der Schreibverbindung des Crawls aus. Aufträge laufen strikt nacheinander in der Reihenfolge des
    /// Einreihens.
    pub async fn submit<T: Send + 'static>(
        &self,
        crawl: CrawlKey,
        f: impl FnOnce(&mut rusqlite::Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, CmdError> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Result<T, CmdError>>();
        let job = Box::new(move |w: &mut Worker| {
            let result = match w.connection(&crawl) {
                Err(e) => Err(e),
                Ok(conn) => {
                    match std::panic::catch_unwind(AssertUnwindSafe(|| {
                        f(conn).map_err(|e| store_error(&e))
                    })) {
                        Ok(r) => r,
                        Err(p) => {
                            // nach einem Panic mitten in einer Transaktion ist die Verbindung unklar
                            w.conns.remove(&crawl);
                            Err(internal("WriteQueue::submit", panic_text(p.as_ref())))
                        }
                    }
                }
            };
            let _ = done_tx.send(result);
        });
        if !self.send(Msg::Job(job)) {
            return Err(internal("WriteQueue::submit", "Warteschlange geschlossen"));
        }
        done_rx
            .await
            .map_err(|_| internal("WriteQueue::submit", "Auftrag ohne Ergebnis verworfen"))?
    }

    /// Führt einen Verwaltungsauftrag auf demselben Schreib-Thread aus.
    pub async fn submit_admin<T: Send + 'static>(
        &self,
        f: impl FnOnce() -> Result<T, CmdError> + Send + 'static,
    ) -> Result<T, CmdError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if !self.send(Msg::Job(Box::new(move |_| {
            let result = std::panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|p| {
                Err(internal("WriteQueue::submit_admin", panic_text(p.as_ref())))
            });
            let _ = tx.send(result);
        }))) {
            return Err(internal(
                "WriteQueue::submit_admin",
                "Warteschlange geschlossen",
            ));
        }
        rx.await
            .map_err(|_| internal("WriteQueue::submit_admin", "Auftrag verworfen"))?
    }

    /// Datei-Auftrag, strikt mit SQL- und Verwaltungsaufträgen serialisiert, ohne implizite Migration.
    pub async fn submit_file<T: Send + 'static>(
        &self,
        key: &CrawlKey,
        f: impl FnOnce(&mut os_store::CrawlFile) -> Result<T, CmdError> + Send + 'static,
    ) -> Result<T, CmdError> {
        let key = key.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        if !self.send(Msg::Job(Box::new(move |w| {
            let result = (|| {
                if !w.files.contains_key(&key) {
                    let path = w
                        .paths
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| no_crawl(&key.0))?;
                    // Prüfen ohne Migration, genau wie bei submit.
                    drop(open_write(&path).map_err(|e| store_error(&e))?);
                    let file = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite)
                        .map_err(|e| store_error(&e))?;
                    file.conn()
                        .busy_timeout(BUSY_TIMEOUT)
                        .map_err(|e| store_error(&e.into()))?;
                    w.files.insert(key.clone(), file);
                }
                let file = w.files.get_mut(&key).expect("Datei gerade geöffnet");
                match std::panic::catch_unwind(AssertUnwindSafe(|| f(file))) {
                    Ok(result) => result,
                    Err(p) => {
                        w.files.remove(&key);
                        Err(internal("WriteQueue::submit_file", panic_text(p.as_ref())))
                    }
                }
            })();
            let _ = tx.send(result);
        }))) {
            return Err(internal(
                "WriteQueue::submit_file",
                "Warteschlange geschlossen",
            ));
        }
        rx.await
            .map_err(|_| internal("WriteQueue::submit_file", "Auftrag verworfen"))?
    }

    /// Wartet, bis alle bisher eingereihten Aufträge gelaufen sind.
    pub async fn flush(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        if self.send(Msg::Job(Box::new(move |_w: &mut Worker| {
            let _ = tx.send(());
        }))) {
            let _ = rx.await;
        }
    }

    /// Beendet den Thread nach den eingereihten Aufträgen und schließt alle Verbindungen.
    pub fn shutdown(&self) {
        let _ = self.send(Msg::Close);
        self.tx.lock().unwrap_or_else(|p| p.into_inner()).take();
        let handle = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(h) = handle {
            if h.thread().id() != std::thread::current().id() {
                let _ = h.join();
            }
        }
    }
}

impl Drop for WriteQueue {
    fn drop(&mut self) {
        self.shutdown();
    }
}
