//! Synchrone Lebenszyklus-Hörer und benannte Abschlussbarrieren.
use super::{hooks::CompletionBarrier, App};
use crate::pool::CrawlKey;
use os_model::CrawlConfig;
use std::sync::{Arc, Mutex};
/// Übergänge einer Crawl-Sitzung.
#[derive(Clone)]
pub enum CrawlEvent {
    Started(CrawlKey),
    Paused(CrawlKey),
    Resumed(CrawlKey),
    Stopped(CrawlKey),
    Completed(CrawlKey),
    Opened(CrawlKey),
    Closed(CrawlKey),
    Cleared(CrawlKey),
    ConfigChanged {
        key: CrawlKey,
        old: Arc<CrawlConfig>,
        new: Arc<CrawlConfig>,
    },
}
pub type CrawlEventCallback = dyn Fn(&App, &CrawlEvent) + Send + Sync;
type Listener = Arc<CrawlEventCallback>;
/// Hörer werden in Registrierungsreihenfolge außerhalb der Sperre aufgerufen.
#[derive(Default)]
pub struct CrawlEventBus {
    listeners: Mutex<Vec<Listener>>,
    barriers: Mutex<Vec<(&'static str, Arc<dyn CompletionBarrier>)>>,
}
impl CrawlEventBus {
    pub fn subscribe(&self, f: Box<CrawlEventCallback>) {
        self.listeners
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(Arc::from(f));
    }
    pub fn emit(&self, app: &App, ev: CrawlEvent) {
        let listeners = self
            .listeners
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for f in listeners {
            f(app, &ev);
        }
    }
    pub fn add_barrier(&self, name: &'static str, b: Arc<dyn CompletionBarrier>) {
        let mut barriers = self.barriers.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = barriers.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = b;
        } else {
            barriers.push((name, b));
        }
    }
    pub async fn wait_barriers(&self, key: &CrawlKey) {
        let barriers = self
            .barriers
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for (_, b) in barriers {
            b.wait(key.clone()).await;
        }
    }
}
