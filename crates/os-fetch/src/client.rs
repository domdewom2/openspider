//! HTTP-Abrufer (`HttpFetcher`) und seine Konfiguration.
//!
//! STUB(M1-P02): P02 ersetzt `fetch` durch den echten Abruf (reqwest ohne Auto-Dekompression).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use os_model::{Clock, CookieMode, CrawlConfig, NoResponseReason};

use crate::error::FetchError;
use crate::http::{Fetch, FetchOutcome, FetchRequest, FetchResult};
use crate::secrets::SecretStore;

/// Basic-Auth-Eintrag mit aufgelöstem Passwort.
#[derive(Clone)]
pub struct BasicAuthEntry {
    /// URL-Präfix, für das der Eintrag gilt.
    pub prefix: String,
    /// Benutzername.
    pub username: String,
    /// Passwort aus dem `SecretStore`.
    pub password: String,
}

/// Konfiguration des Abrufers, abgeleitet aus der Crawl-Konfiguration.
#[derive(Clone)]
pub struct FetchConfig {
    /// HTTP-User-Agent.
    pub user_agent: String,
    /// Gesamtzeit für eine Antwort.
    pub response_timeout: Duration,
    /// Cookie-Verhalten.
    pub cookies: CookieMode,
    /// Basic-Auth-Einträge.
    pub basic_auth: Vec<BasicAuthEntry>,
    /// Resolve-Regeln `host=ip[:port]` (`*.domain` erlaubt).
    pub resolve: Vec<String>,
    /// Höchstzahl Abrufe pro Sekunde (für alle Abrufe inkl. robots.txt).
    pub max_urls_per_sec: Option<f64>,
    /// Höchste Body-Größe in Bytes.
    pub max_body_bytes: u64,
    /// Zusätzliche Wurzelzertifikate (PEM), nur Tests.
    pub extra_root_certs: Vec<Vec<u8>>,
    /// Uhr für Zeitstempel und Antwortzeiten.
    pub clock: Arc<dyn Clock>,
}

impl FetchConfig {
    /// Leitet die Abruf-Konfiguration aus der Crawl-Konfiguration ab.
    // STUB(M1-P02): User-Agent-Voreinstellungen, Rate-Limit-Details und Fehlerfälle ergänzt P02.
    pub fn from_crawl_config(
        cfg: &CrawlConfig,
        secrets: &dyn SecretStore,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, FetchError> {
        let basic_auth = cfg
            .auth
            .basic
            .iter()
            .map(|b| {
                secrets
                    .get(&b.secret_ref)
                    .map(|password| BasicAuthEntry {
                        prefix: b.prefix.clone(),
                        username: b.username.clone(),
                        password,
                    })
                    .ok_or_else(|| {
                        FetchError::Config(format!("Geheimnis {:?} nicht gefunden", b.secret_ref))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(FetchConfig {
            user_agent: cfg
                .user_agent
                .http
                .clone()
                .unwrap_or_else(|| format!("OpenSpider/{}", env!("CARGO_PKG_VERSION"))),
            response_timeout: Duration::from_secs(u64::from(cfg.advanced.response_timeout_s)),
            cookies: cfg.advanced.cookies,
            basic_auth,
            resolve: cfg.network.resolve.clone(),
            max_urls_per_sec: cfg.speed.max_urls_per_sec,
            max_body_bytes: cfg.limits.max_page_kb.saturating_mul(1024),
            extra_root_certs: Vec::new(),
            clock,
        })
    }
}

/// HTTP-Abrufer.
pub struct HttpFetcher {
    cfg: FetchConfig,
}

impl HttpFetcher {
    /// Baut den Abrufer.
    // STUB(M1-P02)
    pub fn new(cfg: FetchConfig) -> Result<Self, FetchError> {
        Ok(HttpFetcher { cfg })
    }
}

impl Fetch for HttpFetcher {
    // STUB(M1-P02): liefert „keine Antwort“, bis P02 den Abruf umsetzt.
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>> {
        Box::pin(async move {
            FetchResult {
                url: req.url,
                started_at: self.cfg.clock.now(),
                outcome: FetchOutcome::NoResponse {
                    reason: NoResponseReason::ConnectionError,
                    detail: "STUB(M1-P02): Abruf nicht implementiert".to_string(),
                    elapsed_ms: 0,
                },
            }
        })
    }
}
