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
