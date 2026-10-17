//! Fenster: ein Ereigniskanal je Fenster und Sichtbarkeit aus zwei Quellen (M2-ui-concept 2.5, UI-143).
//!
//! Sichtbarkeit: `visibilitychange` im Frontend (Command `events.setVisibility`) **und** die Fensterereignisse der
//! Tauri-Hülle (`App::set_visibility`) melden getrennt; ein Fenster gilt nur als sichtbar, wenn beide Quellen
//! „sichtbar“ melden (WKWebView meldet verdeckte Fenster unzuverlässig). Ohne Meldung gilt eine Quelle als sichtbar.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::dto::{SubscriptionId, UiEvent};
use crate::events::EventSink;

use super::WindowId;

#[derive(Debug, Clone, Copy)]
struct Visibility {
    frontend: bool,
    shell: bool,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility {
            frontend: true,
            shell: true,
        }
    }
}

struct Subscription {
    id: String,
    sink: Arc<dyn EventSink>,
}

#[derive(Default)]
struct State {
    next: u64,
    subs: HashMap<WindowId, Subscription>,
    vis: HashMap<WindowId, Visibility>,
}

/// Abonnements und Sichtbarkeit aller Fenster.
#[derive(Default)]
pub struct Windows {
    state: Mutex<State>,
}

impl Windows {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Abonniert ein Fenster; ein früheres Abonnement desselben Fensters wird ersetzt.
    pub fn subscribe(&self, window: WindowId, sink: Arc<dyn EventSink>) -> SubscriptionId {
        let mut st = self.lock();
        st.next += 1;
        let id = format!("sub-{}", st.next);
        st.subs.insert(
            window,
            Subscription {
                id: id.clone(),
                sink,
            },
        );
        SubscriptionId { id }
    }

    /// Beendet ein Abonnement; `false`, wenn es nicht (mehr) besteht.
    pub fn unsubscribe(&self, id: &SubscriptionId) -> bool {
        let mut st = self.lock();
        let before = st.subs.len();
        st.subs.retain(|_, s| s.id != id.id);
        st.subs.len() != before
    }

    /// Ein Fenster wurde geschlossen: Abonnement und Sichtbarkeit werden vergessen (ein neues Fenster gleichen Namens
    /// beginnt wieder als sichtbar).
    pub fn forget(&self, window: &WindowId) {
        let mut st = self.lock();
        st.subs.remove(window);
        st.vis.remove(window);
    }

    /// Fenster mit Abonnement, sortiert nach Name.
    pub fn subscribed(&self) -> Vec<WindowId> {
        let mut v: Vec<WindowId> = self.lock().subs.keys().cloned().collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Meldung der Hülle (Fensterereignisse).
    pub fn set_shell_visible(&self, window: WindowId, visible: bool) {
        self.lock().vis.entry(window).or_default().shell = visible;
    }

    /// Meldung des Frontends (`visibilitychange`).
    pub fn set_frontend_visible(&self, window: WindowId, visible: bool) {
        self.lock().vis.entry(window).or_default().frontend = visible;
    }

    /// Sichtbar = beide Quellen sichtbar.
    pub fn is_visible(&self, window: &WindowId) -> bool {
        self.lock()
            .vis
            .get(window)
            .is_none_or(|v| v.frontend && v.shell)
    }

    /// Sendet an ein Fenster; `false`, wenn es kein (offenes) Abonnement gibt. Eine geschlossene Senke beendet das
    /// Abonnement.
    pub fn send_to(&self, window: &WindowId, ev: UiEvent) -> bool {
        let sub = self
            .lock()
            .subs
            .get(window)
            .map(|s| (s.id.clone(), Arc::clone(&s.sink)));
        let Some((id, sink)) = sub else {
            return false;
        };
        // außerhalb der Sperre: die Senke darf kurz blockieren
        if sink.send(ev).is_ok() {
            return true;
        }
        let mut st = self.lock();
        if st.subs.get(window).is_some_and(|s| s.id == id) {
            st.subs.remove(window);
        }
        false
    }

    /// Sendet an alle abonnierten Fenster; Anzahl der erreichten Fenster.
    pub fn broadcast(&self, ev: &UiEvent) -> usize {
        self.subscribed()
            .iter()
            .filter(|w| self.send_to(w, ev.clone()))
            .count()
    }
}
