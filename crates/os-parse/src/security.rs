//! Sicherheitsfakten einer Seite (ONPAGE-033 … 043): Mixed Content, Formulare, Cross-Origin-Links, protokollrelative
//! Ressourcen; Schlüssel im Namensraum `security.*` (`os_model::ext::NAMESPACES`).
//!
//! M2-F02 legt nur den Aufruf an (`parse_html` ruft `run` immer auf); die Auswertung gehört M2-D11.

// STUB(M2-D11)
use os_model::ParsedPage;

use crate::PageCtx;

/// Trägt Sicherheitsfakten in `page.ext` und `page.forms` ein.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}
