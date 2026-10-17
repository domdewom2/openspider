//! sidebar.siteStructure, responseTimes, segments (UI-066, UI-067, UI-069).
//!
//! STUB(M2-K03): Gerüst von M2-F01. Name, Argument- und Rückgabetyp sowie `long_running` stehen im Katalog
//! (`docs/plan/M2.md` 3.6a, `M2-ui-concept.md` 4.4) und ändert M2-K03 nicht; es ersetzt diese Datei durch die
//! Umsetzung. Bis dahin antwortet jeder Command mit `CmdError::Unsupported`.

stub_commands!(
    "M2-K03",
    [
        (
            "sidebar.siteStructure",
            "SiteStructureArgs",
            "SiteStructure",
            false
        ),
        (
            "sidebar.responseTimes",
            "ResponseTimesArgs",
            "ResponseTimes",
            false
        ),
    ]
);

// STUB(M2-K03)
pub fn on_config_changed(
    _app: &crate::App,
    _key: &crate::pool::CrawlKey,
    _old: &std::sync::Arc<os_model::CrawlConfig>,
    _new: &std::sync::Arc<os_model::CrawlConfig>,
) {
}
