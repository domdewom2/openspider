use os_store::schema::{migrate, MIGRATIONS};
use rusqlite::Connection;

type ExtRow = (i64, String, Option<i64>, Option<f64>, Option<String>, i64);

// spec: TECH-145
#[test]
fn compact_ext_preserves_values_and_change_stamps() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Connection::open(dir.path().join("legacy.ospider")).unwrap();
    migrate(&mut db, 0, &MIGRATIONS[..4]).unwrap();
    db.execute_batch("INSERT INTO url_ext(url_id,key,v_int,v_real,v_text,change_seq) VALUES (1,'linkscore.score',42,NULL,NULL,3),(1,'linkscore.raw',NULL,1.5,NULL,4),(2,'content.text',NULL,NULL,'example',5);").unwrap();
    db.execute(
        "UPDATE url_ext SET v_text=?1 WHERE key='content.text'",
        ["ü".repeat(800)],
    )
    .unwrap();
    let before: Vec<ExtRow> = db
        .prepare("SELECT * FROM url_ext ORDER BY url_id,key")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    migrate(&mut db, 4, MIGRATIONS).unwrap();
    let after: Vec<ExtRow> = db
        .prepare("SELECT * FROM url_ext ORDER BY url_id,key")
        .unwrap()
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(before, after);
}

mod store_common;

// spec: TECH-145
#[test]
fn recrawl_upserts_present_facts_and_removes_only_stale_fetch_facts() {
    use os_model::{ExtKey, ExtWrite, UrlId, Value, WriteBatch};
    use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
    use store_common::{batch_with_urls, new_crawl, page, sink};
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "ext.ospider", StorageProfile::Balanced);
    let mut writer = sink(&path, StorageProfile::Balanced);
    writer.write(batch_with_urls([(1, true)])).unwrap();
    let fact = |key: &str, value| ExtWrite {
        url: UrlId(1),
        key: ExtKey::parse(key).unwrap(),
        value: Value::Int(value),
    };
    writer
        .write(WriteBatch {
            ext: vec![
                fact("head.title_outside", 1),
                fact("content.old", 2),
                fact("linkscore.score", 3),
            ],
            ..Default::default()
        })
        .unwrap();
    let mut replacement = page(1, vec![]);
    replacement.replace = true;
    writer
        .write(WriteBatch {
            pages: vec![replacement],
            ext: vec![fact("head.title_outside", 4)],
            ..Default::default()
        })
        .unwrap();
    let db = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let rows: Vec<(String, i64)> = db
        .conn()
        .prepare("SELECT key,v_int FROM url_ext ORDER BY key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        rows,
        [
            ("head.title_outside".into(), 4),
            ("linkscore.score".into(), 3)
        ]
    );
}

// spec: TECH-145
#[test]
fn long_text_roundtrips_across_writer_and_compatibility_view() {
    use os_model::{ExtKey, ExtWrite, UrlId, Value, WriteBatch};
    use os_store::{CrawlFile, CrawlSink, OpenMode, StorageProfile};
    use store_common::*;
    let dir = tempfile::tempdir().unwrap();
    let path = new_crawl(dir.path(), "text.ospider", StorageProfile::Balanced);
    let mut writer = sink(&path, StorageProfile::Balanced);
    writer.write(batch_with_urls([(1, true)])).unwrap();
    for text in ["ü".repeat(800), "short".into(), "x".repeat(1100)] {
        writer
            .write(WriteBatch {
                ext: vec![ExtWrite {
                    url: UrlId(1),
                    key: ExtKey::from_static("content.text"),
                    value: Value::Text(text.clone()),
                }],
                ..Default::default()
            })
            .unwrap();
        let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
        let actual: String = file
            .conn()
            .query_row(
                "SELECT v_text FROM url_ext WHERE url_id=1 AND key='content.text'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, text);
        assert_eq!(
            count(&file, "SELECT count(*) FROM ext_texts"),
            i64::from(text.len() > 768)
        );
    }
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    file.conn()
        .execute(
            "UPDATE url_ext SET v_text=?1 WHERE key='content.text'",
            ["y".repeat(1400)],
        )
        .unwrap();
    let actual: String = file
        .conn()
        .query_row(
            "SELECT v_text FROM url_ext WHERE key='content.text'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actual, "y".repeat(1400));
    file.conn().execute("INSERT OR REPLACE INTO url_ext(url_id,key,v_text) VALUES(1,'content.text','replacement')", []).unwrap();
    let actual: String = file
        .conn()
        .query_row(
            "SELECT v_text FROM url_ext WHERE key='content.text'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(actual, "replacement");
    assert_eq!(count(&file, "SELECT count(*) FROM ext_texts"), 0);
    file.conn()
        .execute("DELETE FROM url_ext WHERE key='content.text'", [])
        .unwrap();
    assert_eq!(count(&file, "SELECT count(*) FROM ext_texts"), 0);
}
