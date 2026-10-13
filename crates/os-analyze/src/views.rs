//! Ansichten auf die Crawl-Datei: Abfragen mit Filter, Sortierung, Suche und Cursor-Paging.
//!
//! STUB(M1-P14)

use os_model::{CrawlConfig, FilterState, Value};
use os_store::CrawlFile;

use crate::defs::SortKey;
use crate::error::{NameError, ViewError};

/// Welche Spalten geliefert werden.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ColumnSelection {
    /// Die sichtbaren Standardspalten.
    #[default]
    Default,
    /// Alle Spalten.
    All,
    /// Genau diese Spalten (kanonische Namen) in dieser Reihenfolge.
    Named(Vec<String>),
}

/// Abfrage einer Ansicht.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewQuery {
    /// Tab, z. B. `"Internal"`.
    pub tab: String,
    /// Filter, z. B. `"HTML"`.
    pub filter: String,
    /// Spaltenauswahl.
    pub columns: ColumnSelection,
    /// Sortierung (leer = Standard des Tabs).
    pub sort: Vec<SortKey>,
    /// Suchtext über die Adresse.
    pub search: Option<String>,
    /// Zeilen je Seite (1–500).
    pub limit: u32,
    /// Fortsetzung der vorherigen Seite.
    pub cursor: Option<String>,
}

/// Eine Seite des Ergebnisses.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewPage {
    /// Spaltennamen.
    pub columns: Vec<String>,
    /// Zeilen.
    pub rows: Vec<Vec<Value>>,
    /// Gesamtzahl der Treffer.
    pub total: u64,
    /// Cursor der nächsten Seite.
    pub next_cursor: Option<String>,
}

/// Zähler eines Filters.
#[derive(Debug, Clone, PartialEq)]
pub struct FilterCount {
    /// Tab.
    pub tab: String,
    /// Filter (kanonisch, Platzhalter aufgelöst).
    pub filter: String,
    /// Anzeigename mit aktuellen Schwellen.
    pub label: String,
    /// Trefferzahl (nur bei `FilterState::Ready`).
    pub count: Option<u64>,
    /// Anteil an „All“ in Prozent.
    pub pct: Option<f64>,
    /// Zustand des Filters.
    pub state: FilterState,
}

/// Fragt eine Seite einer Ansicht ab.
// STUB(M1-P14)
pub fn query_view(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _q: &ViewQuery,
) -> Result<ViewPage, ViewError> {
    Err(ViewError::NotImplemented("views::query_view (M1-P14)"))
}

/// Streamt alle Zeilen einer Ansicht in konstantem Speicher; liefert die Spaltennamen.
// STUB(M1-P14)
pub fn stream_view(
    _file: &CrawlFile,
    _cfg: &CrawlConfig,
    _q: &ViewQuery,
    _sink: &mut dyn FnMut(&[Value]) -> Result<(), ViewError>,
) -> Result<Vec<String>, ViewError> {
    Err(ViewError::NotImplemented("views::stream_view (M1-P14)"))
}

/// Zähler aller Filter aller Tabs.
// STUB(M1-P14)
pub fn filter_counts(_file: &CrawlFile, _cfg: &CrawlConfig) -> Result<Vec<FilterCount>, ViewError> {
    Ok(Vec::new())
}

/// Löst `"Tab:Filter"` auf (Vorschläge bei Fehlern).
pub fn resolve_name(input: &str) -> Result<(String, String), NameError> {
    crate::names::resolve(input)
}
