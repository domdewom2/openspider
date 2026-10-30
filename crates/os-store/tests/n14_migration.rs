use os_store::schema::{migrate, MIGRATIONS};
use rusqlite::{types::Value, Connection};

// spec: TECH-139
// spec: TECH-140
// spec: TECH-142
// spec: TECH-145
#[test]
fn n14_v5_migration_is_metadata_only_and_preserves_numeric_and_text_facts() {
    let mut db = Connection::open_in_memory().unwrap();
    migrate(&mut db, 0, &MIGRATIONS[..5]).unwrap();
    db.execute_batch(
        "INSERT INTO url_ext(url_id,key,v_int,change_seq) VALUES
        (1,'link.mixed_inlinks',1,7),(2,'link.nofollow_only',NULL,8),
        (3,'link.non_indexable_only',0,9),(4,'linkscore.score',50,10);",
    )
    .unwrap();
    db.execute(
        "INSERT INTO url_ext(url_id,key,v_text) VALUES(1,'content.text',?1)",
        ["ü".repeat(2000)],
    )
    .unwrap();
    let facts = |db: &Connection| {
        db.prepare("SELECT * FROM url_ext ORDER BY url_id,key")
            .unwrap()
            .query_map([], |r| {
                (0..6)
                    .map(|i| r.get::<_, Value>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect::<Vec<_>>()
    };
    let before = facts(&db);
    let indexes: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    migrate(&mut db, 5, MIGRATIONS).unwrap();
    assert_eq!(before, facts(&db));
    assert_eq!(
        indexes,
        db.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap()
    );
    assert_eq!(
        3,
        db.query_row("SELECT COUNT(*) FROM analysis_link_facts", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
    );
    let plan = db
        .prepare("EXPLAIN QUERY PLAN SELECT * FROM analysis_link_facts")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    assert!(plan.iter().any(|p| p == "SCAN e"), "{plan:?}");
    assert!(
        plan.iter()
            .all(|p| !p.contains("url_ext_data_change_seq") && !p.contains("ext_texts")),
        "{plan:?}"
    );
}
