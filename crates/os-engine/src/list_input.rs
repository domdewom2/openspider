//! Listenmodus: Eingabequellen lesen (CRAWL-027).
//!
//! STUB(M1-P19)

use std::path::Path;

use crate::error::EngineError;

/// Ergebnis des Einlesens einer URL-Liste.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListParse {
    /// Erkannte `http(s)`-Zeichenketten in Reihenfolge (noch nicht dedupliziert).
    pub urls: Vec<String>,
    /// Zeilen ohne erkennbare URL.
    pub ignored_lines: u64,
}

/// Liest eine Datei (txt, csv, tsv, xls, xlsx, xml, xml.gz).
// STUB(M1-P19)
pub fn read_list(_path: &Path) -> Result<ListParse, EngineError> {
    Err(EngineError::NotImplemented(
        "list_input::read_list (M1-P19)",
    ))
}

/// Liest eingefügten oder getippten Text.
// STUB(M1-P19)
pub fn parse_text(_s: &str) -> ListParse {
    ListParse::default()
}
