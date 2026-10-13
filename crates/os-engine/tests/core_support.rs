//! Gemeinsame Hilfen der `core_*`-Tests (wird per `#[path]` eingebunden, enthält selbst keine Tests).
#![allow(dead_code)]

use os_engine::testing::FixtureCrawl;
use os_model::CrawlConfig;

/// Eine Zeile der Tabelle `urls` mit den Spalten, die die Kern-Tests prüfen.
#[derive(Debug, Clone, PartialEq)]
pub struct UrlRow {
    pub id: i64,
    pub url: String,
    pub is_internal: bool,
    pub in_tables: bool,
    pub state: String,
    pub reason: Option<String>,
    pub via: String,
    pub depth: Option<i64>,
    pub status_code: Option<i64>,
    pub status: Option<String>,
    pub content_class: String,
    pub redirect_url: Option<String>,
    pub redirect_type: Option<String>,
    pub redirect_target: Option<i64>,
    pub refresh_delay: Option<i64>,
    pub no_response_reason: Option<String>,
}

const SELECT: &str = "SELECT id, url, is_internal, in_tables, state, not_crawled_reason, discovered_via, \
    crawl_depth, status_code, status, content_class, redirect_url, redirect_type, redirect_target_id, \
    meta_refresh_delay, no_response_reason FROM urls";

/// Alle URL-Zeilen in Entdeckungsreihenfolge.
pub fn all_rows(fx: &FixtureCrawl) -> Vec<UrlRow> {
    rows_of(&fx.file, "ORDER BY id")
}

pub fn rows_of(file: &os_store::CrawlFile, tail: &str) -> Vec<UrlRow> {
    let mut stmt = file.conn().prepare(&format!("{SELECT} {tail}")).unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok(UrlRow {
                id: r.get(0)?,
                url: r.get(1)?,
                is_internal: r.get::<_, i64>(2)? != 0,
                in_tables: r.get::<_, i64>(3)? != 0,
                state: r.get(4)?,
                reason: r.get(5)?,
                via: r.get(6)?,
                depth: r.get(7)?,
                status_code: r.get(8)?,
                status: r.get(9)?,
                content_class: r.get(10)?,
                redirect_url: r.get(11)?,
                redirect_type: r.get(12)?,
                redirect_target: r.get(13)?,
                refresh_delay: r.get(14)?,
                no_response_reason: r.get(15)?,
            })
        })
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

/// Die Zeile zur URL (kodierte Form) oder `None`.
pub fn row(fx: &FixtureCrawl, url: &str) -> Option<UrlRow> {
    all_rows(fx).into_iter().find(|r| r.url == url)
}

/// Wie `row`, aber mit Panik und lesbarer Meldung, wenn die Zeile fehlt.
pub fn row_of(fx: &FixtureCrawl, url: &str) -> UrlRow {
    row(fx, url).unwrap_or_else(|| {
        let known: Vec<_> = all_rows(fx).into_iter().map(|r| r.url).collect();
        panic!("keine Zeile für {url}; bekannt: {known:#?}")
    })
}

/// Basis-URL der Testsite mit dem Hostnamen `host` (ohne Port: die Resolve-Regel leitet auf die Testsite).
pub fn url(host: &str, path: &str) -> String {
    format!("http://{host}{path}")
}

/// Pfade der Abrufe am Server (ohne robots.txt), optional nur eines Hosts.
pub fn requested_paths(fx: &FixtureCrawl, host: Option<&str>) -> Vec<String> {
    fx.site
        .requests()
        .into_iter()
        .filter(|r| r.path() != "/robots.txt")
        .filter(|r| host.is_none_or(|h| r.host == h))
        .map(|r| r.path_and_query)
        .collect()
}

pub fn count_requests(fx: &FixtureCrawl, host: &str, path: &str) -> usize {
    fx.site
        .requests()
        .iter()
        .filter(|r| r.host == host && r.path_and_query == path)
        .count()
}

/// Flags (`url_flags`) als `(URL, Flag)`, sortiert.
pub fn flags(fx: &FixtureCrawl) -> Vec<(String, String)> {
    let mut stmt = fx
        .file
        .conn()
        .prepare(
            "SELECT u.url, f.flag FROM url_flags f JOIN urls u ON u.id = f.url_id ORDER BY u.url, f.flag",
        )
        .unwrap();
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .unwrap();
    rows.map(|r| r.unwrap()).collect()
}

pub fn scalar(fx: &FixtureCrawl, sql: &str) -> i64 {
    fx.file.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

/// Konfiguration mit nur einem Thread (streng deterministische Reihenfolge).
pub fn one_thread() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.speed.threads = 1;
    cfg
}

/// Startet einen Crawl gegen die Testsite, ohne auf das Ende zu warten (Stopp-/Pause-Tests).
pub struct Running {
    pub site: os_testsite::TestSite,
    pub handle: os_engine::CrawlHandle,
    pub path: std::path::PathBuf,
    pub dir: tempfile::TempDir,
}

pub async fn launch(
    dir_of_scenario: &std::path::Path,
    start_path: &str,
    mut cfg: CrawlConfig,
) -> Running {
    use std::sync::Arc;
    let site = os_testsite::TestSite::start_dir(dir_of_scenario, 0)
        .await
        .unwrap();
    cfg.network.resolve.push(format!("*.test={}", site.addr()));
    let deps = os_engine::testing::test_deps();
    let fetch_cfg =
        os_fetch::FetchConfig::from_crawl_config(&cfg, &*deps.secrets, deps.clock.clone()).unwrap();
    let fetcher = Arc::new(os_fetch::HttpFetcher::new(fetch_cfg).unwrap());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crawl.ospider");
    let handle = os_engine::start_crawl(
        fetcher,
        os_engine::CrawlRequest {
            config: cfg,
            source: os_engine::CrawlSource::Spider {
                start_url: site.url(start_path),
            },
            file: path.clone(),
            project: "t".to_string(),
            name: "t".to_string(),
        },
        deps,
    )
    .await
    .unwrap();
    Running {
        site,
        handle,
        path,
        dir,
    }
}
