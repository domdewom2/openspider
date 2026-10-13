//! Kollation `OS_TEXT` (M1-P00c, docs/plan/M1.md 3.9): Infrastruktur-Tests ohne Spec-Tag – die Sortierregel
//! selbst testet P14 unter ONPAGE-001.

mod schema_common;

use std::cmp::Ordering;

use os_store::collation::{self, OS_TEXT};
use os_store::{CrawlFile, OpenMode};
use rusqlite::Connection;
use schema_common::*;

const INPUT: [&str; 6] = ["b", "Ä", "a", "A", "z", "ä"];
const EXPECTED: [&str; 6] = ["A", "a", "Ä", "ä", "b", "z"];

fn sorted_via_sql(conn: &Connection) -> Vec<String> {
    conn.execute_batch("CREATE TEMP TABLE words (x TEXT NOT NULL)")
        .unwrap();
    for w in INPUT {
        conn.execute("INSERT INTO words (x) VALUES (?1)", [w])
            .unwrap();
    }
    let sql = format!("SELECT x FROM words ORDER BY x COLLATE {OS_TEXT}, x");
    let out = conn
        .prepare(&sql)
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    conn.execute_batch("DROP TABLE words").unwrap();
    out
}

#[test]
fn sql_order_on_created_connection() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "a.ospider");
    assert_eq!(sorted_via_sql(file.conn()), EXPECTED);
}

#[test]
fn sql_order_on_opened_connection_read_write_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "a.ospider");
    drop(file);
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    assert_eq!(sorted_via_sql(rw.conn()), EXPECTED);
    drop(rw);
    // Nur-Lese-Verbindungen sortieren ebenfalls (die Hilfsabfrage braucht keine Tabelle in der Datei).
    let ro = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let sql = format!(
        "SELECT x FROM (SELECT 'b' AS x UNION ALL SELECT 'Ä' UNION ALL SELECT 'a' UNION ALL SELECT 'A' \
         UNION ALL SELECT 'z' UNION ALL SELECT 'ä') ORDER BY x COLLATE {OS_TEXT}, x"
    );
    let out: Vec<String> = ro
        .conn()
        .prepare(&sql)
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(out, EXPECTED);
}

#[test]
fn rust_compare_gives_same_order() {
    let mut v: Vec<&str> = INPUT.to_vec();
    v.sort_by(|a, b| collation::compare(a, b).then_with(|| a.cmp(b)));
    assert_eq!(v, EXPECTED);
    // Groß/Klein gleichwertig, „ä“ unmittelbar nach „a“ und vor „b“.
    assert_eq!(collation::compare("A", "a"), Ordering::Equal);
    assert_eq!(collation::compare("Ä", "ä"), Ordering::Equal);
    assert_eq!(collation::compare("a", "ä"), Ordering::Less);
    assert_eq!(collation::compare("ä", "b"), Ordering::Less);
    assert_eq!(collation::compare("ä", "az"), Ordering::Less);
    // Sprachunabhängig: ein Funktionswert, kein Locale-Parameter; Wiederholung liefert dasselbe.
    assert_eq!(collation::compare("z", "ä"), Ordering::Greater);
}

#[test]
fn ensure_is_idempotent_and_matches_compare() {
    let conn = Connection::open_in_memory().unwrap();
    collation::ensure(&conn).unwrap();
    collation::ensure(&conn).unwrap();
    assert_eq!(sorted_via_sql(&conn), EXPECTED);
    assert_eq!(OS_TEXT, "OS_TEXT");
    // Ohne ensure kennt SQLite die Kollation nicht.
    let bare = Connection::open_in_memory().unwrap();
    assert!(bare
        .prepare("SELECT 'a' ORDER BY 1 COLLATE OS_TEXT")
        .is_err());
}

#[test]
fn file_is_readable_without_registered_collation_and_integrity_ok() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "a.ospider");
    file.conn()
        .execute(
            "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, url_length, title_1) \
             VALUES (1, 'https://a.test/', 'https://a.test/', 'https://a.test', 1, 'queued', 'start', 0, 15, 'Äpfel')",
            [],
        )
        .unwrap();
    drop(file);

    // Kein Schemaobjekt verwendet OS_TEXT: frische Verbindung ohne Registrierung liest und prüft die Datei.
    let fresh = Connection::open(&path).unwrap();
    let title: String = fresh
        .query_row("SELECT title_1 FROM urls WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "Äpfel");
    let check: String = fresh
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(check, "ok");
    let uses_collation: i64 = fresh
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE sql LIKE '%OS_TEXT%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(uses_collation, 0);
}
