//! Abfragen: Crawl-Path-Report (erster Entdeckungsweg, K-11).
//!
//! Die Funktionen legt das Fachpaket an; M2-F02 stellt nur das Modul bereit, damit `queries/mod.rs` nach Welle 0
//! eingefroren ist.

// STUB(M2-D17)

// STUB(M2-D17)
pub fn detail_rows(
    _: &crate::CrawlFile,
    _: &[os_model::UrlId],
    _: u64,
    _: u64,
) -> Result<super::DetailData, crate::StoreError> {
    Err(crate::StoreError::NotImplemented("M2-D17"))
}
