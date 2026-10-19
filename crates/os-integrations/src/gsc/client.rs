//! HTTP-Pfad der Search-Console-Endpunkte: Lese-Timeout je Anfrage (INTEG-128), Wiederholungen mit Backoff und
//! `Retry-After` (INTEG-005), Kontingentsignale des Anbieters (INTEG-006) und die Drosselung der URL Inspection auf
//! 600 Anfragen je Minute und Property (INTEG-120).
//!
//! Der Netzwerkclient kommt aus [`crate::http::client_for`] (app-weite Proxy-/Truststore-Einstellungen, keine
//! Redirects). Der Lese-Timeout gilt je Anfrage und überschreibt den Gesamt-Timeout des Clients; ein Timeout ist ein
//! vorübergehender Fehler und wird wie 429/5xx wiederholt. Anbietertexte werden nie übernommen (sie können Tokens
//! oder URL-Parameter spiegeln).

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use os_model::Clock;
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::options::{GscOptions, INSPECTION_PER_MINUTE};
use crate::{ApiCtx, ApiError};

/// Produktive Basis von Search Analytics und `sites.list`.
pub const ANALYTICS_BASE: &str = "https://www.googleapis.com";
/// Produktive Basis der URL Inspection.
pub const INSPECTION_BASE: &str = "https://searchconsole.googleapis.com";

/// Endgültiger Fehler einer Anfrage (nach allen Wiederholungen).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Lese-Timeout bei jeder Wiederholung.
    Timeout { after_s: u64 },
    /// Kontingent des Anbieters erschöpft (429/403 mit Kontingentgrund).
    Quota,
    /// 403 ohne Kontingentgrund (keine Berechtigung bzw. URL nicht in der Property).
    Forbidden,
    /// 401: Anmeldung abgelaufen.
    Unauthorized,
    /// Anderer endgültiger HTTP-Status (4xx bzw. 5xx nach allen Wiederholungen, 429 ohne Kontingentgrund).
    Status(u16),
    /// Verbindung fehlgeschlagen.
    Transport,
    /// Antwort nicht lesbar.
    Invalid,
    /// Keine Property gewählt bzw. keine passende Property.
    NoProperty,
}

impl Failure {
    /// Text für `Summary = Error: …` bzw. die Statusmeldung.
    pub fn message(&self) -> String {
        match self {
            Failure::Timeout { .. } => "Read timeout".into(),
            Failure::Quota => "Quota exceeded".into(),
            Failure::Forbidden => "URL not in property".into(),
            Failure::Unauthorized => "Anmeldung abgelaufen – bitte neu verbinden".into(),
            Failure::Status(code) => format!("HTTP {code}"),
            Failure::Transport => "Verbindung fehlgeschlagen".into(),
            Failure::Invalid => "Antwort nicht lesbar".into(),
            Failure::NoProperty => "Keine Property gewählt".into(),
        }
    }

    /// Als Fehler des API-Rahmens.
    pub fn to_api_error(&self) -> ApiError {
        match self {
            Failure::Timeout { after_s } => {
                ApiError::Transport(format!("Read timeout nach {after_s} s"))
            }
            Failure::Quota => ApiError::QuotaExceeded("Anbieterkontingent erschöpft".into()),
            Failure::Forbidden => ApiError::Unauthorized("HTTP 403: Zugang verweigert".into()),
            Failure::Unauthorized => ApiError::Unauthorized("HTTP 401: Zugang verweigert".into()),
            Failure::Status(429) => ApiError::RateLimited("HTTP 429: Anfragelimit erreicht".into()),
            Failure::Status(code) => {
                ApiError::Transport(format!("HTTP {code}: API-Anfrage fehlgeschlagen"))
            }
            Failure::Transport => ApiError::Transport("API-Verbindung fehlgeschlagen".into()),
            Failure::Invalid => ApiError::Transport("API-Antwort unlesbar".into()),
            Failure::NoProperty => ApiError::NotConnected("Search Console: keine Property".into()),
        }
    }
}

/// Welcher Endpunkt angesprochen wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    /// `sites.list` und `searchanalytics.query`.
    Analytics,
    /// `urlInspection/index:inspect`.
    Inspection,
}

/// Ausführer der Search-Console-Anfragen.
pub struct GscHttp {
    client: reqwest::Client,
    analytics_base: String,
    inspection_base: String,
    token: Option<String>,
    read_timeout: Duration,
    retries: u32,
    clock: Arc<dyn Clock>,
    seed: AtomicU64,
    windows: Mutex<HashMap<String, VecDeque<Instant>>>,
    per_minute: usize,
}

impl GscHttp {
    /// Client für eine Abfrage: Basis-URL aus den Optionen (bzw. `OPENSPIDER_API_BASE_GSC`, INTEG-019), Netzwerk aus
    /// dem Kontext.
    pub fn new(
        ctx: &ApiCtx,
        options: &GscOptions,
        token: Option<String>,
        clock: Arc<dyn Clock>,
    ) -> Result<GscHttp, ApiError> {
        let client = crate::http::client_for(ctx)?;
        let configured = options.base_url.as_deref();
        let analytics = crate::http::base_url(
            os_model::ApiKind::Gsc,
            configured,
            ANALYTICS_BASE,
            ctx.allow_api_override,
        );
        // Mit überschriebener Basis-URL laufen beide Endpunkte über denselben Mock.
        let inspection = if analytics == ANALYTICS_BASE {
            INSPECTION_BASE.to_string()
        } else {
            analytics.clone()
        };
        Ok(GscHttp::with_client(
            client, analytics, inspection, token, options, clock,
        ))
    }

    /// Wie [`GscHttp::new`] mit eigenem Client und festen Basis-URLs.
    pub fn with_client(
        client: reqwest::Client,
        analytics_base: String,
        inspection_base: String,
        token: Option<String>,
        options: &GscOptions,
        clock: Arc<dyn Clock>,
    ) -> GscHttp {
        GscHttp {
            client,
            analytics_base: analytics_base.trim_end_matches('/').to_string(),
            inspection_base: inspection_base.trim_end_matches('/').to_string(),
            token,
            read_timeout: options.read_timeout(),
            retries: options.limits.retries,
            clock,
            seed: AtomicU64::new(0x5eed),
            windows: Mutex::default(),
            per_minute: INSPECTION_PER_MINUTE,
        }
    }

    /// Lese-Timeout je Anfrage.
    pub fn read_timeout(&self) -> Duration {
        self.read_timeout
    }

    fn url(&self, endpoint: Endpoint, path: &str) -> String {
        let base = match endpoint {
            Endpoint::Analytics => &self.analytics_base,
            Endpoint::Inspection => &self.inspection_base,
        };
        format!("{base}{path}")
    }

    /// Wartet, bis unter `key` (Property) eine weitere Anfrage im gleitenden Minutenfenster erlaubt ist. Das Fenster
    /// ist eine Sekunde länger als die Minute des Anbieters: Es zählt beim Senden, der Anbieter beim Eintreffen, und
    /// unterschiedliche Laufzeiten dürfen dort keine 601. Anfrage in eine Minute schieben.
    async fn acquire(&self, key: &str) {
        let window = Duration::from_secs(61);
        loop {
            let now = Instant::now();
            let mut windows = self.windows.lock().await;
            let sent = windows.entry(key.to_string()).or_default();
            while sent
                .front()
                .is_some_and(|t| now.duration_since(*t) >= window)
            {
                sent.pop_front();
            }
            if sent.len() < self.per_minute {
                sent.push_back(now);
                return;
            }
            let until = *sent.front().expect("volles Fenster") + window;
            drop(windows);
            tokio::time::sleep_until(until).await;
        }
    }

    /// `GET` mit JSON-Antwort.
    pub async fn get(&self, endpoint: Endpoint, path: &str) -> Result<Value, Failure> {
        let url = self.url(endpoint, path);
        self.execute(|| self.client.get(&url), None).await
    }

    /// `POST` mit JSON-Körper; `rate_key` drosselt auf 600 Anfragen je Minute unter diesem Schlüssel.
    pub async fn post(
        &self,
        endpoint: Endpoint,
        path: &str,
        body: &Value,
        rate_key: Option<&str>,
    ) -> Result<Value, Failure> {
        let url = self.url(endpoint, path);
        let bytes = serde_json::to_vec(body).map_err(|_| Failure::Invalid)?;
        self.execute(
            || {
                self.client
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .body(bytes.clone())
            },
            rate_key,
        )
        .await
    }

    async fn execute(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
        rate_key: Option<&str>,
    ) -> Result<Value, Failure> {
        let seed = self.seed.fetch_add(1, Ordering::Relaxed);
        let mut last = Failure::Transport;
        for attempt in 0..=self.retries {
            if let Some(key) = rate_key {
                self.acquire(key).await;
            }
            let mut request = build().timeout(self.read_timeout);
            if let Some(token) = &self.token {
                request = request.bearer_auth(token);
            }
            let mut delay = None;
            match request.send().await {
                Err(e) if e.is_timeout() => last = self.timeout(),
                Err(_) => return Err(Failure::Transport),
                Ok(response) => {
                    let status = response.status().as_u16();
                    let retry_after = response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| crate::http::retry_after(v, self.clock.now()));
                    match response.bytes().await {
                        // Der Körper kam nicht innerhalb des Lese-Timeouts: wie ein Timeout beim Senden.
                        Err(e) if e.is_timeout() => last = self.timeout(),
                        Err(_) => return Err(Failure::Transport),
                        Ok(body) => {
                            let json = serde_json::from_slice::<Value>(&body).ok();
                            if (200..300).contains(&status) {
                                return json.ok_or(Failure::Invalid);
                            }
                            let quota = json.as_ref().is_some_and(quota_reason);
                            match status {
                                429 | 403 if quota => return Err(Failure::Quota),
                                401 => return Err(Failure::Unauthorized),
                                403 => return Err(Failure::Forbidden),
                                429 | 500 | 502 | 503 | 504 => {
                                    last = Failure::Status(status);
                                    delay = retry_after;
                                }
                                _ => return Err(Failure::Status(status)),
                            }
                        }
                    }
                }
            }
            if attempt < self.retries {
                self.pause(attempt, seed, delay).await;
            }
        }
        Err(last)
    }

    fn timeout(&self) -> Failure {
        Failure::Timeout {
            after_s: self.read_timeout.as_secs(),
        }
    }

    async fn pause(&self, attempt: u32, seed: u64, retry_after: Option<Duration>) {
        let delay = retry_after.unwrap_or_else(|| crate::http::backoff(attempt, seed));
        tokio::time::sleep(delay).await;
    }
}

/// Meldet der Anbieter ein erschöpftes (Tages-)Kontingent?
fn quota_reason(json: &Value) -> bool {
    json.pointer("/error/status").and_then(Value::as_str) == Some("RESOURCE_EXHAUSTED")
        || json
            .pointer("/error/errors")
            .and_then(Value::as_array)
            .is_some_and(|errors| {
                errors.iter().any(|e| {
                    matches!(
                        e["reason"].as_str(),
                        Some("quotaExceeded" | "dailyLimitExceeded")
                    )
                })
            })
}

/// Kodiert eine Property-Kennung als Pfadsegment (`sc-domain:x` → `sc-domain%3Ax`).
pub fn encode_site(site: &str) -> String {
    url::form_urlencoded::byte_serialize(site.as_bytes()).collect()
}
