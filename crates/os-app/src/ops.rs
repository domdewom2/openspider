//! Lang laufende Operationen mit Fortschritt und Abbruch (≤ 100 ms über SQLite-Interrupt).
//!
//! Commands mit `long_running` starten über [`Ops::spawn`] eine Operation und liefern sofort das `OpHandle`. Der
//! Fortschritt (`ProgressSink`) und das Ende (Ergebnis, Fehler, Abbruch) erscheinen im nächsten Tick
//! (`Tick.ops`, [`Ops::take_updates`]); eine Operation, deren Zustand sich seit dem letzten Tick nicht geändert hat,
//! fehlt im Tick (Leerlauf ohne Nachrichten, UI-143). Abbruch: [`Ops::cancel`] setzt die Marke der Operation und weckt
//! den Interrupt-Wächter; der Lese-Pool unterbricht damit die laufende SQLite-Anweisung. Die Marke steht der
//! Operation auch ohne Weiterreichen zur Verfügung (`runtime::current_cancel`), sodass `ReadPool::run` innerhalb der
//! Operation von selbst abbrechbar ist.

use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use os_analyze::analysis::CancelToken;
use os_model::{Clock, IdSource};
use tokio::runtime::Handle;
use tokio::sync::Notify;

use crate::dto::{CmdError, ErrorCode, OpHandle, OpState, OpStatus};
use crate::runtime::error::{internal, panic_text};
use crate::runtime::interrupt::InterruptHub;

/// Höchstens so viele beendete, noch nicht gemeldete Operationen merkt sich die Verwaltung (Schutz vor Anhäufung
/// ohne Abnehmer).
const MAX_FINISHED_UNREPORTED: usize = 256;

struct Entry {
    id: String,
    label_key: String,
    cancellable: bool,
    token: CancelToken,
    done: u64,
    total: Option<u64>,
    state: OpState,
    result: Option<serde_json::Value>,
    error: Option<CmdError>,
    dirty: bool,
}

impl Entry {
    fn status(&self) -> OpStatus {
        OpStatus {
            op_id: self.id.clone(),
            label_key: self.label_key.clone(),
            done: self.done,
            total: self.total,
            state: self.state,
            result: self.result.clone(),
            error: self.error.clone(),
        }
    }

    fn finished(&self) -> bool {
        self.state != OpState::Running
    }
}

struct Shared {
    entries: Mutex<Vec<Entry>>,
    changed: Notify,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Entry>> {
        self.entries.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Meldet Fortschritt einer Operation (erledigt, gesamt).
#[derive(Clone, Default)]
pub struct ProgressSink {
    target: Option<(Arc<Shared>, String)>,
}

impl std::fmt::Debug for ProgressSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressSink")
            .field("op", &self.target.as_ref().map(|(_, id)| id))
            .finish()
    }
}

impl ProgressSink {
    /// Setzt „erledigt“ und „gesamt“. Unveränderte Werte lösen keine Meldung aus.
    pub fn report(&self, done: u64, total: Option<u64>) {
        let Some((shared, id)) = &self.target else {
            return;
        };
        let mut entries = shared.lock();
        if let Some(e) = entries.iter_mut().find(|e| &e.id == id && !e.finished()) {
            if e.done != done || e.total != total {
                e.done = done;
                e.total = total;
                e.dirty = true;
            }
        }
    }

    /// Erhöht „erledigt“ um `n`.
    pub fn advance(&self, n: u64) {
        let Some((shared, id)) = &self.target else {
            return;
        };
        let (done, total) = {
            let entries = shared.lock();
            match entries.iter().find(|e| &e.id == id) {
                Some(e) => (e.done.saturating_add(n), e.total),
                None => return,
            }
        };
        self.report(done, total);
    }
}

/// Alles, was die Verwaltung von außen braucht.
pub struct OpsDeps {
    /// Laufzeit, auf der die Operationen laufen.
    pub handle: Handle,
    /// Interrupt-Wächter (wird beim Abbruch geweckt).
    pub hub: Arc<InterruptHub>,
    /// Quelle der Kennungen (`op_id`).
    pub ids: Arc<dyn IdSource>,
    /// Uhr (Zeitanteil der Kennung).
    pub clock: Arc<dyn Clock>,
}

/// Verwaltung der laufenden Operationen.
#[derive(Clone)]
pub struct Ops {
    shared: Arc<Shared>,
    deps: Arc<OpsDeps>,
}

impl std::fmt::Debug for Ops {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ops")
            .field("entries", &self.shared.lock().len())
            .finish()
    }
}

impl Ops {
    /// Neue Verwaltung.
    pub fn new(deps: OpsDeps) -> Ops {
        Ops {
            shared: Arc::new(Shared {
                entries: Mutex::new(Vec::new()),
                changed: Notify::new(),
            }),
            deps: Arc::new(deps),
        }
    }

    /// Startet `f` im Hintergrund und gibt sofort das `OpHandle` zurück.
    pub fn spawn<F>(&self, label_key: &'static str, cancellable: bool, f: F) -> OpHandle
    where
        F: FnOnce(
                CancelToken,
                ProgressSink,
            ) -> BoxFuture<'static, Result<serde_json::Value, CmdError>>
            + Send
            + 'static,
    {
        let op_id = self
            .deps
            .ids
            .new_crawl_id(self.deps.clock.now())
            .to_string();
        let token = CancelToken::new();
        self.shared.lock().push(Entry {
            id: op_id.clone(),
            label_key: label_key.to_string(),
            cancellable,
            token: token.clone(),
            done: 0,
            total: None,
            state: OpState::Running,
            result: None,
            error: None,
            dirty: true,
        });
        let sink = ProgressSink {
            target: Some((Arc::clone(&self.shared), op_id.clone())),
        };
        let shared = Arc::clone(&self.shared);
        let id = op_id.clone();
        let run_token = token.clone();
        let body = async move {
            let _resources = os_engine::memory::watch_operation(token.clone());
            let outcome = match std::panic::catch_unwind(AssertUnwindSafe(|| f(token, sink))) {
                Ok(fut) => AssertUnwindSafe(fut).catch_unwind().await,
                Err(p) => Err(p),
            };
            let result = match outcome {
                Ok(r) => r,
                Err(p) => Err(internal("Ops::spawn", panic_text(p.as_ref()))),
            };
            finish(&shared, &id, result);
        };
        self.deps
            .handle
            .spawn(crate::runtime::with_cancel(run_token, body));
        OpHandle {
            op_id,
            label_key: label_key.to_string(),
            cancellable,
        }
    }

    /// Bricht die Operation ab; `false`, wenn es sie nicht (mehr) gibt oder sie nicht abbrechbar ist.
    pub fn cancel(&self, op_id: &str) -> bool {
        let token = {
            let entries = self.shared.lock();
            entries
                .iter()
                .find(|e| e.id == op_id && e.cancellable && !e.finished())
                .map(|e| e.token.clone())
        };
        match token {
            Some(t) => {
                t.cancel();
                self.deps.hub.poke();
                true
            }
            None => false,
        }
    }

    /// Bricht alle laufenden abbrechbaren Operationen ab (Beenden); Anzahl der betroffenen Operationen.
    pub fn cancel_all(&self) -> usize {
        let tokens: Vec<CancelToken> = self
            .shared
            .lock()
            .iter()
            .filter(|e| e.cancellable && !e.finished())
            .map(|e| e.token.clone())
            .collect();
        for t in &tokens {
            t.cancel();
        }
        self.deps.hub.poke();
        tokens.len()
    }

    /// Ist die Operation bekannt, und ist sie abbrechbar? (`None` = unbekannt oder schon gemeldet)
    pub fn is_cancellable(&self, op_id: &str) -> Option<bool> {
        self.shared
            .lock()
            .iter()
            .find(|e| e.id == op_id)
            .map(|e| e.cancellable)
    }

    /// Aktueller Stand einer Operation, ohne ihn als gemeldet zu markieren.
    pub fn status(&self, op_id: &str) -> Option<OpStatus> {
        self.shared
            .lock()
            .iter()
            .find(|e| e.id == op_id)
            .map(Entry::status)
    }

    /// Wartet, bis die Operation beendet ist, und liefert ihren Endstand (`None`, wenn sie unbekannt ist).
    pub fn wait(&self, op_id: &str) -> impl Future<Output = Option<OpStatus>> + Send + 'static {
        let shared = Arc::clone(&self.shared);
        let id = op_id.to_string();
        async move {
            loop {
                let notified = shared.changed.notified();
                tokio::pin!(notified);
                // vor der Prüfung anmelden, damit keine Benachrichtigung verloren geht
                notified.as_mut().enable();
                {
                    let entries = shared.lock();
                    match entries.iter().find(|e| e.id == id) {
                        None => return None,
                        Some(e) if e.finished() => return Some(e.status()),
                        Some(_) => {}
                    }
                }
                notified.await;
            }
        }
    }

    /// Stände der Operationen, die sich seit dem letzten Aufruf geändert haben (Fortschritt, Ende); beendete
    /// Operationen verschwinden damit aus der Verwaltung. Das ist die Quelle von `Tick.ops`.
    pub fn take_updates(&self) -> Vec<OpStatus> {
        let mut entries = self.shared.lock();
        let updates: Vec<OpStatus> = entries
            .iter_mut()
            .filter(|e| e.dirty)
            .map(|e| {
                e.dirty = false;
                e.status()
            })
            .collect();
        // beendete Operationen sind mit diesem Aufruf gemeldet und verschwinden
        entries.retain(|e| !e.finished() || e.dirty);
        updates
    }

    /// Anzahl der laufenden Operationen.
    pub fn running(&self) -> usize {
        self.shared.lock().iter().filter(|e| !e.finished()).count()
    }
}

fn finish(shared: &Shared, id: &str, result: Result<serde_json::Value, CmdError>) {
    {
        let mut entries = shared.lock();
        if let Some(e) = entries.iter_mut().find(|e| e.id == id) {
            match result {
                Ok(v) => {
                    e.state = OpState::Done;
                    e.result = Some(v);
                }
                Err(err) if err.code == ErrorCode::Cancelled => {
                    e.state = OpState::Cancelled;
                }
                Err(err) => {
                    e.state = OpState::Failed;
                    e.error = Some(err);
                }
            }
            e.dirty = true;
        }
        let finished = entries.iter().filter(|e| e.finished()).count();
        if finished > MAX_FINISHED_UNREPORTED {
            let mut drop_n = finished - MAX_FINISHED_UNREPORTED;
            entries.retain(|e| {
                if drop_n > 0 && e.finished() {
                    drop_n -= 1;
                    false
                } else {
                    true
                }
            });
        }
    }
    shared.changed.notify_waiters();
}
