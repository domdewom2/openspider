//! Vergleichsmodus im Kern (M2-C01): Auswahl und Rollen (OUT-140), Tausch (OUT-141), Klassifikation je Filter
//! (OUT-142, CRAWL-032), Issues im Vergleich (OUT-143), Voraussetzung „Inhalt“ (CRAWL-033) und Pause durch den
//! Ressourcenwächter (CRAWL-129). Fixture: `out-m2-compare/v1` (Previous) und `out-m2-compare/v2` (Current).

#[path = "compare_common.rs"]
mod common;

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use common::*;
use os_analyze::compare::{
    sort_by_change, CompareClass, CompareError, CompareNote, Comparison, CrawlSide, Side,
};
use os_analyze::views::session::{Budget, RowsReq, SessionSpec, ViewSession};
use os_analyze::views::{self, ColumnSelection, PageStart, ViewQuery};
use os_model::{CancelToken, CrawlMode, UrlId};

fn assert_invariant(rows: &[os_analyze::compare::FilterComparison]) {
    for r in rows {
        let (Some(c), Some(p), Some(k)) = (r.current, r.previous, r.classes) else {
            continue;
        };
        assert_eq!(
            c as i64 - p as i64,
            k[0] as i64 + k[1] as i64 - k[2] as i64 - k[3] as i64,
            "Invariante verletzt für {}:{}",
            r.tab,
            r.filter
        );
    }
}

// spec: OUT-140
#[tokio::test(flavor = "multi_thread")]
async fn the_later_crawl_is_current_whatever_the_selection_order() {
    let (old, new) = (v1().await, v2().await);
    for cmp in [compare(&old, &new, &[]), compare(&new, &old, &[])] {
        assert_eq!(cmp.current().source, new.file.path());
        assert_eq!(cmp.previous().source, old.file.path());
        assert!(cmp.current().meta.started_at > cmp.previous().meta.started_at);
    }
}

// spec: OUT-140
// Unterschiedliche Leistungsprofile (CRAWL-124): v1 mit „Minimaler Speicher“, v2 mit „Maximale Geschwindigkeit“.
#[tokio::test(flavor = "multi_thread")]
async fn comparison_is_available_for_every_storage_profile() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let html = cmp.filter("Internal", "HTML").unwrap();
    assert!(html.classes.is_some());
}

// spec: OUT-140
#[tokio::test(flavor = "multi_thread")]
async fn spider_against_list_is_allowed_with_a_note() {
    let (old, new) = (v1().await, v2().await);
    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("list.ospider");
    std::fs::copy(old.file.path(), &list).unwrap();
    {
        let f = os_store::CrawlFile::open(&list, os_store::OpenMode::ReadWrite).unwrap();
        f.set_meta("mode", "list").unwrap();
    }
    let cmp = Comparison::open(
        CrawlSide::open(&list, dir.path()).unwrap(),
        side(&new),
        &[],
        &CancelToken::new(),
    )
    .unwrap();
    assert!(cmp.notes().contains(&CompareNote::DifferentModes {
        current: CrawlMode::Spider,
        previous: CrawlMode::List,
    }));
}

// spec: OUT-140
#[test]
fn older_files_are_migrated_as_copies_without_touching_the_original() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/crawl-files");
    let dir = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let mut originals = Vec::new();
    for name in ["schema-v1.ospider", "schema-v2.ospider"] {
        let path = dir.path().join(name);
        std::fs::copy(fixtures.join(name), &path).unwrap();
        originals.push((path.clone(), std::fs::read(&path).unwrap()));
    }
    let cmp = Comparison::open(
        CrawlSide::open(&originals[0].0, work.path()).unwrap(),
        CrawlSide::open(&originals[1].0, work.path()).unwrap(),
        &[],
        &CancelToken::new(),
    )
    .unwrap();
    for (path, bytes) in &originals {
        assert_eq!(&std::fs::read(path).unwrap(), bytes, "{}", path.display());
    }
    let migrated: Vec<_> = cmp
        .notes()
        .into_iter()
        .filter(|n| matches!(n, CompareNote::Migrated { .. }))
        .collect();
    assert_eq!(migrated.len(), 2);
    for side in [Side::Current, Side::Previous] {
        let copy = cmp.side(side).migrated.clone().expect("migrierte Kopie");
        assert!(copy.starts_with(work.path()));
        assert_eq!(cmp.side(side).meta.schema_version, os_store::SCHEMA_VERSION);
    }
}

// spec: OUT-142
// spec: CRAWL-032
#[tokio::test(flavor = "multi_thread")]
async fn invariant_holds_for_every_filter_of_every_tab() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let rows = cmp.overview(&CancelToken::new()).unwrap();
    assert!(rows.len() > 100, "alle Filter aller Tabs: {}", rows.len());
    assert!(rows.iter().filter(|r| r.classes.is_some()).count() > 50);
    assert_invariant(&rows);
}

// spec: OUT-142
// spec: CRAWL-032
#[tokio::test(flavor = "multi_thread")]
async fn fixture_changes_land_in_the_expected_classes() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let cur = &new.file;
    let prev = &old.file;

    // Title geändert, vorher „Duplicate“ → Removed (beide Partner verlieren den Duplikat-Status).
    let removed = cmp
        .class_ids("Page Titles", "Duplicate", CompareClass::Removed)
        .unwrap();
    assert_eq!(
        addresses(cur, &removed),
        [www("/dup1.html"), www("/dup2.html")]
    );

    // Behobene 404: Removed in Client Error (4XX), Added in Success (2XX).
    let fixed = vec![www("/missing.html")];
    let ids = cmp
        .class_ids(
            "Response Codes",
            "Client Error (4XX)",
            CompareClass::Removed,
        )
        .unwrap();
    assert_eq!(addresses(cur, &ids), fixed);
    let ids = cmp
        .class_ids("Response Codes", "Success (2XX)", CompareClass::Added)
        .unwrap();
    assert_eq!(addresses(cur, &ids), fixed);

    // Gelöschte Seiten: Missing (Previous-Kennungen); neue Seiten: New.
    let missing = cmp
        .class_ids("Internal", "HTML", CompareClass::Missing)
        .unwrap();
    assert_eq!(
        addresses(prev, &missing),
        [www("/gone1.html"), www("/gone2.html")]
    );
    let new_pages = cmp
        .class_ids("Internal", "HTML", CompareClass::New)
        .unwrap();
    assert_eq!(
        addresses(cur, &new_pages),
        [
            www("/new-noindex.html"),
            www("/new1.html"),
            www("/new2.html"),
            www("/new3.html")
        ]
    );
    // Die 404 war kein HTML; nach der Behebung kommt sie zu Internal:HTML hinzu.
    let added = cmp
        .class_ids("Internal", "HTML", CompareClass::Added)
        .unwrap();
    assert_eq!(addresses(cur, &added), fixed);
    let html = cmp.filter("Internal", "HTML").unwrap();
    assert_eq!(html.class(CompareClass::New), Some(4));
    assert_eq!(html.class(CompareClass::Missing), Some(2));
    assert_eq!(html.class(CompareClass::Removed), Some(0));
    assert_eq!(html.change(), Some(3));
}

// spec: OUT-142
#[tokio::test(flavor = "multi_thread")]
async fn existing_means_crawled_not_only_discovered() {
    let (old, new) = (v1().await, v2().await);
    // Im Previous-Crawl wird /deep.html nur entdeckt, nicht gecrawlt (künstlich zurückgesetzt).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1-discovered.ospider");
    std::fs::copy(old.file.path(), &path).unwrap();
    {
        let f = os_store::CrawlFile::open(&path, os_store::OpenMode::ReadWrite).unwrap();
        f.conn()
            .execute(
                "UPDATE urls SET state = 'queued' WHERE address = ?1",
                [www("/deep.html")],
            )
            .unwrap();
    }
    let cmp = Comparison::open(
        side(&new),
        CrawlSide::open(&path, dir.path()).unwrap(),
        &[],
        &CancelToken::new(),
    )
    .unwrap();
    let deep_cur = id_of(&new.file, &www("/deep.html"));
    assert!(cmp.exists(Side::Current, deep_cur));
    assert_eq!(cmp.partner(Side::Current, deep_cur), None);
    let a_cur = id_of(&new.file, &www("/a.html"));
    assert!(cmp.partner(Side::Current, a_cur).is_some());
}

// spec: OUT-142
// spec: CRAWL-032
// Klick auf eine Zahl: die Haupttabelle listet genau diese URLs (Zeilenzahl = angezeigte Zahl).
#[tokio::test(flavor = "multi_thread")]
async fn clicking_a_number_lists_exactly_that_many_rows() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    for (tab, filter) in [
        ("Internal", "HTML"),
        ("Page Titles", "Duplicate"),
        ("Response Codes", "Success (2XX)"),
        ("Response Codes", "Client Error (4XX)"),
    ] {
        let row = cmp.filter(tab, filter).unwrap();
        for class in CompareClass::ALL {
            let shown = row.class(class).unwrap();
            let total = match class {
                CompareClass::Added | CompareClass::New => {
                    let source = cmp.source(tab, filter, class).unwrap();
                    let side = cmp.current();
                    let session = ViewSession::open(
                        &side.file,
                        &side.config,
                        SessionSpec {
                            tab: tab.into(),
                            filter: filter.into(),
                            search: None,
                            sort: Vec::new(),
                            columns: ColumnSelection::Default,
                            restrict: vec![source],
                        },
                        Budget::default(),
                    )
                    .unwrap();
                    session
                        .rows(
                            &side.file,
                            RowsReq {
                                version: session.version(),
                                offset: 0,
                                limit: 500,
                            },
                        )
                        .unwrap()
                        .total
                }
                CompareClass::Removed | CompareClass::Missing => {
                    // Removed liegt außerhalb des Filters, Missing im Previous-Crawl: Zeilen über die Kennungen
                    // mit den Spalten des Tabs (Filter „All“, auch außerhalb der Grundmenge).
                    let ids = cmp.class_ids(tab, filter, class).unwrap();
                    let side = cmp.side(class.side());
                    let mut q = ViewQuery::new(tab, "All");
                    q.start = PageStart::Offset(0);
                    q.limit = 500;
                    let ids: Vec<UrlId> = ids.iter().map(|i| UrlId(i64::from(i))).collect();
                    views::query_url_ids_in(
                        &os_analyze::tabs::configured_for_file(&side.config, &side.file),
                        &side.file,
                        &side.config,
                        &q,
                        &ids,
                        true,
                    )
                    .unwrap()
                    .total
                }
            };
            assert_eq!(total, shown, "{tab}:{filter} {}", class.as_str());
        }
    }
}

// spec: OUT-141
#[tokio::test(flavor = "multi_thread")]
async fn swapping_turns_added_into_removed_and_new_into_missing() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let before = cmp.overview(&CancelToken::new()).unwrap();
    let version = cmp.version();
    cmp.swap();
    assert_eq!(cmp.current().source, old.file.path());
    assert!(cmp.version() > version);
    let after = cmp.overview(&CancelToken::new()).unwrap();
    assert_invariant(&after);
    let mut checked = 0;
    for b in &before {
        let Some(a) = after
            .iter()
            .find(|a| a.tab == b.tab && a.filter == b.filter)
        else {
            continue;
        };
        let (Some(kb), Some(ka)) = (b.classes, a.classes) else {
            continue;
        };
        assert_eq!(ka[0], kb[2], "{}:{} Added = altes Removed", b.tab, b.filter);
        assert_eq!(ka[2], kb[0], "{}:{} Removed = altes Added", b.tab, b.filter);
        assert_eq!(ka[1], kb[3], "{}:{} New = altes Missing", b.tab, b.filter);
        assert_eq!(ka[3], kb[1], "{}:{} Missing = altes New", b.tab, b.filter);
        assert_eq!(a.current, b.previous);
        checked += 1;
    }
    assert!(checked > 50);
    // Zurücktauschen stellt die Ausgangslage her.
    cmp.swap();
    assert_eq!(cmp.overview(&CancelToken::new()).unwrap(), before);
}

// spec: OUT-143
#[tokio::test(flavor = "multi_thread")]
async fn issues_carry_the_comparison_columns_of_their_filter() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let issues = cmp.issues(&CancelToken::new()).unwrap();
    assert!(!issues.is_empty());
    for i in &issues {
        let tab_row = cmp.filter(&i.filter.tab, &i.filter.filter).unwrap();
        assert_eq!(i.filter, tab_row, "{}", i.id);
        assert_invariant(std::slice::from_ref(&i.filter));
    }
    // Nur im Previous-Crawl vorhandenes Issue erscheint mit 0 im Current-Crawl.
    let dup = issues
        .iter()
        .find(|i| i.filter.tab == "Page Titles" && i.filter.filter == "Duplicate")
        .expect("Duplicate-Issue");
    assert_eq!(dup.filter.current, Some(0));
    assert_eq!(dup.filter.previous, Some(2));
    // Klick auf „Removed“ des Issues liefert dieselben URLs wie der Tab-Vergleich.
    let ids = cmp
        .class_ids(&dup.filter.tab, &dup.filter.filter, CompareClass::Removed)
        .unwrap();
    assert_eq!(ids.len(), dup.filter.class(CompareClass::Removed).unwrap());
}

// spec: OUT-143
#[tokio::test(flavor = "multi_thread")]
async fn a_custom_filter_without_previous_definition_has_zero_previous_hits() {
    let mut config = cfg();
    config.custom_search.filters.push(os_model::SearchFilter {
        name: "Current only".into(),
        expression: "<h1>".into(),
        ..Default::default()
    });
    let (old, new) = (v1().await, v2_with(config).await);
    let mut cmp = compare(&old, &new, &[]);
    let row = cmp.filter("Custom Search", "Current only").unwrap();
    let current = row.current.expect("bereiter Custom-Filter");
    assert!(current > 0);
    assert_eq!(row.previous, Some(0));
    assert_eq!(row.change(), Some(current as i64));
    assert_eq!(row.previous_state, None);
    assert_eq!(row.classes, None);
    assert_eq!(cmp.classes("Custom Search", "Current only").unwrap(), None);
    let overview = cmp.overview(&CancelToken::new()).unwrap();
    assert!(overview.contains(&row));
}

// spec: OUT-143
#[tokio::test(flavor = "multi_thread")]
async fn issues_are_sortable_by_change() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let mut issues = cmp.issues(&CancelToken::new()).unwrap();
    sort_by_change(&mut issues, true);
    let changes: Vec<i64> = issues.iter().filter_map(|i| i.filter.change()).collect();
    assert!(changes.windows(2).all(|w| w[0] >= w[1]), "{changes:?}");
    sort_by_change(&mut issues, false);
    let changes: Vec<i64> = issues.iter().filter_map(|i| i.filter.change()).collect();
    assert!(changes.windows(2).all(|w| w[0] <= w[1]), "{changes:?}");
}

// spec: CRAWL-033
#[tokio::test(flavor = "multi_thread")]
async fn content_needs_stored_html_or_signatures_in_both_crawls() {
    let mut with_html = cfg();
    with_html.extraction.store_html = true;
    let (old, new) = (v1_with(with_html.clone()).await, v2_with(with_html).await);
    let cmp = compare(&old, &new, &[]);
    assert!(cmp.content_available(Side::Current).unwrap());
    assert!(cmp.content_comparable().unwrap());

    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/crawl-files");
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("schema-v1.ospider");
    std::fs::copy(fixtures.join("schema-v1.ospider"), &legacy).unwrap();
    let cmp = Comparison::open(
        side(&new),
        CrawlSide::open(&legacy, dir.path()).unwrap(),
        &[],
        &CancelToken::new(),
    )
    .unwrap();
    assert!(!cmp.content_comparable().unwrap());
}

// spec: CRAWL-129
// Die Vergleichsberechnung pausiert am Checkpoint, statt abzubrechen, und läuft nach „Fortsetzen“ weiter (K2-56).
#[tokio::test(flavor = "multi_thread")]
async fn a_paused_comparison_waits_and_resumes() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let expected = cmp.overview(&CancelToken::new()).unwrap();
    let (a, b) = (side(&old), side(&new));
    let token = CancelToken::new();
    token.pause();
    let worker_token = token.clone();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result =
            Comparison::open(a, b, &[], &worker_token).and_then(|mut c| c.overview(&worker_token));
        tx.send(result).unwrap();
    });
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "pausierter Vergleich darf nicht fertig werden"
    );
    token.resume();
    let rows = rx.recv_timeout(Duration::from_secs(60)).unwrap().unwrap();
    assert_eq!(rows, expected);
}

// spec: CRAWL-129
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_comparison_stops_with_cancelled() {
    let (old, new) = (v1().await, v2().await);
    let mut cmp = compare(&old, &new, &[]);
    let token = CancelToken::new();
    token.cancel();
    assert!(matches!(cmp.overview(&token), Err(CompareError::Cancelled)));
    assert!(matches!(
        Comparison::open(side(&old), side(&new), &[], &token),
        Err(CompareError::Cancelled)
    ));
}
