//! Konfigurationsprüfung über die Fachmodule (CRAWL-140).
pub fn validate(cfg: &os_model::CrawlConfig) -> Vec<os_model::ConfigIssue> {
    let mut issues = os_parse::custom_search::validate(&cfg.custom_search);
    issues.extend(os_parse::custom_extract::validate(&cfg.custom_extraction));
    issues.extend(os_url::rules::validate(cfg));
    issues.extend(os_render::snippets::network_warning(cfg));
    issues
}
