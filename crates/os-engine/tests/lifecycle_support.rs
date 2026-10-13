//! Gemeinsame Hilfen der `lifecycle_*`-Tests (wird per `#[path]` eingebunden, enthält selbst keine Tests).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use os_engine::testing::test_deps;
use os_engine::{
    resume_crawl, start_crawl, CrawlHandle, CrawlRequest, CrawlSource, CrawlSummary, EngineDeps,
};
use os_fetch::{FetchConfig, HttpFetcher};
use os_model::CrawlConfig;
use os_store::{CrawlFile, OpenMode};
use os_testsite::TestSite;

/// Host, unter dem alle Lebenszyklus-Tests die Testsite ansprechen (Auflösung über `network.resolve`), damit
/// URLs in der Crawl-Datei nicht am Port der Testsite hängen und die Datei auf einen „anderen Rechner“ mit
/// anderem Port umziehen kann.
pub const HOST: &str = "site.test";

pub fn start_url() -> String {
    format!("http://{HOST}/")
}

pub fn url(path: &str) -> String {
    format!("http://{HOST}{path}")
}

pub fn copy_dir(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Kopie der Fixture `crawl-site-100` mit `delay_ms` je Seite.
pub fn slow_site_dir(delay_ms: u64) -> tempfile::TempDir {
    let src = os_testsite::fixtures_root().join("crawl-site-100");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&src, dir.path());
    let mut toml = String::new();
    for n in 0..100 {
        let path = if n == 0 {
            "/".to_string()
        } else {
            format!("/page-{n}.html")
        };
        toml.push_str(&format!(
            "[[route]]\npath = \"{path}\"\ndelay_ms = {delay_ms}\n\n"
        ));
    }
    std::fs::write(dir.path().join("scenario.toml"), toml).unwrap();
    dir
}

pub async fn start_site(dir: &Path) -> TestSite {
    TestSite::start_dir(dir, 0).await.unwrap()
}

/// Konfiguration mit einem Thread und der Auflösung `*.test` → Testsite.
pub fn config_for(site: &TestSite, threads: u32) -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = threads;
    cfg.network.resolve = vec![format!("*.test={}", site.addr())];
    cfg
}

pub fn http_fetcher(cfg: &CrawlConfig, deps: &EngineDeps) -> Arc<HttpFetcher> {
    let fetch_cfg =
        FetchConfig::from_crawl_config(cfg, &*deps.secrets, deps.clock.clone()).unwrap();
    Arc::new(HttpFetcher::new(fetch_cfg).unwrap())
}

pub async fn start(cfg: CrawlConfig, file: &Path, deps: EngineDeps) -> CrawlHandle {
    let fetcher = http_fetcher(&cfg, &deps);
    start_crawl(
        fetcher,
        CrawlRequest {
            config: cfg,
            source: CrawlSource::Spider {
                start_url: start_url(),
            },
            file: file.to_path_buf(),
            project: "lifecycle".to_string(),
            name: "lifecycle".to_string(),
        },
        deps,
    )
    .await
    .unwrap()
}

/// Konfiguration der Datei, mit der Auflösung auf die (neue) Testsite und der Anpassung `f`.
pub fn resume_config(
    file: &Path,
    site: &TestSite,
    f: impl FnOnce(&mut CrawlConfig),
) -> CrawlConfig {
    let mut cfg = CrawlFile::open(file, OpenMode::ReadOnly)
        .unwrap()
        .config()
        .unwrap();
    cfg.network.resolve = vec![format!("*.test={}", site.addr())];
    f(&mut cfg);
    cfg
}

pub async fn resume(cfg: CrawlConfig, file: &Path, deps: EngineDeps) -> CrawlHandle {
    let fetcher = http_fetcher(&cfg, &deps);
    resume_crawl(fetcher, file.to_path_buf(), Some(cfg), deps)
        .await
        .unwrap()
}

/// Wartet (Wanduhr, 5 ms Takt), bis `crawled` mindestens `n` erreicht.
pub async fn until_crawled(handle: &CrawlHandle, n: u64) {
    for _ in 0..4_000 {
        if handle.progress().crawled >= n {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("der Crawl erreicht {n} URLs nicht");
}

pub async fn run_to_end(handle: CrawlHandle) -> CrawlSummary {
    handle.wait().await.unwrap()
}

/// Inhalt der Tabelle `urls` als Zeilen (nach Kennung), zum Vergleichen zweier Crawls.
pub fn snapshot(path: &Path) -> Vec<String> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let mut stmt = file
        .conn()
        .prepare(
            "SELECT id, url, state, COALESCE(status_code, -1), COALESCE(crawl_depth, -1), discovered_via, \
             in_tables, inlinks, outlinks, unique_inlinks, COALESCE(not_crawled_reason, '') \
             FROM urls ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], |r| {
        Ok(format!(
            "{} {} {} {} d={} {} t={} in={} out={} uin={} {}",
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, i64>(6)?,
            r.get::<_, i64>(7)?,
            r.get::<_, i64>(8)?,
            r.get::<_, i64>(9)?,
            r.get::<_, String>(10)?,
        ))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// `url → Statuscode` aller abgerufenen URLs (ohne Kennungen; unabhängig von der Entdeckungsreihenfolge).
pub fn statuses(path: &Path) -> BTreeMap<String, i64> {
    let file = CrawlFile::open(path, OpenMode::ReadOnly).unwrap();
    let mut stmt = file
        .conn()
        .prepare("SELECT url, COALESCE(status_code, -1) FROM urls WHERE state = 'done'")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Anfragen am Server, die mehr als einmal eingingen: `host path_and_query` mit Anzahl.
pub fn duplicate_requests(site: &TestSite) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for r in site.requests() {
        *counts
            .entry(format!("{} {}", r.host, r.path_and_query))
            .or_default() += 1;
    }
    counts.into_iter().filter(|(_, n)| *n > 1).collect()
}

pub fn scalar(file: &CrawlFile, sql: &str) -> i64 {
    file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn open_ro(path: &Path) -> CrawlFile {
    CrawlFile::open(path, OpenMode::ReadOnly).unwrap()
}

pub fn deps() -> EngineDeps {
    test_deps()
}

/// Abhängigkeiten mit einer Uhr, die ab `start` läuft (z. B. nach dem letzten Heartbeat eines abgestürzten
/// Prozesses, dessen Zeitstempel von der Systemuhr stammen).
pub fn deps_from(start: os_model::Timestamp) -> EngineDeps {
    let mut d = test_deps();
    d.clock = Arc::new(os_model::FixedClock::new(start, Duration::from_millis(1)));
    d
}

pub fn crawl_file_in(dir: &tempfile::TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}
