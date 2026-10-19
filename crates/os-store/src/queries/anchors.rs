//! Abfragen: Ankertexte und Link-Abfragen der Links-Tabs (Non-Descriptive Anchor Text, Detailansicht).
//!
//! Die Funktionen legt das Fachpaket an; M2-F02 stellt nur das Modul bereit, damit `queries/mod.rs` nach Welle 0
//! eingefroren ist.

// STUB(M2-D08)

/// Erweiterungspunkt für die Ankertext-Kollation (M2-D08).
pub fn register_collations(_conn: &rusqlite::Connection) -> Result<(), crate::StoreError> {
    Ok(())
}

// STUB(M2-D08)
pub fn detail_rows(
    _: &crate::CrawlFile,
    _: &[os_model::UrlId],
    _: u64,
    _: u64,
) -> Result<super::DetailData, crate::StoreError> {
    Err(crate::StoreError::NotImplemented("M2-D08"))
}
