//! Rahmen der Ansichten (M1-P14): SQL-Aufbau, Sortierregel, Suche, Paging, Mehrfachspalten, Zähler.
//! Daten werden direkt per SQL in eine mit `CrawlFile::create` angelegte Datei geschrieben.

#[path = "framework_common.rs"]
mod common;

use common::*;
use os_analyze::views::{
    self, filter_counts_in, plan_in, query_view_in, stream_view_in, ColumnSelection, PageStart,
    SearchSpec, ViewQuery,
};
use os_analyze::{ColumnKind, ColumnSql, ViewError};
use os_model::{CrawlConfig, FilterState, UrlId, Value};
use os_store::CrawlFile;

/// 10 interne URLs (ids 1–8, 10, 11) und eine externe (id 9).
fn sample(dir: &std::path::Path, seed: u64) -> CrawlFile {
    let (_, file) = create(dir, &format!("sample-{seed}.ospider"), seed);
    let ok = |id, addr: &str, title: Option<&str>, len: i64| {
        let mut r = row(id, addr)
            .int("status_code", 200)
            .int("effective_index", 1)
            .text("indexability", "Indexable")
            .int("title_1_len", len);
        r = match title {
            Some(t) => r.text("title_1", t).int("title_count", 1),
            None => r.null("title_1").int("title_count", 0),
        };
        r
    };
    insert(
        &file,
        &[
            ok(1, "b.html", Some("Zebra"), 5)
                .real("text_ratio", 12.3456)
                .int("response_time_ms", 1234)
                .text("hash_md5", "abc"),
            ok(2, "a.html", Some("apple"), 5),
            ok(3, "c.html", Some("Ärger"), 5),
            ok(4, "d.html", Some("Apple"), 5),
            ok(5, "e.html", None, 0).int("status_code", 404),
            ok(6, "f.html", Some(""), 0),
            ok(7, "len-60.html", Some("Sixty"), 60),
            ok(8, "len-61.html", Some("SixtyOne"), 61),
            row(9, "ext.html").external().int("status_code", 200),
            ok(10, "ni.html", None, 0).text("indexability", "Non-Indexable"),
            ok(11, "B.html", Some("Mango"), 5),
        ],
    );
    file
}

fn with_canonicals(file: &CrawlFile) {
    for (url, pos, raw) in [(1, 0, "c1"), (1, 1, "c2"), (1, 2, "c3"), (2, 0, "x1")] {
        file.conn()
            .execute(
                "INSERT INTO canonicals (url_id, source, position, raw, in_head) VALUES (?1, 'Html', ?2, ?3, 1)",
                rusqlite::params![url, pos, raw],
            )
            .unwrap();
    }
}

fn ids(page: &views::ViewPage) -> Vec<i64> {
    page.row_keys.iter().map(|k| k.0).collect()
}

fn query(file: &CrawlFile, cfg: &CrawlConfig, q: &ViewQuery) -> views::ViewPage {
    query_view_in(&[test_tab()], file, cfg, q).unwrap()
}

fn q(filter: &str) -> ViewQuery {
    let mut q = ViewQuery::new("Test", filter);
    q.limit = 500;
    q
}

// spec: ONPAGE-001
#[test]
fn all_is_the_population_and_filters_are_subsets() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let all = query(&file, &cfg, &q("All"));
    assert_eq!(
        all.total, 10,
        "Population = interne URLs, extern ausgeschlossen"
    );
    assert_eq!(
        ids(&all),
        vec![1, 2, 3, 4, 5, 6, 7, 8, 10, 11],
        "ohne Sortierung: UrlId"
    );
    let tab = test_tab();
    for f in &tab.filters {
        let page = query(&file, &cfg, &q(f.id));
        for id in ids(&page) {
            assert!(
                ids(&all).contains(&id),
                "Filter {} enthält URL {id} außerhalb von All",
                f.id
            );
        }
    }
}

// spec: ONPAGE-001
#[test]
fn sort_text_uses_root_collation_and_address_breaks_ties() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut asc = q("All");
    asc.sort = vec![sort("Title 1", false)];
    // apple/Apple sind gleichwertig → Address aufsteigend (a.html vor d.html); „Ärger“ direkt nach „a…“;
    // leere Werte (NULL und '') am Ende, untereinander nach Address.
    assert_eq!(
        ids(&query(&file, &cfg, &asc)),
        vec![2, 4, 3, 11, 7, 8, 1, 5, 6, 10]
    );
    let mut desc = q("All");
    desc.sort = vec![sort("Title 1", true)];
    assert_eq!(
        ids(&query(&file, &cfg, &desc)),
        vec![1, 8, 7, 11, 3, 2, 4, 5, 6, 10],
        "absteigend: leere Werte bleiben am Ende, Gleichstand weiter nach Address aufsteigend"
    );
}

// spec: ONPAGE-001
#[test]
fn sort_address_is_bytewise_and_numbers_sort_numerically() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut by_address = q("All");
    by_address.sort = vec![sort("Address", false)];
    let page = query(&file, &cfg, &by_address);
    // Bytevergleich: „B.html“ (0x42) vor „a.html“ (0x61), obwohl die Spalte als Text deklariert ist.
    assert_eq!(addresses(&page)[..3], ["B.html", "a.html", "b.html"]);

    let mut by_len = q("All");
    by_len.sort = vec![sort("Title 1 Length", true)];
    // 61, 60, dann 5er (Address aufsteigend), dann die 0er.
    assert_eq!(
        ids(&query(&file, &cfg, &by_len)),
        vec![8, 7, 11, 2, 1, 3, 4, 5, 6, 10]
    );
}

// spec: ONPAGE-001
#[test]
fn default_sort_of_the_tab_applies_and_unknown_column_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut tab = test_tab();
    tab.default_sort = vec![sort("Address", true)];
    let page = query_view_in(&[tab], &file, &cfg, &q("All")).unwrap();
    assert_eq!(addresses(&page)[0], "ni.html");

    let mut bad = q("All");
    bad.sort = vec![sort("Titel 1", false)];
    match query_view_in(&[test_tab()], &file, &cfg, &bad) {
        Err(ViewError::Name(os_analyze::NameError::Unknown { suggestions, .. })) => {
            assert!(
                suggestions.contains(&"Title 1".to_string()),
                "{suggestions:?}"
            );
        }
        other => panic!("unbekannte Sortierspalte muss abgelehnt werden: {other:?}"),
    }
}

// spec: ONPAGE-001
#[test]
fn simple_search_is_case_insensitive_substring_on_address_within_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut s = q("All");
    s.search = Some(SearchSpec::Simple("len-6".into()));
    let page = query(&file, &cfg, &s);
    assert_eq!(addresses(&page), vec!["len-60.html", "len-61.html"]);
    assert_eq!(page.total, 2);

    let mut upper = q("All");
    upper.search = Some(SearchSpec::Simple("LEN-6".into()));
    assert_eq!(
        query(&file, &cfg, &upper).total,
        2,
        "Groß-/Kleinschreibung egal"
    );

    // Innerhalb des aktiven Filters: „Errors“ enthält nur e.html.
    let mut within = q("Errors");
    within.search = Some(SearchSpec::Simple("len-6".into()));
    assert_eq!(query(&file, &cfg, &within).total, 0);
    let mut within = q("Errors");
    within.search = Some(SearchSpec::Simple(".html".into()));
    assert_eq!(addresses(&query(&file, &cfg, &within)), vec!["e.html"]);
}

// spec: ONPAGE-001
#[test]
fn offset_and_cursor_paging_reproduce_the_full_order() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    for (column, desc) in [
        ("Title 1", false),
        ("Title 1", true),
        ("Title 1 Length", false),
        ("Address", true),
    ] {
        let mut full = q("All");
        full.sort = vec![sort(column, desc)];
        let expected = ids(&query(&file, &cfg, &full));

        // Offset-Paging
        let mut got = Vec::new();
        let mut offset = 0;
        loop {
            let mut page_q = full.clone();
            page_q.limit = 3;
            page_q.start = PageStart::Offset(offset);
            let page = query(&file, &cfg, &page_q);
            assert_eq!(page.total, 10);
            got.extend(ids(&page));
            offset += 3;
            if page.next_cursor.is_none() {
                break;
            }
        }
        assert_eq!(got, expected, "Offset-Paging {column} desc={desc}");

        // Cursor-Paging
        let mut got = Vec::new();
        let mut start = PageStart::Offset(0);
        let mut pages = 0;
        loop {
            let mut page_q = full.clone();
            page_q.limit = 3;
            page_q.start = start.clone();
            let page = query(&file, &cfg, &page_q);
            assert_eq!(page.total, 10, "total bleibt die Gesamtzahl der Ansicht");
            got.extend(ids(&page));
            pages += 1;
            match page.next_cursor {
                Some(c) => start = PageStart::Cursor(c),
                None => break,
            }
        }
        assert_eq!(got, expected, "Cursor-Paging {column} desc={desc}");
        assert_eq!(pages, 4, "10 Zeilen in Seiten zu 3");
    }
}

// spec: ONPAGE-001
#[test]
fn exact_page_boundary_has_no_next_cursor() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut all = q("All");
    all.limit = 10;
    let page = query(&file, &cfg, &all);
    assert_eq!(page.rows.len(), 10);
    assert!(page.next_cursor.is_none());
    all.limit = 9;
    assert!(query(&file, &cfg, &all).next_cursor.is_some());
}

// spec: ONPAGE-001
#[test]
fn foreign_cursors_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let other = sample(dir.path(), 2);
    let cfg = CrawlConfig::default();
    let mut first = q("All");
    first.limit = 2;
    first.sort = vec![sort("Title 1", false)];
    let cursor = query(&file, &cfg, &first).next_cursor.unwrap();

    let try_with = |file: &CrawlFile, mut query_: ViewQuery| {
        query_.start = PageStart::Cursor(cursor.clone());
        query_view_in(&[test_tab()], file, &cfg, &query_)
    };
    // gleiche Abfrage, gleicher Crawl: gültig
    assert!(try_with(&file, first.clone()).is_ok());
    // andere Sortierung
    let mut desc = first.clone();
    desc.sort = vec![sort("Title 1", true)];
    assert!(matches!(
        try_with(&file, desc),
        Err(ViewError::ForeignCursor)
    ));
    // anderer Filter
    let mut other_filter = first.clone();
    other_filter.filter = "Errors".into();
    assert!(matches!(
        try_with(&file, other_filter),
        Err(ViewError::ForeignCursor)
    ));
    // andere Suche
    let mut searched = first.clone();
    searched.search = Some(SearchSpec::Simple("a".into()));
    assert!(matches!(
        try_with(&file, searched),
        Err(ViewError::ForeignCursor)
    ));
    // anderer Crawl
    assert!(matches!(
        try_with(&other, first.clone()),
        Err(ViewError::ForeignCursor)
    ));
    // unlesbar
    let mut garbage = first;
    garbage.start = PageStart::Cursor("!!!kein-cursor".into());
    assert!(matches!(
        query_view_in(&[test_tab()], &file, &cfg, &garbage),
        Err(ViewError::ForeignCursor)
    ));
}

// spec: ONPAGE-001
#[test]
fn limit_must_be_between_1_and_500() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    for bad in [0u32, 501] {
        let mut query_ = q("All");
        query_.limit = bad;
        assert!(matches!(
            query_view_in(&[test_tab()], &file, &cfg, &query_),
            Err(ViewError::InvalidLimit(n)) if n == bad
        ));
    }
    for good in [1u32, 500] {
        let mut query_ = q("All");
        query_.limit = good;
        assert!(query_view_in(&[test_tab()], &file, &cfg, &query_).is_ok());
    }
}

// spec: ONPAGE-001
#[test]
fn version_is_the_change_seq_and_row_keys_are_url_ids() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    set_change_seq(&file, 7);
    let page = query(&file, &cfg, &q("Errors"));
    assert_eq!(page.version, 7);
    assert_eq!(page.row_keys, vec![UrlId(5)]);
    let counts = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    assert!(counts.iter().all(|c| c.version == 7));
    set_change_seq(&file, 8);
    assert_eq!(query(&file, &cfg, &q("All")).version, 8);
}

// spec: ONPAGE-001
#[test]
fn stream_view_yields_all_rows_in_order_with_the_header() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    with_canonicals(&file);
    let cfg = CrawlConfig::default();
    let mut query_ = q("All");
    query_.sort = vec![sort("Title 1", false)];
    query_.limit = 2; // wird beim Streamen ignoriert
    let mut rows: Vec<Vec<Value>> = Vec::new();
    let header = stream_view_in(&[test_tab()], &file, &cfg, &query_, &mut |r| {
        rows.push(r.to_vec());
        Ok(())
    })
    .unwrap();
    let page = query(&file, &cfg, &{
        let mut full = query_.clone();
        full.limit = 500;
        full
    });
    assert_eq!(header, page.columns);
    assert_eq!(rows, page.rows);
    assert_eq!(rows.len(), 10);
}

// spec: ONPAGE-001
#[test]
fn cell_values_follow_the_column_kind() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let mut query_ = q("All");
    query_.columns = ColumnSelection::Named(vec![
        "Indexable".into(),
        "Text Ratio".into(),
        "Response Time".into(),
        "Hash".into(),
        "Status Code".into(),
    ]);
    let page = query(&file, &cfg, &query_);
    assert_eq!(
        page.rows[0],
        vec![
            Value::Bool(true),
            Value::Float(12.35),
            Value::Float(1.234),
            Value::Text("abc".into()),
            Value::Int(200)
        ]
    );
    assert_eq!(page.rows[1][1], Value::Null, "fehlende Werte bleiben leer");
}

// spec: ONPAGE-001
#[test]
fn column_selection_default_all_and_named() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let default = query(&file, &cfg, &q("All"));
    assert_eq!(
        default.columns,
        vec![
            "Address",
            "Title 1",
            "Title 1 Length",
            "Status Code",
            "Indexable",
            "Canonical 1"
        ]
    );
    let mut all = q("All");
    all.columns = ColumnSelection::All;
    let page = query(&file, &cfg, &all);
    assert_eq!(
        page.columns.last().map(String::as_str),
        Some("Ext Spalte"),
        "Erweiterung zuletzt"
    );
    let mut named = q("All");
    named.columns = ColumnSelection::Named(vec!["Hash".into(), "Address".into()]);
    assert_eq!(query(&file, &cfg, &named).columns, vec!["Hash", "Address"]);
    named.columns = ColumnSelection::Named(vec!["Adress".into()]);
    assert!(matches!(
        query_view_in(&[test_tab()], &file, &cfg, &named),
        Err(ViewError::Name(os_analyze::NameError::Unknown { .. }))
    ));
    // Spalten ausblenden ändert die Zeilenmenge nicht.
    let mut only_address = q("Errors");
    only_address.columns = ColumnSelection::Named(vec!["Address".into()]);
    assert_eq!(
        query(&file, &cfg, &only_address).row_keys,
        query(&file, &cfg, &q("Errors")).row_keys
    );
}

// spec: TECH-004
#[test]
fn multi_columns_expand_to_the_maximum_over_the_whole_view() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    with_canonicals(&file);
    let cfg = CrawlConfig::default();
    let mut first = q("All");
    first.limit = 2;
    first.columns = ColumnSelection::Named(vec!["Address".into(), "Canonical".into()]);
    let page = query(&file, &cfg, &first);
    assert_eq!(
        page.columns,
        vec!["Address", "Canonical 1", "Canonical 2", "Canonical 3"],
        "Maximum 3 (URL 1), auch wenn die Seite es nicht zeigt"
    );
    // Reihenfolge des Auftretens (position), fehlende Werte leer statt null.
    assert_eq!(
        page.rows[0],
        vec!["b.html".into(), "c1".into(), "c2".into(), "c3".into()]
    );
    assert_eq!(
        page.rows[1],
        vec![
            Value::Text("a.html".into()),
            Value::Text("x1".into()),
            Value::Text(String::new()),
            Value::Text(String::new())
        ]
    );
    // Eine spätere Seite ohne Canonicals hat dieselben Spalten.
    first.start = PageStart::Offset(4);
    let later = query(&file, &cfg, &first);
    assert_eq!(later.columns, page.columns);
    assert!(later.rows[0][1..]
        .iter()
        .all(|v| *v == Value::Text(String::new())));
    // Das Maximum gilt für die Ansicht (Filter), nicht für die ganze Datei: „Errors“ hat keine Canonicals.
    let mut errors = q("Errors");
    errors.columns = ColumnSelection::Named(vec!["Canonical".into()]);
    assert_eq!(query(&file, &cfg, &errors).columns, vec!["Canonical 1"]);
}

// spec: TECH-004
#[test]
fn multi_columns_can_be_sorted_by_a_numbered_column() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    with_canonicals(&file);
    let cfg = CrawlConfig::default();
    let mut query_ = q("All");
    query_.sort = vec![sort("Canonical 1", false)];
    let page = query(&file, &cfg, &query_);
    // „c1“ (URL 1) < „x1“ (URL 2); alle übrigen sind leer und folgen am Ende.
    assert_eq!(&ids(&page)[..2], &[1, 2]);
}

// spec: TECH-004
#[test]
fn numbered_names_resolve_for_selection_and_sort() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    with_canonicals(&file);
    let cfg = CrawlConfig::default();

    // Auswahl über den nummerierten Namen (auch in anderer Schreibweise): genau diese eine Spalte.
    let mut one = q("All");
    one.columns = ColumnSelection::Named(vec!["Address".into(), "canonical 2".into()]);
    let page = query(&file, &cfg, &one);
    assert_eq!(page.columns, vec!["Address", "Canonical 2"]);
    assert_eq!(page.rows[0][1], Value::Text("c2".into()));
    // Fehlender Wert: leerer Text, nie null.
    assert_eq!(page.rows[1][1], Value::Text(String::new()));

    // Der Grundname wählt weiterhin alle Indizes.
    one.columns = ColumnSelection::Named(vec!["Canonical".into()]);
    assert_eq!(query(&file, &cfg, &one).columns.len(), 3);

    // Ungültige Nummern sind unbekannte Spalten.
    for bad in ["Canonical 0", "Canonical x", "Canonical 1 URL", "Canonical"] {
        let mut b = q("All");
        b.columns = ColumnSelection::Named(vec![bad.into()]);
        let r = query_view_in(&[test_tab()], &file, &cfg, &b);
        if bad == "Canonical" {
            assert!(r.is_ok());
        } else {
            assert!(
                matches!(r, Err(ViewError::Name(_))),
                "{bad}: {r:?} statt unbekannter Spalte"
            );
        }
    }

    // Sortierung nach der zweiten Teilspalte: „c2“ (URL 1) vor allen leeren Werten.
    let mut sorted = q("All");
    sorted.sort = vec![sort("Canonical 2", false)];
    assert_eq!(ids(&query(&file, &cfg, &sorted))[0], 1);
}

/// Test-Tab mit verschränkter Mehrfachspalte „Pair“: je Index erst `raw`, dann `source`.
fn paired_tab() -> os_analyze::TabDef {
    use os_analyze::defs::PairedPart;
    let mut tab = test_tab();
    tab.columns.push(os_analyze::ColumnDef {
        key: "Pair",
        sql: ColumnSql::MultiPaired {
            base: "Pair",
            parts: &[
                PairedPart {
                    suffix: "",
                    list_sql: "SELECT raw FROM canonicals WHERE url_id = ?1 ORDER BY position",
                    kind: ColumnKind::Text,
                },
                PairedPart {
                    suffix: "Src",
                    list_sql: "SELECT source FROM canonicals WHERE url_id = ?1 ORDER BY position",
                    kind: ColumnKind::Text,
                },
            ],
        },
        kind: ColumnKind::Text,
        default_visible: false,
        extension: false,
        ..os_analyze::ColumnDef::META
    });
    tab
}

// spec: TECH-004
#[test]
fn paired_multi_columns_interleave_the_parts_per_index() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    with_canonicals(&file);
    file.conn()
        .execute("UPDATE canonicals SET source = 'Http' WHERE raw = 'c2'", [])
        .unwrap();
    let cfg = CrawlConfig::default();
    let tabs = [paired_tab()];
    let run = |cols: &[&str], sort_by: Option<&str>| {
        let mut query_ = q("All");
        query_.columns = ColumnSelection::Named(cols.iter().map(|c| (*c).into()).collect());
        if let Some(c) = sort_by {
            query_.sort = vec![sort(c, true)];
        }
        query_view_in(&tabs, &file, &cfg, &query_)
    };

    // Maximum 3 Indizes (URL 1); je Index Wert, dann Teilspalte „Src“.
    let page = run(&["Address", "Pair"], None).unwrap();
    assert_eq!(
        page.columns,
        vec![
            "Address",
            "Pair 1",
            "Pair 1 Src",
            "Pair 2",
            "Pair 2 Src",
            "Pair 3",
            "Pair 3 Src"
        ]
    );
    let text = |s: &str| Value::Text(s.into());
    assert_eq!(
        page.rows[0],
        vec![
            text("b.html"),
            text("c1"),
            text("Html"),
            text("c2"),
            text("Http"),
            text("c3"),
            text("Html")
        ]
    );
    // Fehlende Werte sind leere Texte.
    assert_eq!(page.rows[1][3], text(""));
    assert_eq!(page.rows[1][4], text(""));

    // Einzelne Teilspalte über den nummerierten Namen; Reihenfolge der Auswahl bleibt erhalten.
    let part = run(&["Pair 2 Src", "Pair 1"], None).unwrap();
    assert_eq!(part.columns, vec!["Pair 2 Src", "Pair 1"]);
    assert_eq!(part.rows[0], vec![text("Http"), text("c1")]);
    assert!(
        run(&["Pair 4 Src"], None).is_ok(),
        "Index über dem Maximum ist leer"
    );
    assert!(run(&["Pair 1 Nope"], None).is_err());

    // Sortierung nach einer Teilspalte (absteigend: „x1“ vor „c1“).
    let sorted = run(&["Address"], Some("Pair 1")).unwrap();
    assert_eq!(sorted.row_keys[0].0, 2);
    let by_src = run(&["Address"], Some("Pair 1 Src")).unwrap();
    assert_eq!(by_src.row_keys.len(), sorted.row_keys.len());
}

// spec: ONPAGE-001
#[test]
fn filter_default_sort_comes_from_the_registry_before_the_tab_default() {
    use os_analyze::tabs;
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();

    // Die Registry kennt nur den Filter „Duplicate“ der vier Element-Tabs.
    for (tab, column) in [
        ("Page Titles", "Title 1"),
        ("Meta Description", "Meta Description 1"),
        ("H1", "H1-1"),
        ("H2", "H2-1"),
    ] {
        let keys = tabs::filter_default_sort(tab, "Duplicate").expect(tab);
        assert_eq!(
            (keys.len(), keys[0].column.as_str(), keys[0].descending),
            (1, column, false)
        );
        assert!(tabs::filter_default_sort(tab, "All").is_none());
        assert!(tabs::filter_default_sort(tab, "Missing").is_none());
    }
    assert!(tabs::filter_default_sort("Internal", "Duplicate").is_none());
    for tab in ["Page Titles", "Meta Description", "H1", "H2"] {
        let def = tabs::all().into_iter().find(|t| t.id == tab).unwrap();
        assert!(def.default_sort.is_empty(), "{tab}: keine Tab-Sortierung");
    }

    // Der Test-Tab trägt die Kennung „Page Titles“: „Duplicate“ sortiert nach Title 1, „All“ bleibt in
    // Entdeckungsreihenfolge (UrlId, K-17).
    let make_tab = |default_sort: Vec<os_analyze::SortKey>| {
        let mut tab = test_tab();
        tab.id = "Page Titles";
        tab.default_sort = default_sort;
        tab.filters.push(os_analyze::FilterDef {
            id: "Duplicate",
            predicate: "1",
            kind: os_analyze::FilterKind::Live,
            requires: Vec::new(),
            ignore_non_indexable: false,
            default_columns: &[],
        });
        tab
    };
    let tab = make_tab(Vec::new());
    let run = |filter: &str, sort_by: Option<SortKeyArg>| {
        let mut query_ = ViewQuery::new("Page Titles", filter);
        query_.limit = 500;
        if let Some((c, d)) = sort_by {
            query_.sort = vec![sort(c, d)];
        }
        query_view_in(std::slice::from_ref(&tab), &file, &cfg, &query_).unwrap()
    };
    type SortKeyArg = (&'static str, bool);

    let all = run("All", None);
    let mut by_id = ids(&all);
    by_id.sort_unstable();
    assert_eq!(ids(&all), by_id, "All: Entdeckungsreihenfolge");

    let dup = run("Duplicate", None);
    let by_title = run("Duplicate", Some(("Title 1", false)));
    assert_eq!(
        ids(&dup),
        ids(&by_title),
        "Duplicate: nach Title 1 gruppiert"
    );
    assert_ne!(ids(&dup), by_id);
    assert_eq!(dup.rows.len(), all.rows.len());

    // Eine ausdrückliche Sortierung der Abfrage hat Vorrang vor der Registry.
    let explicit = run("Duplicate", Some(("Address", true)));
    assert_eq!(addresses(&explicit)[0], "ni.html");

    // Die Registry geht dem Tab-Standard voraus; ohne Registry-Eintrag gilt der Tab-Standard.
    let with_tab_default = make_tab(vec![sort("Address", true)]);
    let mut dq = ViewQuery::new("Page Titles", "Duplicate");
    dq.limit = 500;
    let page = query_view_in(std::slice::from_ref(&with_tab_default), &file, &cfg, &dq).unwrap();
    assert_eq!(ids(&page), ids(&dup), "Registry vor TabDef.default_sort");
    let mut aq = ViewQuery::new("Page Titles", "All");
    aq.limit = 500;
    let page = query_view_in(&[with_tab_default], &file, &cfg, &aq).unwrap();
    assert_eq!(
        addresses(&page)[0],
        "ni.html",
        "ohne Registry: TabDef.default_sort"
    );
}

// spec: ONPAGE-190
#[test]
fn counts_match_the_filter_views_and_carry_percentages() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let counts = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    assert_eq!(counts.len(), test_tab().filters.len());
    let get = |f: &str| counts.iter().find(|c| c.filter == f).unwrap();

    assert_eq!(get("All").count, Some(10));
    assert_eq!(get("All").pct, Some(100.0));
    assert_eq!(get("Missing Title").count, Some(3));
    assert_eq!(get("Missing Title").pct, Some(30.0));
    assert_eq!(get("Errors").count, Some(1));
    assert_eq!(get("Errors").pct, Some(10.0));
    // Dynamisches Label, kanonische Kennung bleibt mit Platzhalter.
    let over = get("Over {title.max_chars} Characters");
    assert_eq!(over.label, "Over 60 Characters");
    assert_eq!(over.count, Some(1));

    // Zahl in der Übersicht = Zeilenzahl der Filteransicht (alle Filter im Zustand Ready).
    for c in &counts {
        if c.state == FilterState::Ready {
            let page = query(&file, &cfg, &q(&c.filter));
            assert_eq!(c.count, Some(page.total), "Filter {}", c.filter);
        } else {
            assert_eq!((c.count, c.pct), (None, None), "Filter {}", c.filter);
        }
    }
}

// spec: ONPAGE-001
#[test]
fn percentages_round_commercially_to_one_decimal() {
    assert_eq!(views::percent(3, 7), 42.9);
    assert_eq!(views::percent(1, 3), 33.3);
    assert_eq!(views::percent(2, 3), 66.7);
    assert_eq!(
        views::percent(0, 0),
        0.0,
        "leere Grundmenge: keine Division durch 0"
    );
    assert_eq!(views::percent(5, 0), 0.0);
    assert_eq!(views::percent(1, 8), 12.5);
    assert_eq!(views::percent(1, 16), 6.3, "6,25 rundet kaufmännisch auf");
    assert_eq!(views::percent(7, 7), 100.0);
}

// spec: ONPAGE-001
#[test]
fn percentage_in_counts_for_three_of_seven() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "seven.ospider", 3);
    let rows: Vec<Row> = (1..=7)
        .map(|i| row(i, &format!("p{i}.html")).int("status_code", if i <= 3 { 404 } else { 200 }))
        .collect();
    insert(&file, &rows);
    let counts = filter_counts_in(&[test_tab()], &file, &CrawlConfig::default()).unwrap();
    let errors = counts.iter().find(|c| c.filter == "Errors").unwrap();
    assert_eq!((errors.count, errors.pct), (Some(3), Some(42.9)));

    // Leere Grundmenge: 0 von 0 = 0,0 und kein Zustand statt Zahl.
    let (_, empty) = create(dir.path(), "empty.ospider", 4);
    let counts = filter_counts_in(&[test_tab()], &empty, &CrawlConfig::default()).unwrap();
    let errors = counts.iter().find(|c| c.filter == "Errors").unwrap();
    assert_eq!((errors.count, errors.pct), (Some(0), Some(0.0)));
}

// spec: ONPAGE-001, ONPAGE-002
#[test]
fn thresholds_come_from_the_config_without_recrawl() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let mut cfg = CrawlConfig::default();
    let name = "Over {title.max_chars} Characters";
    assert_eq!(ids(&query(&file, &cfg, &q(name))), vec![8]);
    cfg.title.max_chars = 50;
    assert_eq!(ids(&query(&file, &cfg, &q(name))), vec![7, 8]);
    let counts = filter_counts_in(&[test_tab()], &file, &cfg).unwrap();
    assert_eq!(
        counts.iter().find(|c| c.filter == name).unwrap().label,
        "Over 50 Characters"
    );
    // Auch der aufgelöste Name (aktueller Wert oder eine andere Zahl) wählt den Filter.
    assert_eq!(
        ids(&query(&file, &cfg, &q("Over 50 Characters"))),
        vec![7, 8]
    );
    assert_eq!(
        ids(&query(&file, &cfg, &q("over x characters"))),
        vec![7, 8]
    );
}

// spec: ONPAGE-001
#[test]
fn ignore_non_indexable_excludes_only_flagged_filters_and_never_all() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let mut cfg = CrawlConfig::default();
    assert!(cfg.issues.ignore_non_indexable, "Default an");
    // Missing Title: 5, 6, 10 – der Filter mit Flag lässt die nicht indexierbare URL 10 weg.
    assert_eq!(
        ids(&query(&file, &cfg, &q("Missing Title"))),
        vec![5, 6, 10]
    );
    assert_eq!(
        ids(&query(&file, &cfg, &q("Missing Title (Indexable)"))),
        vec![5, 6]
    );
    assert_eq!(
        query(&file, &cfg, &q("All")).total,
        10,
        "All bleibt unberührt"
    );
    let plan = plan_in(&[test_tab()], &q("Missing Title (Indexable)"), &cfg).unwrap();
    assert!(plan.filter_sql.contains("Non-Indexable"));
    // Umschalten wirkt sofort.
    cfg.issues.ignore_non_indexable = false;
    assert_eq!(
        ids(&query(&file, &cfg, &q("Missing Title (Indexable)"))),
        vec![5, 6, 10]
    );
}

// spec: ONPAGE-001
#[test]
fn plan_exposes_population_filter_search_params_and_order() {
    let cfg = CrawlConfig::default();
    let mut query_ = q("Over {title.max_chars} Characters");
    query_.search = Some(SearchSpec::Simple("abc".into()));
    query_.sort = vec![sort("Title 1", true)];
    let plan = plan_in(&[test_tab()], &query_, &cfg).unwrap();
    assert_eq!(plan.population_sql, "(u.is_internal = 1)");
    assert_eq!(plan.filter_sql, "(u.title_1_len > :max)");
    assert_eq!(
        plan.search_sql.as_deref(),
        Some("instr(lower(u.address), lower(:q)) > 0")
    );
    assert!(plan.params.contains(&(":max".to_string(), Value::Int(60))));
    assert!(plan
        .params
        .contains(&(":q".to_string(), Value::Text("abc".into()))));
    assert!(plan.order_by.starts_with("ORDER BY "));
    assert!(plan.order_by.contains("COLLATE OS_TEXT"));
    assert!(plan.order_by.ends_with("u.address ASC, u.id ASC"));
    let plain = plan_in(&[test_tab()], &q("All"), &cfg).unwrap();
    assert_eq!(plain.order_by, "ORDER BY u.id ASC");
    assert_eq!(plain.search_sql, None);
}

// spec: ONPAGE-001
#[test]
fn unknown_tab_or_filter_is_reported_with_suggestions() {
    let dir = tempfile::tempdir().unwrap();
    let file = sample(dir.path(), 1);
    let cfg = CrawlConfig::default();
    let bad_tab = ViewQuery::new("Tset", "All");
    assert!(matches!(
        query_view_in(&[test_tab()], &file, &cfg, &bad_tab),
        Err(ViewError::Name(os_analyze::NameError::Unknown { .. }))
    ));
    let bad_filter = ViewQuery::new("Test", "Eroors");
    match query_view_in(&[test_tab()], &file, &cfg, &bad_filter) {
        Err(ViewError::Name(os_analyze::NameError::Unknown { suggestions, .. })) => {
            assert_eq!(suggestions.first().map(String::as_str), Some("Test:Errors"));
        }
        other => panic!("{other:?}"),
    }
}

// spec: ONPAGE-001, ONPAGE-190
#[test]
fn views_work_on_a_read_only_connection() {
    let dir = tempfile::tempdir().unwrap();
    let (path, file) = create(dir.path(), "ro.ospider", 6);
    insert(
        &file,
        &[row(1, "a.html").int("status_code", 404), row(2, "b.html")],
    );
    drop(file);
    let ro = CrawlFile::open(&path, os_store::OpenMode::ReadOnly).unwrap();
    let cfg = CrawlConfig::default();
    let mut errors = q("Errors");
    errors.sort = vec![sort("Address", false)];
    assert_eq!(ids(&query(&ro, &cfg, &errors)), vec![1]);
    let counts = filter_counts_in(&[test_tab()], &ro, &cfg).unwrap();
    assert_eq!(
        counts.iter().find(|c| c.filter == "All").unwrap().count,
        Some(2)
    );
}
