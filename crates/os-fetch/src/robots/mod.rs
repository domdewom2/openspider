//! robots.txt: eigener Parser nach RFC 9309 und der `RobotsGate` (Abruf je Origin, Fehlerpolitik, Modi).
//!
//! STUB(M1-P03): P03 ersetzt Parser und Gate; die Signaturen bleiben (M1-Plan 3.6). Weitere Dateien
//! (z. B. `parser.rs`, `gate.rs`) legt P03 unter `robots/` an.

use std::sync::Arc;

use os_model::{Clock, CrawlConfig, RobotsMode, RobotsUnreachablePolicy, RobotsWrite};

use crate::http::{Fetch, FetchResult};

/// Konfiguration des `RobotsGate`.
#[derive(Clone)]
pub struct RobotsGateConfig {
    /// robots.txt-Token, mit dem der Crawler Gruppen auswählt.
    pub token: String,
    /// Umgang mit robots.txt.
    pub mode: RobotsMode,
    /// Verhalten bei nicht erreichbarer robots.txt.
    pub unreachable_policy: RobotsUnreachablePolicy,
    /// Wiederholungen bei Serverfehlern.
    pub retries: u32,
    /// Wartezeit zwischen Wiederholungen in Sekunden.
    pub retry_delay_s: u32,
    /// Uhr (Wartezeiten und Zeitstempel).
    pub clock: Arc<dyn Clock>,
}

impl RobotsGateConfig {
    /// Leitet die Konfiguration aus der Crawl-Konfiguration ab.
    pub fn from_crawl_config(cfg: &CrawlConfig, clock: Arc<dyn Clock>) -> Self {
        RobotsGateConfig {
            token: cfg
                .user_agent
                .robots_token
                .clone()
                .unwrap_or_else(|| cfg.user_agent.preset.clone()),
            mode: cfg.robots.mode,
            unreachable_policy: cfg.robots.unreachable_policy,
            retries: cfg.robots.retries,
            retry_delay_s: cfg.robots.retry_delay_s,
            clock,
        }
    }
}

/// Entscheidet je URL, ob robots.txt den Abruf erlaubt; ruft die robots.txt je Origin genau einmal ab.
pub struct RobotsGate {
    #[allow(dead_code)]
    cfg: RobotsGateConfig,
}

/// Ergebnis einer Prüfung.
#[derive(Debug, Clone)]
pub struct RobotsCheck {
    /// Entscheidung.
    pub decision: RobotsDecision,
    /// Nur beim Erstabruf je Origin: Abrufergebnis und Schreibmodell der robots.txt.
    pub robots_fetch: Option<(FetchResult, RobotsWrite)>,
}

/// Entscheidung des Gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RobotsDecision {
    /// Abruf erlaubt.
    Allowed,
    /// Abruf gesperrt.
    Blocked {
        /// Treffende Regel als „Zeile n: Regeltext“.
        matched_line: Option<String>,
        /// URL der robots.txt.
        robots_url: String,
        /// Gesperrt, weil die robots.txt nicht erreichbar war.
        unreachable: bool,
    },
    /// Modus „Ignorieren, aber melden“: abrufen, Sperre nur protokollieren.
    ReportOnly {
        /// Die URL wäre gesperrt gewesen.
        would_block: bool,
        /// Treffende Regel als „Zeile n: Regeltext“.
        matched_line: Option<String>,
        /// URL der robots.txt.
        robots_url: String,
    },
}

impl RobotsGate {
    /// Legt das Gate an.
    pub fn new(cfg: RobotsGateConfig) -> Self {
        RobotsGate { cfg }
    }

    /// Prüft eine URL. Stub: erlaubt alles und ruft nichts ab.
    // STUB(M1-P03)
    pub async fn check<F: Fetch + ?Sized>(&self, _fetcher: &F, _url: &url::Url) -> RobotsCheck {
        RobotsCheck {
            decision: RobotsDecision::Allowed,
            robots_fetch: None,
        }
    }
}

/// Eine treffende Regel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsMatch {
    /// `true` bei `Allow`, `false` bei `Disallow`.
    pub allowed: bool,
    /// Zeilennummer (1-basiert) in der robots.txt.
    pub line_no: u32,
    /// Regeltext der Zeile.
    pub line: String,
}

/// Ausgewertete robots.txt.
#[derive(Debug, Clone, Default)]
pub struct RobotsTxt {
    sitemaps: Vec<String>,
}

/// Wertet den Text einer robots.txt aus.
// STUB(M1-P03): Stub sammelt nur die `Sitemap:`-Zeilen.
pub fn parse(body: &str) -> RobotsTxt {
    let sitemaps = body
        .lines()
        .filter_map(|l| {
            let (key, value) = l.split_once(':')?;
            key.trim()
                .eq_ignore_ascii_case("sitemap")
                .then(|| value.trim().to_string())
        })
        .filter(|v| !v.is_empty())
        .collect();
    RobotsTxt { sitemaps }
}

impl RobotsTxt {
    /// Bestimmt die maßgebliche Regel für Token und Pfad samt Query; `None` = keine Regel trifft (erlaubt).
    // STUB(M1-P03)
    pub fn evaluate(&self, _token: &str, _path_and_query: &str) -> Option<RobotsMatch> {
        None
    }

    /// `Sitemap:`-Einträge in Dateireihenfolge.
    pub fn sitemaps(&self) -> &[String] {
        &self.sitemaps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_sitemap_lines() {
        let txt = parse("User-agent: *\nDisallow: /x\nSitemap: https://site.test/s.xml\n");
        assert_eq!(txt.sitemaps(), ["https://site.test/s.xml"]);
        assert_eq!(txt.evaluate("openspider", "/x"), None);
    }
}
