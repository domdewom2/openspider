//! Testhelfer für alle Pakete, die gegen einen echten Crawl der Fixtures testen (Feature `testing`).
//!
//! STUB(M1-P15): Der Stub startet die Testsite und legt eine leere Crawl-Datei an; P15 crawlt tatsächlich.
//! Die API bleibt stabil.

use std::sync::Arc;
use std::time::Duration;

use os_fetch::MemorySecretStore;
use os_model::{CrawlConfig, CrawlMode, FixedClock, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

use crate::types::EngineDeps;

/// Ergebnis von `crawl_fixture*`: laufende Testsite, geöffnete Crawl-Datei und Arbeitsverzeichnis.
pub struct FixtureCrawl {
    /// Laufende Testsite (wird beim Drop beendet).
    pub site: os_testsite::TestSite,
    /// Die Crawl-Datei.
    pub file: CrawlFile,
    /// Arbeitsverzeichnis mit der Crawl-Datei (wird beim Drop gelöscht).
    pub dir: tempfile::TempDir,
}

/// Deterministische Abhängigkeiten: `FixedClock` (ab 2026-01-01, 1 ms je Abfrage), `SeededIds(42)`,
/// Profil „Ausgewogen“, `MemorySecretStore`, keine eigene Senke, kein Hook.
pub fn test_deps() -> EngineDeps {
    let start = Timestamp::from_db("2026-01-01T00:00:00.000Z").expect("gültiger Startzeitpunkt");
    EngineDeps {
        clock: Arc::new(FixedClock::new(start, Duration::from_millis(1))),
        ids: Arc::new(SeededIds::new(42)),
        profile: StorageProfile::Balanced,
        secrets: Arc::new(MemorySecretStore::new()),
        sink_factory: None,
        on_finished: None,
        owner_kind: "cli",
    }
}

/// Crawlt das Szenario ab der Basis-URL der Testsite.
pub async fn crawl_fixture(scenario: &str, cfg: CrawlConfig) -> FixtureCrawl {
    crawl_fixture_from(scenario, "/", cfg).await
}

/// Crawlt das Szenario ab `start_path`.
pub async fn crawl_fixture_from(
    scenario: &str,
    start_path: &str,
    cfg: CrawlConfig,
) -> FixtureCrawl {
    crawl_fixture_with(scenario, start_path, cfg, test_deps()).await
}

/// Wie `crawl_fixture_from`, aber mit eigenen Abhängigkeiten (Hook, langsame Senke, `ManualClock`).
pub async fn crawl_fixture_with(
    scenario: &str,
    start_path: &str,
    mut cfg: CrawlConfig,
    deps: EngineDeps,
) -> FixtureCrawl {
    let site = os_testsite::TestSite::start(scenario)
        .await
        .unwrap_or_else(|e| panic!("Testsite {scenario:?} startet nicht: {e}"));
    cfg.network.resolve.push(format!("*.test={}", site.addr()));
    let dir = tempfile::tempdir().expect("temporäres Verzeichnis");
    let now = deps.clock.now();
    let init = CrawlInit {
        crawl_id: deps.ids.new_crawl_id(now),
        mode: CrawlMode::Spider,
        start_url: Some(site.url(start_path)),
        list_source: None,
        project: "test".to_string(),
        name: scenario.to_string(),
        config: cfg,
        created_at: now,
    };
    let file = CrawlFile::create(&dir.path().join("crawl.ospider"), &init, deps.profile)
        .expect("Crawl-Datei anlegen");
    FixtureCrawl { site, file, dir }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deps_are_deterministic() {
        let a = test_deps();
        let b = test_deps();
        let now = a.clock.now();
        assert_eq!(a.ids.new_crawl_id(now), b.ids.new_crawl_id(b.clock.now()));
        assert!(a.sink_factory.is_none() && a.on_finished.is_none());
    }

    #[tokio::test]
    async fn crawl_fixture_stub_creates_empty_crawl_file() {
        let fx = crawl_fixture("out-basic-site", CrawlConfig::default()).await;
        let meta = fx.file.meta().unwrap();
        assert_eq!(meta.mode, CrawlMode::Spider);
        assert_eq!(meta.start_url.as_deref(), Some(fx.site.url("/").as_str()));
        let urls: i64 = fx
            .file
            .conn()
            .query_row("SELECT COUNT(*) FROM urls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(urls, 0);
        let cfg = fx.file.config().unwrap();
        assert_eq!(
            cfg.network.resolve,
            vec![format!("*.test={}", fx.site.addr())]
        );
    }
}
