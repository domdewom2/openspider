//! Abruf-Schnittstelle: `Fetch`-Trait, Anfrage und Ergebnis, dazu Hilfen für Statustexte und
//! Statusklassen (CRAWL-012, ONPAGE-065).

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
    /// Reason-Phrase der Antwort, nur gesetzt, wenn die gesendete Phrase vom IANA-Standardtext des Statuscodes
    /// abweicht. Nicht-ASCII-Phrasen liefern hyper/httparse nicht; dann ist der Wert `None`. Quelle für die
    /// Anzeige bleibt [`status_text`].
    pub reason_phrase: Option<String>,
    /// Anzeigetext (Reason-Phrase oder IANA-Standardtext).
    pub status_text: String,
    /// HTTP-Version, z. B. `HTTP/1.1`.
    pub http_version: String,
    /// Header in Antwortreihenfolge (Namen klein). Werte sensibler Header (`Set-Cookie`,
    /// `Authorization` …) sind bereits durch `<redacted>` ersetzt (K-13, OUT-243).
    pub headers: Vec<(String, String)>,
    /// Body (dekomprimiert, ggf. gekürzt).
    pub body: Bytes,
    /// Größe des dekomprimierten Bodys in Bytes.
    pub size_bytes: u64,
    /// Übertragene Bytes (K-02, CRAWL-022): die über die Leitung empfangenen Body-Bytes –
    /// komprimiert, ohne Statuszeile, Header, Chunk-Rahmen und 1xx-Zwischenantworten; Antworten
    /// ohne Body (HEAD, 204, 304) → 0. Die Größe des Kopfs wird nicht gespeichert.
    pub transferred_bytes: u64,
    /// Antwortzeit in Millisekunden.
    pub response_time_ms: u32,
    /// Body wegen `max_body_bytes` gekürzt.
    pub truncated: bool,
    /// Defekte Kompression (CRAWL-011): Detailtext des Fehlers. Der empfangene `status` bleibt
    /// erhalten, `status_text` ist dann „Connection Error“; `body` enthält höchstens den bis zum
    /// Fehler dekodierten Anfang.
    pub decode_error: Option<String>,
    /// MD5 des Bodys (hex).
    pub md5_hex: String,
}

/// Anzeigetext zum Statuscode: die gesendete Reason-Phrase, sonst der IANA-Standardtext, für
/// unbekannte Codes leer (CRAWL-012).
pub fn status_text(status: u16, reason_phrase: Option<&str>) -> String {
    match reason_phrase.map(str::trim).filter(|r| !r.is_empty()) {
        Some(reason) => reason.to_string(),
        None => iana_reason(status).unwrap_or("").to_string(),
    }
}

/// Standardtext laut IANA-Registry „HTTP Status Codes“; `None` für nicht registrierte, nicht
/// zugewiesene und „(Unused)“-Codes (z. B. 218, 306, 418, 499, 520).
pub fn iana_reason(status: u16) -> Option<&'static str> {
    Some(match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Range Not Satisfiable",
        417 => "Expectation Failed",
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended (OBSOLETED)",
        511 => "Network Authentication Required",
        _ => return None,
    })
}

/// Klasse eines Statuscodes für die Filter des Tabs „Response Codes“ (CRAWL-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusClass {
    /// Keine Antwort (Statuscode 0).
    NoResponse,
    /// 1xx (nur als Endstatus, z. B. 101; Zwischenantworten werden übersprungen).
    Informational,
    /// 2xx.
    Success,
    /// 3xx.
    Redirection,
    /// 4xx.
    ClientError,
    /// 5xx.
    ServerError,
    /// OS-Erweiterung „Sonstige“: Codes außerhalb von 100–599.
    Other,
}

impl StatusClass {
    /// Klasse zum Statuscode; jeder Code gehört genau zu einer Klasse.
    pub fn of(status: u16) -> Self {
        match status {
            0 => StatusClass::NoResponse,
            100..=199 => StatusClass::Informational,
            200..=299 => StatusClass::Success,
            300..=399 => StatusClass::Redirection,
            400..=499 => StatusClass::ClientError,
            500..=599 => StatusClass::ServerError,
            _ => StatusClass::Other,
        }
    }
}

/// HTTP-Version als Anzeigetext (`HTTP/1.1`, `HTTP/2`).
pub(crate) fn version_label(v: reqwest::Version) -> String {
    match v {
        reqwest::Version::HTTP_09 => "HTTP/0.9".to_string(),
        reqwest::Version::HTTP_10 => "HTTP/1.0".to_string(),
        reqwest::Version::HTTP_11 => "HTTP/1.1".to_string(),
        reqwest::Version::HTTP_2 => "HTTP/2".to_string(),
        reqwest::Version::HTTP_3 => "HTTP/3".to_string(),
        other => format!("{other:?}"),
    }
}

/// Länge von Statuszeile und Header-Block, wie sie über die Leitung gingen (`HTTP/1.1 200 OK\r\n`,
/// je Header `name: wert\r\n`, abschließend `\r\n`). Seit K-02 (N08) nicht mehr Teil von
/// `Übertragen` (`transferred_bytes` zählt nur den Body); nur noch als Messhilfe getestet.
#[allow(dead_code)]
pub(crate) fn head_len(
    version: &str,
    status: u16,
    status_text: &str,
    headers: &[(String, String)],
) -> u64 {
    let mut line = format!("{version} {status}");
    // HTTP/1.x: `HTTP-version SP status-code SP [reason-phrase]` – das zweite Leerzeichen steht immer.
    if version.starts_with("HTTP/1") || !status_text.is_empty() {
        line.push(' ');
        line.push_str(status_text);
    }
    let mut n = line.len() + 2;
    for (name, value) in headers {
        n += name.len() + 2 + value.len() + 2;
    }
    (n + 2) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: CRAWL-012, ONPAGE-065
    #[test]
    fn reason_phrase_wins_otherwise_iana_otherwise_empty() {
        assert_eq!(status_text(200, Some("Alles Gut")), "Alles Gut");
        assert_eq!(status_text(200, Some("  ")), "OK");
        assert_eq!(status_text(200, None), "OK");
        assert_eq!(status_text(404, None), "Not Found");
        assert_eq!(status_text(413, None), "Content Too Large");
        for unknown in [218, 299, 418, 499, 520, 530, 600, 999, 306, 509] {
            assert_eq!(status_text(unknown, None), "", "{unknown}");
        }
        // gesendeter Text hat auch bei unbekanntem Code Vorrang
        assert_eq!(status_text(218, Some("This is fine")), "This is fine");
    }

    // spec: CRAWL-012
    #[test]
    fn every_listed_fixture_code_has_exactly_one_class() {
        let cases = [
            (0, StatusClass::NoResponse),
            (103, StatusClass::Informational),
            (200, StatusClass::Success),
            (204, StatusClass::Success),
            (218, StatusClass::Success),
            (299, StatusClass::Success),
            (301, StatusClass::Redirection),
            (304, StatusClass::Redirection),
            (308, StatusClass::Redirection),
            (400, StatusClass::ClientError),
            (418, StatusClass::ClientError),
            (451, StatusClass::ClientError),
            (499, StatusClass::ClientError),
            (500, StatusClass::ServerError),
            (520, StatusClass::ServerError),
            (599, StatusClass::ServerError),
            (600, StatusClass::Other),
            (999, StatusClass::Other),
        ];
        for (code, class) in cases {
            assert_eq!(StatusClass::of(code), class, "{code}");
        }
    }

    // spec: CRAWL-011
    #[test]
    fn head_length_counts_status_line_headers_and_blank_line() {
        let headers = vec![
            ("content-type".to_string(), "text/html".to_string()),
            ("content-length".to_string(), "5".to_string()),
        ];
        let wire = "HTTP/1.1 200 OK\r\ncontent-type: text/html\r\ncontent-length: 5\r\n\r\n";
        assert_eq!(head_len("HTTP/1.1", 200, "OK", &headers), wire.len() as u64);
        // HTTP/1.x: auch ohne Reason-Phrase steht das Leerzeichen nach dem Code
        assert_eq!(
            head_len("HTTP/1.1", 218, "", &[]),
            "HTTP/1.1 218 \r\n\r\n".len() as u64
        );
        assert_eq!(
            head_len("HTTP/2", 218, "", &[]),
            "HTTP/2 218\r\n\r\n".len() as u64
        );
    }
}
