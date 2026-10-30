//! Prozessweites Sitzungsregister; Rückgaben sind unabhängige Schnappschüsse.
use super::WindowId;
use crate::pool::CrawlKey;
use os_engine::CrawlHandle;
use os_model::{CrawlConfig, CrawlState};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
/// Steuerung eines laufenden Crawls.
pub struct EngineControl {
    pub handle: Mutex<Option<CrawlHandle>>,
}
/// Sitzungsstand eines Fensters.
#[derive(Clone)]
pub struct SessionInfo {
    pub key: CrawlKey,
    pub path: PathBuf,
    pub config: Arc<CrawlConfig>,
    pub state: CrawlState,
    pub writable: bool,
    pub engine: Option<Arc<EngineControl>>,
}
/// Zuordnung Fenster → Sitzung.
#[derive(Default)]
pub struct SessionRegistry(Mutex<Registry>);

#[derive(Default)]
struct Registry {
    sessions: HashMap<WindowId, SessionInfo>,
    // Local lifecycle transitions outrank reads started before their durable write.
    awaiting_snapshot: HashMap<CrawlKey, CrawlState>,
}
impl Registry {
    fn prune_transitions(&mut self) {
        self.awaiting_snapshot
            .retain(|key, _| self.sessions.values().any(|s| &s.key == key));
    }
}
impl SessionRegistry {
    /// Publish an engine transition to all windows sharing this crawl.
    pub fn update_state(&self, key: &CrawlKey, state: CrawlState) {
        let mut registry = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if registry.sessions.values().any(|s| &s.key == key) {
            registry.awaiting_snapshot.insert(key.clone(), state);
        }
        for s in registry.sessions.values_mut().filter(|s| &s.key == key) {
            s.state = state;
        }
    }

    /// Acknowledge a local transition only when the durable snapshot catches up.
    /// Reject stale snapshots without publishing their status or counters.
    pub(crate) fn refresh_state(&self, key: &CrawlKey, state: CrawlState) -> bool {
        let mut registry = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if registry
            .awaiting_snapshot
            .get(key)
            .is_some_and(|pending| *pending != state)
        {
            return false;
        }
        registry.awaiting_snapshot.remove(key);
        for s in registry.sessions.values_mut().filter(|s| &s.key == key) {
            s.state = state;
        }
        true
    }

    pub fn current(&self, w: &WindowId) -> Option<SessionInfo> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .get(w)
            .cloned()
    }
    pub fn set_current(&self, w: WindowId, s: SessionInfo) {
        self.insert_current(w, s, false);
    }

    /// A new engine run supersedes unacknowledged transitions from its previous run.
    pub(crate) fn start_current(&self, w: WindowId, s: SessionInfo) {
        self.insert_current(w, s, true);
    }

    fn insert_current(&self, w: WindowId, mut s: SessionInfo, new_run: bool) {
        let mut registry = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if new_run {
            registry.awaiting_snapshot.insert(s.key.clone(), s.state);
        } else if let Some(state) = registry.awaiting_snapshot.get(&s.key) {
            s.state = *state;
        }
        registry.sessions.insert(w, s);
        registry.prune_transitions();
    }
    pub fn clear(&self, w: &WindowId) {
        let mut registry = self.0.lock().unwrap_or_else(|p| p.into_inner());
        registry.sessions.remove(w);
        registry.prune_transitions();
    }
    pub fn update_config(&self, key: &CrawlKey, cfg: Arc<CrawlConfig>) {
        for s in self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .values_mut()
            .filter(|s| &s.key == key)
        {
            s.config = cfg.clone();
        }
    }
    pub fn rename_path(&self, key: &CrawlKey, new: PathBuf) {
        for s in self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .values_mut()
            .filter(|s| &s.key == key)
        {
            s.path = new.clone();
        }
    }
    pub fn all(&self) -> Vec<SessionInfo> {
        let mut entries: Vec<_> = self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sessions
            .iter()
            .map(|(w, s)| (w.0.clone(), s.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.into_iter().map(|(_, s)| s).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(key: &CrawlKey) -> SessionInfo {
        SessionInfo {
            key: key.clone(),
            path: PathBuf::from("fixture.ospider"),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Running,
            writable: true,
            engine: None,
        }
    }

    // spec: CRAWL-142 [M3]
    // spec: OUT-251
    #[test]
    fn stale_snapshots_cannot_undo_local_pause_or_completion() {
        for state in [CrawlState::Paused, CrawlState::Completed] {
            let registry = SessionRegistry::default();
            let key = CrawlKey("fixture".into());
            let main = WindowId("main".into());
            let other = WindowId("other".into());
            registry.set_current(main.clone(), session(&key));
            registry.update_state(&key, state);
            registry.set_current(other.clone(), session(&key));
            assert!(!registry.refresh_state(&key, CrawlState::Running));
            assert!(registry.all().iter().all(|s| s.state == state));
            assert!(registry.refresh_state(&key, state));
            // Once acknowledged, new external transitions are observable again.
            assert!(registry.refresh_state(&key, CrawlState::Running));
            assert!(registry
                .all()
                .iter()
                .all(|s| s.state == CrawlState::Running));
            registry.update_state(&key, state);
            registry.clear(&main);
            assert!(!registry.refresh_state(&key, CrawlState::Running));
            registry.clear(&other);
            registry.set_current(main.clone(), session(&key));
            assert_eq!(registry.current(&main).unwrap().state, CrawlState::Running);
            assert!(registry.refresh_state(&key, CrawlState::Running));
        }
    }

    // spec: CRAWL-128
    // spec: OUT-251
    #[test]
    fn a_new_engine_run_replaces_the_previous_unacknowledged_completion() {
        let registry = SessionRegistry::default();
        let key = CrawlKey("fixture".into());
        let main = WindowId("main".into());
        registry.set_current(main.clone(), session(&key));
        registry.update_state(&key, CrawlState::Completed);
        registry.start_current(main.clone(), session(&key));
        assert_eq!(registry.current(&main).unwrap().state, CrawlState::Running);
        assert!(!registry.refresh_state(&key, CrawlState::Completed));
        assert_eq!(registry.current(&main).unwrap().state, CrawlState::Running);
        assert!(registry.refresh_state(&key, CrawlState::Running));
        assert!(registry.refresh_state(&key, CrawlState::Completed));
    }
}
