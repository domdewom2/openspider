//! Linkpositionen (INTEG-080 ff.): Klassifikation des Link-Pfades nach den geordneten Regeln aus
//! `link_positions.rules`.
//!
//! M2-F02 legt den Aufruf in `links::extract` an. Die Standardregeln müssen die Positionen von M1 exakt
//! reproduzieren (nächster Vorfahre `head`/`nav`/`header`/`main`/`aside`/`footer`, sonst `Content`); solange M2-D16
//! die Regelauswertung nicht liefert, gibt der Stub die Position aus der Vorfahrensuche unverändert zurück.

// STUB(M2-D16)
use os_model::{LinkPosition, LinkPositionsConfig};

/// Position eines Links. `link_path` ist der absolute XPath des Elements, `landmark` die Position, die die
/// Vorfahrensuche von M1 ergeben hat, `rules` die konfigurierten Regeln.
pub fn classify(
    _link_path: &str,
    landmark: LinkPosition,
    _rules: &LinkPositionsConfig,
) -> LinkPosition {
    landmark
}

// STUB(M2-D16)
pub fn element_path(_doc: &crate::dom::Document, node: scraper::ElementRef<'_>) -> String {
    crate::dom::xpath(node)
}
