//! hreflang-Annotationen (TECH-071 ff.).
//!
//! STUB(M1-P06)

use os_model::HreflangRef;

use crate::dom::Document;
use crate::ParseConfig;

/// Extrahiert die Annotationen; zweiter Wert = wegen des 500er-Limits verworfene Annotationen.
// STUB(M1-P06)
pub fn extract(_doc: &Document, _base: &url::Url, _cfg: &ParseConfig) -> (Vec<HreflangRef>, u32) {
    (Vec::new(), 0)
}
