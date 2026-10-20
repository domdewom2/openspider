//! Gemeinsame Hilfen der Vergleichstests (M2-C01); als `#[path]`-Modul eingebunden, eigenes Testziel ohne Tests.
//!
//! Fixtures: `out-m2-compare/v1` und `out-m2-compare/v2` (zwei Stände derselben Site unter `www.site.test`, siehe
//! `tests/fixtures/out-m2-compare/README.md`) und `out-m2-compare/mapping` (Staging- und Live-Host).
#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;

use os_analyze::compare::{Comparison, CrawlSide};
use os_engine::testing::{crawl_fixture_with, test_deps, FixtureCrawl};
use os_model::{CancelToken, CrawlConfig, FixedClock, MappingRule, SeededIds, Timestamp};
use os_store::StorageProfile;

/// Standardkonfiguration der Vergleichs-Crawls.
pub fn cfg() -> CrawlConfig {
    CrawlConfig::default()
}

/// Crawlt einen Stand der Site; `start` ist der Startzeitpunkt (ISO), `seed` macht die Crawl-Kennung eindeutig.
pub async fn crawl_at(
    scenario: &str,
    start_url: &str,
    cfg: CrawlConfig,
    start: &str,
    seed: u64,
    profile: StorageProfile,
) -> FixtureCrawl {
    let mut deps = test_deps();
    deps.clock = Arc::new(FixedClock::new(
        Timestamp::from_db(start).expect("Startzeitpunkt"),
        Duration::from_millis(1),
    ));
    deps.ids = Arc::new(SeededIds::new(seed));
    deps.profile = profile;
    crawl_fixture_with(scenario, start_url, cfg, deps).await
}

/// Stand 1 (Previous): 01.01.2026, Profil „Minimaler Speicher“.
pub async fn v1() -> FixtureCrawl {
    v1_with(cfg()).await
}

/// Stand 1 mit eigener Konfiguration.
pub async fn v1_with(cfg: CrawlConfig) -> FixtureCrawl {
    crawl_at(
        "out-m2-compare/v1",
        "http://www.site.test/",
        cfg,
        "2026-01-01T00:00:00.000Z",
        41,
        StorageProfile::MinMemory,
    )
    .await
}

/// Stand 2 (Current): 01.02.2026, Profil „Maximale Geschwindigkeit“.
pub async fn v2() -> FixtureCrawl {
    v2_with(cfg()).await
}

/// Stand 2 mit eigener Konfiguration.
pub async fn v2_with(cfg: CrawlConfig) -> FixtureCrawl {
    crawl_at(
        "out-m2-compare/v2",
        "http://www.site.test/",
        cfg,
        "2026-02-01T00:00:00.000Z",
        42,
        StorageProfile::MaxSpeed,
    )
    .await
}

/// Staging-Host (Trailing Slashes), älter.
pub async fn staging() -> FixtureCrawl {
    crawl_at(
        "out-m2-compare/mapping",
        "http://staging.site.test/",
        cfg(),
        "2026-01-01T00:00:00.000Z",
        51,
        StorageProfile::Balanced,
    )
    .await
}

/// Live-Host (ohne Trailing Slashes), neuer.
pub async fn live() -> FixtureCrawl {
    crawl_at(
        "out-m2-compare/mapping",
        "http://www.site.test/",
        cfg(),
        "2026-02-01T00:00:00.000Z",
        52,
        StorageProfile::Balanced,
    )
    .await
}

/// Öffnet eine Fixture-Datei als Vergleichsseite (lesend).
pub fn side(fx: &FixtureCrawl) -> CrawlSide {
    CrawlSide::open(fx.file.path(), fx.dir.path()).expect("Crawl-Datei öffnen")
}

/// Vergleich `a` gegen `b` (Rollen nach Startzeitpunkt).
pub fn compare(a: &FixtureCrawl, b: &FixtureCrawl, rules: &[MappingRule]) -> Comparison {
    Comparison::open(side(a), side(b), rules, &CancelToken::new()).expect("Vergleich öffnen")
}

/// Mapping-Regel.
pub fn rule(pattern: &str, replacement: &str) -> MappingRule {
    MappingRule {
        pattern: pattern.into(),
        replacement: replacement.into(),
    }
}

/// Adressen zu Kennungen einer Seite, sortiert.
pub fn addresses(
    file: &os_store::CrawlFile,
    ids: &os_analyze::compare::RoaringBitmap,
) -> Vec<String> {
    let mut stmt = file
        .conn()
        .prepare("SELECT address FROM urls WHERE id = ?1")
        .unwrap();
    let mut out: Vec<String> = ids
        .iter()
        .map(|id| stmt.query_row([i64::from(id)], |r| r.get(0)).unwrap())
        .collect();
    out.sort();
    out
}

/// Kennung einer URL (Adresse) in einer Datei.
pub fn id_of(file: &os_store::CrawlFile, address: &str) -> i64 {
    file.conn()
        .query_row("SELECT id FROM urls WHERE address = ?1", [address], |r| {
            r.get(0)
        })
        .unwrap_or_else(|e| panic!("{address}: {e}"))
}

/// Vollständige URL auf dem Live-Host.
pub fn www(path: &str) -> String {
    format!("http://www.site.test{path}")
}
