//! Gemeinsame Lesefunktionen für Export, MCP und Engine.
//!
//! STUB(M1-P07)

use os_model::{
    ContentClass, Indexability, IndexabilityStatus, LinkType, NotCrawledReason, RedirectType,
    UrlId, UrlState,
};
use rusqlite::{Connection, OptionalExtension};

use os_model::HeaderRow;

use crate::error::StoreError;

/// Sortierschlüssel einer Link-Abfrage (Standard-Linkformat, OUT-031/032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSortKey {
    /// Spaltenname des Standard-Linkformats.
    pub column: String,
    /// Absteigend sortieren.
    pub desc: bool,
}

/// Seitenanfrage für paginierte Abfragen.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageReq {
    /// Anzahl zu überspringender Zeilen.
    pub offset: u64,
    /// Höchstzahl Zeilen (1–500).
    pub limit: u32,
    /// Sortierung; leer = Standardreihenfolge (OUT-031/032).
    pub sort: Vec<LinkSortKey>,
}

impl PageReq {
    /// Seitenanfrage mit Standardsortierung.
    pub fn new(offset: u64, limit: u32) -> Self {
        PageReq {
            offset,
            limit,
            sort: Vec::new(),
        }
    }
}

impl Default for PageReq {
    fn default() -> Self {
        PageReq::new(0, 50)
    }
}

/// Eine Seite eines Abfrageergebnisses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    /// Zeilen dieser Seite.
    pub items: Vec<T>,
    /// Gesamtzahl der Zeilen.
    pub total: u64,
}

/// Eine URL-Zeile (Kerndaten).
#[derive(Debug, Clone, PartialEq)]
pub struct UrlRow {
    /// Kennung.
    pub id: UrlId,
    /// URL Encoded Address.
    pub url: String,
    /// Anzeigeform.
    pub address: String,
    /// Intern oder extern.
    pub is_internal: bool,
    /// Zustand.
    pub state: UrlState,
    /// Grund, falls nicht gecrawlt.
    pub not_crawled_reason: Option<NotCrawledReason>,
    /// Crawl-Tiefe.
    pub crawl_depth: Option<u32>,
    /// Statuscode (0 = keine Antwort).
    pub status_code: Option<u16>,
    /// Statustext.
    pub status: Option<String>,
    /// Inhaltsklasse.
    pub content_class: ContentClass,
    /// Indexierbarkeit.
    pub indexability: Option<Indexability>,
    /// Grund der Nicht-Indexierbarkeit.
    pub indexability_status: Option<IndexabilityStatus>,
}

/// Ein Link im Standard-Linkformat (Inlinks/Outlinks).
#[derive(Debug, Clone, PartialEq)]
pub struct LinkView {
    /// Linktyp.
    pub link_type: LinkType,
    /// Quell-URL.
    pub source_id: UrlId,
    /// Adresse der Quelle.
    pub source_address: String,
    /// Ziel-URL, falls bekannt.
    pub target_id: Option<UrlId>,
    /// Adresse des Ziels.
    pub target_address: String,
    /// Ankertext.
    pub anchor: Option<String>,
    /// Alt-Text.
    pub alt_text: Option<String>,
    /// Link wird verfolgt.
    pub follow: bool,
    /// Statuscode des Ziels.
    pub status_code: Option<u16>,
    /// Statustext des Ziels.
    pub status: Option<String>,
}

/// Ein Glied einer Weiterleitungskette.
#[derive(Debug, Clone, PartialEq)]
pub struct RedirectHop {
    /// URL.
    pub id: UrlId,
    /// Adresse.
    pub address: String,
    /// Statuscode.
    pub status_code: Option<u16>,
    /// Art der Weiterleitung zum nächsten Glied.
    pub redirect_type: Option<RedirectType>,
}

/// Weiterleitungskette ab einer URL.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RedirectChain {
    /// Glieder von der Start-URL bis zum Ende.
    pub hops: Vec<RedirectHop>,
    /// Die Kette läuft im Kreis.
    pub is_loop: bool,
    /// Abbruch durch `max_hops`.
    pub limit_reached: bool,
}

/// Sucht eine URL über ihre Adresse (URL Encoded Address).
// STUB(M1-P07)
pub fn url_by_address(_conn: &Connection, _url: &str) -> Result<Option<UrlRow>, StoreError> {
    Ok(None)
}

/// Eingehende Links einer URL.
// STUB(M1-P07)
pub fn inlinks(
    _conn: &Connection,
    _id: UrlId,
    _page: PageReq,
) -> Result<Page<LinkView>, StoreError> {
    Ok(Page {
        items: Vec::new(),
        total: 0,
    })
}

/// Ausgehende Links einer URL.
// STUB(M1-P07)
pub fn outlinks(
    _conn: &Connection,
    _id: UrlId,
    _page: PageReq,
) -> Result<Page<LinkView>, StoreError> {
    Ok(Page {
        items: Vec::new(),
        total: 0,
    })
}

/// Gespeicherte HTTP-Header einer URL (Anfrage und Antwort).
// STUB(M1-P07)
pub fn headers(_conn: &Connection, _id: UrlId) -> Result<Vec<HeaderRow>, StoreError> {
    Ok(Vec::new())
}

/// Weiterleitungskette ab `start` (höchstens `max_hops` Glieder).
// STUB(M1-P07)
pub fn redirect_chain(
    _conn: &Connection,
    _start: UrlId,
    _max_hops: u32,
) -> Result<RedirectChain, StoreError> {
    Ok(RedirectChain::default())
}

/// Aktuelle Sequenz des Änderungsfeeds (`crawl_meta.change_seq`); `0`, wenn der Schlüssel fehlt.
/// Der Writer (P07) erhöht sie je `WriteBatch`; Löschungen setzen zusätzlich `reset_seq` (3.9).
pub fn change_seq(conn: &Connection) -> Result<u64, StoreError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM crawl_meta WHERE key = 'change_seq'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// URLs mit `urls.change_seq > seq`, aufsteigend nach `(change_seq, id)`, höchstens `limit`.
// STUB(M1-P07)
pub fn changed_since(_conn: &Connection, _seq: u64, _limit: u32) -> Result<Vec<UrlId>, StoreError> {
    Ok(Vec::new())
}
