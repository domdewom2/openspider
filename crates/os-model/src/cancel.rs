//! Geteilte Abbruch-/Pausenmarke ohne Abhängigkeit auf Analyse oder Engine.
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct State {
    cancelled: bool,
    paused: bool,
    progress: f32,
    callbacks: std::collections::BTreeMap<u64, Arc<dyn Fn() + Send + Sync>>,
    next_callback: u64,
}

impl std::fmt::Debug for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("State")
            .field("cancelled", &self.cancelled)
            .field("paused", &self.paused)
            .field("progress", &self.progress)
            .finish_non_exhaustive()
    }
}

/// Scoped cancellation callback. Dropping it unregisters the callback.
pub struct CancelRegistration {
    token: CancelToken,
    id: u64,
}
impl Drop for CancelRegistration {
    fn drop(&mut self) {
        self.token
            .state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .callbacks
            .remove(&self.id);
    }
}

/// Klone teilen Abbruch, Pause und Fortschritt (CRAWL-141, K2-56).
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    state: Arc<(Mutex<State>, Condvar)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("abgebrochen")]
pub struct Cancelled;

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        let mut state = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.cancelled {
            return;
        }
        state.cancelled = true;
        // Keep callbacks serialized with unregister: a dropped registration can never
        // interrupt a later operation on the same connection. Callbacks must not reenter.
        for callback in state.callbacks.values() {
            callback();
        }
        self.state.1.notify_all();
    }
    /// Registers a short, non-reentrant callback; already-cancelled tokens invoke it immediately.
    pub fn on_cancel(&self, callback: impl Fn() + Send + Sync + 'static) -> CancelRegistration {
        let mut state = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        let id = state.next_callback;
        state.next_callback += 1;
        if state.cancelled {
            callback();
        }
        state.callbacks.insert(id, Arc::new(callback));
        CancelRegistration {
            token: self.clone(),
            id,
        }
    }
    pub fn is_cancelled(&self) -> bool {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancelled
    }
    pub fn pause(&self) {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paused = true;
    }
    pub fn resume(&self) {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paused = false;
        self.state.1.notify_all();
    }
    pub fn checkpoint(&self) -> Result<(), Cancelled> {
        let mut state = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        while state.paused && !state.cancelled {
            state = self.state.1.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if state.cancelled {
            Err(Cancelled)
        } else {
            Ok(())
        }
    }
    pub fn set_progress(&self, value: f32) {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .progress = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
    pub fn progress(&self) -> f32 {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .progress
    }
}
