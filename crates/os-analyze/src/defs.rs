//! Definition der Tabs, Spalten und Filter (M1-Plan 3.6). Die Laufzeit (P14) baut daraus SQL;
//! die Tab-Pakete (P20–P25) liefern die Definitionen.

use os_model::{AnalysisStep, CrawlConfig, Value};

/// Ein Tab der Ansicht, z. B. „Internal“.
pub struct TabDef {
    /// Kanonischer Name, z. B. `"Internal"`.
    pub id: &'static str,
    /// SQL-Bedingung über den Alias `u` (Tabelle `urls`), die die Grundmenge des Tabs bestimmt.
    pub population: &'static str,
    /// Spalten in Standardreihenfolge.
    pub columns: Vec<ColumnDef>,
    /// Filter in Anzeigereihenfolge.
    pub filters: Vec<FilterDef>,
    /// Standardsortierung.
    pub default_sort: Vec<SortKey>,
    /// Benannte SQL-Parameter (`:name`) aus der Konfiguration, z. B. Schwellen.
    pub params: fn(&CrawlConfig) -> Vec<(&'static str, Value)>,
}

/// Eine Spalte eines Tabs.
pub struct ColumnDef {
    /// Kanonischer Exportname.
    pub key: &'static str,
    /// SQL-Ausdruck.
    pub sql: ColumnSql,
    /// Darstellungsart.
    pub kind: ColumnKind,
    /// In der Standardansicht sichtbar.
    pub default_visible: bool,
}

/// Wie eine Spalte berechnet wird.
pub enum ColumnSql {
    /// Ein SQL-Ausdruck über `u`.
    Expr(&'static str),
    /// Mehrfachspalte (TECH-004): `base` ist der Spaltenname ohne Nummer; `list_sql` liefert die Werte
    /// einer URL (`?1` = `url_id`, mit `ORDER BY`).
    Multi {
        /// Grundname der Spalte, z. B. `"Canonical Link Element"`.
        base: &'static str,
        /// SQL für die Werteliste.
        list_sql: &'static str,
    },
}

/// Darstellungsart einer Spalte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Text.
    Text,
    /// Ganzzahl.
    Int,
    /// Dezimalzahl mit festen Nachkommastellen.
    Float {
        /// Nachkommastellen.
        decimals: u8,
    },
    /// Wahrheitswert.
    Bool,
    /// Zeitstempel.
    Timestamp,
    /// Hash (immer Text).
    Hash,
    /// Millisekunden als Sekunden mit 3 Stellen.
    Seconds3,
}

/// Ein Filter eines Tabs.
pub struct FilterDef {
    /// Kanonische Kennung; Schwellen als Platzhalter, z. B. `"Over {title.max_chars} Characters"`.
    pub id: &'static str,
    /// SQL-Prädikat über `u` mit benannten Parametern.
    pub predicate: &'static str,
    /// Live oder Nachlauf.
    pub kind: FilterKind,
    /// Voraussetzungen (Konfiguration, Rendering).
    pub requires: Vec<Requirement>,
    /// `issues.ignore_non_indexable` wirkt auf diesen Filter (CRAWL-089).
    pub ignore_non_indexable: bool,
}

impl FilterDef {
    /// Platzhalter-Filter (M1-P00b): Prädikat `"0"`, bei `"All"` die Grundmenge (`"1"`).
    pub fn placeholder(id: &'static str, kind: FilterKind, requires: &[&'static str]) -> FilterDef {
        FilterDef {
            id,
            predicate: if id == "All" { "1" } else { "0" },
            kind,
            requires: requires.iter().map(|k| Requirement::Config(k)).collect(),
            ignore_non_indexable: false,
        }
    }
}

/// Wann ein Filter ausgewertet werden kann.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    /// Sofort per SQL (während und nach dem Crawl).
    Live,
    /// Erst nach dem genannten Analyse-Teilschritt (CRAWL-141, TECH-005).
    PostCrawl(AnalysisStep),
}

/// Voraussetzung eines Filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// Konfigurationsschlüssel, der `true` sein muss.
    Config(&'static str),
    /// JavaScript-Rendering.
    Rendering,
}

/// Sortierschlüssel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// Spaltenname (`ColumnDef.key`).
    pub column: String,
    /// Absteigend sortieren.
    pub descending: bool,
}

/// Platzhalter-Tab (M1-P00b): Spalte „Address“, Grundmenge `"1"`, keine Parameter.
pub fn placeholder_tab(id: &'static str, filters: Vec<FilterDef>) -> TabDef {
    TabDef {
        id,
        population: "1",
        columns: vec![ColumnDef {
            key: "Address",
            sql: ColumnSql::Expr("u.address"),
            kind: ColumnKind::Text,
            default_visible: true,
        }],
        filters,
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: |_| Vec::new(),
    }
}
