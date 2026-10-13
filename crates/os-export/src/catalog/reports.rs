//! Report-Katalog (Crawl Overview, Redirects; OUT-080 ff., OUT-112).
//!
//! Das Register nennt je Report den kanonischen Namen (`--save-report "Redirects:Redirect Chains"`), den
//! Dateinamen-Stamm (OUT-008: `redirect_chains.csv`, `crawl_overview.csv`) und die Voraussetzungen
//! (OUT-112). Name, Dateiname und Voraussetzungen kommen aus einer Quelle, damit Hilfetexte, CLI-Prüfung und
//! Export nicht auseinanderlaufen.

use os_analyze::names::column_suggestions;
use os_analyze::NameError;

use crate::naming;

/// Voraussetzung eines Reports (OUT-112).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportRequirement {
    /// Die Crawl-Analyse (CRAWL-141) muss gelaufen sein; vorher fehlen Filter im Report.
    CrawlAnalysis,
    /// Konfigurationsschlüssel, der `true` sein muss (z. B. Extraktion eingeschaltet).
    Config {
        /// Konfigurationspfad, z. B. `extraction.hash`.
        key: &'static str,
        /// Was einzuschalten ist, für den Hinweis (`cookie extraction`).
        what: &'static str,
    },
    /// JavaScript-Rendering.
    Rendering,
}

/// Art des Reports (Zuordnung zur Umsetzung).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    /// Crawl Overview (OUT-080).
    CrawlOverview,
    /// Alle Weiterleitungen (OUT-081).
    AllRedirects,
    /// Weiterleitungsketten ab 2 Sprüngen oder Schleife (OUT-082).
    RedirectChains,
}

/// Ein Report des Katalogs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportDef {
    /// Kanonischer Name (`"Redirects:Redirect Chains"`).
    pub name: &'static str,
    /// Grundlage des Dateinamens (OUT-008), z. B. `"Redirect Chains"` → `redirect_chains.csv`.
    pub file_stem: &'static str,
    /// Voraussetzungen (OUT-112).
    pub requires: &'static [ReportRequirement],
    /// Art.
    pub kind: ReportKind,
}

impl ReportDef {
    /// Dateiname mit Endung (`csv`, `xlsx`).
    pub fn file_name(&self, ext: &str) -> String {
        naming::file_name(self.file_stem, None, ext)
    }
}

/// Alle Reports in Menüreihenfolge.
pub const REPORTS: &[ReportDef] = &[
    ReportDef {
        name: "Crawl Overview",
        file_stem: "Crawl Overview",
        requires: &[ReportRequirement::CrawlAnalysis],
        kind: ReportKind::CrawlOverview,
    },
    ReportDef {
        name: "Redirects:All Redirects",
        file_stem: "All Redirects",
        requires: &[],
        kind: ReportKind::AllRedirects,
    },
    ReportDef {
        name: "Redirects:Redirect Chains",
        file_stem: "Redirect Chains",
        requires: &[],
        kind: ReportKind::RedirectChains,
    },
];

/// Alle kanonischen Reportnamen (für `--help save-report`, `--list-reports` und Vorschläge).
pub fn names() -> Vec<String> {
    REPORTS.iter().map(|r| r.name.to_string()).collect()
}

/// Dateinamen-Stämme aller Reports; Eingabe für den Kollisionstest `naming::collisions` (OUT-008).
pub fn file_stems() -> Vec<&'static str> {
    REPORTS.iter().map(|r| r.file_stem).collect()
}

/// Vergleichsform: Kleinschreibung, Leerräume um `:` und innerhalb zusammengefasst.
fn normalize(name: &str) -> String {
    name.split(':')
        .map(|part| part.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join(":")
        .to_lowercase()
}

/// Löst einen Reportnamen auf (case-insensitiv, Leerräume um `:` tolerant; ein vorangestelltes `Reports:` wie
/// im Multi-Export-Baum von OUT-075 ist erlaubt). Bei unbekanntem Namen liefert der Fehler ähnliche Namen
/// (Levenshtein ≤ 3, K-08).
pub fn resolve(name: &str) -> Result<&'static ReportDef, NameError> {
    let wanted = normalize(name);
    let find = |w: &str| REPORTS.iter().find(|r| normalize(r.name) == w);
    if let Some(def) = find(&wanted) {
        return Ok(def);
    }
    if let Some(def) = wanted.strip_prefix("reports:").and_then(find) {
        return Ok(def);
    }
    let all = names();
    Err(NameError::Unknown {
        input: name.to_string(),
        suggestions: column_suggestions(all.iter().map(String::as_str), name),
    })
}
