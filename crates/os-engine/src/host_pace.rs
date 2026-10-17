//! Hostbezogene Parallelität; Erweiterungspunkt M2-E03 (CRAWL-064).
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use url::Url;

pub struct HostPermit {
    _permit: Option<OwnedSemaphorePermit>,
}
pub struct HostPacer {
    cap: Option<u32>,
    gates: Mutex<HashMap<String, Arc<Semaphore>>>,
}
impl HostPacer {
    pub fn new(cap: Option<u32>) -> Self {
        Self {
            cap: cap.filter(|n| *n > 0),
            gates: Mutex::new(HashMap::new()),
        }
    }
    pub async fn acquire(&self, url: &Url) -> HostPermit {
        let Some(cap) = self.cap else {
            return HostPermit { _permit: None };
        };
        let key = format!(
            "{}:{}",
            url.host_str().unwrap_or(""),
            url.port_or_known_default().unwrap_or(0)
        );
        let sem = {
            let mut gates = self.gates.lock().unwrap_or_else(|p| p.into_inner());
            Arc::clone(
                gates
                    .entry(key)
                    .or_insert_with(|| Arc::new(Semaphore::new(cap as usize))),
            )
        };
        HostPermit {
            _permit: sem.acquire_owned().await.ok(),
        }
    }
    /// Vollständiger Kontext für die Tempo-Fachlogik, ohne weitere Worker-Änderungen.
    // STUB(M2-E03)
    pub async fn before_fetch(
        &self,
        _url: &Url,
        _crawl_delay: Option<f64>,
        _cfg: &os_model::CrawlConfig,
        _clock: &dyn os_model::Clock,
    ) {
    }

    /// Antwortzeit, Retry-After und Konfiguration bleiben beim Besitzer der Host-Steuerung.
    // STUB(M2-E03)
    pub fn on_result(
        &self,
        url: &Url,
        result: &os_fetch::FetchResult,
        _cfg: &os_model::CrawlConfig,
        _clock: &dyn os_model::Clock,
    ) {
        if let os_fetch::FetchOutcome::Response(response) = &result.outcome {
            self.on_response(url, response.status, None);
        }
    }

    /// Adaptive Drosselung folgt in M2-E03.
    // STUB(M2-E03)
    pub fn on_response(&self, _url: &Url, _status: u16, _retry_after: Option<Duration>) {}
}
