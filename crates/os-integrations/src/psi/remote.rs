//! Remote-Quelle: PageSpeed-Insights-API v5 (INTEG-130, INTEG-131, INTEG-142, INTEG-143).

use os_model::{ApiKind, PsiDevice};
use serde_json::Value;

use crate::{http, ApiCtx, ApiError};

/// Produktive Basis-URL der API.
pub const PRODUCTION_BASE: &str = "https://www.googleapis.com";
/// Pfad des Endpunkts.
pub const RUN_PATH: &str = "/pagespeedonline/v5/runPagespeed";
/// URL, gegen die „Verbinden“ den Schlüssel prüft.
pub const PROBE_URL: &str = "https://example.com/";

/// Prüfliste unter der Meldung „PSI-Zugriff abgelehnt“ (INTEG-130).
pub const ACCESS_CHECKLIST: [&str; 3] = [
    "Ist im Google-Cloud-Projekt des Schlüssels die „PageSpeed Insights API“ freigeschaltet?",
    "Wurde der Schlüssel eben erst angelegt? Dann später erneut verbinden.",
    "Ist der Schlüssel auf bestimmte APIs oder Referrer beschränkt?",
];

/// Basis-URL laut Konfiguration bzw. Mock-Überschreibung (INTEG-019).
pub fn base(ctx: &ApiCtx) -> String {
    http::base_url(
        ApiKind::Psi,
        ctx.cfg.api.psi.base_url.as_deref(),
        PRODUCTION_BASE,
        ctx.allow_api_override,
    )
}

/// Request-URL für eine Seite: `url`, `key`, `strategy` (Standard `mobile`, INTEG-131), `category=performance`.
pub fn request_url(
    base: &str,
    page: &str,
    key: &str,
    device: PsiDevice,
) -> Result<url::Url, ApiError> {
    let mut url = url::Url::parse(&format!("{}{RUN_PATH}", base.trim_end_matches('/')))
        .map_err(|_| ApiError::Transport("Ungültige API-Basis-URL".into()))?;
    url.query_pairs_mut()
        .append_pair("url", page)
        .append_pair("key", key)
        .append_pair("strategy", super::settings::strategy(device))
        .append_pair("category", "performance");
    Ok(url)
}

/// Abgelehnter Zugriff beim Verbinden (403 `SERVICE_DISABLED`, 400 `API_KEY_INVALID`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDenied {
    /// HTTP-Status.
    pub status: u16,
    /// Grund laut Google (`SERVICE_DISABLED`, `API_KEY_INVALID`, sonst der Fehlerstatus).
    pub reason: String,
}

impl AccessDenied {
    /// Statuszeile `Fehler: PSI-Zugriff abgelehnt (<Grund>)`.
    pub fn status_line(&self) -> String {
        format!("Fehler: PSI-Zugriff abgelehnt ({})", self.reason)
    }

    /// Prüfliste unter der Statuszeile.
    pub fn checklist(&self) -> &'static [&'static str; 3] {
        &ACCESS_CHECKLIST
    }
}

/// Grund einer Google-Fehlerantwort: `details[].reason`, sonst `errors[].reason`, sonst `error.status`.
pub fn error_reason(body: &Value) -> Option<String> {
    let err = body.get("error")?;
    let from_details = err.get("details").and_then(Value::as_array).and_then(|d| {
        d.iter()
            .find_map(|x| x.get("reason").and_then(Value::as_str))
    });
    from_details
        .or_else(|| err.pointer("/errors/0/reason").and_then(Value::as_str))
        .or_else(|| err.get("status").and_then(Value::as_str))
        .map(|s| {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
                .take(64)
                .collect()
        })
}

/// Ergebnis von „Verbinden“ (Remote).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectError {
    /// Google hat den Schlüssel abgelehnt.
    Denied(AccessDenied),
    /// Netz- oder sonstiger Fehler.
    Other(ApiError),
}

impl From<ConnectError> for ApiError {
    fn from(e: ConnectError) -> Self {
        match e {
            ConnectError::Denied(d) => ApiError::Provider {
                status: d.status,
                message: d.status_line(),
            },
            ConnectError::Other(e) => e,
        }
    }
}

/// „Verbinden“ (Remote): genau eine Testanfrage mit dem Schlüssel, ohne Wiederholung (INTEG-130).
pub async fn check_key(ctx: &ApiCtx, key: &str) -> Result<(), ConnectError> {
    if key.trim().is_empty() {
        return Err(ConnectError::Other(ApiError::NotConnected(
            "PSI-API-Schlüssel fehlt".into(),
        )));
    }
    let url = request_url(&base(ctx), PROBE_URL, key, ctx.cfg.api.psi.device)
        .map_err(ConnectError::Other)?;
    let response = http::client_for(ctx)
        .map_err(ConnectError::Other)?
        .get(url)
        .send()
        .await
        .map_err(|_| {
            ConnectError::Other(ApiError::Transport("API-Verbindung fehlgeschlagen".into()))
        })?;
    let status = response.status().as_u16();
    if response.status().is_success() {
        return Ok(());
    }
    let body: Value = response
        .bytes()
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    match status {
        400 | 401 | 403 => Err(ConnectError::Denied(AccessDenied {
            status,
            reason: error_reason(&body).unwrap_or_else(|| format!("HTTP {status}")),
        })),
        _ => Err(ConnectError::Other(ApiError::Transport(format!(
            "HTTP {status}: API-Anfrage fehlgeschlagen"
        )))),
    }
}
