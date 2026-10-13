//! Verbindung zwischen robots.txt-Entscheidung und Engine.
//!
//! STUB(M1-P15b)

use os_fetch::robots::RobotsDecision;
use os_model::{Clock, CrawlConfig, FetchFacts, NoResponseReason};

use crate::sitemaps::SitemapSeed;

/// Sitemap-Ausgangspunkte aus den `Sitemap:`-Zeilen der robots.txt (bei `sitemaps.crawl_linked` bzw.
/// `sitemaps.auto_discover_robots`).
// STUB(M1-P15b): leer
pub fn sitemap_seeds(_robots_sitemaps: &[String], _cfg: &CrawlConfig) -> Vec<SitemapSeed> {
    Vec::new()
}

/// Zeile für eine durch robots.txt gesperrte URL (Status 0, „Blocked by Robots.txt“).
// STUB(M1-P15b)
pub fn blocked_fetch_facts(decision: &RobotsDecision, clock: &dyn Clock) -> FetchFacts {
    let (reason, matched_line, robots_url) = match decision {
        RobotsDecision::Blocked {
            matched_line,
            robots_url,
            unreachable,
        } => (
            if *unreachable {
                NoResponseReason::BlockedByRobotsUnreachable
            } else {
                NoResponseReason::BlockedByRobots
            },
            matched_line.clone(),
            Some(robots_url.clone()),
        ),
        RobotsDecision::ReportOnly {
            matched_line,
            robots_url,
            ..
        } => (
            NoResponseReason::BlockedByRobots,
            matched_line.clone(),
            Some(robots_url.clone()),
        ),
        RobotsDecision::Allowed => (NoResponseReason::BlockedByRobots, None, None),
    };
    FetchFacts {
        crawl_timestamp: clock.now(),
        status_code: 0,
        status: reason.status_text().to_string(),
        no_response_reason: Some(reason),
        robots_blocked: true,
        robots_matched_line: matched_line,
        robots_txt_url: robots_url,
        ..FetchFacts::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use os_model::{FixedClock, Timestamp};
    use std::time::Duration;

    #[test]
    fn blocked_facts_have_status_zero_and_blocked_text() {
        let clock = FixedClock::new(
            Timestamp::from_db("2026-01-01T00:00:00.000Z").unwrap(),
            Duration::from_millis(1),
        );
        let decision = RobotsDecision::Blocked {
            matched_line: Some("Zeile 2: Disallow: /privat".to_string()),
            robots_url: "https://site.test/robots.txt".to_string(),
            unreachable: false,
        };
        let facts = blocked_fetch_facts(&decision, &clock);
        assert_eq!(facts.status_code, 0);
        assert_eq!(facts.status, "Blocked by Robots.txt");
        assert!(facts.robots_blocked);
        assert_eq!(
            facts.robots_matched_line.as_deref(),
            Some("Zeile 2: Disallow: /privat")
        );
    }

    #[test]
    fn no_sitemap_seeds_in_stub() {
        assert!(sitemap_seeds(
            &["https://site.test/s.xml".to_string()],
            &CrawlConfig::default()
        )
        .is_empty());
    }
}
