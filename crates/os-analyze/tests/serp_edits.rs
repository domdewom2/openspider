#[path = "framework_common.rs"]
mod common;
use os_store::{
    queries::serp::{self, SerpEdit},
    CrawlFile, OpenMode,
};
// spec: ONPAGE-194
#[test]
fn edits_survive_reopen_without_changing_crawl_values_and_reset() {
    let d = tempfile::tempdir().unwrap();
    let (path, file) = common::create(d.path(), "serp.ospider", 19);
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/onpage-m2-elements/store.sql"
        ))
        .unwrap();
    let edit = SerpEdit {
        device: "tablet".into(),
        title: Some("Über Häuser 東京".into()),
        description: Some(String::new()),
        edited_at: "2026-10-17T12:00:00Z".into(),
    };
    serp::save(file.conn(), 1, &edit).unwrap();
    drop(file);
    let f = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(serp::load(f.conn(), 1).unwrap(), Some(edit));
    assert_eq!(
        f.conn()
            .query_row("SELECT title_1 FROM urls WHERE id=1", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "Kontakt"
    );
    serp::reset(f.conn(), 1).unwrap();
    assert_eq!(serp::load(f.conn(), 1).unwrap(), None);
}

// spec: ONPAGE-194
#[test]
fn upsert_keeps_url_edits_independent_of_device_and_other_urls() {
    let d = tempfile::tempdir().unwrap();
    let (_, file) = common::create(d.path(), "edits.ospider", 20);
    file.conn()
        .execute_batch(include_str!(
            "../../../tests/fixtures/onpage-m2-elements/store.sql"
        ))
        .unwrap();
    assert_eq!(serp::load(file.conn(), 1).unwrap(), None);
    let mut edit = SerpEdit {
        device: "desktop".into(),
        title: Some("First".into()),
        description: None,
        edited_at: "2026-10-17T12:00:00Z".into(),
    };
    serp::save(file.conn(), 1, &edit).unwrap();
    serp::save(file.conn(), 2, &edit).unwrap();
    edit.device = "mobile".into();
    edit.title = Some(String::new());
    edit.description = Some("Edited description".into());
    edit.edited_at = "2026-10-17T12:01:00Z".into();
    serp::save(file.conn(), 1, &edit).unwrap();
    assert_eq!(serp::load(file.conn(), 1).unwrap(), Some(edit));
    assert_eq!(
        serp::load(file.conn(), 2)
            .unwrap()
            .unwrap()
            .title
            .as_deref(),
        Some("First")
    );
    let view = os_analyze::views::query_view(
        &file,
        &os_model::CrawlConfig::default(),
        &os_analyze::views::ViewQuery::new("Page Titles", "Same as H1"),
    )
    .unwrap();
    assert_eq!(view.rows.len(), 1);
    let title = view.columns.iter().position(|c| c == "Title 1").unwrap();
    assert_eq!(view.rows[0][title], os_model::Value::Text("Kontakt".into()));
    serp::reset(file.conn(), 1).unwrap();
    serp::reset(file.conn(), 1).unwrap();
    assert_eq!(serp::load(file.conn(), 1).unwrap(), None);
    assert!(serp::load(file.conn(), 2).unwrap().is_some());
}
