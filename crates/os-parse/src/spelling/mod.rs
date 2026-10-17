//! Rechtschreib- und Grammatikprüfung (ONPAGE-153/154): Sprache erkennen, Text prüfen, Befunde nach `page.ext`
//! (Zähler) und – über die Engine – `WriteBatch.spelling`.
//!
//! M2-F02 legt Modul und Aufruf an (`parse_html` ruft `run` nur bei eingeschalteter Rechtschreib- oder
//! Grammatikprüfung auf); die Prüfung gehört M2-D19.

// STUB(M2-D19)
use os_model::ParsedPage;

use crate::PageCtx;

/// Prüft den Text der Seite.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}
