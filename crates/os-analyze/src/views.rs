//! Ansichten auf die Crawl-Datei: Abfragen mit Filter, Sortierung, Suche und Cursor-Paging
//! (ONPAGE-001, ONPAGE-007, ONPAGE-190, TECH-004, TECH-005).
//!
//! Aufbau: [`plan`] erzeugt aus `TabDef`/`FilterDef` und Konfiguration einen [`ViewPlan`] (Population ∧
//! Filter ∧ CRAWL-089-Ausschluss ∧ Suche, Parameter, `ORDER BY`). [`query_view`], [`stream_view`] und
//! [`filter_counts`] setzen auf diesem Plan auf; M2 berechnet Filter-Bitmaps aus demselben Plan.
//!
//! Sortierregel (eine für Tabs, Exporte und MCP): Sortierschlüssel → `Address` bytewise → `UrlId`;
//! `ColumnKind::Text` mit `COLLATE OS_TEXT`, `ColumnKind::Url` und `Address` bytewise; leere Werte (`NULL`
//! und `''`) stehen bei auf- und absteigender Sortierung am Ende; ohne Sortierung und ohne `default_sort`
//! gilt `UrlId` aufsteigend.
//!
//! Die Funktionen `*_in` nehmen eine eigene Tab-Liste statt `tabs::all()` (Tests, abweichende Registry).

mod build;
mod counts;
mod exec;
pub mod session;

use os_model::{CrawlConfig, FilterState, UrlId, Value};
use os_store::CrawlFile;

use crate::defs::{SortKey, TabDef};
use crate::error::{NameError, ViewError};
use crate::state::filter_state_for_file;
use crate::tabs;

pub use build::column_name_resolves;

/// `FROM`-Quelle jeder Ansicht: die URL-Zeile `u` und ihre Zähler `c` (`url_counters`, Schema v2). Die Spalten der
/// Tabs lesen die Link-Zähler (`Inlinks`, `Outlinks`, …) aus `c`, nie aus den Zählerspalten von `urls`. Der
/// `LEFT JOIN` über den Primärschlüssel entfällt, wenn keine Spalte, kein Filter und keine Sortierung `c` nutzt.
pub const URLS_FROM: &str = "urls u LEFT JOIN url_counters c ON c.url_id = u.id";
pub use counts::percent;

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
    /// Tatsächlich sichtbare Suchspalten; unabhängig von den Export-/Ausgabespalten.
    pub search_visible: Option<Vec<String>>,
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
            search_visible: None,
            limit: 50,
            start: PageStart::Offset(0),
        }
    }
}

/// Suchangabe einer Abfrage.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchSpec {
    /// Teilstring, ohne Beachtung der Groß-/Kleinschreibung, über die Adresse.
    Simple(String),
    /// Regelbaum über Spalten (UI-011, K-6; ab M2). Die Auswertung liefert M2-K02 (`search::compile`); bis dahin
    /// schränkt diese Angabe die Ansicht nicht ein.
    Rules {
        /// Der Regelbaum.
        tree: crate::search::RuleNode,
        /// Groß-/Kleinschreibung beachten.
        case_sensitive: bool,
    },
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
    /// Kennung des Filters wie `FilterDef.id` (Platzhalter unaufgelöst, z. B. `Over {title.max_chars}
    /// Characters`); der Anzeigename mit aktuellen Schwellen steht in `label`.
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
    /// Benannte Parameter mit führendem `:` (`:q` = Suchtext) und ihren Werten.
    pub params: Vec<(String, Value)>,
    /// Vollständige `ORDER BY`-Klausel (mit Schlüsselwort) nach der Sortierregel (3.9).
    pub order_by: String,
}

/// Baut den SQL-Plan zu einer Abfrage.
pub fn plan(q: &ViewQuery, cfg: &CrawlConfig) -> Result<ViewPlan, ViewError> {
    plan_in(&tabs::configured(cfg), q, cfg)
}

/// Wie [`plan`], gegen eine eigene Tab-Liste.
pub fn plan_in(tabs: &[TabDef], q: &ViewQuery, cfg: &CrawlConfig) -> Result<ViewPlan, ViewError> {
    Ok(build::build(tabs, q, cfg)?.plan)
}

/// Fragt eine Seite einer Ansicht ab. Hat der Filter einen Zustand statt Zahlen (`FilterState` ≠ `Ready`),
/// ist die Seite leer (`total` 0): nie Teilergebnisse, nie eine 0 an Stelle des Zustands (ONPAGE-007).
pub fn query_view(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
) -> Result<ViewPage, ViewError> {
    query_view_in(&tabs::configured_for_file(cfg, file), file, cfg, q)
}

/// Wie [`query_view`], gegen eine eigene Tab-Liste.
pub fn query_view_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
) -> Result<ViewPage, ViewError> {
    let _search = crate::search::prepare_view(tabs, file, cfg, q)?;
    let q = &crate::search::prepared_query(q);
    let built = build::build(tabs, q, cfg)?;
    let ready = is_ready(file, cfg, &built)?;
    exec::query_page(file, &built, q, ready)
}

/// Streamt alle Zeilen einer Ansicht in konstantem Speicher (ohne `limit` und `start`); liefert die
/// Spaltennamen.
pub fn stream_view(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    sink: &mut dyn FnMut(&[Value]) -> Result<(), ViewError>,
) -> Result<Vec<String>, ViewError> {
    stream_view_in(&tabs::configured_for_file(cfg, file), file, cfg, q, sink)
}

/// Wie [`stream_view`], gegen eine eigene Tab-Liste.
pub fn stream_view_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    q: &ViewQuery,
    sink: &mut dyn FnMut(&[Value]) -> Result<(), ViewError>,
) -> Result<Vec<String>, ViewError> {
    let _search = crate::search::prepare_view(tabs, file, cfg, q)?;
    let q = &crate::search::prepared_query(q);
    let built = build::build(tabs, q, cfg)?;
    let ready = is_ready(file, cfg, &built)?;
    exec::stream_rows(file, &built, ready, sink)
}

/// Zustand des Filters einer Abfrage.
pub fn state_of(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
) -> Result<FilterState, ViewError> {
    state_of_in(
        &tabs::configured_for_file(cfg, file),
        file,
        cfg,
        tab,
        filter,
    )
}

/// Wie [`state_of`], gegen eine eigene Tab-Liste.
pub fn state_of_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
    tab: &str,
    filter: &str,
) -> Result<FilterState, ViewError> {
    let built = build::build(tabs, &ViewQuery::new(tab, filter), cfg)?;
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let snapshot = counts::analysis_snapshot(file)?;
    filter_state_for_file(built.filter, built.tab, file, cfg, &cfg_json, &snapshot)
}

fn is_ready(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    built: &build::Built<'_>,
) -> Result<bool, ViewError> {
    let cfg_json = serde_json::to_value(cfg).unwrap_or(serde_json::Value::Null);
    let snapshot = counts::analysis_snapshot(file)?;
    Ok(
        filter_state_for_file(built.filter, built.tab, file, cfg, &cfg_json, &snapshot)?
            .yields_rows(),
    )
}

/// Zähler aller Filter aller Tabs: Trefferzahl und Prozent nur bei `Ready`, sonst der Zustand.
pub fn filter_counts(file: &CrawlFile, cfg: &CrawlConfig) -> Result<Vec<FilterCount>, ViewError> {
    filter_counts_in(&tabs::configured_for_file(cfg, file), file, cfg)
}

/// Wie [`filter_counts`], gegen eine eigene Tab-Liste.
pub fn filter_counts_in(
    tabs: &[TabDef],
    file: &CrawlFile,
    cfg: &CrawlConfig,
) -> Result<Vec<FilterCount>, ViewError> {
    let mut counts = counts::counts_in(tabs, file, cfg)?;
    // Stub-Tabs und Stub-Filter (M2, `TabDef::is_stub`/`FilterDef::is_stub`) liefern keine Zähler, bis ihr Paket sie
    // umsetzt: Berichte und Übersichten zeigen nur Befüllbares (Crawl Overview bleibt für M1-Crawls unverändert).
    counts.retain(|c| {
        tabs.iter().find(|t| t.id == c.tab).is_none_or(|t| {
            !t.is_stub() && !t.filters.iter().any(|f| f.id == c.filter && f.is_stub())
        })
    });
    Ok(counts)
}

/// Löst `"Tab:Filter"` auf (Vorschläge bei Fehlern).
pub fn resolve_name(input: &str) -> Result<(String, String), NameError> {
    crate::names::resolve(input)
}
