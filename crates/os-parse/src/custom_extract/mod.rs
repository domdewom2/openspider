//! Custom Extraction (INTEG-040 ff.): XPath 1.0 und 2.0 – 3.1, CSS-Selektor und Regex nach
//! `ParsedPage.custom_extraction`.
//!
//! M2-F02 legt Modul und Aufruf an (`parse_html` ruft `run` nur bei mindestens einem Extraktor auf); die Auswertung
//! gehört M2-D15.

// STUB(M2-D15)
use os_model::ParsedPage;

use crate::PageCtx;

/// Wertet jeden Extraktor aus und setzt `page.custom_extraction`.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}
