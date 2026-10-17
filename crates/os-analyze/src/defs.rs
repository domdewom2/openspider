//! Definition der Tabs, Spalten und Filter (M1-Plan 3.6). Die Laufzeit (P14) baut daraus SQL;
//! die Tab-Pakete (P20–P25) liefern die Definitionen.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

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
///
/// **Metadaten für die UI (K-4, ab M2):** `default_width`, `align`, `sortable`, `search_ops` und `needs` beschreiben, wie
/// die Oberfläche die Spalte zeigt, sortiert und durchsucht (UI-007, UI-008, UI-011). Neue Spalten legt man mit
/// [`ColumnDef::new`] an; wer ein Struct-Literal braucht, ergänzt `..ColumnDef::META` – `default_width = 0` heißt „aus
/// der Spaltenart ableiten“, das erledigt [`apply_meta_defaults`] (aufgerufen von `tabs::all()`), sodass jede Spalte
/// ohne eigene Werte die Standardbreite, Ausrichtung und Operatorenmenge ihrer Art bekommt.
pub struct ColumnDef {
    /// Kanonischer Exportname.
    pub key: &'static str,
    /// SQL-Ausdruck.
    pub sql: ColumnSql,
    /// Darstellungsart.
    pub kind: ColumnKind,
    /// In der Standardansicht sichtbar.
    pub default_visible: bool,
    /// [OS-Erweiterung]: im Default-Export ausgeblendet, sonst hinter den Referenzspalten.
    pub extension: bool,
    /// Standardbreite in Pixeln; 0 = aus der Spaltenart ableiten ([`apply_meta_defaults`]).
    pub default_width: u16,
    /// Ausrichtung der Zellen.
    pub align: Align,
    /// Die Spalte lässt sich sortieren.
    pub sortable: bool,
    /// Operatoren der Regelsuche für diese Spalte (UI-011).
    pub search_ops: SearchOps,
    /// Wovon die Spalte abhängt (sonst leer mit Hinweis, UI-016); `None` = immer befüllbar.
    pub needs: Option<Needs>,
}

impl ColumnDef {
    /// Vorlage für Struct-Literale (`..ColumnDef::META`): Metadaten noch nicht gesetzt (`default_width = 0`).
    pub const META: ColumnDef = ColumnDef {
        key: "",
        sql: ColumnSql::Expr(""),
        kind: ColumnKind::Text,
        default_visible: true,
        extension: false,
        default_width: 0,
        align: Align::Left,
        sortable: true,
        search_ops: SearchOps::NONE,
        needs: None,
    };

    /// Spalte mit einem SQL-Ausdruck; die Metadaten leiten sich aus der Spaltenart ab.
    pub fn new(
        key: &'static str,
        sql: &'static str,
        kind: ColumnKind,
        default_visible: bool,
    ) -> ColumnDef {
        ColumnDef {
            key,
            sql: ColumnSql::Expr(sql),
            kind,
            default_visible,
            ..ColumnDef::META
        }
    }

    /// Dieselbe Spalte als `[OS-Erweiterung]` (standardmäßig ausgeblendet).
    pub fn into_extension(mut self) -> ColumnDef {
        self.extension = true;
        self.default_visible = false;
        self
    }

    /// Setzt die Abhängigkeit der Spalte (UI-016).
    pub fn with_needs(mut self, needs: Needs) -> ColumnDef {
        self.needs = Some(needs);
        self
    }

    /// Spalte mit zur Laufzeit bekanntem Namen (Custom Search/Extraction/JavaScript, Segmente, API-Metriken): der Name
    /// wird einmal je Wert festgehalten ([`intern`]), weil `ColumnDef.key` ein `&'static str` ist.
    pub fn dynamic(
        key: &str,
        sql: &str,
        kind: ColumnKind,
        default_visible: bool,
    ) -> ColumnDef {
        ColumnDef::new(intern(key), intern(sql), kind, default_visible)
    }
}

/// Hält einen Text für die Dauer des Prozesses fest und liefert dieselbe Referenz für gleiche Texte. Gedacht für die
/// wenigen Namen dynamischer Spalten (Filter- und Extraktornamen, höchstens je 100); nicht für Zellwerte.
pub fn intern(text: &str) -> &'static str {
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(found) = pool.get(text) {
        return found;
    }
    let leaked: &'static str = Box::leak(text.to_owned().into_boxed_str());
    pool.insert(leaked);
    leaked
}

/// Ausrichtung der Zellen einer Spalte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// Linksbündig (Text, URLs).
    #[default]
    Left,
    /// Rechtsbündig (Zahlen).
    Right,
    /// Zentriert (Wahrheitswerte).
    Center,
}

/// Operatoren der Regelsuche, die eine Spalte unterstützt (UI-011, K-6); Bitmenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOps(pub u16);

impl SearchOps {
    /// Keine Angabe (wird aus der Spaltenart abgeleitet).
    pub const NONE: SearchOps = SearchOps(0);
    /// Enthält / enthält nicht.
    pub const CONTAINS: u16 = 1 << 0;
    /// Gleich / ungleich.
    pub const EQUALS: u16 = 1 << 1;
    /// Beginnt mit / endet mit.
    pub const AFFIX: u16 = 1 << 2;
    /// Regulärer Ausdruck.
    pub const REGEX: u16 = 1 << 3;
    /// Größer / kleiner / Bereich.
    pub const COMPARE: u16 = 1 << 4;
    /// Ist leer / ist nicht leer.
    pub const EMPTY: u16 = 1 << 5;
    /// Wahr / falsch.
    pub const BOOL: u16 = 1 << 6;

    /// Operatoren für Text und URLs.
    pub const TEXT: SearchOps =
        SearchOps(Self::CONTAINS | Self::EQUALS | Self::AFFIX | Self::REGEX | Self::EMPTY);
    /// Operatoren für Zahlen und Zeitpunkte.
    pub const NUMBER: SearchOps = SearchOps(Self::EQUALS | Self::COMPARE | Self::EMPTY);
    /// Operatoren für Wahrheitswerte.
    pub const FLAG: SearchOps = SearchOps(Self::BOOL | Self::EMPTY);

    /// Ob der Operator (eine der Konstanten) unterstützt wird.
    pub fn has(self, op: u16) -> bool {
        self.0 & op != 0
    }

    /// Ob keine Operatoren gesetzt sind.
    pub fn is_none(self) -> bool {
        self.0 == 0
    }
}

/// Wovon eine Spalte abhängt (Hinweis „nicht anwendbar“ statt leerer Zellen, UI-016).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// Konfigurationsschlüssel, der `true` sein muss.
    Config(&'static str),
    /// JavaScript-Rendering.
    Rendering,
    /// Ein Teilschritt der Crawl-Analyse (CRAWL-141).
    CrawlAnalysis(AnalysisStep),
    /// Eine verbundene API-Quelle (`ga4`, `gsc`, `psi`, …).
    Api(&'static str),
}

/// Standardwerte der Metadaten nach Spaltenart (Breite in Pixeln, Ausrichtung, Operatoren).
pub fn meta_for_kind(kind: ColumnKind) -> (u16, Align, SearchOps) {
    match kind {
        ColumnKind::Url => (320, Align::Left, SearchOps::TEXT),
        ColumnKind::Text => (240, Align::Left, SearchOps::TEXT),
        ColumnKind::Hash => (260, Align::Left, SearchOps::TEXT),
        ColumnKind::Int => (96, Align::Right, SearchOps::NUMBER),
        ColumnKind::Float { .. } => (96, Align::Right, SearchOps::NUMBER),
        ColumnKind::Seconds3 => (96, Align::Right, SearchOps::NUMBER),
        ColumnKind::Bool => (80, Align::Center, SearchOps::FLAG),
        ColumnKind::Timestamp => (176, Align::Left, SearchOps::NUMBER),
    }
}

/// Setzt für jede Spalte, die noch keine eigenen Werte trägt (`default_width = 0`), Breite, Ausrichtung und
/// Operatorenmenge ihrer Art. Mehrfachspalten (TECH-004) bekommen die Werte der Art ihrer ersten Teilspalte.
pub fn apply_meta_defaults(columns: &mut [ColumnDef]) {
    for c in columns {
        if c.default_width != 0 {
            continue;
        }
        let kind = match &c.sql {
            ColumnSql::MultiPaired { parts, .. } => parts.first().map_or(c.kind, |p| p.kind),
            _ => c.kind,
        };
        let (width, align, ops) = meta_for_kind(kind);
        c.default_width = width;
        c.align = align;
        if c.search_ops.is_none() {
            c.search_ops = ops;
        }
    }
}

/// Liefert zur Laufzeit Spalten, die nicht im Code stehen (Custom Search/Extraction/JavaScript, Segmente, API-Metriken,
/// Linkmetriken; K-4). Ein Tab-Modul meldet seine Quelle über `column_source()`; `tabs::column_sources()` sammelt sie.
pub trait ColumnSource: Send + Sync {
    /// Die Spalten für die Konfiguration `cfg` (leer, wenn nichts konfiguriert ist).
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef>;
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
    /// Verschränkte Mehrfachspalte (TECH-004, TECH-071): je Index `n` mehrere Teilspalten hintereinander,
    /// z. B. `HTML hreflang 1`, `HTML hreflang 1 URL`, `HTML hreflang 2`, `HTML hreflang 2 URL`, …
    ///
    /// Die Teilspalten heißen `<base> <n>` (Suffix leer) bzw. `<base> <n> <suffix>`. Die Anzahl der Indizes ist
    /// das Maximum über alle Teillisten und alle Zeilen der Ansicht (mindestens 1); fehlende Werte sind leere
    /// Texte. `ColumnDef.key` benennt die ganze Spalte (Auswahl aller Indizes), `ColumnDef.kind` gilt nur für
    /// den Fallback – maßgeblich ist die Art der jeweiligen Teilspalte.
    MultiPaired {
        /// Grundname der Teilspalten, z. B. `"HTML hreflang"`.
        base: &'static str,
        /// Teilspalten je Index in Ausgabereihenfolge (mindestens eine).
        parts: &'static [PairedPart],
    },
}

/// Eine Teilspalte einer [`ColumnSql::MultiPaired`]-Spalte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairedPart {
    /// Namenszusatz hinter der Nummer, z. B. `"URL"`; leer für die erste Teilspalte (`<base> <n>`).
    pub suffix: &'static str,
    /// SQL für die Werteliste dieser Teilspalte (`?1` = `url_id`, mit `ORDER BY`).
    pub list_sql: &'static str,
    /// Darstellungsart der Werte (bestimmt Sortierung und Zellwert).
    pub kind: ColumnKind,
}

/// Darstellungsart einer Spalte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnKind {
    /// Text (sortiert mit `COLLATE OS_TEXT`).
    Text,
    /// URL (bytewise sortiert, Export als Text).
    Url,
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

impl TabDef {
    /// Ist der Tab ein Stub (leerer Platzhalter aus `tabs::stub_tab`: Grundmenge `0`, nur die Spalte `Address`)?
    pub fn is_stub(&self) -> bool {
        self.population == "0" && self.columns.len() == 1 && self.columns[0].key == "Address"
    }
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
    /// Ist der Filter ein Stub (Prädikat `0`: ein M2-Paket hat ihn noch nicht umgesetzt)? `All` zählt nie als Stub.
    pub fn is_stub(&self) -> bool {
        self.predicate == "0" && !self.id.eq_ignore_ascii_case("All")
    }

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
        columns: vec![ColumnDef::new("Address", "u.address", ColumnKind::Text, true)],
        filters,
        default_sort: vec![SortKey {
            column: "Address".to_string(),
            descending: false,
        }],
        params: |_| Vec::new(),
    }
}
