//! Anbieterfelder und ihre Darstellungsarten. Unbekannte Metriken werden vor dem Abruf abgewiesen.
use os_model::{ApiKind, ApiLevel};

#[derive(Clone, Copy)]
pub struct Metric {
    pub id: &'static str,
    pub label: &'static str,
    pub decimal: bool,
    pub traffic: bool,
    pub free: bool,
}
macro_rules! metrics {
    ($($id:literal => $label:literal, $decimal:literal, $traffic:literal, $free:literal;)+) => {
        &[$(Metric { id: $id, label: $label, decimal: $decimal, traffic: $traffic, free: $free }),+]
    };
}
pub fn catalog(kind: ApiKind) -> &'static [Metric] {
    match kind {
        ApiKind::Majestic => metrics! {
            "ExtBackLinks" => "External Backlinks", false, false, true;
            "RefDomains" => "Referring Domains", false, false, true;
            "TrustFlow" => "Trust Flow", false, false, true;
            "CitationFlow" => "Citation Flow", false, false, true;
            "RefIPs" => "Referring IPs", false, false, true;
            "RefSubNets" => "Referring Subnets", false, false, true;
            "IndexedURLs" => "Indexed URLs", false, false, true;
            "ExtBackLinksEDU" => "External Backlinks EDU", false, false, true;
            "ExtBackLinksGOV" => "External Backlinks GOV", false, false, true;
            "RefDomainsEDU" => "Referring Domains EDU", false, false, true;
            "RefDomainsGOV" => "Referring Domains GOV", false, false, true;
        },
        ApiKind::Ahrefs => metrics! {
            "domain_rating" => "Domain Rating", true, false, true;
            "url_rating" => "URL Rating", true, false, true;
            "backlinks" => "Backlinks", false, false, true;
            "refdomains" => "Referring Domains", false, false, true;
            "backlinks_dofollow" => "Dofollow Backlinks", false, false, true;
            "backlinks_nofollow" => "Nofollow Backlinks", false, false, true;
            "backlinks_redirect" => "Redirect Backlinks", false, false, true;
            "refips" => "Referring IPs", false, false, true;
            "refips_subnets" => "Referring Class-C Subnets", false, false, true;
            "linked_domains" => "Linked Root Domains", false, false, true;
            "outgoing_links" => "Outgoing Links External", false, false, true;
            "org_keywords" => "Organic Keywords", false, true, true;
            "org_keywords_1_3" => "Organic Keywords Top 3", false, true, true;
            "org_keywords_top10" => "Organic Keywords Top 10", false, true, true;
            "org_keywords_4_10" => "Organic Keywords 4–10", false, true, true;
            "org_traffic" => "Organic Traffic", false, true, true;
            "org_cost" => "Traffic Value", true, true, true;
        },
        ApiKind::Moz => metrics! {
            "page_authority" => "Page Authority", false, false, true;
            "domain_authority" => "Domain Authority", false, false, true;
            "spam_score" => "Spam Score", false, false, true;
            "brand_authority_score" => "Brand Authority", false, false, false;
            "link_propensity" => "Link Propensity", true, false, false;
            "root_domains_linking" => "Root Domains Linking", false, false, false;
            "external_pages_linking" => "External Pages Linking", false, false, false;
            "pages_linking" => "Pages Linking", false, false, false;
            "nofollow_root_domains" => "Nofollow Root Domains", false, false, false;
            "last_crawled" => "Last Crawled", false, false, false;
        },
        _ => &[],
    }
}
pub fn level_label(level: ApiLevel) -> &'static str {
    match level {
        ApiLevel::Url => "Exact URL",
        ApiLevel::UrlBoth => "Exact URL (HTTP + HTTPS)",
        ApiLevel::Subdomain => "Subdomain",
        ApiLevel::Domain => "Domain",
    }
}
pub fn supports_level(kind: ApiKind, level: ApiLevel) -> bool {
    level != ApiLevel::UrlBoth || kind == ApiKind::Ahrefs
}

/// INTEG-153: Der aktuelle v3-Batch-Endpunkt bietet diese Spec-Metriken nicht an.
/// Eine UI kann sie mit dieser Begründung deaktivieren, statt erfundene API-Felder zu senden.
pub fn unavailable_metrics(kind: ApiKind) -> &'static [(&'static str, &'static str)] {
    if kind == ApiKind::Ahrefs {
        &[
            (
                "referring_pages",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "backlinks_text",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "backlinks_image",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "backlinks_canonical",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "backlinks_gov",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "backlinks_edu",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "refdomains_gov",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "refdomains_edu",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "outgoing_links_internal",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "org_traffic_top3",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
            (
                "org_traffic_top10",
                "Kein entsprechendes Feld im v3-Batch-Vertrag",
            ),
        ]
    } else {
        &[]
    }
}
