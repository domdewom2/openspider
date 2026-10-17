//! Lesbarkeit (ONPAGE-149 … 152, Option `extraction.readability`): Flesch & Co. als `readability.*` in `page.ext`;
//! dazu MinHash-Signatur für Nahezu-Duplikate (ONPAGE-144, `duplicates.near_enabled`).
//!
//! M2-F02 legt nur den Aufruf an; die Auswertung gehört M2-D02.

// STUB(M2-D02)
use os_model::ParsedPage;

use crate::PageCtx;

/// Trägt Lesbarkeitswerte (und die MinHash-Signatur) ein.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}
