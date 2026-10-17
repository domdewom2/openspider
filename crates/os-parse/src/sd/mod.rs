//! Strukturierte Daten (TECH-180 ff.): JSON-LD, Microdata und RDFa nach `ParsedPage.structured_data`.
//!
//! M2-F02 legt Modul und Aufruf an (`parse_html` ruft `run` nur bei mindestens einem eingeschalteten Format auf);
//! Unterstufen: `jsonld`, `microdata`, `rdfa`, `model` (M2-D09). Die Validierung (`validate/`) gehört M2-D20.

pub mod jsonld;
pub mod microdata;
pub mod model;
pub mod rdfa;

use os_model::ParsedPage;

use crate::PageCtx;

/// Extrahiert die eingeschalteten Formate.
pub fn run(_ctx: &PageCtx<'_>, _page: &mut ParsedPage) {}

// STUB(M2-D09)

