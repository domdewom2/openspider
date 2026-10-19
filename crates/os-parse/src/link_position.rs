//! Linkpositionen (INTEG-080 ff.): Klassifikation des Link-Pfades nach den geordneten Regeln aus
//! `link_positions.rules`.
//!
//! Die erste passende Regel gewinnt, unabhängig vom nächsten Landmark-Vorfahren. Kopfelemente
//! behalten außerhalb der Regelliste die Position `Head` (K2-45).

use os_model::{LinkPosition, LinkPositionsConfig};

/// Position eines Links. `link_path` ist der absolute XPath des Elements, `landmark` die Position, die die
/// Vorfahrensuche von M1 ergeben hat, `rules` die konfigurierten Regeln.
pub fn classify(
    link_path: &str,
    landmark: LinkPosition,
    rules: &LinkPositionsConfig,
) -> LinkPosition {
    if landmark == LinkPosition::Head {
        return LinkPosition::Head;
    }
    rules
        .rules
        .iter()
        .find(|rule| {
            rule.contains.is_empty() || rule.contains.iter().any(|part| link_path.contains(part))
        })
        .map(|rule| LinkPosition::from(rule.position.clone()))
        .unwrap_or(LinkPosition::Content)
}

/// Stabiler XPath mit Geschwisterindizes und zusätzlichen ID-/Klassen-Prädikaten. Attributwerte
/// bleiben case-sensitiv; XPath-Literale maskieren Anführungszeichen ohne Informationsverlust.
/// Kopfelemente behalten die bestehende kopfrelative Pfadform.
pub fn element_path(_doc: &crate::dom::Document, node: scraper::ElementRef<'_>) -> String {
    if crate::dom::in_head(node) {
        return crate::dom::xpath(node);
    }
    let mut segments = Vec::new();
    for element in
        std::iter::once(node).chain(node.ancestors().filter_map(scraper::ElementRef::wrap))
    {
        let name = element.value().name();
        let position = element
            .prev_siblings()
            .filter_map(scraper::ElementRef::wrap)
            .filter(|sibling| sibling.value().name() == name)
            .count()
            + 1;
        let mut segment = format!("{name}[{position}]");
        for attribute in ["id", "class"] {
            if let Some(value) = element.value().attr(attribute) {
                segment.push_str(&format!("[@{attribute}={}]", xpath_literal(value)));
            }
        }
        segments.push(segment);
    }
    segments.reverse();
    format!("/{}", segments.join("/"))
}

fn xpath_literal(value: &str) -> String {
    if !value.contains('\'') {
        format!("'{value}'")
    } else if !value.contains('"') {
        format!("\"{value}\"")
    } else {
        let parts: Vec<_> = value.split('\'').map(|part| format!("'{part}'")).collect();
        format!("concat({})", parts.join(",\"'\","))
    }
}
