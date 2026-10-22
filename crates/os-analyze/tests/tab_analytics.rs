//! Tab „Analytics“ (INTEG-097, INTEG-098, INTEG-100, CRAWL-089) gegen die GA4-Fixture des Mocks: die Testsite mit acht
//! Seiten (`os_testsite::mocks::ga4::SITE_SCENARIO`) wird gecrawlt, die GA4-Werte (`SITE_ROWS`) stehen danach so in
//! `api_values`, wie die GA4-Quelle sie schreibt (Quelle `ga4`, Metrik = API-Name).

use os_analyze::analysis::{self, CancelToken};
use os_analyze::defs::SortKey;
use os_analyze::search::{Rule, RuleNode, RuleOp};
use os_analyze::tabs;
use os_analyze::views::{self, ColumnSelection, SearchSpec, ViewQuery};
use os_engine::testing::{crawl_dir_with, test_deps, FixtureCrawl};
use os_model::{CrawlConfig, FilterState, Value};
use os_testsite::mocks::ga4::{EXPECTED_FILTERS, SITE_ROWS, SITE_SCENARIO};

const HOST: &str = "http://integ.test";

fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.api.ga4.enabled = true;
    cfg.speed.threads = 1;
    // /orphan ist nur über die Sitemap aus der robots.txt erreichbar (CRAWL-058).
    cfg.sitemaps.crawl_linked = true;
    cfg
}

async fn crawl(cfg: &CrawlConfig) -> (tempfile::TempDir, FixtureCrawl) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("scenario.toml"), SITE_SCENARIO).unwrap();
    let fx = crawl_dir_with(dir.path(), &format!("{HOST}/"), cfg.clone(), test_deps()).await;
    store_site_values(&fx);
    (dir, fx)
}

fn url_id(fx: &FixtureCrawl, path: &str) -> i64 {
    fx.file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address = ?1",
            [format!("{HOST}{path}")],
            |r| r.get(0),
        )
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn put(fx: &FixtureCrawl, path: &str, metric: &str, value: f64) {
    fx.file
        .conn()
        .execute(
            "INSERT OR REPLACE INTO api_values (url_id, source, metric, level, v_real, fetched_at) \
             VALUES (?1, 'ga4', ?2, 'url', ?3, '2026-10-19T00:00:00.000Z')",
            rusqlite::params![url_id(fx, path), metric, value],
        )
        .unwrap();
}

fn store_site_values(fx: &FixtureCrawl) {
    for row in SITE_ROWS {
        for (metric, value) in row.metrics() {
            put(fx, row.path, &metric, value);
        }
    }
}

fn analyse(fx: &FixtureCrawl, cfg: &CrawlConfig) {
    assert!(
        analysis::run(&fx.file, cfg, &CancelToken::new(), &|_, _| {})
            .unwrap()
            .complete
    );
}

fn counts(fx: &FixtureCrawl, cfg: &CrawlConfig) -> Vec<(String, Option<u64>, FilterState)> {
    views::filter_counts_in(&tabs::configured(cfg), &fx.file, cfg)
        .unwrap()
        .into_iter()
        .filter(|c| c.tab == "Analytics")
        .map(|c| (c.filter, c.count, c.state))
        .collect()
}

fn count(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str) -> u64 {
    counts(fx, cfg)
        .into_iter()
        .find(|(f, ..)| f == filter)
        .and_then(|(_, n, _)| n)
        .unwrap_or_else(|| panic!("{filter}"))
}

fn addresses(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new("Analytics", filter);
    q.limit = 100;
    let page = views::query_view_in(&tabs::configured(cfg), &fx.file, cfg, &q).unwrap();
    page.rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(t) => t.trim_start_matches(HOST).to_string(),
            other => panic!("{other:?}"),
        })
        .collect()
}

// spec: INTEG-097
#[tokio::test]
async fn every_crawled_html_url_has_a_row_with_standard_ga4_columns() {
    let cfg = config();
    let (_dir, fx) = crawl(&cfg).await;
    let mut q = ViewQuery::new("Analytics", "All");
    q.columns = ColumnSelection::All;
    q.limit = 100;
    let page = views::query_view_in(&tabs::configured(&cfg), &fx.file, &cfg, &q).unwrap();
    assert_eq!(
        page.columns,
        [
            "Address",
            "Status Code",
            "Indexability",
            "Indexability Status",
            "GA4 Sessions",
            "GA4 Engaged Sessions",
            "GA4 Engagement Rate",
            "GA4 Views",
            "GA4 Conversions",
            "GA4 Event Count",
            "GA4 Total Revenue"
        ]
    );
    assert_eq!(page.total, 8);
    let row = |path: &str| {
        page.rows
            .iter()
            .find(|r| r[0] == Value::Text(format!("{HOST}{path}")))
            .unwrap_or_else(|| panic!("{path}"))
            .clone()
    };
    // GA4 liefert 0 Sitzungen → 0, nicht leer; ohne Zeile bleiben alle GA4-Zellen leer.
    assert_eq!(row("/zero")[4], Value::Int(0));
    assert!(row("/nodata")[4..].iter().all(|v| *v == Value::Null));
    assert_eq!(row("/five")[4], Value::Int(5));
    assert_eq!(row("/")[6], Value::Float(0.6));
    // Ohne GA4 im Profil keine leeren GA4-Spalten.
    let off = CrawlConfig::default();
    let tab = tabs::configured(&off)
        .into_iter()
        .find(|t| t.id == "Analytics")
        .unwrap();
    assert!(tab.columns.iter().all(|c| !c.key.starts_with("GA4 ")));
}

// spec: INTEG-098
#[tokio::test]
async fn five_filters_match_the_expected_fixture_counts() {
    let cfg = config();
    let (_dir, fx) = crawl(&cfg).await;
    // Vor der Crawl-Analyse bleibt „Orphan URLs“ leer.
    assert_eq!(
        counts(&fx, &cfg)
            .into_iter()
            .find(|(f, ..)| f == "Orphan URLs")
            .unwrap()
            .2,
        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
    );
    analyse(&fx, &cfg);
    for (filter, expected) in EXPECTED_FILTERS {
        assert_eq!(count(&fx, &cfg, filter), *expected, "{filter}");
    }
    assert_eq!(addresses(&fx, &cfg, "Bounce Rate Above 70%"), ["/three"]);
    assert_eq!(addresses(&fx, &cfg, "No GA Data"), ["/nodata"]);
    assert_eq!(
        addresses(&fx, &cfg, "Non-Indexable with GA Data"),
        ["/noindex"]
    );
    assert!(addresses(&fx, &cfg, "Orphan URLs").is_empty());
    let mut above = addresses(&fx, &cfg, "Sessions Above 0");
    above.sort();
    assert_eq!(
        above,
        ["/", "/about", "/five", "/noindex", "/orphan", "/three"]
    );
    // Ein geliefertes 0 zählt als Daten: /zero fehlt in „No GA Data“ und in „Sessions Above 0“.
    // Landingpage-Werte zählen ebenso; Bounce Rate nur mit mindestens einer Sitzung derselben Dimension.
    put(&fx, "/five", "bounceRate@landing", 0.8);
    put(&fx, "/five", "sessions@landing", 2.0);
    put(&fx, "/zero", "bounceRate@landing", 0.9);
    put(&fx, "/zero", "sessions@landing", 0.0);
    assert_eq!(
        addresses(&fx, &cfg, "Bounce Rate Above 70%"),
        ["/five", "/three"]
    );
    // Ohne GA4 im Profil: Hinweis statt Zahl.
    let off = CrawlConfig::default();
    assert!(counts(&fx, &off).iter().filter(|(f, ..)| f != "All").all(
        |(_, _, state)| matches!(state, FilterState::NeedsConfig(k) if k == "api.ga4.enabled")
    ));
}

// spec: CRAWL-089
#[tokio::test]
async fn orphan_filter_respects_ignore_non_indexable_but_non_indexable_filter_does_not() {
    let mut cfg = config();
    let (_dir, fx) = crawl(&cfg).await;
    analyse(&fx, &cfg);
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET indexability = 'Non-Indexable', indexability_status = 'noindex' WHERE address = ?1",
            [format!("{HOST}/orphan")],
        )
        .unwrap();
    assert!(cfg.issues.ignore_non_indexable);
    assert_eq!(
        counts(&fx, &cfg)
            .into_iter()
            .find(|(f, ..)| f == "Orphan URLs")
            .unwrap()
            .2,
        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
    );
    assert_eq!(count(&fx, &cfg, "Non-Indexable with GA Data"), 2);
    cfg.issues.ignore_non_indexable = false;
    assert_eq!(
        counts(&fx, &cfg)
            .into_iter()
            .find(|(f, ..)| f == "Orphan URLs")
            .unwrap()
            .2,
        FilterState::NeedsConfig("api.ga4.crawl_new_urls".into())
    );
    assert_eq!(count(&fx, &cfg, "Non-Indexable with GA Data"), 2);
}

// spec: INTEG-100
#[tokio::test]
async fn ga4_columns_work_in_rule_search_and_sorting() {
    let cfg = config();
    let (_dir, fx) = crawl(&cfg).await;
    let tabs = tabs::configured(&cfg);
    let mut q = ViewQuery::new("Analytics", "All");
    q.search = Some(SearchSpec::Rules {
        tree: RuleNode::Rule(Rule {
            column: "GA4 Sessions".into(),
            op: RuleOp::Greater,
            value: "100".into(),
        }),
        case_sensitive: false,
    });
    let page = views::query_view_in(&tabs, &fx.file, &cfg, &q).unwrap();
    // „größer als“ ist streng: /about mit genau 100 Sitzungen fehlt.
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0][0], Value::Text(format!("{HOST}/")));
    let mut sorted = ViewQuery::new("Analytics", "All");
    sorted.sort = vec![SortKey {
        column: "GA4 Sessions".into(),
        descending: true,
    }];
    let page = views::query_view_in(&tabs, &fx.file, &cfg, &sorted).unwrap();
    assert_eq!(
        page.rows[..3]
            .iter()
            .map(|r| r[0].clone())
            .collect::<Vec<_>>(),
        [
            Value::Text(format!("{HOST}/")),
            Value::Text(format!("{HOST}/about")),
            Value::Text(format!("{HOST}/noindex"))
        ]
    );
}

// spec: INTEG-092
// spec: INTEG-093
#[tokio::test]
async fn selected_metrics_and_landing_page_columns_follow_the_profile() {
    let mut cfg = config();
    cfg.api.ga4.metrics = vec![
        "sessions@landing".into(),
        "screenPageViews".into(),
        "bounceRate".into(),
        "customEvent:fixture_score".into(),
    ];
    let (_dir, fx) = crawl(&cfg).await;
    put(&fx, "/five", "sessions@landing", 4.0);
    let mut q = ViewQuery::new("Analytics", "All");
    q.columns = ColumnSelection::All;
    q.limit = 100;
    let page = views::query_view_in(&tabs::configured(&cfg), &fx.file, &cfg, &q).unwrap();
    assert_eq!(
        page.columns[4..],
        [
            "GA4 Sessions (Landing Page)",
            "GA4 Views",
            "GA4 Bounce Rate",
            "GA4 Fixture Score"
        ]
    );
    let five = page
        .rows
        .iter()
        .find(|r| r[0] == Value::Text(format!("{HOST}/five")))
        .unwrap();
    assert_eq!(five[4], Value::Int(4));
    assert_eq!(five[5], Value::Int(10));
    assert_eq!(five[6], Value::Float(0.7));
    assert_eq!(five[7], Value::Float(1.25));
}
