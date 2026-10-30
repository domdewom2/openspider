use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::crawl_fixture_from;
use os_model::{CrawlConfig, Value};

fn paths(file: &os_store::CrawlFile, cfg: &CrawlConfig, tab: &str, filter: &str) -> Vec<String> {
    let mut q = ViewQuery::new(tab, filter);
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    q.limit = 500;
    query_view(file, cfg, &q)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.trim_start_matches("http://site.test").to_string(),
            v => panic!("address: {v:?}"),
        })
        .collect()
}

// spec: TECH-051 [M2]
// spec: TECH-031, TECH-071 [M2]
// spec: TECH-281
#[tokio::test]
async fn populations_follow_response_and_content_class() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    let directives = paths(&fx.file, &cfg, "Directives", "All");
    assert!(directives.contains(&"/asset.css".into()));
    assert!(!directives.contains(&"/gone.html".into()));
    assert!(!directives.contains(&"/redirect".into()));
    for tab in ["Pagination", "Hreflang"] {
        let all = paths(&fx.file, &cfg, tab, "All");
        assert!(all.contains(&"/".into()));
        for excluded in ["/gone.html", "/redirect", "/asset.css"] {
            assert!(!all.contains(&excluded.into()), "{tab}: {excluded}");
        }
    }
    let validation = paths(&fx.file, &cfg, "Validation", "All");
    for included in ["/gone.html", "/redirect", "/asset.css"] {
        assert!(validation.contains(&included.into()), "{included}");
    }
}

// spec: TECH-011 [M2]
// spec: TECH-021, TECH-023, TECH-082, TECH-084, TECH-086
#[tokio::test]
async fn problem_annotations_have_no_values_or_indexability_effect_with_validation_on_or_off() {
    for enabled in [false, true] {
        let mut cfg = CrawlConfig::default();
        cfg.advanced.html_validation = enabled;
        let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
        for path in ["/body.html", "/attr.html", "/header.html"] {
            let (count, status): (i64, Option<String>) = fx
                .file
                .conn()
                .query_row(
                    "SELECT canonical_count, indexability_status FROM urls WHERE url = ?1",
                    [format!("http://site.test{path}")],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(count, 0, "{path}");
            assert_ne!(status.as_deref(), Some("Canonicalised"));
        }
        assert!(paths(
            &fx.file,
            &cfg,
            "Canonicals",
            "Invalid Attribute In Annotation"
        )
        .contains(&"/attr.html".into()));
        assert!(paths(
            &fx.file,
            &cfg,
            "Canonicals",
            "Invalid Attribute In Annotation"
        )
        .contains(&"/header.html".into()));
        assert!(
            !paths(&fx.file, &cfg, "Hreflang", "Contains Hreflang").contains(&"/body.html".into())
        );
        assert!(paths(&fx.file, &cfg, "Hreflang", "Missing").contains(&"/body.html".into()));
        if enabled {
            assert_eq!(
                paths(&fx.file, &cfg, "Canonicals", "Outside <head>"),
                vec!["/body.html"]
            );
            assert_eq!(
                paths(&fx.file, &cfg, "Hreflang", "Outside <head>"),
                vec!["/body.html"]
            );
        }
    }
}

// spec: TECH-001, TECH-063, TECH-068, ONPAGE-059
#[tokio::test]
async fn expiration_priority_and_header_self_refresh_use_fetch_time() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    for (path, expected) in [
        ("/expired.html", Some("Unavailable After")),
        ("/both.html", Some("noindex")),
        ("/future.html", None),
        ("/header.html", Some("Redirected")),
        ("/meta.html", None),
        ("/header-bare.html", None),
    ] {
        let status: Option<String> = fx
            .file
            .conn()
            .query_row(
                "SELECT indexability_status FROM urls WHERE url = ?1",
                [format!("http://site.test{path}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status.as_deref(), expected, "{path}");
    }
    assert!(paths(
        &fx.file,
        &cfg,
        "Response Codes",
        "Internal Redirection (HTTP Refresh)"
    )
    .contains(&"/header.html".into()));
}

// spec: TECH-020
#[tokio::test]
async fn unlinked_lists_internal_canonical_target_and_clears_after_hyperlink() {
    use os_analyze::analysis::{runner::run_only, CancelToken};
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    let analyze = || {
        run_only(
            &fx.file,
            &cfg,
            &CancelToken::new(),
            &|_, _| {},
            &[os_model::AnalysisStep::Canonicals],
        )
        .unwrap()
    };
    analyze();
    assert_eq!(
        paths(&fx.file, &cfg, "Canonicals", "Unlinked"),
        vec!["/target.html"]
    );
    fx.file.conn().execute("INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) SELECT s.id,t.id,'/target.html','Hyperlink',100,1 FROM urls s, urls t WHERE s.url='http://site.test/' AND t.url='http://site.test/target.html'", []).unwrap();
    analyze();
    assert!(paths(&fx.file, &cfg, "Canonicals", "Unlinked").is_empty());
}

// spec: TECH-075, TECH-076, TECH-077, TECH-078, TECH-079
#[tokio::test]
async fn return_links_ignore_default_alias_and_real_hyperlinks_count_as_linked() {
    use os_analyze::analysis::{runner::run_only, CancelToken};
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    run_only(
        &fx.file,
        &cfg,
        &CancelToken::new(),
        &|_, _| {},
        &[os_model::AnalysisStep::Hreflang],
    )
    .unwrap();
    for filter in [
        "Unlinked Hreflang URLs",
        "Missing Return Links",
        "Inconsistent Language & Region Return Links",
        "Non-Canonical Return Links",
        "Noindex Return Links",
    ] {
        assert!(
            paths(&fx.file, &cfg, "Hreflang", filter).is_empty(),
            "{filter}"
        );
    }
}

// spec: TECH-063, TECH-068
#[tokio::test]
async fn recompute_keeps_expiration_and_http_refresh_when_meta_option_changes() {
    let cfg = CrawlConfig::default();
    let mut fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    let mut changed = cfg.clone();
    changed.advanced.respect_self_ref_meta_refresh = true;
    os_engine::recompute::d06::run(&mut fx.file, &cfg, &changed, &os_model::CancelToken::new())
        .unwrap();
    for (path, expected) in [
        ("/expired.html", "Unavailable After"),
        ("/header.html", "Redirected"),
        ("/meta.html", "Self Referencing Meta Refresh"),
    ] {
        let status: String = fx
            .file
            .conn()
            .query_row(
                "SELECT indexability_status FROM urls WHERE url = ?1",
                [format!("http://site.test{path}")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, expected, "{path}");
    }
}

// spec: TECH-079
#[tokio::test]
async fn noindex_return_links_also_checks_the_self_annotation() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET effective_index=0 WHERE url='http://site.test/de.html'",
            [],
        )
        .unwrap();
    let mut actual = paths(&fx.file, &cfg, "Hreflang", "Noindex Return Links");
    actual.sort();
    assert_eq!(actual, ["/de.html", "/lang.html"]);
}

// spec: TECH-022
// spec: TECH-018
#[tokio::test]
async fn fragment_is_retained_for_filtering_but_removed_from_both_canonical_columns() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    fx.file
        .conn()
        .execute(
            "UPDATE canonicals SET raw=raw || '#top', resolved=resolved || '#top'",
            [],
        )
        .unwrap();
    assert!(!paths(&fx.file, &cfg, "Canonicals", "Contains Fragment URL").is_empty());
    for source in ["html", "http"] {
        fx.file
            .conn()
            .execute("UPDATE canonicals SET source=?1", [source])
            .unwrap();
        let mut q = ViewQuery::new("Canonicals", "Contains Fragment URL");
        q.columns = ColumnSelection::Named(vec![if source == "html" {
            "Canonical Link Element"
        } else {
            "HTTP Canonical"
        }
        .into()]);
        let rows = query_view(&fx.file, &cfg, &q).unwrap().rows;
        assert!(!rows.is_empty());
        assert!(rows
            .iter()
            .flatten()
            .all(|v| !matches!(v, Value::Text(s) if s.contains('#'))));
    }
}

// spec: TECH-041
#[tokio::test]
async fn fetched_error_pagination_target_is_also_a_sequence_error() {
    use os_analyze::analysis::{runner::run_only, CancelToken};
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    fx.file.conn().execute("INSERT INTO pagination(url_id,source,position,rel,raw,url,target_id,in_head) SELECT s.id,'html',0,'next','/gone.html',t.url,t.id,1 FROM urls s,urls t WHERE s.url='http://site.test/' AND t.url='http://site.test/gone.html'", []).unwrap();
    let run = || {
        run_only(
            &fx.file,
            &cfg,
            &CancelToken::new(),
            &|_, _| {},
            &[os_model::AnalysisStep::Pagination],
        )
        .unwrap()
    };
    run();
    assert_eq!(paths(&fx.file, &cfg, "Pagination", "Sequence Error"), ["/"]);
    fx.file.conn().execute("UPDATE urls SET state='queued',status_code=NULL WHERE url='http://site.test/gone.html'", []).unwrap();
    run();
    assert!(paths(&fx.file, &cfg, "Pagination", "Sequence Error").is_empty());
}

// spec: TECH-078
#[tokio::test]
async fn noncanonical_return_filter_lists_the_alternate_target_and_updates_live() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    fx.file.conn().execute("UPDATE urls SET canonical_count=1,canonical_self=0 WHERE url='http://site.test/de.html'", []).unwrap();
    assert_eq!(
        paths(&fx.file, &cfg, "Hreflang", "Non-Canonical Return Links"),
        ["/de.html"]
    );
    fx.file
        .conn()
        .execute(
            "UPDATE urls SET canonical_self=1 WHERE url='http://site.test/de.html'",
            [],
        )
        .unwrap();
    assert!(paths(&fx.file, &cfg, "Hreflang", "Non-Canonical Return Links").is_empty());
}

// spec: TECH-064
#[tokio::test]
async fn refresh_filter_and_meta_column_only_include_meta_elements() {
    let cfg = CrawlConfig::default();
    let fx = crawl_fixture_from("n09-technical", "http://site.test/", cfg.clone()).await;
    let actual = paths(&fx.file, &cfg, "Directives", "Refresh");
    assert!(!actual.contains(&"/header.html".into()));
    assert!(actual.contains(&"/meta.html".into()));
    assert!(!actual.contains(&"/header-bare.html".into()));
    let mut q = ViewQuery::new("Directives", "All");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Meta Refresh".into()]);
    let rows = query_view(&fx.file, &cfg, &q).unwrap().rows;
    let header = rows
        .iter()
        .find(|r| r[0] == Value::Text("http://site.test/header.html".into()))
        .unwrap();
    assert_eq!(header[1], Value::Text(String::new()));
}
