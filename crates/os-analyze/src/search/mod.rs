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

/// Kontext der Spaltensuche (UI-011), Fachumsetzung M2-K02.
pub struct SearchCtx<'a> {
    pub tab: &'a crate::TabDef,
    pub columns: &'a [crate::ColumnDef],
    pub visible: &'a [&'a str],
}
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("nicht implementiert: {0}")]
    NotImplemented(&'static str),
    #[error("Suche abgebrochen")]
    Cancelled,
    #[error("Zeitlimit der Suche erreicht")]
    Deadline,
    #[error(transparent)]
    Store(#[from] os_store::StoreError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
}
// STUB(M2-K02)
pub fn compile_ctx(
    spec: &crate::views::SearchSpec,
    _ctx: &SearchCtx<'_>,
) -> Result<CompiledSearch, SearchError> {
    match spec {
        crate::views::SearchSpec::Simple(text) => Ok(CompiledSearch {
            sql: "instr(lower(u.address), lower(:q)) > 0".into(),
            params: vec![(":q".into(), os_model::Value::Text(text.clone()))],
        }),
        crate::views::SearchSpec::Rules { .. } => Err(SearchError::NotImplemented("M2-K02")),
    }
}
impl CompiledSearch {
    pub fn matches_bitmap(
        &self,
        file: &os_store::CrawlFile,
        deadline: std::time::Instant,
        cancel: &os_model::CancelToken,
    ) -> Result<RoaringBitmap, SearchError> {
        cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
        if std::time::Instant::now() >= deadline {
            return Err(SearchError::Deadline);
        }
        let mut stmt = file
            .conn()
            .prepare(&format!("SELECT u.id FROM urls u WHERE {}", self.sql))?;
        for (name, value) in &self.params {
            if let Some(index) = stmt.parameter_index(name)? {
                let value = match value {
                    os_model::Value::Null => rusqlite::types::Value::Null,
                    os_model::Value::Bool(v) => rusqlite::types::Value::Integer(i64::from(*v)),
                    os_model::Value::Int(v) => rusqlite::types::Value::Integer(*v),
                    os_model::Value::Float(v) => rusqlite::types::Value::Real(*v),
                    os_model::Value::Text(v) => rusqlite::types::Value::Text(v.clone()),
                };
                stmt.raw_bind_parameter(index, value)?;
            }
        }
        let mut rows = stmt.raw_query();
        let mut result = RoaringBitmap::new();
        while let Some(row) = rows.next()? {
            cancel.checkpoint().map_err(|_| SearchError::Cancelled)?;
            if std::time::Instant::now() >= deadline {
                return Err(SearchError::Deadline);
            }
            if let Ok(id) = u32::try_from(row.get::<_, i64>(0)?) {
                result.insert(id);
            }
        }
        Ok(result)
    }
}
