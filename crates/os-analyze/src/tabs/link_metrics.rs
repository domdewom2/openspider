//! Link Metrics (INTEG-150/151/157), dieselben dynamischen Spalten auch im Internal-Tab.
use crate::defs::{ColumnDef, ColumnKind, ColumnSource, Needs, TabDef};
use os_model::{ApiLevel, CrawlConfig};
use std::collections::BTreeSet;

pub fn tab() -> TabDef {
    let mut tab = crate::defs::placeholder_tab("Link Metrics", vec![]);
    tab.population = "u.in_tables=1 AND ((u.is_internal=1 AND u.content_class='HTML') OR EXISTS(SELECT 1 FROM api_values lm WHERE lm.url_id=u.id AND lm.source IN ('majestic','ahrefs','moz')))";
    tab.columns = vec![
        ColumnDef::new("Address", "u.address", ColumnKind::Url, true),
        ColumnDef::new("Status Code", "u.status_code", ColumnKind::Int, true),
        ColumnDef::new("Title 1", "u.title_1", ColumnKind::Text, true),
    ];
    tab
}
pub fn column_source() -> Option<Box<dyn ColumnSource>> {
    Some(Box::new(LinkColumns))
}
struct LinkColumns;
impl ColumnSource for LinkColumns {
    fn columns(&self, cfg: &CrawlConfig) -> Vec<ColumnDef> {
        columns(cfg, &[])
    }
    fn columns_for_file(&self, cfg: &CrawlConfig, file: &os_store::CrawlFile) -> Vec<ColumnDef> {
        let stored: Vec<(String,String,String)> = file.conn().prepare("SELECT DISTINCT source,metric,level FROM api_values WHERE source IN ('majestic','ahrefs','moz') ORDER BY source,metric,level").and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?.collect()).unwrap_or_default();
        columns(cfg, &stored)
    }
}
fn columns(cfg: &CrawlConfig, stored: &[(String, String, String)]) -> Vec<ColumnDef> {
    let mut chosen = BTreeSet::new();
    for (source, settings) in [
        ("majestic", &cfg.api.majestic),
        ("ahrefs", &cfg.api.ahrefs),
        ("moz", &cfg.api.moz),
    ] {
        if settings.enabled {
            for metric in &settings.metrics {
                for level in &settings.levels {
                    chosen.insert((source.to_string(), metric.clone(), level.to_string()));
                }
            }
        }
    }
    chosen.extend(stored.iter().cloned());
    chosen.into_iter().filter_map(|(source, metric, level)| {
        let provider = match source.as_str() { "majestic" => "Majestic", "ahrefs" => "Ahrefs", "moz" => "Moz", _ => return None };
        let level_name = match level.parse::<ApiLevel>().ok()? { ApiLevel::Url => "Exact URL", ApiLevel::UrlBoth => "Exact URL (HTTP + HTTPS)", ApiLevel::Domain => "Domain", ApiLevel::Subdomain => "Subdomain" };
        let (label, kind) = label(&metric);
        let quote = |s: &str| s.replace('\'', "''");
        let expression = match kind {
            ColumnKind::Text => "a.v_text".to_string(),
            ColumnKind::Float { .. } => "COALESCE(a.v_real,CAST(a.v_text AS REAL))".into(),
            _ => "COALESCE(CAST(a.v_text AS INTEGER),CAST(a.v_real AS INTEGER))".into(),
        };
        Some(ColumnDef::dynamic(&format!("{provider} {label} ({level_name})"), &format!("(SELECT {expression} FROM api_values a WHERE a.url_id=u.id AND a.source='{}' AND a.metric='{}' AND a.level='{}')", quote(&source),quote(&metric),quote(&level)),kind,true).with_needs(Needs::Api(crate::defs::intern(&source))))
    }).collect()
}
fn label(metric: &str) -> (&str, ColumnKind) {
    let name = match metric {
        "ExtBackLinks" => "External Backlinks",
        "RefDomains" | "refdomains" => "Referring Domains",
        "TrustFlow" => "Trust Flow",
        "CitationFlow" => "Citation Flow",
        "RefIPs" | "refips" => "Referring IPs",
        "RefSubNets" => "Referring Subnets",
        "IndexedURLs" => "Indexed URLs",
        "ExtBackLinksEDU" => "External Backlinks EDU",
        "ExtBackLinksGOV" => "External Backlinks GOV",
        "RefDomainsEDU" => "Referring Domains EDU",
        "RefDomainsGOV" => "Referring Domains GOV",
        "domain_rating" => "Domain Rating",
        "url_rating" => "URL Rating",
        "backlinks" => "Backlinks",
        "backlinks_dofollow" => "Dofollow Backlinks",
        "backlinks_nofollow" => "Nofollow Backlinks",
        "backlinks_redirect" => "Redirect Backlinks",
        "refips_subnets" => "Referring Class-C Subnets",
        "linked_domains" => "Linked Root Domains",
        "outgoing_links" => "Outgoing Links External",
        "org_keywords" => "Organic Keywords",
        "org_keywords_1_3" => "Organic Keywords Top 3",
        "org_keywords_top10" => "Organic Keywords Top 10",
        "org_keywords_4_10" => "Organic Keywords 4–10",
        "org_traffic" => "Organic Traffic",
        "org_cost" => "Traffic Value",
        "page_authority" => "Page Authority",
        "domain_authority" => "Domain Authority",
        "spam_score" => "Spam Score",
        "brand_authority_score" => "Brand Authority",
        "link_propensity" => "Link Propensity",
        "root_domains_linking" => "Root Domains Linking",
        "external_pages_linking" => "External Pages Linking",
        "pages_linking" => "Pages Linking",
        "nofollow_root_domains" => "Nofollow Root Domains",
        "last_crawled" => "Last Crawled",
        _ => metric,
    };
    let kind = match metric {
        "domain_rating" | "url_rating" | "link_propensity" | "org_cost" => {
            ColumnKind::Float { decimals: 2 }
        }
        "last_crawled" => ColumnKind::Text,
        _ => ColumnKind::Int,
    };
    (name, kind)
}
