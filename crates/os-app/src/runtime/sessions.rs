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
pub struct SessionRegistry(Mutex<HashMap<WindowId, SessionInfo>>);
impl SessionRegistry {
    /// Publish an engine transition to all windows sharing this crawl.
    pub fn update_state(&self, key: &CrawlKey, state: CrawlState) {
        for s in self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values_mut()
            .filter(|s| &s.key == key)
        {
            s.state = state;
        }
    }

    pub fn current(&self, w: &WindowId) -> Option<SessionInfo> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(w)
            .cloned()
    }
    pub fn set_current(&self, w: WindowId, s: SessionInfo) {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(w, s);
    }
    pub fn clear(&self, w: &WindowId) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).remove(w);
    }
    pub fn update_config(&self, key: &CrawlKey, cfg: Arc<CrawlConfig>) {
        for s in self
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
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
            .iter()
            .map(|(w, s)| (w.0.clone(), s.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries.into_iter().map(|(_, s)| s).collect()
    }
}
