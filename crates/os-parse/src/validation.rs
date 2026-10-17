//! HTML-Validierung auf Token-Ebene (TECH-008, Option `advanced.html_validation`): füllt `ParsedPage.validation`.
//!
//! M2-F02 legt nur den Aufruf an (`parse_html` ruft `run` nur bei eingeschalteter Option auf); die Auswertung
//! gehört M2-D10.

// STUB(M2-D10)
use os_model::ParsedPage;

use crate::PageCtx;

/// Wertet das Dokument aus und setzt `page.validation`.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}
