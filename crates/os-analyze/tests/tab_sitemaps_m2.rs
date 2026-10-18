//! P1-Sitemaps gegen gespeicherte F05-Daten, unabhängig vom Streaming-Parser (M2-E02).
use os_analyze::analysis::{self, CancelToken};
use os_analyze::views::{filter_counts, query_view, state_of, ColumnSelection, ViewQuery};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, FilterState, Timestamp, UrlId, Value};
use os_store::{queries, CrawlFile, CrawlInit, StorageProfile};

fn fixture() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let at = Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap();
    let file = CrawlFile::create(
        &dir.path().join("test.ospider"),
        &CrawlInit {
            crawl_id: SeededIds::new(1).new_crawl_id(at),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "".into(),
            name: "".into(),
            config: CrawlConfig::default(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/tech-m2-sitemaps/store.sql"
        ))
        .unwrap();
    (dir, file)
}

fn addresses(file: &CrawlFile, filter: &str) -> Vec<String> {
    query_view(
        file,
        &CrawlConfig::default(),
        &ViewQuery::new("Sitemaps", filter),
    )
    .unwrap()
    .rows
    .into_iter()
    .map(|r| match &r[0] {
        Value::Text(s) => s.strip_prefix("https://site.test").unwrap_or(s).into(),
        other => panic!("{other:?}"),
    })
    .collect()
}

// spec: TECH-216
#[test]
fn multiple_sitemaps_waits_for_analysis_and_counts_distinct_urlsets() {
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    assert_eq!(
        state_of(&file, &cfg, "Sitemaps", "URLs In Multiple Sitemaps").unwrap(),
        FilterState::NeedsCrawlAnalysis
    );
    assert!(addresses(&file, "URLs In Multiple Sitemaps").is_empty());
    assert!(
        analysis::run(&file, &cfg, &CancelToken::new(), &|_, _| {})
            .unwrap()
            .complete
    );
    assert_eq!(
        addresses(&file, "URLs In Multiple Sitemaps"),
        ["/both", "/orphan"]
    );
    let mut q = ViewQuery::new("Sitemaps", "All");
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Sitemaps".into()]);
    let page = query_view(&file, &cfg, &q).unwrap();
    let row = page
        .rows
        .iter()
        .find(|r| r[0] == Value::Text("https://site.test/dup-same-file".into()))
        .unwrap();
    assert_eq!(row[1], Value::Int(1));
}

// spec: TECH-217
#[test]
fn entry_limit_is_strict_and_live_for_urlsets_and_indexes() {
    let (_dir, file) = fixture();
    assert_eq!(
        addresses(&file, "XML Sitemap With Over 50k URLs"),
        ["/a.xml", "/index.xml"]
    );
    let counts = filter_counts(&file, &CrawlConfig::default()).unwrap();
    let count = counts
        .iter()
        .find(|c| c.tab == "Sitemaps" && c.filter == "XML Sitemap With Over 50k URLs")
        .unwrap();
    assert_eq!(count.state, FilterState::Ready);
    assert_eq!(count.count, Some(2));
    assert!(addresses(&file, "All")
        .iter()
        .all(|s| !s.ends_with("xml") && !s.ends_with("gz")));
    file.conn()
        .execute("DELETE FROM sitemap_entries", [])
        .unwrap();
    file.conn()
        .execute("DELETE FROM sitemaps WHERE kind = 'urlset'", [])
        .unwrap();
    assert_eq!(
        addresses(&file, "XML Sitemap With Over 50k URLs"),
        ["/index.xml"]
    );
    assert!(addresses(&file, "All").is_empty());
}

// spec: TECH-217
#[test]
fn sitemap_files_are_exclusive_to_limit_filters_even_if_previously_discovered_as_pages() {
    let (_dir, file) = fixture();
    file.conn().execute_batch(
        "UPDATE urls SET in_tables = 1, content_class = 'HTML', indexability = 'Indexable' WHERE id = 101;"
    ).unwrap();
    analysis::run(
        &file,
        &CrawlConfig::default(),
        &CancelToken::new(),
        &|_, _| {},
    )
    .unwrap();
    for filter in [
        "All",
        "URLs In Sitemap",
        "URLs Not In Sitemap",
        "Orphan URLs",
        "URLs In Multiple Sitemaps",
    ] {
        assert!(
            !addresses(&file, filter).contains(&"/a.xml".into()),
            "{filter}"
        );
    }
    assert_eq!(
        addresses(&file, "XML Sitemap With Over 50k URLs"),
        ["/a.xml", "/index.xml"]
    );
}

// spec: TECH-218
#[test]
fn size_limit_uses_stored_uncompressed_bytes_and_excludes_exact_boundary() {
    let (_dir, file) = fixture();
    assert_eq!(
        addresses(&file, "XML Sitemap With Over 50MB"),
        ["/huge.xml.gz", "/index.xml"]
    );
    assert_eq!(
        state_of(
            &file,
            &CrawlConfig::default(),
            "Sitemaps",
            "XML Sitemap With Over 50MB"
        )
        .unwrap(),
        FilterState::Ready
    );
    file.conn()
        .execute("UPDATE sitemaps SET size_bytes = NULL", [])
        .unwrap();
    assert!(addresses(&file, "XML Sitemap With Over 50MB").is_empty());
}

// spec: TECH-219
#[test]
fn xml_inlinks_name_the_sitemap_and_leave_link_counters_unchanged() {
    let (_dir, file) = fixture();
    let links = queries::inlinks(file.conn(), UrlId(2), queries::PageReq::new(0, 10)).unwrap();
    let xml: Vec<_> = links
        .items
        .iter()
        .filter(|l| l.link_type == os_model::LinkType::XmlSitemap)
        .collect();
    assert_eq!(xml.len(), 2);
    let from: Vec<_> = xml.iter().map(|l| l.source_address.as_str()).collect();
    assert_eq!(
        from,
        ["https://site.test/a.xml", "https://site.test/b.xml.gz"]
    );
    let counters = os_store::counters::recompute(file.conn(), 2).unwrap();
    assert_eq!(counters.inlinks, 1);
    assert_eq!(counters.unique_inlinks, 1);
    let child = queries::inlinks(file.conn(), UrlId(5), queries::PageReq::new(0, 10)).unwrap();
    assert_eq!(child.items.len(), 1);
    assert_eq!(child.items[0].link_type, os_model::LinkType::Hyperlink);
}

// spec: TECH-216
#[test]
fn paused_step_waits_for_resume_and_cancelled_step_preserves_results() {
    let (_dir, file) = fixture();
    let token = CancelToken::new();
    token.pause();
    let worker = token.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let state =
            analysis::steps::sitemaps::run(&file, &CrawlConfig::default(), &worker).unwrap();
        done_tx.send(()).unwrap();
        (file, state)
    });
    started_rx.recv().unwrap();
    assert!(done_rx
        .recv_timeout(std::time::Duration::from_millis(50))
        .is_err());
    token.resume();
    done_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    let (file, state) = thread.join().unwrap();
    assert_eq!(state, os_model::StepState::Done);
    let before = queries::change_seq(file.conn()).unwrap();
    token.pause();
    token.cancel();
    assert_eq!(
        analysis::steps::sitemaps::run(&file, &CrawlConfig::default(), &token).unwrap(),
        os_model::StepState::Pending
    );
    assert_eq!(queries::change_seq(file.conn()).unwrap(), before);
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM url_flags WHERE flag = 'orphan'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        3
    );
}
