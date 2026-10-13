//! Abruf-Schnittstelle: `Fetch`-Trait, Anfrage und Ergebnis.
//!
//! STUB(M1-P02): P02 ergänzt nur Hilfsfunktionen, die Signaturen bleiben.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use os_model::{NoResponseReason, Timestamp};

use crate::cookies::CookieJar;

/// Abruf eines einzelnen URL-Objekts. Redirects verfolgt der Aufrufer (die Engine), nicht der Fetcher.
pub trait Fetch: Send + Sync + 'static {
    /// Ruft die URL ab.
    fn fetch(&self, req: FetchRequest) -> Pin<Box<dyn Future<Output = FetchResult> + Send + '_>>;
}

/// Zweck des Abrufs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchPurpose {
    /// HTML-Seite oder andere URL.
    Page,
    /// Ressource einer Seite (Bild, CSS, JavaScript …).
    Resource,
    /// Externe URL (Statusprüfung).
    External,
    /// robots.txt.
    RobotsTxt,
    /// XML-Sitemap.
    Sitemap,
}

/// Anfrage.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// Abzurufende URL.
    pub url: url::Url,
    /// Zweck.
    pub purpose: FetchPurpose,
    /// Cookie-Speicher der Crawl-Sitzung (je nach `advanced.cookies`).
    pub cookies: Option<Arc<CookieJar>>,
    /// Höchstzahl gelesener Body-Bytes (dekomprimiert).
    pub max_body_bytes: u64,
}

/// Ergebnis eines Abrufs.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// Abgerufene URL.
    pub url: url::Url,
    /// Startzeitpunkt (Uhr des Fetchers).
    pub started_at: Timestamp,
    /// Antwort oder Fehler.
    pub outcome: FetchOutcome,
}

/// Antwort oder Fehlschlag.
#[derive(Debug, Clone)]
pub enum FetchOutcome {
    /// HTTP-Antwort.
    Response(HttpResponse),
    /// Keine Antwort (Status 0).
    NoResponse {
        /// Grund.
        reason: NoResponseReason,
        /// Technische Einzelheit für das Protokoll.
        detail: String,
        /// Verstrichene Zeit bis zum Fehlschlag.
        elapsed_ms: u32,
    },
}

/// HTTP-Antwort.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Statuscode.
    pub status: u16,
    /// Reason-Phrase der Antwort, falls gesendet.
    pub reason_phrase: Option<String>,
    /// Anzeigetext (Reason-Phrase oder IANA-Standardtext).
    pub status_text: String,
    /// HTTP-Version, z. B. `HTTP/1.1`.
    pub http_version: String,
    /// Header in Antwortreihenfolge.
    pub headers: Vec<(String, String)>,
    /// Body (dekomprimiert, ggf. gekürzt).
    pub body: Bytes,
    /// Größe des dekomprimierten Bodys in Bytes.
    pub size_bytes: u64,
    /// Übertragene Bytes (Kopf + komprimierter Body).
    pub transferred_bytes: u64,
    /// Antwortzeit in Millisekunden.
    pub response_time_ms: u32,
    /// Body wegen `max_body_bytes` gekürzt.
    pub truncated: bool,
    /// Defekte Kompression (Status „Connection Error“).
    pub decode_error: Option<String>,
    /// MD5 des Bodys (hex).
    pub md5_hex: String,
}
