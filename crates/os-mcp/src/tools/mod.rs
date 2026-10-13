//! Lesende Werkzeuge des MCP-Servers (AGENT-004, AGENT-013): `list_crawls`, `crawl_overview`, `query_urls`,
//! `url_details`, `list_issues`. Alle seitenstämmigen Felder gehören ausschließlich in `ToolOutput.untrusted`.

pub mod crawl_overview;
pub mod list_crawls;
pub mod list_issues;
pub mod query_urls;
pub mod url_details;

use crate::tool::Tool;

/// Alle Werkzeuge (nach Freigabe gefiltert durch `permissions`).
// STUB(M1-P27): noch keine Werkzeuge
pub fn all() -> Vec<Box<dyn Tool>> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn stub_has_no_tools_yet() {
        assert!(super::all().is_empty());
    }
}
