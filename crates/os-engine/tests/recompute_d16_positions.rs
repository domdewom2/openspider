use os_engine::{recompute, EngineError};
use os_model::{
    CancelToken, ContentClass, CrawlConfig, CrawlMode, DiscoveryKind, FetchFacts, IdSource,
    LinkPosition, LinkPositionRule, LinkRow, NewUrl, PageWrite, SeededIds, Timestamp, UrlId,
    UrlState, WriteBatch,
};
use os_parse::{dom::Document, links, ParseConfig};
use os_store::{queries, CrawlFile, CrawlInit, CrawlSink, SqliteSink, StorageProfile};

const HTML: &str = include_str!("../../../tests/fixtures/integ-link-positions/breadcrumb.html");

fn custom_config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.link_positions.rules.insert(
        0,
        LinkPositionRule {
            position: "Breadcrumb".into(),
            contains: vec!["breadcrumb".into()],
        },
    );
    cfg
}

fn fixture() -> (tempfile::TempDir, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("positions.ospider");
    let now = Timestamp::from_db("2026-10-18T00:00:00.000Z").unwrap();
    let cfg = CrawlConfig::default();
    let init = CrawlInit {
        config: cfg.clone(),
        crawl_id: SeededIds::new(42).new_crawl_id(now),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "fixture".into(),
        name: "link positions".into(),
        created_at: now,
    };
    let file = CrawlFile::create(&path, &init, StorageProfile::Balanced).unwrap();
    let urls = ["/", "/target"]
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let url = format!("https://site.test{path}");
            NewUrl {
                id: UrlId(i as i64 + 1),
                address: url.clone(),
                url_length: url.len() as u32,
                url,
                origin: "https://site.test".into(),
                is_internal: true,
                in_tables: true,
                discovered_via: DiscoveryKind::Link,
                crawl_depth: Some(i as u32),
                folder_depth: 0,
                state: UrlState::Done,
                not_crawled_reason: None,
            }
        })
        .collect();
    let doc = Document::parse(HTML);
    let base = url::Url::parse("https://site.test/").unwrap();
    let mut rows: Vec<_> = links::extract(&doc, &base, &ParseConfig::from_crawl_config(&cfg))
        .into_iter()
        .map(|link| LinkRow {
            target_id: (link.href_raw == "/target").then_some(UrlId(2)),
            target_raw: link.href_raw,
            link_type: link.link_type,
            position: link.doc_order,
            anchor: link.anchor,
            follow: true,
            link_path: Some(link.xpath),
            link_position: Some(link.position),
            in_head: Some(link.in_head),
            ..Default::default()
        })
        .collect();
    // Pfadlose Verweise (z. B. aus HTTP-Headern) bleiben unberührt.
    rows.push(LinkRow {
        target_id: Some(UrlId(2)),
        target_raw: "/target".into(),
        position: 1000,
        link_position: None,
        follow: true,
        ..Default::default()
    });
    let mut sink = SqliteSink::open(&path, StorageProfile::Balanced).unwrap();
    sink.write(WriteBatch {
        new_urls: urls,
        pages: vec![PageWrite {
            url_id: UrlId(1),
            fetch: FetchFacts {
                crawl_timestamp: now,
                status_code: 200,
                status: "OK".into(),
                content_class: ContentClass::Html,
                ..Default::default()
            },
            links: rows,
            replace: false,
            page: None,
            indexability: os_model::Indexability::Indexable,
            indexability_status: None,
            images: vec![],
            canonicals: vec![],
            directives: vec![],
            refresh: vec![],
            hreflang: vec![],
            pagination: vec![],
            headers: vec![],
            html: None,
        }],
        ..Default::default()
    })
    .unwrap();
    drop(sink);
    (dir, file)
}

fn outlinks(file: &CrawlFile) -> Vec<queries::LinkView> {
    queries::outlinks(file.conn(), UrlId(1), queries::PageReq::new(0, 50))
        .unwrap()
        .items
}

// spec: INTEG-082
// spec: INTEG-084
#[test]
fn rules_recompute_from_persisted_paths_and_flow_into_both_link_views() {
    let (_dir, mut file) = fixture();
    let old = CrawlConfig::default();
    let new = custom_config();
    let before = outlinks(&file);
    let seq = file.get_meta("change_seq").unwrap();
    let result = recompute::run_triggered(&mut file, &old, &new, &CancelToken::default()).unwrap();
    assert_eq!(result.ran, ["M2-D16"]);
    assert_eq!(result.changed, [UrlId(1)]);
    assert_ne!(file.get_meta("change_seq").unwrap(), seq);
    let after = outlinks(&file);
    assert_eq!(
        after
            .iter()
            .map(|l| l.link_path.clone())
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|l| l.link_path.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        after
            .iter()
            .map(|l| l.link_position.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("Head"),
            Some("Breadcrumb"),
            Some("Breadcrumb"),
            Some("Content"),
            Some("Breadcrumb"),
            None
        ]
    );
    assert_eq!(
        queries::inlinks(file.conn(), UrlId(2), queries::PageReq::default())
            .unwrap()
            .items
            .len(),
        5
    );
    let mut filter = queries::PageReq::new(0, 1);
    filter.position = Some("Breadcrumb".into());
    let filtered = queries::inlinks(file.conn(), UrlId(2), filter.clone()).unwrap();
    assert_eq!(filtered.total, 3);
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(
        filtered.items[0].link_position.as_deref(),
        Some("Breadcrumb")
    );
    filter.position = Some("Content".into());
    let content = queries::inlinks(file.conn(), UrlId(2), filter.clone()).unwrap();
    assert_eq!(content.total, 1);
    assert_eq!(content.items[0].anchor.as_deref(), Some("Content"));
    assert_eq!(
        queries::outlinks(file.conn(), UrlId(1), filter)
            .unwrap()
            .total,
        1
    );
    // Reset entspricht denselben Ergebnissen wie vor der Neuzuordnung.
    recompute::run_triggered(&mut file, &new, &old, &CancelToken::default()).unwrap();
    assert_eq!(outlinks(&file), before);
}

// spec: INTEG-082
#[test]
fn unchanged_classifications_do_not_write_and_cancel_keeps_all_previous_facts() {
    let (_dir, mut file) = fixture();
    let old = CrawlConfig::default();
    let before = outlinks(&file);
    let seq = file.get_meta("change_seq").unwrap();
    let mut no_matches = old.clone();
    no_matches.link_positions.rules.insert(
        0,
        LinkPositionRule {
            position: "Unused".into(),
            contains: vec!["not-in-the-fixture".into()],
        },
    );
    let outcome =
        recompute::run_triggered(&mut file, &old, &no_matches, &CancelToken::default()).unwrap();
    assert_eq!(outcome.ran, ["M2-D16"]);
    assert!(outcome.changed.is_empty());
    assert_eq!(file.get_meta("change_seq").unwrap(), seq);
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(matches!(
        recompute::d16::run(&mut file, &old, &custom_config(), &cancel),
        Err(EngineError::Cancelled)
    ));
    assert_eq!(outlinks(&file), before);
    assert_eq!(file.get_meta("change_seq").unwrap(), seq);
}

// spec: INTEG-082
#[test]
fn reordering_custom_rules_and_default_reset_recompute_without_saved_html() {
    let (_dir, mut file) = fixture();
    let old = CrawlConfig::default();
    let custom = custom_config();
    recompute::d16::run(&mut file, &old, &custom, &CancelToken::default()).unwrap();
    let mut reordered = custom.clone();
    reordered.link_positions.rules.swap(0, 1);
    recompute::d16::run(&mut file, &custom, &reordered, &CancelToken::default()).unwrap();
    let rows = outlinks(&file);
    assert_eq!(rows[4].link_position.as_deref(), Some("Navigation"));
    assert_eq!(rows[1].link_position.as_deref(), Some("Breadcrumb"));
    assert_eq!(
        rows[0].link_position.as_deref(),
        Some(LinkPosition::Head.as_str())
    );
}

// spec: INTEG-080
// spec: INTEG-082
// spec: INTEG-084
#[tokio::test]
async fn crawl_persists_custom_positions_and_paths_in_both_directions() {
    let crawl = os_engine::testing::crawl_fixture_from(
        "integ-link-positions",
        "/breadcrumb.html",
        custom_config(),
    )
    .await;
    let source = queries::url_by_address(crawl.file.conn(), &crawl.site.url("/breadcrumb.html"))
        .unwrap()
        .unwrap();
    let target = queries::url_by_address(crawl.file.conn(), &crawl.site.url("/target"))
        .unwrap()
        .unwrap();
    let mut filter = queries::PageReq::new(0, 50);
    filter.position = Some("Breadcrumb".into());
    let outgoing = queries::outlinks(crawl.file.conn(), source.id, filter.clone()).unwrap();
    let incoming = queries::inlinks(crawl.file.conn(), target.id, filter).unwrap();
    assert_eq!(outgoing.total, 3);
    assert_eq!(incoming, outgoing);
    assert_eq!(
        outgoing.items[0].link_path.as_deref(),
        Some("/html[1]/body[1]/ol[1][@class='breadcrumb']/li[1]/a[1]")
    );
}
