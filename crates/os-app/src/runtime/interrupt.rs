//! Brücke zwischen `CancelToken` und `rusqlite::InterruptHandle`.
//!
//! Der `CancelToken` aus `os-analyze` ist eine reine Marke ohne Rückruf. Wer eine Datenbankarbeit ausführt (Lese-Pool),
//! meldet hier die Marke und das Interrupt-Handle der Verbindung an. Ein kleiner Wächter-Thread unterbricht die
//! Verbindung, sobald die Marke abgebrochen ist, und wiederholt das alle 10 ms, bis die Arbeit abgemeldet wird (ein
//! Interrupt, der vor dem Start einer Anweisung eintrifft, ginge sonst verloren). `Ops::cancel` weckt den Wächter
//! sofort; der Abbruch wirkt damit innerhalb von 100 ms (UI-141).

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use os_analyze::analysis::CancelToken;
use rusqlite::InterruptHandle;

const SWEEP: Duration = Duration::from_millis(10);

struct State {
    next: u64,
    entries: HashMap<u64, (CancelToken, InterruptHandle)>,
    shutdown: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Der Wächter. Ein Exemplar je `App`.
pub struct InterruptHub {
    shared: Arc<Shared>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

/// Meldung einer laufenden Datenbankarbeit; das Abmelden geschieht beim Verwerfen.
pub struct Registration {
    shared: Arc<Shared>,
    id: u64,
}

impl Drop for Registration {
    fn drop(&mut self) {
        self.shared.lock().entries.remove(&self.id);
    }
}

impl InterruptHub {
    /// Startet den Wächter-Thread.
    pub fn new() -> InterruptHub {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                next: 0,
                entries: HashMap::new(),
                shutdown: false,
            }),
            wake: Condvar::new(),
        });
        let worker = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("os-app-interrupt".into())
            .spawn(move || watch(&worker))
            .ok();
        InterruptHub {
            shared,
            thread: Mutex::new(thread),
        }
    }

    /// Meldet eine Arbeit an. Ist die Marke bereits abgebrochen, wird sofort unterbrochen.
    pub fn register(&self, token: &CancelToken, handle: InterruptHandle) -> Registration {
        let id = {
            let mut st = self.shared.lock();
            let id = st.next;
            st.next += 1;
            st.entries.insert(id, (token.clone(), handle));
            id
        };
        // Danach prüfen: Wer abbricht, setzt erst die Marke und weckt dann; wer anmeldet, trägt erst ein und prüft dann.
        // So sieht mindestens einer von beiden den anderen.
        if token.is_cancelled() {
            self.poke();
        }
        Registration {
            shared: Arc::clone(&self.shared),
            id,
        }
    }

    /// Weckt den Wächter (nach `CancelToken::cancel`).
    pub fn poke(&self) {
        self.shared.wake.notify_all();
    }

    /// Anzahl angemeldeter Arbeiten (Tests).
    pub fn active(&self) -> usize {
        self.shared.lock().entries.len()
    }
}

impl Default for InterruptHub {
    fn default() -> Self {
        InterruptHub::new()
    }
}

impl Drop for InterruptHub {
    fn drop(&mut self) {
        self.shared.lock().shutdown = true;
        self.shared.wake.notify_all();
        let handle = self.thread.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(h) = handle {
            if h.thread().id() != std::thread::current().id() {
                let _ = h.join();
            }
        }
    }
}

fn watch(shared: &Shared) {
    let mut st = shared.lock();
    loop {
        if st.shutdown {
            return;
        }
        for (token, handle) in st.entries.values() {
            if token.is_cancelled() {
                handle.interrupt();
            }
        }
        st = if st.entries.is_empty() {
            shared.wake.wait(st).unwrap_or_else(|p| p.into_inner())
        } else {
            shared
                .wake
                .wait_timeout(st, SWEEP)
                .unwrap_or_else(|p| p.into_inner())
                .0
        };
    }
}
