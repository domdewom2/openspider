//! Issue-Katalog (UI-063/UI-064): Klassifizierung, eigene Texte (de/en) und Issue-Zeilen aus Filterzählern.
//!
//! STUB(M1-P12): P12 legt `core/issues/catalog.toml` an und füllt Katalog und Texte.

use os_model::{IssuePriority, IssueType, Value};

use crate::views::FilterCount;

/// Sprache der Katalogtexte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Deutsch.
    De,
    /// Englisch.
    En,
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

/// Der Katalog (`core/issues/catalog.toml`).
// STUB(M1-P12): leer
pub fn catalog() -> &'static [CatalogEntry] {
    &[]
}

/// Texte zu einem `doc_key`; ersetzt die Platzhalter `{max}`, `{min}`, `{unit}`, `{threshold}` durch die Werte
/// aus `params` (`TabDef.params(cfg)`).
// STUB(M1-P12)
pub fn texts(_doc_key: &str, _lang: Lang, _params: &[(&str, Value)]) -> Option<IssueTexts> {
    None
}

/// Issue-Zeilen aus den Filterzählern (nur Zähler > 0 im Zustand „bereit“).
// STUB(M1-P12)
pub fn issues_from_counts(_counts: &[FilterCount]) -> Vec<IssueRow> {
    Vec::new()
}
