//! M2-N28: Ein vor der UI-061-Festlegung gespeichertes Summary-Stück (gleiche Programmversion, gleicher
//! Datenstand) darf den alten Wert von `Total URLs Encountered` nicht mehr liefern.
use super::*;
use os_model::{CrawlMode, Timestamp};
use os_store::{CrawlInit, OpenMode, StorageProfile};

// spec: UI-061
// spec: UI-147
#[test]
fn a_summary_piece_stored_before_n28_is_computed_again() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let path = dir.path().join("n28-facts.ospider");
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000281".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n28".into(),
            name: "n28".into(),
            config: cfg.clone(),
            created_at: Timestamp::UNIX_EPOCH,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.conn()
        .execute_batch(include_str!(
            "../../../../apps/desktop/src/ipc/__fixtures__/k06/fx-ui-basic/scenario.sql"
        ))
        .unwrap();
    // Die robots.txt des Hosts: ein Hilfsabruf ohne Tabellen-Sichtbarkeit (K-54).
    file.conn()
        .execute_batch(
            "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
             folder_depth, url_length, status_code) VALUES (100, 'https://site.test/robots.txt', \
             'https://site.test/robots.txt', 'https://site.test', 1, 0, 'done', 'robots_txt', 0, 28, 200);
             INSERT INTO robots_files (origin, robots_url, fetch_status, status_code, attempts, body) \
             VALUES ('https://site.test', 'https://site.test/robots.txt', 'ok', 200, 1, 'User-agent: *');",
        )
        .unwrap();
    file.set_meta("state", "Completed").unwrap();
    let current = os_store::queries::app_support::overview_summary(file.conn()).unwrap();
    let encountered = current[0].1;
    // Stück wie vor N28 gespeichert: alte Signatur, alter Wert (alle Zeilen), aktueller Datenstand.
    let v = version(&file).unwrap();
    let mut old = current.clone();
    old[0].1 = encountered + 1;
    old[1].1 += 1;
    let legacy = crate::cursor::query_hash(&["summary-v1", env!("CARGO_PKG_VERSION")]);
    assert!(view_cache::save(
        &file,
        SUMMARY_KEY,
        &legacy,
        &view_cache::Entry {
            step_stamp: v.stamp.clone(),
            built_seq: v.seq,
            data: serde_json::to_vec(&old).unwrap(),
        },
    )
    .unwrap());
    forget(&file);
    drop(file);
    let reopened = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let (facts, _) = overview_facts(&reopened, &cfg, &os_model::CancelToken::new()).unwrap();
    assert_eq!(facts.summary, current);
    assert_eq!(
        facts.summary[0],
        ("Total URLs Encountered".to_string(), encountered)
    );
}
