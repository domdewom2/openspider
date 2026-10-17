//! Ressourcenschutz (CRAWL-129, K-3): Speicherstufen `normal`/`warning`/`paused` (80 % bzw. 90 % der RAM-Obergrenze) und
//! Messung der Dateigröße für `Progress.memory_stage`/`Progress.file_bytes`.
//!
//! M2-F02 legt nur das Modul an; die Umsetzung gehört M2-E02.

// STUB(M2-E02)

/// Eingaben des sekündlichen Ressourcenwächters (CRAWL-129).
pub struct MemoryInputs<'a> {
    pub progress: &'a os_model::Progress,
    pub config: &'a os_model::CrawlConfig,
}
#[derive(Debug, PartialEq, Eq)]
pub enum MemoryAction {
    None,
    Throttle(u32),
    Pause(String),
}
// STUB(M2-E02)
pub fn tick(_input: &MemoryInputs<'_>) -> MemoryAction {
    MemoryAction::None
}
