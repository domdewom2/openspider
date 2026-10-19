//! HTTP-Zugriff auf die Admin API und die Data API (INTEG-090, -096, -099, -101).
//!
//! Beide APIs liegen in Produktion auf eigenen Hosts; eine überschriebene Basis-URL (INTEG-019) lenkt beide auf
//! denselben Mock. Anders als der allgemeine Transport des Rahmens behält der GA4-Client die (redigierte) Meldung des
//! Anbieters bei 4xx, weil INTEG-091/-092/-101 sie im Status anzeigen bzw. auswerten. Wiederholungen bei 429/5xx folgen
//! INTEG-005 (Backoff mit Jitter, `Retry-After` hat Vorrang), `RESOURCE_EXHAUSTED` beendet den Abruf (INTEG-099).

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use os_model::config::ApiLimitsConfig;
use os_model::{ApiKind, Clock};
use serde_json::Value;
use tokio::sync::{Mutex, Semaphore};

use crate::{http, ApiCtx, ApiError};

/// Produktionshost der Admin API.
pub const ADMIN_API: &str = "https://analyticsadmin.googleapis.com";
/// Produktionshost der Data API.
pub const DATA_API: &str = "https://analyticsdata.googleapis.com";
/// Höchstens so viele gleichzeitige Anfragen je Property (INTEG-099).
pub const MAX_CONCURRENT_PER_PROPERTY: usize = 10;

/// Basis-URLs beider APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    /// Admin API (Konten, Properties, Datenstreams).
    pub admin: String,
    /// Data API (Reports, Metadaten, Kompatibilität).
    pub data: String,
}

impl Endpoints {
    /// Endpunkte für einen Abruf: Produktion oder die freigegebene Überschreibung `api.ga4.base_url` (INTEG-019).
    pub fn for_ctx(ctx: &ApiCtx) -> Endpoints {
        let base = http::base_url(
            ApiKind::Ga4,
            ctx.cfg.api.ga4.base_url.as_deref(),
            ADMIN_API,
            ctx.allow_api_override,
        );
        if base == ADMIN_API {
            Endpoints {
                admin: ADMIN_API.into(),
                data: DATA_API.into(),
            }
        } else {
            let base = base.trim_end_matches('/').to_string();
            Endpoints {
                admin: base.clone(),
                data: base,
            }
        }
    }
}

/// Ein Konto mit seinen Properties (Admin API `accountSummaries`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ga4Account {
    /// Ressourcenname `accounts/<id>`.
    pub name: String,
    /// Anzeigename.
    pub display_name: String,
    /// GA4-Properties des Kontos.
    pub properties: Vec<Ga4Property>,
}

/// Eine Property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ga4Property {
    /// Ressourcenname `properties/<id>`.
    pub name: String,
    /// Anzeigename.
    pub display_name: String,
}

/// Ein Web-Datenstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ga4Stream {
    /// Ressourcenname `properties/<id>/dataStreams/<id>`.
    pub name: String,
    /// Anzeigename.
    pub display_name: String,
    /// Hostname der Standard-URL (`integ.test`).
    pub host: Option<String>,
}

/// Stammdaten einer Property.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropertyInfo {
    /// Währung (`currencyCode`).
    pub currency: Option<String>,
    /// Zeitzone (`timeZone`).
    pub time_zone: Option<String>,
}

/// Wiederholungsstrategie einer Anfrage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retry {
    /// Nach INTEG-005.
    Default,
    /// Genau ein Versuch (Prüfabruf, Kompatibilität).
    Once,
}

/// Client einer Property bzw. eines Kontos.
pub struct Ga4Client {
    client: reqwest::Client,
    endpoints: Endpoints,
    token: String,
    limits: ApiLimitsConfig,
    permits: Arc<Semaphore>,
    window: Mutex<VecDeque<tokio::time::Instant>>,
    clock: Arc<dyn Clock>,
}

impl Ga4Client {
    /// Client für einen Abruf; `permits` begrenzt die gleichzeitigen Anfragen (je Property geteilt).
    pub fn new(
        ctx: &ApiCtx,
        token: &str,
        permits: Arc<Semaphore>,
        clock: Arc<dyn Clock>,
    ) -> Result<Ga4Client, ApiError> {
        Ok(Ga4Client {
            client: http::client_for(ctx)?,
            endpoints: Endpoints::for_ctx(ctx),
            token: token.to_string(),
            limits: ctx.cfg.api.ga4.limits.clone(),
            permits,
            window: Mutex::default(),
            clock,
        })
    }

    /// Endpunkte dieses Clients.
    pub fn endpoints(&self) -> &Endpoints {
        &self.endpoints
    }

    async fn acquire_window(&self) {
        let Some(limit) = self.limits.requests_per_window else {
            return;
        };
        let window = Duration::from_secs(u64::from(self.limits.window_s.max(1)));
        loop {
            let now = tokio::time::Instant::now();
            let mut sent = self.window.lock().await;
            while sent
                .front()
                .is_some_and(|t| now.duration_since(*t) >= window)
            {
                sent.pop_front();
            }
            if sent.len() < limit as usize {
                sent.push_back(now);
                return;
            }
            let until = *sent.front().expect("nicht leer") + window;
            drop(sent);
            tokio::time::sleep_until(until).await;
        }
    }

    /// Redigierte Meldung des Anbieters (ohne Token, ohne Steuerzeichen, höchstens 300 Zeichen).
    fn redact(&self, message: &str) -> String {
        let message = if self.token.is_empty() {
            message.to_string()
        } else {
            message.replace(&self.token, "[redacted]")
        };
        message
            .chars()
            .filter(|c| !c.is_control())
            .take(300)
            .collect()
    }

    /// Sendet eine Anfrage und liefert die JSON-Antwort.
    pub async fn send(
        &self,
        request: reqwest::RequestBuilder,
        retry: Retry,
    ) -> Result<Value, ApiError> {
        let _permit = self
            .permits
            .acquire()
            .await
            .map_err(|_| ApiError::Transport("GA4-Abruf beendet".into()))?;
        let retries = match retry {
            Retry::Default => self.limits.retries,
            Retry::Once => 0,
        };
        let request = request.bearer_auth(&self.token);
        for attempt in 0..=retries {
            self.acquire_window().await;
            let response = request
                .try_clone()
                .ok_or_else(|| ApiError::Transport("GA4-Anfrage nicht wiederholbar".into()))?
                .send()
                .await
                .map_err(|_| ApiError::Transport("GA4-Verbindung fehlgeschlagen".into()))?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| http::retry_after(v, self.clock.now()));
            let bytes = response
                .bytes()
                .await
                .map_err(|_| ApiError::Transport("GA4-Antwort unlesbar".into()))?;
            let json: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            if (200..300).contains(&status) {
                return Ok(json);
            }
            let message = self.redact(
                json.pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("GA4-Anfrage abgelehnt"),
            );
            if json.pointer("/error/status").and_then(Value::as_str) == Some("RESOURCE_EXHAUSTED") {
                return Err(ApiError::QuotaExceeded(message));
            }
            let retryable = matches!(status, 429 | 500 | 502 | 503 | 504);
            if !retryable || attempt == retries {
                return Err(match status {
                    401 | 403 => ApiError::Unauthorized(format!("HTTP {status}: {message}")),
                    429 => ApiError::RateLimited(format!("HTTP 429: {message}")),
                    _ => ApiError::Provider { status, message },
                });
            }
            let seed = self.clock.now().unix_millis() as u64;
            tokio::time::sleep(retry_after.unwrap_or_else(|| http::backoff(attempt, seed))).await;
        }
        unreachable!("die Schleife endet mit dem letzten Versuch")
    }

    fn get(&self, base: &str, path: &str) -> reqwest::RequestBuilder {
        self.client.get(format!("{base}{path}"))
    }

    fn post(&self, path: &str, body: &Value) -> reqwest::RequestBuilder {
        self.client
            .post(format!("{}{path}", self.endpoints.data))
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.to_string())
    }

    /// Konten und Properties (Admin API, seitenweise über `pageToken`).
    pub async fn account_summaries(&self, retry: Retry) -> Result<Vec<Ga4Account>, ApiError> {
        let mut out = Vec::new();
        let mut token: Option<String> = None;
        loop {
            let mut path = "/v1beta/accountSummaries?pageSize=200".to_string();
            if let Some(t) = &token {
                path.push_str("&pageToken=");
                path.push_str(
                    &url::form_urlencoded::byte_serialize(t.as_bytes()).collect::<String>(),
                );
            }
            let page = self
                .send(self.get(&self.endpoints.admin, &path), retry)
                .await?;
            for a in page["accountSummaries"].as_array().into_iter().flatten() {
                out.push(Ga4Account {
                    name: a["account"].as_str().unwrap_or_default().to_string(),
                    display_name: a["displayName"].as_str().unwrap_or_default().to_string(),
                    properties: a["propertySummaries"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|p| {
                            p["propertyType"]
                                .as_str()
                                .is_none_or(|t| t == "PROPERTY_TYPE_ORDINARY")
                        })
                        .map(|p| Ga4Property {
                            name: p["property"].as_str().unwrap_or_default().to_string(),
                            display_name: p["displayName"].as_str().unwrap_or_default().to_string(),
                        })
                        .collect(),
                });
            }
            token = page["nextPageToken"]
                .as_str()
                .filter(|t| !t.is_empty())
                .map(str::to_string);
            if token.is_none() {
                return Ok(out);
            }
        }
    }

    /// Web-Datenstreams einer Property.
    pub async fn data_streams(&self, property: &str) -> Result<Vec<Ga4Stream>, ApiError> {
        let page = self
            .send(
                self.get(
                    &self.endpoints.admin,
                    &format!("/v1beta/properties/{property}/dataStreams"),
                ),
                Retry::Default,
            )
            .await?;
        Ok(page["dataStreams"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["type"].as_str() == Some("WEB_DATA_STREAM"))
            .map(|s| Ga4Stream {
                name: s["name"].as_str().unwrap_or_default().to_string(),
                display_name: s["displayName"].as_str().unwrap_or_default().to_string(),
                host: s
                    .pointer("/webStreamData/defaultUri")
                    .and_then(Value::as_str)
                    .and_then(stream_host),
            })
            .collect())
    }

    /// Währung und Zeitzone einer Property.
    pub async fn property(&self, property: &str) -> Result<PropertyInfo, ApiError> {
        let p = self
            .send(
                self.get(
                    &self.endpoints.admin,
                    &format!("/v1beta/properties/{property}"),
                ),
                Retry::Default,
            )
            .await?;
        Ok(PropertyInfo {
            currency: p["currencyCode"].as_str().map(str::to_string),
            time_zone: p["timeZone"].as_str().map(str::to_string),
        })
    }

    /// Metadaten der Property (`getMetadata`).
    pub async fn metadata(&self, property: &str) -> Result<Value, ApiError> {
        self.send(
            self.get(
                &self.endpoints.data,
                &format!("/v1beta/properties/{property}/metadata"),
            ),
            Retry::Default,
        )
        .await
    }

    /// `checkCompatibility` (ein Versuch, INTEG-101).
    pub async fn check_compatibility(
        &self,
        property: &str,
        body: &Value,
    ) -> Result<Value, ApiError> {
        self.send(
            self.post(
                &format!("/v1beta/properties/{property}:checkCompatibility"),
                body,
            ),
            Retry::Once,
        )
        .await
    }

    /// `runReport`.
    pub async fn run_report(&self, property: &str, body: &Value) -> Result<Value, ApiError> {
        self.send(
            self.post(&format!("/v1beta/properties/{property}:runReport"), body),
            Retry::Default,
        )
        .await
    }
}

/// Hostname aus der Standard-URL eines Datenstreams (`https://integ.test` → `integ.test`).
pub fn stream_host(uri: &str) -> Option<String> {
    let parsed = url::Url::parse(uri)
        .or_else(|_| url::Url::parse(&format!("https://{uri}")))
        .ok()?;
    parsed.host_str().map(str::to_lowercase)
}
