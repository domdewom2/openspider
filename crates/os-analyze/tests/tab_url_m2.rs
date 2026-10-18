use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_model::{CrawlConfig, CrawlMode, FilterState, IdSource, SeededIds, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

fn file(dir: &std::path::Path) -> CrawlFile {
    let now = Timestamp::from_db("2026-10-18T00:00:00.000Z").unwrap();
    CrawlFile::create(
        &dir.join("urls.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(4).new_crawl_id(now),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "fixture".into(),
            name: "URL M2".into(),
            config: CrawlConfig::default(),
            created_at: now,
        },
        StorageProfile::Balanced,
    )
    .unwrap()
}

fn insert(file: &CrawlFile, id: i64, path: &str) {
    let url = format!("https://site.test{path}");
    file.conn().execute("INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,status_code,content_class) VALUES (?1,?2,?2,'https://site.test',1,'done','link',0,0,200,'HTML')", rusqlite::params![id,url]).unwrap();
}
fn results(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<i64> {
    let mut q = ViewQuery::new("URL", filter);
    q.limit = 500;
    q.columns = ColumnSelection::Named(vec!["Address".into()]);
    views::query_view(file, cfg, &q)
        .unwrap()
        .row_keys
        .iter()
        .map(|id| id.0)
        .collect()
}
fn fixture_filter(filter: &str) {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let rows: Vec<(String, Vec<String>)> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/onpage-m2-url/filters.json"
    ))
    .unwrap();
    let mut expected = Vec::new();
    for (index, (path, filters)) in rows.iter().enumerate() {
        let id = index as i64 + 1;
        insert(&file, id, path);
        if filters.iter().any(|f| f == filter) {
            expected.push(id);
        }
    }
    // Population excludes external, unanswered and hidden URLs even if their text matches.
    for id in 101..104 {
        insert(&file, id, &format!("/search/search?q=test&utm_source={id}"));
    }
    file.conn()
        .execute(
            "UPDATE urls SET url=url||id,address=address||id WHERE id>=101",
            [],
        )
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET is_internal=0 WHERE id=101", [])
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET status_code=NULL WHERE id=102", [])
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET in_tables=0 WHERE id=103", [])
        .unwrap();
    let mut actual = results(&file, &CrawlConfig::default(), filter);
    actual.sort();
    assert_eq!(actual, expected, "{filter}");
}

// spec: ONPAGE-075
#[test]
fn repeated_segments_are_exact_case_sensitive_and_path_only() {
    fixture_filter("Repetitive Path");
}
// spec: ONPAGE-077
#[test]
fn search_segments_and_nonempty_parameters_follow_fixture() {
    fixture_filter("Internal Search");
}
// spec: ONPAGE-080
#[test]
fn ga_names_are_case_insensitive_and_query_only() {
    fixture_filter("GA Tracking Parameters");
}

// spec: ONPAGE-077
#[test]
fn search_configuration_changes_without_recrawl() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    insert(&file, 1, "/search?q=abc");
    insert(&file, 2, "/?q=abc");
    insert(&file, 3, "/find?needle=abc");
    let mut cfg = CrawlConfig::default();
    cfg.url.internal_search_params.clear();
    assert_eq!(results(&file, &cfg, "Internal Search"), [1]);
    cfg.url.internal_search_segments = vec!["find".into()];
    cfg.url.internal_search_params = vec!["needle".into()];
    assert_eq!(results(&file, &cfg, "Internal Search"), [3]);
}

// spec: ONPAGE-077
#[test]
fn persisted_searchaction_templates_match_literals_and_placeholders() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    for (id, path) in [
        (1, "/find?needle=shoes&lang=de"),
        (2, "/find?needle=shoes&lang=en"),
        (3, "/findXneedle=shoes&lang=de"),
        (4, "/a[1]?needle=x"),
        (5, "/find?needle=boots&lang=de"),
    ] {
        insert(&file, id, path);
    }
    let template = r#"{"@type":"WebSite","potentialAction":{"@type":"SearchAction","target":{"@type":"EntryPoint","urlTemplate":"https://site.test/find?needle={search_term_string}&lang=de"}}}"#;
    for id in 1..=3 {
        file.conn().execute("INSERT INTO sd_items (url_id,format,type_path,json) VALUES (?1,'json-ld','WebSite',?2)",rusqlite::params![id,template]).unwrap();
    }
    let template2 =
        r#"{"@type":["Thing","SearchAction"],"target":"https://site.test/a[1]?needle={q}"}"#;
    file.conn().execute("INSERT INTO sd_items (url_id,format,type_path,json) VALUES (4,'json-ld','SearchAction',?1)",[template2]).unwrap();
    file.conn().execute("INSERT INTO sd_items (url_id,format,type_path,json) VALUES (5,'json-ld','SearchAction','broken json')",[]).unwrap();
    assert_eq!(
        results(&file, &CrawlConfig::default(), "Internal Search"),
        [1, 4]
    );
}

// spec: ONPAGE-079
#[test]
fn broken_bookmarks_use_known_base_html_targets_and_decoded_fragments() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    insert(&file, 1, "/page");
    let targets = r#"["kontakt","anker-alt","café","a&b","%61"]"#;
    file.conn()
        .execute(
            "INSERT INTO url_ext (url_id,key,v_text) VALUES (1,'urlfilter.bookmark_targets',?1)",
            [targets],
        )
        .unwrap();
    let fragments = [
        "kontakt",
        "preise",
        "Kontakt",
        "top",
        "",
        "!/route",
        ":~:text=abc",
        "anker-alt",
        "caf%C3%A9",
        "a%26b",
        "%2561",
        "%70reise",
    ];
    for (index, fragment) in fragments.iter().enumerate() {
        insert(&file, index as i64 + 2, &format!("/page#{fragment}"));
    }
    insert(&file, 20, "/unknown#x");
    insert(&file, 21, "/error");
    insert(&file, 22, "/error#x");
    file.conn()
        .execute(
            "INSERT INTO url_ext (url_id,key,v_text) VALUES (21,'urlfilter.bookmark_targets','[]')",
            [],
        )
        .unwrap();
    file.conn()
        .execute("UPDATE urls SET status_code=404 WHERE id=21", [])
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.advanced.crawl_fragments = true;
    assert_eq!(results(&file, &cfg, "Broken Bookmark"), [3, 4, 13]);
    file.conn()
        .execute("UPDATE urls SET content_class='PDF' WHERE id=1", [])
        .unwrap();
    assert!(results(&file, &cfg, "Broken Bookmark").is_empty());
    cfg.advanced.crawl_fragments = false;
    assert!(results(&file, &cfg, "Broken Bookmark").is_empty());
    let counts = views::filter_counts(&file, &cfg).unwrap();
    assert_eq!(
        counts
            .iter()
            .find(|c| c.tab == "URL" && c.filter == "Broken Bookmark")
            .unwrap()
            .state,
        FilterState::NeedsConfig("advanced.crawl_fragments".into())
    );
}

// spec: CRAWL-103
// spec: CRAWL-104
// spec: CRAWL-108
#[tokio::test]
async fn discovery_rewrites_deduplicate_and_cdns_follow_html_links() {
    let mut cfg = CrawlConfig::default();
    cfg.rewrite.remove_parameters = vec!["sid".into()];
    cfg.rewrite.regex_replace = vec![
        os_model::RegexReplace {
            pattern: "^http://".into(),
            replacement: "https://".into(),
        },
        os_model::RegexReplace {
            pattern: "/index\\.html".into(),
            replacement: "/".into(),
        },
    ];
    cfg.cdns = vec!["cdn.other.test".into(), "other.test/assets/".into()];
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/onpage-m2-url");
    let fx = os_engine::testing::crawl_dir_with(
        &dir,
        "http://site.test/",
        cfg,
        os_engine::testing::test_deps(),
    )
    .await;
    assert_eq!(fx.site.count("site.test", "/x/"), 1);
    assert_eq!(fx.site.count("site.test", "/x/index.html"), 0);
    assert_eq!(fx.site.count("cdn.other.test", "/seite2.html"), 1);
    let row: (String, i64) = fx
        .file
        .conn()
        .query_row(
            "SELECT url,is_internal FROM urls WHERE url LIKE '%/x/?%'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(row, ("https://site.test/x/?keep=%2F".into(), 1));
    let start: String = fx
        .file
        .conn()
        .query_row("SELECT url FROM urls WHERE id=1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(start, "http://site.test/");
    assert!(fx
        .site
        .requests()
        .iter()
        .any(|r| r.host == "site.test" && r.path_and_query == "/" && !r.tls));
    assert!(fx
        .site
        .requests()
        .iter()
        .any(|r| r.host == "site.test" && r.path_and_query.starts_with("/x/?") && r.tls));
    for (url, internal) in [
        ("https://cdn.other.test/seite.html", 1),
        ("https://cdn.other.test/img.png", 1),
        ("https://other.test/assets/a.css", 1),
        ("https://other.test/blog/", 0),
    ] {
        let actual: i64 = fx
            .file
            .conn()
            .query_row("SELECT is_internal FROM urls WHERE url=?1", [url], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(actual, internal, "{url}");
    }
    let outlinks = os_store::queries::outlinks(
        fx.file.conn(),
        os_model::UrlId(1),
        os_store::queries::PageReq::default(),
    )
    .unwrap();
    assert!(outlinks.items.iter().any(|link| {
        link.original_url == "http://site.test/x/index.html?sid=1&keep=%2F"
            && link.target_address == "https://site.test/x/?keep=%2F"
    }));
}

// spec: CRAWL-104
#[tokio::test]
async fn list_inputs_stay_unchanged_while_discovered_urls_are_rewritten() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/onpage-m2-url");
    let site = os_testsite::TestSite::start_dir(&dir, 0).await.unwrap();
    let mut cfg = CrawlConfig {
        mode: CrawlMode::List,
        ..Default::default()
    };
    cfg.limits.max_depth = Some(1);
    cfg.rewrite.remove_all_parameters = true;
    cfg.rewrite.regex_replace = vec![os_model::RegexReplace {
        pattern: "/index\\.html".into(),
        replacement: "/".into(),
    }];
    let deps = os_engine::testing::test_deps();
    let fetcher = os_engine::testing::site_fetcher(&site, &cfg, &deps);
    let temp = tempfile::tempdir().unwrap();
    let request = os_engine::CrawlRequest {
        config: cfg,
        source: os_engine::CrawlSource::List {
            items: vec!["http://site.test/list/index.html".into()],
            label: "rewrite-list".into(),
        },
        file: temp.path().join("list.ospider"),
        project: "fixture".into(),
        name: "rewrite-list".into(),
    };
    os_engine::start_crawl(fetcher, request, deps)
        .await
        .unwrap()
        .wait()
        .await
        .unwrap();
    assert_eq!(site.count("site.test", "/list/index.html"), 1);
    assert_eq!(site.count("site.test", "/list/"), 0);
    assert_eq!(site.count("site.test", "/x/"), 1);
    assert_eq!(site.count("site.test", "/x/index.html"), 0);
}
