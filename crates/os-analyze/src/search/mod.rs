//! Suchgrammatik und Regelauswertung (K-6, UI-011): Textsyntax ↔ Regelbaum, SQL-Prädikat bzw. Bitmap-Filter.
//!
//! M2-F02 stellt die Signaturen bereit, an die `views::SearchSpec::Rules` und die View-Sitzungen (K-1) anschließen;
//! Parser und Auswertung (Regex mit Schrittlimit, Gesamtabbruch nach 2 s, parallele Adresssuche) gehören M2-K02.

use roaring::RoaringBitmap;

use crate::error::ViewError;

/// Operator einer Regel (UI-011).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleOp {
    /// Enthält.
    Contains,
    /// Enthält nicht.
    NotContains,
    /// Gleich.
    Equals,
    /// Ungleich.
    NotEquals,
    /// Beginnt mit.
    StartsWith,
    /// Endet mit.
    EndsWith,
    /// Passt auf den regulären Ausdruck.
    Regex,
    /// Größer als.
    Greater,
    /// Kleiner als.
    Less,
    /// Größer oder gleich.
    GreaterOrEqual,
    /// Kleiner oder gleich.
    LessOrEqual,
    /// Ist leer.
    IsEmpty,
    /// Ist nicht leer.
    IsNotEmpty,
}

/// Eine einzelne Regel: Spalte, Operator und Vergleichswert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Kanonischer Spaltenname (`ColumnDef.key`).
    pub column: String,
    /// Operator.
    pub op: RuleOp,
    /// Vergleichswert (leer bei `IsEmpty`/`IsNotEmpty`).
    pub value: String,
}

/// Regelbaum einer Suche (UI-011): Regeln, verknüpft mit UND, ODER und NICHT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleNode {
    /// Alle Teilbäume müssen zutreffen.
    And(Vec<RuleNode>),
    /// Mindestens ein Teilbaum muss zutreffen.
    Or(Vec<RuleNode>),
    /// Der Teilbaum darf nicht zutreffen.
    Not(Box<RuleNode>),
    /// Eine einzelne Regel.
    Rule(Rule),
}

/// Ergebnis von [`compile`]: Prädikat über `u` (Alias der Tabelle `urls`) samt Parametern.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledSearch {
    /// SQL-Bedingung über `u` mit benannten Parametern.
    pub sql: String,
    /// Benannte Parameter mit führendem `:`.
    pub params: Vec<(String, os_model::Value)>,
}

/// Fehler der Textsyntax mit Position (Zeichen ab 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Position des Fehlers im Text.
    pub position: usize,
    /// Meldungsschlüssel.
    pub code: String,
}

// STUB(M2-K02)
/// Wandelt die Textsyntax einer Suche in einen Regelbaum.
pub fn parse(_text: &str) -> Result<RuleNode, ParseError> {
    Err(ParseError {
        position: 0,
        code: "not-implemented".to_string(),
    })
}

// STUB(M2-K02)
/// Übersetzt einen Regelbaum in ein SQL-Prädikat (gleiche Semantik für Ansicht, Export und MCP).
pub fn compile(_tree: &RuleNode, _case_sensitive: bool) -> Result<CompiledSearch, ViewError> {
    Err(ViewError::NotImplemented("search::compile (M2-K02)"))
}

/// Lieferant einer Menge von URL-Kennungen als Bitmap: Segmente (K-14) und Vergleichsklassen (K-15) schränken eine
/// Ansicht über diesen Weg ein, ohne dass `views` ihre Logik kennt (K-1).
pub trait FilterSource: Send + Sync {
    /// Kennung der Quelle (z. B. `segment:12`, `compare:added`); Teil des Cache-Schlüssels.
    fn key(&self) -> String;

    /// Die URL-Kennungen, die die Quelle zulässt.
    fn bitmap(&self, file: &os_store::CrawlFile) -> Result<RoaringBitmap, ViewError>;
}
