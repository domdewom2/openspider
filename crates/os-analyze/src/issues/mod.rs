//! Issue-Katalog (UI-063/UI-064): Klassifizierung, eigene Texte (de/en) und Issue-Zeilen aus Filterzählern.
//!
//! Quellen (alle im Repo, per `include_str!` eingebettet):
//! - `core/issues/catalog.toml` – die Klassifizierungsdatei von M1 (Typ/Priorität mit Regelcodes); ab M2 kommt je Tab
//!   eine Datei `core/issues/catalog.d/<tab>.toml` hinzu (`catalog::SPLIT_FILES`, Loader in `catalog.rs`),
//! - `core/issues/texts/{de,en}.toml` – die drei Texte `what`/`why`/`fix` je Issue (Schlüssel
//!   `issues.<id>.what|why|fix`); ab M2 je Tab `core/issues/texts/{de,en}.d/<tab>.toml`,
//! - `docs/issues/<doc_key>.md` – Doku-Seite je Eintrag (erzeugt aus Katalog und Texten, siehe [`doc_page`]).
//!
//! Platzhalter der Texte (`{max}`, `{min}`, `{unit}`, `{threshold}`) ersetzt [`texts`] mit den Werten aus
//! `TabDef.params(cfg)`: Ein Parameter wird über seinen Konfigurationspfad gefunden (`title.max_chars`);
//! Schreibweisen mit `_` statt `.` und ein führendes `:` (benannter SQL-Parameter) gelten gleich. Fehlt der
//! Parameter, gilt der Standardwert aus `CrawlConfig::default()`.

mod catalog;
mod render;

use std::sync::OnceLock;

use os_model::{FilterState, IssuePriority, IssueType, Value};

use crate::views::FilterCount;

pub use catalog::{
    m1_source, override_share, parse_catalog, texts_m1_source, validate, CatalogError,
    CatalogProblem, SplitFiles, ALLOWED_PLACEHOLDERS, OVERRIDE_REASON_PREFIXES, PRIORITY_RULES,
    SPLIT_FILES, TYPE_RULES,
};
pub use render::{doc_page, filter_label, matches_filter};

/// Sprache der Katalogtexte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// Deutsch.
    De,
    /// Englisch.
    En,
}

impl Lang {
    /// Alle Sprachen mit Texten.
    pub const ALL: &'static [Lang] = &[Lang::De, Lang::En];

    /// Sprachkürzel (`"de"`, `"en"`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }
}

/// Ein Eintrag des Issue-Katalogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Kennung des Issues.
    pub id: String,
    /// Tab.
    pub tab: String,
    /// Filter.
    pub filter: String,
    /// Typ.
    pub issue_type: IssueType,
    /// Priorität.
    pub priority: IssuePriority,
    /// Herleitungsregel des Typs (UI-063).
    pub type_rule: String,
    /// Herleitungsregel der Priorität (UI-063).
    pub priority_rule: String,
    /// Begründung, wenn der Benchmark von der Herleitung abweicht.
    pub override_reason: Option<String>,
    /// Filter braucht die Crawl-Analyse.
    pub needs_crawl_analysis: bool,
    /// Schlüssel der Texte.
    pub doc_key: String,
    /// Platzhalter der Texte (`{max}`, `{min}`, `{unit}`, `{threshold}`) und der Konfigurationspfad, aus dem
    /// ihr Wert stammt, z. B. `("max", "title.max_chars")`.
    pub text_params: Vec<(String, String)>,
}

/// Beschreibung und Behebung eines Issues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueTexts {
    /// Was das Issue ist.
    pub what: String,
    /// Warum es relevant ist.
    pub why: String,
    /// Wie sich das Issue beheben lässt.
    pub fix: String,
}

/// Eine Issue-Zeile (Filter mit Treffern).
#[derive(Debug, Clone, PartialEq)]
pub struct IssueRow {
    /// Kennung des Issues.
    pub id: String,
    /// Tab.
    pub tab: String,
    /// Filter.
    pub filter: String,
    /// Typ.
    pub issue_type: IssueType,
    /// Priorität.
    pub priority: IssuePriority,
    /// Anzahl betroffener URLs.
    pub count: u64,
    /// Anteil an allen URLs des Tabs in Prozent.
    pub pct: Option<f64>,
}

/// Der Katalog: `core/issues/catalog.toml` (M1) plus je Tab `core/issues/catalog.d/<tab>.toml` (ab M2).
///
/// # Panics
/// Wenn eine eingebettete Katalogdatei nicht lesbar ist (Build-Fehler; `tests/issues_catalog.rs` und
/// `tests/catalog_split.rs` fangen das ab).
pub fn catalog() -> &'static [CatalogEntry] {
    static CATALOG: OnceLock<Vec<CatalogEntry>> = OnceLock::new();
    CATALOG.get_or_init(|| match catalog::load_all() {
        Ok(entries) => entries,
        Err(e) => panic!("Issue-Katalog ist ungültig: {e}"),
    })
}

/// Der Katalogeintrag zu einem `doc_key`.
pub fn entry_by_doc_key(doc_key: &str) -> Option<&'static CatalogEntry> {
    catalog().iter().find(|e| e.doc_key == doc_key)
}

/// Der Katalogeintrag zu Tab und Filter. Der Filtername darf der Katalogschreibweise mit Platzhaltern
/// (`Over {title.max_chars} Characters`) oder der aufgelösten Form (`Over 65 Characters`) entsprechen;
/// Groß-/Kleinschreibung zählt nicht (K-08).
pub fn entry_for(tab: &str, filter: &str) -> Option<&'static CatalogEntry> {
    catalog()
        .iter()
        .find(|e| e.tab.eq_ignore_ascii_case(tab) && matches_filter(&e.filter, filter))
}

/// Texte zu einem `doc_key`; ersetzt die Platzhalter `{max}`, `{min}`, `{unit}`, `{threshold}` durch die Werte
/// aus `params` (`TabDef.params(cfg)`). Fehlt der Text in der gewünschten Sprache, gilt der englische;
/// ein unbekannter `doc_key` liefert `None`.
pub fn texts(doc_key: &str, lang: Lang, params: &[(&str, Value)]) -> Option<IssueTexts> {
    let entry = entry_by_doc_key(doc_key)?;
    let raw =
        render::raw_texts(&entry.id, lang).or_else(|| render::raw_texts(&entry.id, Lang::En))?;
    Some(render::fill(entry, &raw, params))
}

/// Die ungefüllten Texte eines Eintrags (mit Platzhaltern), z. B. für Prüfungen.
pub fn raw_texts(doc_key: &str, lang: Lang) -> Option<IssueTexts> {
    let entry = entry_by_doc_key(doc_key)?;
    render::raw_texts(&entry.id, lang)
}

fn type_order(t: IssueType) -> u8 {
    match t {
        IssueType::Issue => 0,
        IssueType::Warning => 1,
        IssueType::Opportunity => 2,
    }
}

fn priority_order(p: IssuePriority) -> u8 {
    match p {
        IssuePriority::High => 0,
        IssuePriority::Medium => 1,
        IssuePriority::Low => 2,
    }
}

/// Issue-Zeilen aus den Filterzählern (nur Zähler > 0 im Zustand „bereit“), sortiert nach Typ
/// (Issue → Warning → Opportunity), Priorität (High → Medium → Low), `count` absteigend, Name (UI-062).
pub fn issues_from_counts(counts: &[FilterCount]) -> Vec<IssueRow> {
    let mut rows: Vec<IssueRow> = counts
        .iter()
        .filter(|c| c.state == FilterState::Ready)
        .filter_map(|c| {
            let n = c.count.filter(|n| *n > 0)?;
            let entry = entry_for(&c.tab, &c.filter).or_else(|| entry_for(&c.tab, &c.label))?;
            Some(IssueRow {
                id: entry.id.clone(),
                tab: c.tab.clone(),
                filter: c.filter.clone(),
                issue_type: entry.issue_type,
                priority: entry.priority,
                count: n,
                pct: c.pct,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        type_order(a.issue_type)
            .cmp(&type_order(b.issue_type))
            .then(priority_order(a.priority).cmp(&priority_order(b.priority)))
            .then(b.count.cmp(&a.count))
            .then_with(|| (&a.tab, &a.filter).cmp(&(&b.tab, &b.filter)))
    });
    rows
}

pub(crate) fn texts_source(lang: Lang) -> &'static str {
    catalog::texts_all(lang)
}
