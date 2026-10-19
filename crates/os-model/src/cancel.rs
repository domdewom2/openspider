//! Geteilte Abbruch-/Pausenmarke ohne Abhängigkeit auf Analyse oder Engine.
use std::sync::{Arc, Condvar, Mutex, Weak};

#[derive(Debug, Default)]
struct State {
    cancelled: bool,
    paused: bool,
    progress: f32,
}

/// Klone teilen Abbruch, Pause und Fortschritt (CRAWL-141, K2-56).
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    state: Arc<(Mutex<State>, Condvar)>,
    hooks: Arc<Mutex<Vec<Weak<CancelHook>>>>,
}

#[derive(Default)]
struct CancelHook {
    callback: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl std::fmt::Debug for CancelHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CancelHook").finish_non_exhaustive()
    }
}

impl CancelHook {
    fn invoke(&self) {
        let mut callback = self.callback.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(callback) = callback.take() {
            // Sperre bis zum Ende halten: Drop der Registrierung wartet auf den Hook.
            callback();
        }
    }
}

/// Lebensdauer eines Abbruch-Hooks. Nach Drop kann der Hook nicht mehr laufen.
#[must_use = "Die Registrierung muss für die Dauer der abbrechbaren Arbeit gehalten werden."]
#[derive(Debug)]
pub struct CancelRegistration(Arc<CancelHook>);

impl Drop for CancelRegistration {
    fn drop(&mut self) {
        self.0
            .callback
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("abgebrochen")]
pub struct Cancelled;

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.state
            .0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .cancelled = true;
        self.state.1.notify_all();
        let hooks = std::mem::take(&mut *self.hooks.lock().unwrap_or_else(|e| e.into_inner()));
        for hook in hooks.into_iter().filter_map(|hook| hook.upgrade()) {
            hook.invoke();
        }
    }

    /// Registriert einen einmaligen Interrupt-Hook; bei bereits erfolgtem Abbruch sofort.
    /// Der Hook muss kurz sein und darf seine eigene Registrierung nicht freigeben.
    /// Die Registrierung hält keine Datenbankverbindung, nur den übergebenen Hook.
    pub fn on_cancel(&self, callback: impl FnOnce() + Send + 'static) -> CancelRegistration {
        let hook = Arc::new(CancelHook {
            callback: Mutex::new(Some(Box::new(callback))),
        });
        {
            let mut hooks = self.hooks.lock().unwrap_or_else(|e| e.into_inner());
            hooks.retain(|hook| hook.strong_count() > 0);
            hooks.push(Arc::downgrade(&hook));
        }
        // Registrierung und Abbruch dürfen gleichzeitig laufen; invoke nimmt den Hook einmal.
        if self.is_cancelled() {
            hook.invoke();
        }
        CancelRegistration(hook)
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
