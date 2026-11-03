//! UI-102: Zerlegung der Overview-Tabs für das Diagramm zum gewählten Übersichtseintrag.
//!
//! Die Zerlegung nennt nur Filterkennungen; die Zähler liefert `sidebar.overview` ohnehin (keine neue Zählung).
//! Mindestumfang laut UI-102: Internal und External mit allen Inhaltstyp-Filtern außer `All`, Response Codes mit den
//! sieben Kategorien der Sicht ohne Präfix. Weitere Tabs nur mit einem Kern-Test, der die Disjunktheit belegt
//! (`tests/cmd_sidebar_partition_*.rs`).

use crate::dto::{FilterInfo, OverviewPartition};
use os_analyze::defs::TabDef;
use os_model::{CrawlConfig, RobotsMode};

/// Filter der Grundmenge jeder Zerlegung.
const BASE: &str = "All";

/// Response Codes, Sicht ohne Präfix: jede URL hat genau eine Statusklasse (Weiterleitungsarten und Ketten sind
/// Teilmengen von `Redirection (3XX)` und gehören deshalb nicht dazu). Ausnahme `ignore_but_report`: siehe
/// [`BLOCKED`].
const RESPONSE_CODES: &[&str] = &[
    "Blocked by Robots.txt",
    "No Response",
    "Success (2XX)",
    "Redirection (3XX)",
    "Client Error (4XX)",
    "Server Error (5XX)",
    "Other",
];

/// Im Robots-Modus `ignore_but_report` ruft der Crawler gesperrte URLs trotzdem ab und markiert sie als gesperrt
/// (`robots_blocked = 1` mit echtem Status). Dann liegt eine URL zugleich in `Blocked by Robots.txt` und in ihrer
/// Statusklasse; der Filter gehört in diesem Modus nicht zur Zerlegung (Entscheidung Orchestrator 03.11.2026). Gesperrte
/// URLs ohne Status fallen dann in „Nicht zugeordnet“.
const BLOCKED: &str = "Blocked by Robots.txt";

/// Inhaltstyp-Filter: Gleichheit auf der einwertigen Spalte `urls.content_class` (ONPAGE-021 … 024), damit
/// paarweise disjunkt.
fn is_content_class(predicate: &str) -> bool {
    predicate
        .strip_prefix("u.content_class = '")
        .and_then(|rest| rest.strip_suffix('\''))
        .is_some_and(|class| !class.is_empty() && !class.contains('\''))
}

/// Zerlegung eines Tabs aus seiner Definition und den Filtern der Overview-Antwort (Reihenfolge = Katalog).
/// `None` für Tabs ohne Zerlegung oder wenn die Grundmenge bzw. jede Kategorie in der Antwort fehlt.
pub(crate) fn partition(
    tab: &TabDef,
    present: &[FilterInfo],
    cfg: &CrawlConfig,
) -> Option<OverviewPartition> {
    let shown = |id: &str| present.iter().any(|f| f.id == id);
    let reported = cfg.robots.mode == RobotsMode::IgnoreButReport;
    let category = |f: &os_analyze::defs::FilterDef| match tab.id {
        "Internal" | "External" => f.id != BASE && is_content_class(f.predicate),
        "Response Codes" => RESPONSE_CODES.contains(&f.id) && !(reported && f.id == BLOCKED),
        _ => false,
    };
    if !matches!(tab.id, "Internal" | "External" | "Response Codes") {
        return None;
    }
    if !shown(BASE) {
        return None;
    }
    let filters: Vec<String> = tab
        .filters
        .iter()
        .filter(|f| category(f) && shown(f.id))
        .map(|f| f.id.to_string())
        .collect();
    (!filters.is_empty()).then(|| OverviewPartition {
        base: BASE.into(),
        filters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> CrawlConfig {
        CrawlConfig::default()
    }

    fn tab(id: &str) -> TabDef {
        os_analyze::tabs::configured(&os_model::CrawlConfig::default())
            .into_iter()
            .find(|t| t.id == id)
            .unwrap()
    }

    fn present(tab: &TabDef) -> Vec<FilterInfo> {
        let cfg = os_model::CrawlConfig::default();
        tab.filters
            .iter()
            .map(|f| crate::commands::catalog::info(tab.id, f, &cfg))
            .collect()
    }

    // spec: UI-102
    #[test]
    fn minimum_scope_follows_the_catalog_order() {
        let internal = tab("Internal");
        let p = partition(&internal, &present(&internal), &cfg()).unwrap();
        assert_eq!(p.base, "All");
        assert_eq!(
            p.filters,
            [
                "HTML",
                "JavaScript",
                "CSS",
                "Images",
                "PDF",
                "Unknown",
                "Plugins",
                "Other",
                "Media",
                "Fonts",
                "XML"
            ]
        );
        let external = tab("External");
        let p = partition(&external, &present(&external), &cfg()).unwrap();
        assert_eq!(p.filters.len(), 11);
        assert!(!p.filters.iter().any(|f| f == "All"));
        let codes = tab("Response Codes");
        let p = partition(&codes, &present(&codes), &cfg()).unwrap();
        assert_eq!(p.filters, RESPONSE_CODES);
    }

    // spec: UI-102
    #[test]
    fn overlapping_tabs_and_missing_filters_have_no_partition() {
        let titles = tab("Page Titles");
        assert_eq!(partition(&titles, &present(&titles), &cfg()), None);
        let codes = tab("Response Codes");
        let mut shown = present(&codes);
        shown.retain(|f| f.id != "No Response");
        let p = partition(&codes, &shown, &cfg()).unwrap();
        assert!(!p.filters.iter().any(|f| f == "No Response"));
        shown.retain(|f| f.id != "All");
        assert_eq!(partition(&codes, &shown, &cfg()), None);
        assert!(!is_content_class("u.content_class = 'A' OR 1"));
        assert!(!is_content_class("u.content_class = ''"));
        assert!(is_content_class("u.content_class = 'XML'"));
    }

    // spec: UI-102
    #[test]
    fn ignore_but_report_drops_blocked_from_the_response_codes_partition() {
        let mut reported = cfg();
        reported.robots.mode = RobotsMode::IgnoreButReport;
        let codes = tab("Response Codes");
        let p = partition(&codes, &present(&codes), &reported).unwrap();
        assert_eq!(p.filters, RESPONSE_CODES[1..]);
        assert!(!p.filters.iter().any(|f| f == BLOCKED));
        let internal = tab("Internal");
        assert_eq!(
            partition(&internal, &present(&internal), &reported),
            partition(&internal, &present(&internal), &cfg())
        );
        let mut ignored = cfg();
        ignored.robots.mode = RobotsMode::Ignore;
        let p = partition(&codes, &present(&codes), &ignored).unwrap();
        assert_eq!(p.filters, RESPONSE_CODES);
    }
}
