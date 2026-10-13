//! Cookie-Speicher nach RFC 6265.
//!
//! STUB(M1-P02): P02 ergänzt Zugriff für Anfragen und Antworten.

use std::sync::Mutex;

/// Cookie-Speicher einer Crawl-Sitzung (Wrapper um `cookie_store`).
#[derive(Debug, Default)]
pub struct CookieJar {
    #[allow(dead_code)]
    store: Mutex<cookie_store::CookieStore>,
}

impl CookieJar {
    /// Leerer Speicher.
    pub fn new() -> Self {
        Self::default()
    }
}
