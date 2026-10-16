//! Parse-Modell ab M2 (M2-F01, `docs/plan/M2.md` 3.3): Ergänzungen zu [`crate::ParsedPage`].
//!
//! Die Typen sind bewusst schlank und tragen nur, was der Store in den Tabellen von Schema v3 ablegt; die
//! Fachpakete (D09 strukturierte Daten, D10 Validierung, D11 Formulare, D14/D15 Custom Search/Extraction) füllen sie.

/// Ein strukturiertes Datenelement einer Seite (TECH-180 ff.): ein Wurzelknoten je Format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdItem {
    /// Format: `json-ld`, `microdata` oder `rdfa`.
    pub format: String,
    /// Typpfad, z. B. `Product` oder `Product/Offer`.
    pub type_path: String,
    /// Der Knoten als JSON (kanonisch serialisiert).
    pub json: String,
    /// Fehler beim Lesen (Parse-Fehler), sonst `None`.
    pub parse_error: Option<String>,
}

/// Ein Formular der Seite (ONPAGE-034/035).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormRef {
    /// Reihenfolge im Dokument.
    pub idx: u32,
    /// `action` roh; `None` = Attribut fehlt.
    pub action: Option<String>,
    /// Aufgelöste Ziel-URL.
    pub action_resolved: Option<String>,
    /// `method` kleingeschrieben (`get`, `post`, …); leer = Attribut fehlt.
    pub method: String,
}

/// Ein Befund der HTML-Validierung auf Token-Ebene (TECH-008).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssueFact {
    /// Maschinenlesbarer Code, z. B. `element-outside-head`.
    pub code: String,
    /// Betroffenes Element (kleingeschrieben), falls vorhanden.
    pub element: Option<String>,
    /// Der Befund liegt im `<head>` (sonst im `<body>`).
    pub in_head: bool,
}

/// Fakten der HTML-Validierung einer Seite (TECH-008, `advanced.html_validation`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ValidationFacts {
    /// Befunde in Dokumentreihenfolge.
    pub issues: Vec<ValidationIssueFact>,
    /// Anzahl Tokenfehler im `<head>`.
    pub head_errors: u32,
    /// Anzahl Tokenfehler im `<body>`.
    pub body_errors: u32,
    /// Größe des HTML-Dokuments in Byte (Grundlage der 2-MB-Filter).
    pub document_bytes: u64,
}

/// Ein Treffer eines Custom-Search-Filters (INTEG-020 ff.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// Index des Filters in `custom_search.filters`.
    pub def_index: u32,
    /// Anzahl der Treffer.
    pub count: u32,
    /// Fehler bei der Auswertung (z. B. XPath ungültig, Zeitlimit).
    pub error: Option<String>,
}

/// Ein extrahierter Wert eines Custom-Extractors (INTEG-040 ff.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedValue {
    /// Index des Extraktors in `custom_extraction.extractors`.
    pub def_index: u32,
    /// Laufende Nummer des Wertes innerhalb des Extraktors (ab 0, Dokumentreihenfolge).
    pub number: u32,
    /// Wert.
    pub value: String,
    /// Fehler bei der Auswertung.
    pub error: Option<String>,
}
