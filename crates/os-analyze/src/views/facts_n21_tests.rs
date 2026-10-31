use super::*;
use os_model::{CrawlMode, Timestamp};
use os_store::{CrawlInit, OpenMode, StorageProfile};

fn fixture() -> (tempfile::TempDir, CrawlConfig, CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let cfg = CrawlConfig::default();
    let file = CrawlFile::create(
        &dir.path().join("n21-facts.ospider"),
        &CrawlInit {
            crawl_id: "00000000-0000-0000-0000-000000000211".parse().unwrap(),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "n21".into(),
            name: "n21".into(),
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
    file.set_meta("state", "Completed").unwrap();
    (dir, cfg, file)
}

// spec: UI-145
// spec: UI-147
#[test]
fn overview_facts_are_persisted_per_data_version_and_reused_after_reopen() {
    let (_dir, cfg, file) = fixture();
    let (facts, pending) = overview_facts(&file, &cfg, &os_model::CancelToken::new()).unwrap();
    assert!(pending, "a computed result awaits the writer");
    assert_eq!(
        facts.summary,
        os_store::queries::app_support::overview_summary(file.conn()).unwrap()
    );
    assert_eq!(
        facts.counts,
        super::super::filter_counts(&file, &cfg).unwrap()
    );
    assert!(facts.counts.iter().any(|c| c.count.unwrap_or(0) > 0));
    // Same data version: memory hit, still pending.
    assert_eq!(
        overview_facts(&file, &cfg, &os_model::CancelToken::new())
            .unwrap()
            .0,
        facts
    );

    assert!(persist(&file).unwrap());
    assert!(!persist(&file).unwrap(), "persisted once");
    forget(&file);
    let path = file.path().to_path_buf();
    drop(file);

    // Reopened read-only: answered from view_cache without recomputation.
    let reopened = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    let (stored, pending) = overview_facts(&reopened, &cfg, &os_model::CancelToken::new()).unwrap();
    assert!(!pending);
    assert_eq!(*stored, *facts);
    assert!(
        !persist(&reopened).unwrap(),
        "read-only files store nothing"
    );
    drop(reopened);

    // A new data version must not reuse the stored counts.
    let file = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    forget(&file);
    file.conn()
        .execute_batch("UPDATE urls SET status_code=404 WHERE id=1")
        .unwrap();
    let seq = os_store::queries::change_seq(file.conn()).unwrap() + 1;
    file.set_meta("change_seq", &seq.to_string()).unwrap();
    let (fresh, pending) = overview_facts(&file, &cfg, &os_model::CancelToken::new()).unwrap();
    assert!(pending);
    assert_eq!(
        fresh.counts,
        super::super::filter_counts(&file, &cfg).unwrap()
    );
    assert_ne!(fresh.counts, facts.counts);

    // An analysis/reset stamp change at the same sequence invalidates as well.
    assert!(persist(&file).unwrap());
    forget(&file);
    file.set_meta("reset_seq", &seq.to_string()).unwrap();
    assert!(
        overview_facts(&file, &cfg, &os_model::CancelToken::new())
            .unwrap()
            .1
    );
}

// spec: UI-145
#[test]
fn running_crawls_keep_overview_facts_in_memory_only() {
    let (_dir, cfg, file) = fixture();
    file.set_meta("state", "Running").unwrap();
    let (_, pending) = overview_facts(&file, &cfg, &os_model::CancelToken::new()).unwrap();
    assert!(pending);
    assert!(!persist(&file).unwrap());
    let stored: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM view_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, 0);
}

// spec: UI-145
#[test]
fn split_counting_equals_the_serial_counts() {
    let (_dir, cfg, file) = fixture();
    let before = crate::views::PARALLEL_PASSES.with(|n| n.get());
    crate::views::FORCE_PARALLEL.with(|f| f.set(true));
    let result = overview_facts(&file, &cfg, &os_model::CancelToken::new());
    crate::views::FORCE_PARALLEL.with(|f| f.set(false));
    let (facts, _) = result.unwrap();
    assert!(crate::views::PARALLEL_PASSES.with(|n| n.get()) > before);
    assert_eq!(
        facts.counts,
        super::super::filter_counts(&file, &cfg).unwrap()
    );
}

// spec: UI-145
// spec: UI-147
#[test]
fn interrupted_counting_resumes_from_stored_tabs() {
    let (_dir, cfg, file) = fixture();
    let cancel = os_model::CancelToken::new();
    let tabs = crate::tabs::configured_for_file(&cfg, &file).len();
    assert!(!overview_progress(&file, &cfg, &cancel, 1).unwrap());
    assert!(persist(&file).unwrap());
    // Closing the crawl loses memory, not the stored pieces.
    forget(&file);
    let stored = |file: &CrawlFile| -> i64 {
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM view_cache WHERE key LIKE 'n21:counts:%'",
                [],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert_eq!(stored(&file), 1);
    assert!(!overview_progress(&file, &cfg, &cancel, 1).unwrap());
    assert!(persist(&file).unwrap());
    assert_eq!(stored(&file), 2);
    let (facts, pending) = overview_facts(&file, &cfg, &cancel).unwrap();
    assert!(pending);
    assert!(persist(&file).unwrap());
    assert_eq!(stored(&file) as usize, tabs);
    assert_eq!(
        facts.counts,
        super::super::filter_counts(&file, &cfg).unwrap()
    );
}
