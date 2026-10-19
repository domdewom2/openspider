#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_model::{CrawlConfig, Value};
fn count(file: &os_store::CrawlFile, cfg: &CrawlConfig, filter: &str) -> u64 {
    query_view(file, cfg, &ViewQuery::new("Content", filter))
        .unwrap()
        .total
}
// spec: ONPAGE-147
#[test]
fn phrases_change_without_recrawl_and_only_status_200_matches() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "soft.ospider", 8);
    for (id, status, text) in [
        (1, 200, "seite nicht gefunden"),
        (2, 404, "seite nicht gefunden"),
        (3, 200, "produkt nicht mehr verfügbar"),
    ] {
        insert(
            &file,
            &[row(id, &format!("{id}.html"))
                .text("content_class", "HTML")
                .int("status_code", status)
                .text("indexability", "Indexable")],
        );
        file.conn()
            .execute(
                "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.soft404_text',?2)",
                rusqlite::params![id, text],
            )
            .unwrap();
    }
    let mut cfg = CrawlConfig::default();
    assert_eq!(count(&file, &cfg, "Soft 404 Pages"), 1);
    cfg.content.soft404_phrases = vec!["Produkt nicht mehr verfügbar".into()];
    let mut q = ViewQuery::new("Content", "Soft 404 Pages");
    q.columns = ColumnSelection::All;
    let page = query_view(&file, &cfg, &q).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(
        page.rows[0].last(),
        Some(&Value::Text("produkt nicht mehr verfügbar".into()))
    );
    cfg.content.soft404_phrases.clear();
    assert_eq!(count(&file, &cfg, "Soft 404 Pages"), 0);
}
// spec: ONPAGE-019
#[test]
fn internal_and_content_readability_are_identical_and_off_is_null() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "columns.ospider", 8);
    insert(
        &file,
        &[row(1, "page.html")
            .text("content_class", "HTML")
            .int("status_code", 200)],
    );
    file.conn()
        .execute(
            "INSERT INTO url_ext(url_id,key,v_real) VALUES (1,'readability.flesch',49.9)",
            [],
        )
        .unwrap();
    let mut cfg = CrawlConfig::default();
    for enabled in [true, false] {
        cfg.extraction.readability = enabled;
        for tab in ["Internal", "Content"] {
            let mut q = ViewQuery::new(tab, "All");
            q.columns = ColumnSelection::Named(vec!["Flesch Reading Ease Score".into()]);
            let page = query_view(&file, &cfg, &q).unwrap();
            assert_eq!(
                page.rows[0][0],
                if enabled {
                    Value::Float(49.9)
                } else {
                    Value::Null
                }
            );
        }
    }
}

// spec: CRAWL-057
// spec: ONPAGE-166
#[tokio::test(start_paused = true)]
async fn pdf_fixture_links_are_crawled_and_content_facts_reach_the_writer() {
    use os_engine::testing::{sim_deps, MemorySink, SimFetcher, SimSite};
    let pdf = include_str!("../../../tests/fixtures/onpage-m2-content/text.pdf");
    let site = SimSite::new()
        .page("https://site.test/", "<a href='/doc.pdf'>PDF</a>")
        .asset("https://site.test/doc.pdf", "application/pdf", pdf)
        .page("https://site.test/next", "<main>Target</main>");
    let fetcher = SimFetcher::new(site);
    let sink = MemorySink::new();
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.pdf.store = true;
    let handle = os_engine::start_crawl(
        fetcher.clone(),
        os_engine::CrawlRequest {
            config: cfg,
            source: os_engine::CrawlSource::Spider {
                start_url: "https://site.test/".into(),
            },
            file: dir.path().join("pdf.ospider"),
            project: "fixture".into(),
            name: "PDF".into(),
        },
        sim_deps(&sink),
    )
    .await
    .unwrap();
    assert_eq!(
        handle.wait().await.unwrap().state,
        os_model::CrawlState::Completed
    );
    assert_eq!(fetcher.count("https://site.test/next"), 1);
    let id = sink
        .new_urls()
        .iter()
        .find(|u| u.url == "https://site.test/doc.pdf")
        .unwrap()
        .id;
    let batches = sink.batches();
    let page = batches
        .iter()
        .flat_map(|b| &b.pages)
        .find(|p| p.url_id == id)
        .unwrap();
    assert_eq!(page.page.as_ref().unwrap().word_count, Some(6));
    assert!(page.links.iter().any(|link| link.pdf_page == Some(1)));
    assert!(batches.iter().flat_map(|b| &b.ext).any(|f| f.url == id
        && f.key.as_str() == "readability.flesch"
        && f.value == Value::Float(100.0)));
}
