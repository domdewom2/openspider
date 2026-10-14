//! Lesende Werkzeuge des MCP-Servers (AGENT-004, AGENT-010 … AGENT-013): `list_crawls`, `crawl_overview`,
//! `query_urls`, `url_details`, `list_issues`. Alle seitenstämmigen Felder (Titel, Texte, Ankertexte, Alt-Texte,
//! Header, Adressen …) stehen ausschließlich in `ToolOutput.untrusted`; die Zusammenfassung enthält nur
//! Zähler, Zustände und Katalogdaten.
//!
//! Die Werkzeuge öffnen die Crawl-Datei je Aufruf nur lesend und rechnen über dieselben Funktionen wie
//! Oberfläche und Export (`os_analyze::views`, `os_analyze::issues`, `os_export::reports`): keine zweite
//! Zählung, keine eigene Sortierung.

pub mod common;
pub mod crawl_overview;
pub mod list_crawls;
pub mod list_issues;
pub mod query_urls;
pub mod url_details;

use crate::tool::Tool;

/// Alle Werkzeuge (nach Freigabe gefiltert durch `permissions`), in der Reihenfolge von `tools/list`.
pub fn all() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(list_crawls::ListCrawls),
        Box::new(crawl_overview::CrawlOverview),
        Box::new(query_urls::QueryUrls),
        Box::new(url_details::UrlDetails),
        Box::new(list_issues::ListIssues),
    ]
}

#[cfg(test)]
mod tests {
    use crate::tool::ToolClass;

    // spec: AGENT-010, AGENT-011, AGENT-012, AGENT-013
    #[test]
    fn the_five_reading_tools_are_registered_with_class_read() {
        let tools = super::all();
        let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert_eq!(
            names,
            [
                "list_crawls",
                "crawl_overview",
                "query_urls",
                "url_details",
                "list_issues"
            ]
        );
        assert!(tools.iter().all(|t| t.class() == ToolClass::Read));
        for t in &tools {
            assert_eq!(t.input_schema()["type"], "object", "{}", t.name());
            assert!(!t.description().is_empty());
        }
    }
}
