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

/// Passwortgeschützter Konfigurationsexport (INTEG-002, K2-52).
/// Format: magic/version (8), Argon2id-Salt (16), GCM-Nonce (12), Ciphertext + Tag.
/// Argon2id v19: m=19456 KiB, t=2, p=1; AES-256-GCM authentifiziert auch den Header.
/// Fehler enthalten weder Passwort noch Nutzdaten.
pub fn encrypt_config_export(plain: &[u8], password: &str) -> Result<Vec<u8>, ApiError> {
    use aes_gcm::{
        aead::{rand_core::RngCore, Aead, KeyInit, OsRng, Payload},
        Aes256Gcm,
    };
    if password.is_empty() {
        return Err(ApiError::Transport("Export-Passwort fehlt".into()));
    }
    let mut header = Vec::from(*b"OSCFG\0\0\x01");
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    header.extend_from_slice(&salt);
    header.extend_from_slice(&nonce);
    let mut key = os_store::profile::config_export_key(password, &salt)
        .map_err(|_| ApiError::Transport("Export-Schlüsselableitung fehlgeschlagen".into()))?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| ApiError::Transport("Export-Verschlüsselung fehlgeschlagen".into()))?;
    key.fill(0);
    let body = cipher
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: plain,
                aad: &header,
            },
        )
        .map_err(|_| ApiError::Transport("Export-Verschlüsselung fehlgeschlagen".into()))?;
    header.extend_from_slice(&body);
    Ok(header)
}

/// Entschlüsselt das portable Exportformat; falsches Passwort oder beschädigte Daten
/// liefern dieselbe Meldung. Liest keine Dateien und schreibt keine Geheimnisse.
pub fn decrypt_config_export(bytes: &[u8], password: &str) -> Result<Vec<u8>, ApiError> {
    use aes_gcm::{
        aead::{Aead, KeyInit, Payload},
        Aes256Gcm,
    };
    let fail = || ApiError::Transport("Export nicht entschlüsselbar".into());
    if bytes.len() < 52 || &bytes[..8] != b"OSCFG\0\0\x01" || password.is_empty() {
        return Err(fail());
    }
    let mut key =
        os_store::profile::config_export_key(password, &bytes[8..24]).map_err(|_| fail())?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| fail())?;
    key.fill(0);
    cipher
        .decrypt(
            bytes[24..36].into(),
            Payload {
                msg: &bytes[36..],
                aad: &bytes[..36],
            },
        )
        .map_err(|_| fail())
}
