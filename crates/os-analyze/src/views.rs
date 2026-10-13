//! Ansichten auf die Crawl-Datei: Abfragen mit Filter, Sortierung, Suche und Cursor-Paging.
//!
//! STUB(M1-P14)

use os_model::{CrawlConfig, FilterState, UrlId, Value};
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

/// Abfrage einer Ansicht. Neue Felder kommen ohne Umbau der Aufrufer hinzu: mit [`ViewQuery::new`] bzw.
/// `Default` erzeugen und die gewünschten Felder danach zuweisen.
#[non_exhaustive]
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
    /// Suche (M1: einfacher Teilstring über die Adresse).
    pub search: Option<SearchSpec>,
    /// Zeilen je Seite (1–500).
    pub limit: u32,
    /// Wo die Seite beginnt.
    pub start: PageStart,
}

impl ViewQuery {
    /// Abfrage für `tab` und `filter`; alles Übrige wie bei `Default`.
    pub fn new(tab: impl Into<String>, filter: impl Into<String>) -> Self {
        ViewQuery {
            tab: tab.into(),
            filter: filter.into(),
            ..ViewQuery::default()
        }
    }
}

impl Default for ViewQuery {
    fn default() -> Self {
        ViewQuery {
            tab: String::new(),
            filter: String::new(),
            columns: ColumnSelection::Default,
            sort: Vec::new(),
            search: None,
            limit: 50,
            start: PageStart::Offset(0),
        }
    }
}

/// Suchangabe einer Abfrage. M2 ergänzt `Rules { … }`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchSpec {
    /// Teilstring, ohne Beachtung der Groß-/Kleinschreibung, über die Adresse.
    Simple(String),
}

/// Beginn einer Seite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageStart {
    /// Fortsetzung über einen Cursor (MCP/CLI, AGENT-005).
    Cursor(String),
    /// Versatz in Zeilen (Oberfläche, Exporte).
    Offset(u64),
}

/// Eine Seite des Ergebnisses.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct ViewPage {
    /// Spaltennamen.
    pub columns: Vec<String>,
    /// Zeilen.
    pub rows: Vec<Vec<Value>>,
    /// Zeilenschlüssel (`UrlId`) in Zeilenreihenfolge.
    pub row_keys: Vec<UrlId>,
    /// Gesamtzahl der Treffer.
    pub total: u64,
    /// Cursor der nächsten Seite.
    pub next_cursor: Option<String>,
    /// `crawl_meta.change_seq` beim Lesen.
    pub version: u64,
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
    /// `crawl_meta.change_seq` beim Lesen.
    pub version: u64,
}

/// SQL-Plan einer Abfrage; `query_view`, `stream_view` und `filter_counts` bauen darauf auf (M2 berechnet
/// Filter-Bitmaps aus demselben Plan).
#[derive(Debug, Clone, PartialEq)]
pub struct ViewPlan {
    /// Grundmenge des Tabs (Bedingung über `u`).
    pub population_sql: String,
    /// Filterprädikat, inkl. CRAWL-089-Ausschluss.
    pub filter_sql: String,
    /// Suchbedingung, falls eine Suche angegeben ist.
    pub search_sql: Option<String>,
    /// Benannte Parameter (`:name`) mit ihren Werten.
    pub params: Vec<(String, Value)>,
    /// `ORDER BY`-Klausel nach der Sortierregel (3.9).
    pub order_by: String,
}

/// Baut den SQL-Plan zu einer Abfrage.
// STUB(M1-P14)
pub fn plan(_q: &ViewQuery, _cfg: &CrawlConfig) -> Result<ViewPlan, ViewError> {
    Err(ViewError::NotImplemented("views::plan (M1-P14)"))
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
