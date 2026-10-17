//! Rahmen der API-Quellen (04 A): das Trait [`ApiSource`] und seine Hilfstypen.
//!
//! Jede Quelle (GA4, Search Console, PageSpeed, Majestic, Ahrefs, Moz) setzt [`ApiSource`] um: sie liefert zu einer
//! Menge URLs [`ApiValueWrite`]s, die der Crawl-Pfad über `WriteBatch.api_values` in die Crawl-Datei schreibt (Schema v3,
//! `api_values`). Die Futures sind geboxt, damit das Trait objektsicher bleibt (`Arc<dyn ApiSource>` in Engine und App).
//!
//! STUB(M2-I01): M2-F02 legt Trait und Typen an; Ablauf, Warteschlange, Wiederholungen und Ratenbegrenzung liefert
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
    /// Netzwerk- oder Protokollfehler.
    #[error("Abruf fehlgeschlagen: {0}")]
    Transport(String),
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
}

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

    /// Aktueller Stand der Quelle.
    fn status(&self) -> ApiStatus;
}
