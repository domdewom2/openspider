//! Verbindung zwischen robots.txt-Entscheidung und Engine (CRAWL-058, CRAWL-098/099).
//!
//! * [`sitemap_seeds`] macht aus den `Sitemap:`-Zeilen einer robots.txt Ausgangspunkte für
//!   [`crate::sitemaps::ingest`].
//! * [`blocked_fetch_facts`] bildet die Zeile einer durch robots.txt gesperrten URL (Status 0, „Blocked by
//!   Robots.txt“, treffende Zeile und robots.txt-URL aus der Entscheidung).
//!
//! Das Gate selbst (`RobotsGate`, Konfiguration über `RobotsGateConfig::from_crawl_config(&cfg)`) gehört P15;
//! hier steckt keine Entscheidungslogik über Sperren.

use crate::{
    frontier::{F_QUEUED, F_RULE_OK},
    worker::{header_rows, plain_page, Shared},
    writer_thread::WriterGone,
};
use os_fetch::robots::RobotsDecision;
use os_fetch::{Fetch, FetchResult};
use os_model::{Clock, CrawlConfig, FetchFacts, NoResponseReason, RobotsMode};
use os_model::{DiscoveryKind, UrlState, WriteBatch};
use os_url::ScopeClass;
use std::sync::Arc;
use url::Url;

use crate::sitemaps::SitemapSeed;

/// Herkunft der Seeds aus der robots.txt.
const FOUND_VIA_ROBOTS: &str = "robots";

/// Sitemap-Ausgangspunkte aus den `Sitemap:`-Zeilen der robots.txt (bei `sitemaps.crawl_linked` bzw.
/// `sitemaps.auto_discover_robots`).
///
/// * Ohne `crawl_linked` oder ohne `auto_discover_robots` gibt es keine Seeds (CRAWL-058: „Option aus:
///   robots.txt-Sitemaps werden nicht abgerufen“); bei `robots.mode = ignore` findet keine Autodiscovery statt.
/// * Nur absolute http(s)-URLs werden übernommen; die Signatur kennt die URL der robots.txt nicht. Relative
///   Angaben löst [`sitemap_seeds_at`] auf.
/// * Doppelte Angaben (nach Normalisierung) bleiben einmal in Dateireihenfolge stehen; `found_via = "robots"`.
pub fn sitemap_seeds(robots_sitemaps: &[String], cfg: &CrawlConfig) -> Vec<SitemapSeed> {
    seeds_from(robots_sitemaps, None, cfg)
}

/// Wie [`sitemap_seeds`], löst relative Angaben aber gegen die URL der robots.txt auf (CRAWL-058, OS-Festlegung:
/// laut Standard sollen die Angaben absolut sein).
pub fn sitemap_seeds_at(
    robots_url: &url::Url,
    robots_sitemaps: &[String],
    cfg: &CrawlConfig,
) -> Vec<SitemapSeed> {
    seeds_from(robots_sitemaps, Some(robots_url), cfg)
}

fn seeds_from(
    robots_sitemaps: &[String],
    base: Option<&url::Url>,
    cfg: &CrawlConfig,
) -> Vec<SitemapSeed> {
    let sm = &cfg.sitemaps;
    if !sm.crawl_linked || !sm.auto_discover_robots || cfg.robots.mode == RobotsMode::Ignore {
        return Vec::new();
    }
    let mut seen = std::collections::HashSet::new();
    let mut seeds = Vec::new();
    for raw in robots_sitemaps {
        let raw = raw.trim();
        let parsed = match base {
            Some(base) => base.join(raw).ok(),
            None => url::Url::parse(raw).ok(),
        };
        let Some(url) = parsed.filter(|u| matches!(u.scheme(), "http" | "https") && u.has_host())
        else {
            continue;
        };
        if seen.insert(os_url::normalize::normalize(&url).encoded) {
            seeds.push(SitemapSeed {
                url,
                found_via: FOUND_VIA_ROBOTS.to_string(),
            });
        }
    }
    seeds
}

/// Zeile für eine durch robots.txt gesperrte URL (Status 0, „Blocked by Robots.txt“).
///
/// Für `RobotsDecision::Blocked` ist das die vollständige Zeile: Status 0, `no_response_reason`
/// `BlockedByRobots` (bzw. `BlockedByRobotsUnreachable`, wenn die robots.txt nicht erreichbar war),
/// `robots_blocked`, treffende Zeile und robots.txt-URL aus der Entscheidung.
///
/// `RobotsDecision::ReportOnly` („Ignorieren, aber melden“, CRAWL-099) sperrt nicht: Die URL wird abgerufen und
/// behält ihre echten Abruffakten. Die Funktion liefert dann nur die robots-Angaben (kein Status, kein Grund) und
/// [`annotate_report_only`] überträgt sie auf die echten Fakten; `Allowed` ergibt leere Fakten mit Zeitstempel.
pub fn blocked_fetch_facts(decision: &RobotsDecision, clock: &dyn Clock) -> FetchFacts {
    let mut facts = FetchFacts {
        crawl_timestamp: clock.now(),
        ..FetchFacts::default()
    };
    match decision {
        RobotsDecision::Blocked {
            matched_line,
            robots_url,
            unreachable,
        } => {
            let reason = if *unreachable {
                NoResponseReason::BlockedByRobotsUnreachable
            } else {
                NoResponseReason::BlockedByRobots
            };
            facts.status_code = 0;
            facts.status = reason.status_text().to_string();
            facts.no_response_reason = Some(reason);
            facts.robots_blocked = true;
            facts.robots_matched_line = matched_line.clone();
            facts.robots_txt_url = Some(robots_url.clone());
        }
        RobotsDecision::ReportOnly { .. } => annotate_report_only(&mut facts, decision),
        RobotsDecision::Allowed => {}
    }
    facts
}

/// Überträgt die Meldung des Modus „Ignorieren, aber melden“ auf die Fakten eines echten Abrufs:
/// `robots_blocked` ist wahr, wenn die URL gesperrt gewesen wäre, dazu treffende Zeile und robots.txt-URL.
/// Status, Inhalt und Antwortzeit bleiben unberührt. Andere Entscheidungen ändern nichts.
pub fn annotate_report_only(facts: &mut FetchFacts, decision: &RobotsDecision) {
    if let RobotsDecision::ReportOnly {
        would_block,
        matched_line,
        robots_url,
    } = decision
    {
        facts.robots_blocked = *would_block;
        facts.robots_matched_line = if *would_block {
            matched_line.clone()
        } else {
            None
        };
        facts.robots_txt_url = Some(robots_url.clone());
    }
}

pub(crate) async fn record_robots<F: Fetch>(
    sh: &Arc<Shared<F>>,
    result: FetchResult,
    write: os_model::RobotsWrite,
) -> Result<Vec<SitemapSeed>, WriterGone> {
    let cfg = &*sh.cfg;
    let parsed_robots = Url::parse(&write.robots_url).ok();
    let norm_robots = parsed_robots.as_ref().map(os_url::normalize);
    let internal = norm_robots
        .as_ref()
        .is_some_and(|n| sh.scope.classify(n) == ScopeClass::Internal);
    // `Sitemap:`-Zeilen gelten nur für interne Hosts: die Sitemaps fremder Hosts gehören nicht zu diesem Crawl.
    let seeds = if internal {
        write
            .body
            .as_deref()
            .map(|body| {
                let listed: Vec<String> = os_fetch::robots::parse(body).sitemaps().to_vec();
                // relative Angaben (`Sitemap: /sitemap_index.xml`) löst die URL der robots.txt auf (CRAWL-058)
                match &parsed_robots {
                    Some(base) => crate::robots_glue::sitemap_seeds_at(base, &listed, cfg),
                    None => crate::robots_glue::sitemap_seeds(&listed, cfg),
                }
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Fortsetzen: die robots.txt dieses Origins stammt aus der Crawl-Datei; Zeile und Eintrag bleiben unverändert.
    if sh.primed_origins.contains(&write.origin) {
        return Ok(seeds);
    }

    let mut batch = WriteBatch::default();
    if let Some(norm) = norm_robots {
        let slot = sh.writer.reserve().await?;
        let mut core = sh.lock_core();
        if core.lookup(&norm.encoded).is_none() {
            let id = core.alloc_id();
            // K-54 (CRAWL-098): die robots.txt jedes Hosts, intern wie extern, wird als URL-Zeile ohne
            // Tabellen-Sichtbarkeit gespeichert; sie erscheint in keinem Tab, Filter, Zähler oder Export.
            let flags = F_QUEUED | F_RULE_OK;
            core.register(&norm.encoded, id, None, flags);
            batch.new_urls.push(os_model::NewUrl {
                id,
                url: norm.encoded.clone(),
                address: norm.display.clone(),
                origin: norm.origin.clone(),
                is_internal: internal,
                in_tables: false,
                discovered_via: DiscoveryKind::RobotsTxt,
                crawl_depth: None,
                folder_depth: os_url::folder_depth(&norm.url),
                url_length: os_url::url_length(&norm.display),
                state: UrlState::Done,
                not_crawled_reason: None,
            });
            let mut page = plain_page(id, &result, cfg);
            page.headers = header_rows(&result, cfg);
            batch.pages.push(page);
        }
        batch.robots.push(write);
        slot.send(batch);
        drop(core);
    } else {
        batch.robots.push(write);
        sh.writer.send(batch).await?;
    }
    Ok(seeds)
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{FixedClock, Timestamp};
    use std::time::Duration;

    fn clock() -> FixedClock {
        FixedClock::new(
            Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap(),
            Duration::from_millis(1),
        )
    }

    fn blocked(unreachable: bool) -> RobotsDecision {
        RobotsDecision::Blocked {
            matched_line: Some("Zeile 2: Disallow: /privat".to_string()),
            robots_url: "https://site.test/robots.txt".to_string(),
            unreachable,
        }
    }

    fn sitemaps_on() -> CrawlConfig {
        let mut cfg = CrawlConfig::default();
        cfg.sitemaps.crawl_linked = true;
        cfg
    }

    fn strs(seeds: &[SitemapSeed]) -> Vec<String> {
        seeds.iter().map(|s| s.url.to_string()).collect()
    }

    // spec: CRAWL-098
    #[test]
    fn blocked_facts_have_status_zero_and_blocked_text() {
        let facts = blocked_fetch_facts(&blocked(false), &clock());
        assert_eq!(facts.status_code, 0);
        assert_eq!(facts.status, "Blocked by Robots.txt");
        assert_eq!(
            facts.no_response_reason,
            Some(NoResponseReason::BlockedByRobots)
        );
        assert!(facts.robots_blocked);
        assert_eq!(
            facts.robots_matched_line.as_deref(),
            Some("Zeile 2: Disallow: /privat")
        );
        assert_eq!(
            facts.robots_txt_url.as_deref(),
            Some("https://site.test/robots.txt")
        );
        assert_eq!(
            facts.crawl_timestamp,
            Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap()
        );
        // Kein Inhalt, keine Größe, keine Antwortzeit: die URL wurde nie angefragt.
        assert_eq!(facts.size_bytes, 0);
        assert!(facts.response_time_ms.is_none() && facts.content_type.is_none());
    }

    // spec: CRAWL-098
    #[test]
    fn unreachable_robots_txt_blocks_with_its_own_reason() {
        let facts = blocked_fetch_facts(&blocked(true), &clock());
        assert_eq!(facts.status_code, 0);
        assert_eq!(
            facts.status,
            "Blocked by Robots.txt (robots.txt unreachable)"
        );
        assert_eq!(
            facts.no_response_reason,
            Some(NoResponseReason::BlockedByRobotsUnreachable)
        );
        assert!(facts.robots_blocked);
    }

    // spec: CRAWL-098
    #[test]
    fn blocked_without_matched_line_keeps_the_robots_url() {
        let d = RobotsDecision::Blocked {
            matched_line: None,
            robots_url: "https://site.test/robots.txt".to_string(),
            unreachable: true,
        };
        let facts = blocked_fetch_facts(&d, &clock());
        assert_eq!(facts.robots_matched_line, None);
        assert_eq!(
            facts.robots_txt_url.as_deref(),
            Some("https://site.test/robots.txt")
        );
    }

    // spec: CRAWL-099
    #[test]
    fn report_only_keeps_the_real_fetch_and_only_adds_the_robots_note() {
        let d = RobotsDecision::ReportOnly {
            would_block: true,
            matched_line: Some("Zeile 8: Disallow: /privat/".to_string()),
            robots_url: "https://site.test/robots.txt".to_string(),
        };
        // Keine Sperrzeile: kein Status 0, kein Grund.
        let only = blocked_fetch_facts(&d, &clock());
        assert_eq!(only.status_code, 0);
        assert_eq!(only.no_response_reason, None);
        assert!(only.status.is_empty());
        assert!(only.robots_blocked);

        // „/privat/x wird mit 200 abgerufen und als gesperrt gemeldet“.
        let mut real = FetchFacts {
            status_code: 200,
            status: "OK".to_string(),
            size_bytes: 123,
            ..FetchFacts::default()
        };
        annotate_report_only(&mut real, &d);
        assert_eq!(real.status_code, 200);
        assert_eq!(real.status, "OK");
        assert_eq!(real.size_bytes, 123);
        assert!(real.robots_blocked);
        assert_eq!(
            real.robots_matched_line.as_deref(),
            Some("Zeile 8: Disallow: /privat/")
        );
        assert_eq!(
            real.robots_txt_url.as_deref(),
            Some("https://site.test/robots.txt")
        );

        // Wäre nicht gesperrt: keine Sperrmeldung, keine treffende Zeile.
        let allowed = RobotsDecision::ReportOnly {
            would_block: false,
            matched_line: Some("Zeile 9: Allow: /privat/oeffentlich".to_string()),
            robots_url: "https://site.test/robots.txt".to_string(),
        };
        let mut real = FetchFacts::default();
        annotate_report_only(&mut real, &allowed);
        assert!(!real.robots_blocked);
        assert_eq!(real.robots_matched_line, None);

        // Andere Entscheidungen lassen die Fakten unverändert.
        let mut real = FetchFacts {
            status_code: 200,
            ..FetchFacts::default()
        };
        annotate_report_only(&mut real, &RobotsDecision::Allowed);
        assert_eq!(real.status_code, 200);
        assert!(!real.robots_blocked && real.robots_txt_url.is_none());
        assert!(!blocked_fetch_facts(&RobotsDecision::Allowed, &clock()).robots_blocked);
    }

    // spec: TECH-210
    #[test]
    fn robots_sitemaps_need_crawl_linked_and_auto_discover() {
        let lines = vec!["https://site.test/sitemap_index.xml".to_string()];
        // Standard: „Verlinkte XML-Sitemaps crawlen“ ist aus → nichts abrufen.
        assert!(sitemap_seeds(&lines, &CrawlConfig::default()).is_empty());

        let seeds = sitemap_seeds(&lines, &sitemaps_on());
        assert_eq!(strs(&seeds), ["https://site.test/sitemap_index.xml"]);
        assert_eq!(seeds[0].found_via, "robots");

        let mut cfg = sitemaps_on();
        cfg.sitemaps.auto_discover_robots = false;
        assert!(sitemap_seeds(&lines, &cfg).is_empty());
    }

    // spec: TECH-210
    #[test]
    fn ignoring_robots_txt_disables_autodiscovery() {
        let lines = vec!["https://site.test/s.xml".to_string()];
        let mut cfg = sitemaps_on();
        cfg.robots.mode = RobotsMode::Ignore;
        assert!(sitemap_seeds(&lines, &cfg).is_empty());
        // „Ignorieren, aber melden“ liest die robots.txt weiterhin.
        cfg.robots.mode = RobotsMode::IgnoreButReport;
        assert_eq!(sitemap_seeds(&lines, &cfg).len(), 1);
    }

    // spec: TECH-210
    #[test]
    fn seeds_keep_file_order_drop_duplicates_and_non_http_values() {
        let lines: Vec<String> = [
            "https://site.test/b.xml",
            "  https://site.test/a.xml  ",
            "https://SITE.test/b.xml",
            "https://site.test/b.xml#frag",
            "ftp://site.test/c.xml",
            "/relativ.xml",
            "kein url",
            "",
            "https://other.test/news.xml",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let seeds = sitemap_seeds(&lines, &sitemaps_on());
        assert_eq!(
            strs(&seeds),
            [
                "https://site.test/b.xml",
                "https://site.test/a.xml",
                "https://other.test/news.xml"
            ]
        );
    }

    // spec: TECH-210
    #[test]
    fn relative_sitemap_lines_resolve_against_the_robots_url() {
        let robots = url::Url::parse("https://site.test/dir/robots.txt").unwrap();
        let lines: Vec<String> = [
            "/sitemap_index.xml",
            "news.xml",
            "https://site.test/sitemap_index.xml",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let seeds = sitemap_seeds_at(&robots, &lines, &sitemaps_on());
        assert_eq!(
            strs(&seeds),
            [
                "https://site.test/sitemap_index.xml",
                "https://site.test/dir/news.xml"
            ]
        );
        assert!(sitemap_seeds_at(&robots, &lines, &CrawlConfig::default()).is_empty());
    }

    // spec: TECH-210
    #[test]
    fn sitemap_lines_of_the_robots_fixture_become_seeds() {
        let path = os_testsite::fixtures_root().join("crawl-robots/robots.txt");
        let text = std::fs::read_to_string(path)
            .expect("Fixture crawl-robots/robots.txt")
            .replace("{{BASE_URL}}", "http://127.0.0.1:8080");
        let txt = os_fetch::robots::parse(&text);
        assert_eq!(txt.sitemaps().len(), 2);

        let robots = url::Url::parse("http://127.0.0.1:8080/robots.txt").unwrap();
        let seeds = sitemap_seeds_at(&robots, txt.sitemaps(), &sitemaps_on());
        assert_eq!(
            strs(&seeds),
            [
                "http://127.0.0.1:8080/sitemap.xml",
                "http://127.0.0.1:8080/sitemap-news.xml"
            ]
        );
        assert!(seeds.iter().all(|s| s.found_via == "robots"));
    }
}
