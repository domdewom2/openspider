//! Rahmen der API-Quellen (04 A): das Trait [`ApiSource`] und seine Hilfstypen.
//!
//! Jede Quelle (GA4, Search Console, PageSpeed, Majestic, Ahrefs, Moz) setzt [`ApiSource`] um: sie liefert zu einer
//! Menge URLs [`ApiValueWrite`]s, die der Crawl-Pfad über `WriteBatch.api_values` in die Crawl-Datei schreibt (Schema v3,
//! `api_values`). Die Futures sind geboxt, damit das Trait objektsicher bleibt (`Arc<dyn ApiSource>` in Engine und App).
//!
//! M2-F02 legt Trait und Typen an; Ablauf, Warteschlange, Wiederholungen und Ratenbegrenzung liefert
//! M2-I01 (`queue`), die Quellen die Pakete I02 – I05.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use os_fetch::SecretStore;
use os_model::{ApiKind, ApiValueWrite, CrawlConfig, UrlId};

/// Geboxtes, sendbares Future (Rückgabe der asynchronen Trait-Methoden).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Fehler beim Abruf einer Quelle. Die Meldungen enthalten nie Geheimnisse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApiError {
    /// Noch nicht umgesetzt (Stub).
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
    /// Die Quelle ist nicht verbunden (kein Token, keine Zugangsdaten).
    #[error("nicht verbunden: {0}")]
    NotConnected(String),
    /// Anmeldung abgelaufen oder verweigert.
    #[error("Anmeldung fehlgeschlagen: {0}")]
    Unauthorized(String),
    /// Tageslimit oder Ratenbegrenzung erreicht (die Warteschlange pausiert).
    #[error("Limit erreicht: {0}")]
    RateLimited(String),
    /// Anbieter- oder lokales Tageskontingent erschöpft.
    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),
    /// Redigierte Antwort des Verbindungs-Prüfabrufs.
    #[error("HTTP {status}: {message}")]
    Provider { status: u16, message: String },
    /// Netzwerk- oder Protokollfehler.
    #[error("Abruf fehlgeschlagen: {0}")]
    Transport(String),
}

impl ApiError {
    pub fn http_status(&self) -> Option<u16> {
        if let Self::Provider { status, .. } = self {
            return Some(*status);
        }
        let message = match self {
            Self::Unauthorized(s)
            | Self::Transport(s)
            | Self::RateLimited(s)
            | Self::QuotaExceeded(s) => s,
            _ => return None,
        };
        let status = message
            .strip_prefix("HTTP ")?
            .get(..3)?
            .parse::<u16>()
            .ok()?;
        (100..=599).contains(&status).then_some(status)
    }
    /// Anbietertexte können Zugangsdaten spiegeln; öffentliche Meldungen bleiben kontrolliert.
    pub fn public_message(&self) -> String {
        match self {
            Self::NotImplemented(_) => "API-Quelle noch nicht verfügbar".into(),
            Self::NotConnected(_) => "API nicht verbunden".into(),
            Self::Unauthorized(_) => "Anmeldung abgelaufen – bitte neu verbinden".into(),
            Self::RateLimited(_) => "Anfragelimit erreicht".into(),
            Self::QuotaExceeded(_) => "Quota exceeded".into(),
            Self::Provider { status, message } => format!("HTTP {status}: {message}"),
            Self::Transport(_) => "API-Abruf fehlgeschlagen".into(),
        }
    }
}

/// Eine URL, zu der Werte abgerufen werden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlRef {
    /// Kennung der URL in der Crawl-Datei; `None` für URLs, die die Quelle liefert und der Crawl nicht kennt.
    pub url_id: Option<UrlId>,
    /// Adresse der URL (URL Encoded Address).
    pub url: String,
}

/// Zustand einer Quelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiState {
    /// Nicht verbunden.
    Disconnected,
    /// Verbunden, im Leerlauf.
    Idle,
    /// Ruft gerade Werte ab.
    Fetching,
    /// Pausiert (Limit erreicht).
    Paused,
    /// Letzter Abruf mit Fehler beendet.
    Failed,
}

/// Stand einer Quelle für Statuszeile und `api_status` (UI-081).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiStatus {
    /// Quelle.
    pub kind: ApiKind,
    /// Zustand.
    pub state: ApiState,
    /// URLs gesamt.
    pub urls_total: u64,
    /// URLs abgerufen.
    pub urls_done: u64,
    /// URLs mit Fehler.
    pub urls_failed: u64,
    /// Hinweis (z. B. Grund der Pause), ohne Geheimnisse.
    pub detail: Option<String>,
}

/// Umgebung eines Abrufs: Konfiguration des Crawls und Geheimnisse.
#[derive(Clone)]
pub struct ApiCtx {
    /// Wirksame Konfiguration (Abschnitt `api` und die Zeiträume).
    pub cfg: CrawlConfig,
    /// Quelle der Geheimnisse (Schlüsselbund bzw. Speicher-Store in Tests, C13).
    pub secrets: Arc<dyn SecretStore>,
    /// App-weite Netzwerkeinstellungen (K2-33).
    pub network: os_model::settings::NetworkSettings,
    /// Explizite Laufzeitfreigabe für Basis-URLs im Release.
    pub allow_api_override: bool,
}

/// Vom Anbieter gemeldete Kontingentdimension, etwa GA4 tokensPerDay/tokensPerHour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderQuota {
    pub dimension: String,
    pub used: u64,
    pub limit: Option<u64>,
    pub resets_at: Option<os_model::Timestamp>,
}

/// Ergebnisse eines Abrufs, einschließlich URL-bezogener Anbieterfehler.
pub type FetchOutcomes = Vec<(UrlRef, Result<Vec<ApiValueWrite>, ApiError>)>;

/// Eine API-Quelle.
pub trait ApiSource: Send + Sync {
    /// Welche Quelle das ist.
    fn kind(&self) -> ApiKind;

    /// Ruft die Werte zu `urls` ab.
    fn fetch<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, Result<Vec<ApiValueWrite>, ApiError>>;

    /// URL-bezogene Ergebnisse für Anbieter mit gemischten Bündelantworten.
    /// Bestehende Quellen behalten ihren bisherigen Alles-oder-Fehler-Vertrag.
    fn fetch_outcomes<'a>(
        &'a self,
        urls: &'a [UrlRef],
        ctx: &'a ApiCtx,
    ) -> BoxFuture<'a, FetchOutcomes> {
        Box::pin(async move {
            let result = self.fetch(urls, ctx).await;
            urls.iter()
                .map(|url| {
                    let outcome = result
                        .as_ref()
                        .map(|values| {
                            values
                                .iter()
                                .filter(|value| value.url_id == url.url_id && value.url == url.url)
                                .cloned()
                                .collect()
                        })
                        .map_err(Clone::clone);
                    (url.clone(), outcome)
                })
                .collect()
        })
    }

    /// Genau ein Prüfabruf. Quellen können Kontoname, Ablauf und Scopes ergänzen.
    /// I02/I03 müssen account_id und label aus der Property-/Site-Liste oder Konto-Antwort
    /// setzen: OAuth mit minimalem Scope liefert selbst keine stabile Google-Identität.
    fn test_connection<'a>(
        &'a self,
        ctx: &'a ApiCtx,
        credential: &'a crate::credentials::Credential,
    ) -> BoxFuture<'a, Result<crate::credentials::Credential, ApiError>> {
        Box::pin(async move {
            crate::http::probe(
                self.kind(),
                ctx,
                credential
                    .api_key
                    .as_deref()
                    .or(credential.access_token.as_deref()),
            )
            .await?;
            Ok(credential.clone())
        })
    }

    /// Kontingente aus der letzten Anbieterantwort (leere Liste vor dem ersten Abruf).
    fn quotas(&self) -> Vec<ProviderQuota> {
        Vec::new()
    }

    /// Aktueller Stand der Quelle.
    fn status(&self) -> ApiStatus;
}
