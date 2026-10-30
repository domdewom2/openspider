//! M2-D06: fachliche Fixture-Prüfung über echte Crawl- und Store-Ergebnisse.
use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, FilterState, UrlId, Value};
use std::collections::{BTreeMap, BTreeSet};

async fn fixture() -> (FixtureCrawl, CrawlConfig) {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture("tech-m2-directives", cfg.clone()).await;
    // Prüfmarker liefert seit D10 der Crawl; hier für alle URLs gesetzt (gespeicherter Vertrag).
    fx.file
        .conn()
        .execute(
            "INSERT OR REPLACE INTO url_ext(url_id, key, v_int) SELECT id, 'validation.checked', 1 FROM urls",
            [],
        )
        .unwrap();
    (fx, cfg)
}

fn addresses(fx: &FixtureCrawl, cfg: &CrawlConfig, filter: &str) -> BTreeSet<String> {
    let mut q = ViewQuery::new("Directives", filter);
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    q.limit = 500;
    views::query_view(&fx.file, cfg, &q)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| {
            let Value::Text(s) = &r[0] else {
                panic!("Adresse")
            };
            s.replace(&fx.site.base_url(), "")
        })
        .collect()
}

// spec: TECH-057
// spec: TECH-058
// spec: TECH-059
// spec: TECH-061
// spec: TECH-062
// spec: TECH-063
// spec: TECH-065
#[tokio::test]
async fn directive_filters_match_fixture_goldens_including_invalid_and_agent_values() {
    let (fx, cfg) = fixture().await;
    let expected: BTreeMap<String, BTreeSet<String>> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/tech-m2-directives/filters.json"
    ))
    .unwrap();
    for (filter, expected) in expected {
        assert_eq!(addresses(&fx, &cfg, &filter), expected, "{filter}");
    }
    let status: String = fx
        .file
        .conn()
        .query_row(
            "SELECT indexability FROM urls WHERE address LIKE '%/unavailable-past.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(status, "Non-Indexable");
}

// spec: TECH-008 [M2]
// spec: TECH-065
#[tokio::test]
async fn outside_head_reports_disabled_missing_partial_and_complete_validation() {
    let (fx, mut cfg) = fixture().await;
    assert_eq!(addresses(&fx, &cfg, "Outside <head>").len(), 3);
    cfg.advanced.html_validation = false;
    assert!(matches!(
        views::state_of(&fx.file, &cfg, "Directives", "Outside <head>").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    assert!(addresses(&fx, &cfg, "Outside <head>").is_empty());
    // Rohwerte bleiben trotz ausgeschalteter Prüfung sichtbar.
    assert!(addresses(&fx, &cfg, "Noindex").contains("/body.html"));
    cfg.advanced.html_validation = true;
    fx.file.conn().execute("DELETE FROM url_ext WHERE key = 'validation.checked' AND url_id = (SELECT id FROM urls WHERE address LIKE '%/body.html')", []).unwrap();
    assert!(matches!(
        views::state_of(&fx.file, &cfg, "Directives", "Outside <head>").unwrap(),
        FilterState::Incomplete { unchecked: 1 }
    ));
    assert!(!addresses(&fx, &cfg, "Outside <head>").contains("/body.html"));
    fx.file
        .conn()
        .execute("DELETE FROM url_ext WHERE key = 'validation.checked'", [])
        .unwrap();
    assert_eq!(
        views::state_of(&fx.file, &cfg, "Directives", "Outside <head>").unwrap(),
        FilterState::NeedsRecrawl
    );
}

// spec: TECH-066 [M2]
#[tokio::test]
async fn details_use_the_smallest_nonnegative_snippet_value_from_applicable_sources() {
    let (fx, cfg) = fixture().await;
    for (path, expected) in [
        ("max", Some("12")),
        ("max-zero", Some("0")),
        ("max-invalid", None),
        ("conflict", Some("30")),
    ] {
        let id: i64 = fx
            .file
            .conn()
            .query_row(
                "SELECT id FROM urls WHERE address = ?1",
                [format!("{}/{path}.html", fx.site.base_url())],
                |r| r.get(0),
            )
            .unwrap();
        let details = os_analyze::tabs::directives::url_details(&fx.file, &cfg, UrlId(id)).unwrap();
        assert_eq!(details.first().map(|(_, v)| v.as_str()), expected, "{path}");
    }
}

// spec: TECH-009
// spec: CRAWL-089
#[tokio::test]
async fn ignore_non_indexable_never_suppresses_directives_filters() {
    let (fx, mut cfg) = fixture().await;
    let expected = addresses(&fx, &cfg, "Noindex");
    assert!(expected.contains("/header.html"));
    cfg.issues.ignore_non_indexable = false;
    assert_eq!(addresses(&fx, &cfg, "Noindex"), expected);
    for f in os_analyze::tabs::directives::tab().filters {
        assert!(!f.ignore_non_indexable, "{}", f.id);
    }
}

// spec: TECH-051 [M2]
#[tokio::test]
async fn respect_hidden_store_contract_keeps_directives_visible_and_indexability_is_standard() {
    let (fx, cfg) = fixture().await;
    let before = addresses(&fx, &cfg, "All");
    fx.file.conn().execute("INSERT INTO url_flags(url_id, flag) SELECT id, 'respect_hidden' FROM urls WHERE indexability_status = 'noindex'", []).unwrap();
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET in_tables = 0 WHERE indexability_status = 'noindex'",
            [],
        )
        .unwrap();
    assert_eq!(addresses(&fx, &cfg, "All"), before);
    assert!(addresses(&fx, &cfg, "Noindex").contains("/body.html"));
    let mut q = ViewQuery::new("Directives", "All");
    q.limit = 500;
    let page = views::query_view(&fx.file, &cfg, &q).unwrap();
    assert_eq!(
        &page.columns[page.columns.len() - 2..],
        ["Indexability", "Indexability Status"]
    );
    // Eine fremde Ausblendung ohne Respect-Marker bleibt verborgen.
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET in_tables = 0 WHERE address LIKE '%/noarchive.html'",
            [],
        )
        .unwrap();
    assert!(!addresses(&fx, &cfg, "All").contains("/noarchive.html"));
}

// spec: TECH-051 [M2]
#[tokio::test]
async fn respect_canonical_is_not_a_directives_visibility_exception() {
    let (fx, cfg) = fixture().await;
    fx.file.conn().execute("INSERT INTO url_flags(url_id, flag) SELECT id, 'respect_hidden' FROM urls WHERE address LIKE '%/noarchive.html'", []).unwrap();
    fx.file.conn().execute("UPDATE urls SET in_tables = 0, indexability = 'Non-Indexable', indexability_status = 'Canonicalised' WHERE address LIKE '%/noarchive.html'", []).unwrap();
    assert!(!addresses(&fx, &cfg, "All").contains("/noarchive.html"));
    assert!(!addresses(&fx, &cfg, "NoArchive").contains("/noarchive.html"));
}
