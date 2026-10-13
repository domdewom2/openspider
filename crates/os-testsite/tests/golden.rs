mod common;

use std::path::PathBuf;

use common::get_at;
use os_testsite::golden::{assert_golden, assert_golden_with, normalize};
use os_testsite::{fixtures_root, RequestRecord, TestSite};

fn render(log: &[RequestRecord]) -> String {
    let mut out = String::new();
    for r in log {
        out.push_str(&format!(
            "#{} conn={} {} {} {} tls={} at {}\n",
            r.seq, r.conn_id, r.method, r.host, r.path_and_query, r.tls, r.ts
        ));
        for (k, v) in &r.headers {
            out.push_str(&format!("    {k}: {v}\n"));
        }
    }
    out
}

fn temp_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("os-testsite-golden-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

// spec: TECH-007
#[tokio::test]
async fn request_log_matches_checked_in_golden() {
    let site = TestSite::start("testsite-hosts").await.unwrap();
    get_at(
        site.addr(),
        "site.test",
        "/who?x=1",
        &[("Referer", &site.url("/start"))],
    )
    .await;
    get_at(
        site.addr2(),
        "other.test",
        "/",
        &[("X-Request-Id", "0192f0c1-7a3b-7c2e-9d4f-0123456789ab")],
    )
    .await;

    let bases = site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    // Ports und Zeitstempel sind je Lauf verschieden, das normalisierte Log nicht.
    let actual = normalize(&render(&site.requests()), &bases);
    assert!(actual.contains("<TS>") && actual.contains("{BASE}/start") && actual.contains("<ID>"));
    assert_golden(
        fixtures_root().join("testsite-golden/request-log.txt"),
        &actual,
    );
}

// spec: TECH-007
#[test]
fn assert_golden_compares_and_updates() {
    let file = temp_file("a/b/out.txt");
    // Ohne Datei: Fehler mit Hinweis auf UPDATE_GOLDEN.
    let missing = std::panic::catch_unwind(|| assert_golden_with(&file, "x\n", false));
    assert!(missing.is_err());

    // Aktualisieren legt auch fehlende Verzeichnisse an.
    assert_golden_with(&file, "line1\nline2\n", true);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "line1\nline2\n");

    // Gleiche Datei besteht, auch bei CRLF in der Datei (Git-Autocrlf).
    assert_golden_with(&file, "line1\nline2\n", false);
    std::fs::write(&file, "line1\r\nline2\r\n").unwrap();
    assert_golden_with(&file, "line1\nline2\n", false);

    // Abweichung schlägt fehl und nennt die Zeile.
    let diff = std::panic::catch_unwind(|| assert_golden_with(&file, "line1\nCHANGED\n", false));
    let msg = *diff.unwrap_err().downcast::<String>().unwrap();
    assert!(msg.contains("Zeile 2") && msg.contains("CHANGED"), "{msg}");
    let _ = std::fs::remove_dir_all(file.parent().unwrap().parent().unwrap());
}

// spec: TECH-007
#[test]
fn normalize_replaces_bases_timestamps_ms_and_ids() {
    let text = "GET http://127.0.0.1:4711/a took 12ms at 2026-10-13T08:00:00.123Z (crawl 0192f0c1-7a3b-7c2e-9d4f-0123456789ab) https://127.0.0.1:4711/b";
    assert_eq!(
        normalize(text, &["http://127.0.0.1:4711", "https://127.0.0.1:4711"]),
        "GET {BASE}/a took <MS> at <TS> (crawl <ID>) {BASE}/b"
    );
}
