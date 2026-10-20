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
    /// Passt nicht auf den regulären Ausdruck.
    NotRegex,
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
    /// SQL-Bedingung über `u` mit benannten Parametern. Rules benötigt `prepare`.
    /// Simple behält den historischen ASCII-SQL-Plan; Unicode-Konsumenten verwenden
    /// `matches_bitmap` oder `prepare(...).sql()` (Views übernehmen das automatisch).
    pub sql: String,
    /// Benannte Parameter mit führendem `:`.
    pub params: Vec<(String, os_model::Value)>,
    program: eval::Program,
}

/// Fehler der Textsyntax mit Position (Zeichen ab 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// Position des Fehlers im Text.
    pub position: usize,
    /// Meldungsschlüssel.
    pub code: String,
}

/// Lieferant einer Menge von URL-Kennungen als Bitmap: Segmente (K-14) und Vergleichsklassen (K-15) schränken eine
/// Ansicht über diesen Weg ein, ohne dass `views` ihre Logik kennt (K-1).
pub trait FilterSource: Send + Sync {
    /// Kennung der Quelle (z. B. `segment:12`, `compare:added`); Teil des Cache-Schlüssels.
    fn key(&self) -> String;

    /// Die URL-Kennungen, die die Quelle zulässt.
    fn bitmap(&self, file: &os_store::CrawlFile) -> Result<RoaringBitmap, ViewError>;
}

mod eval;
mod folding;
mod parser;
mod view;
pub use eval::{compile, compile_ctx, search_budget, validate_regex, SearchCtx, SearchError};
pub use parser::format;
pub use view::SearchFilter;
pub use view::{
    compile_visible, extend_query_hash, prepare_view, prepared_query, query_identity,
    PreparedSearch,
};

/// Textsyntax → Regelbaum; Positionen sind Unicode-Zeichen ab 0.
pub fn parse(text: &str) -> Result<RuleNode, ParseError> {
    parser::parse(text)
}
