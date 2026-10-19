//! M2-I04: Tab „PageSpeed“ gegen eine Crawl-Datei mit PSI-Werten aus den Mock-Golden-Werten
//! (`os_testsite::mocks::psi`): Spalten, Filter, Status/Fehler, Gruppenauswahl.
use std::collections::BTreeSet;

use os_analyze::{
    tabs::{self, pagespeed},
    views::{self, ColumnSelection, ViewQuery},
};
use os_model::{CrawlConfig, FilterState, IdSource, SeededIds, Timestamp, Value};
use os_testsite::mocks::psi as fx;

const AT: &str = "2026-10-19T08:00:00Z";

struct Fixture {
    _dir: tempfile::TempDir,
    file: os_store::CrawlFile,
}

fn url(id: i64) -> String {
    format!("https://site.test/p{id}")
}

fn put(file: &os_store::CrawlFile, id: i64, metric: &str, cell: &fx::Cell) {
    let (real, text) = match cell {
        fx::Cell::Num(n) => (Some(*n), None),
        fx::Cell::Text(t) => (None, Some((*t).to_string())),
    };
    file.conn()
        .execute(
            "INSERT OR REPLACE INTO api_values(url_id,source,metric,level,v_real,v_text,fetched_at) VALUES(?1,'psi',?2,'url',?3,?4,?5)",
            (id, metric, real, text, AT),
        )
        .unwrap();
}

fn put_all(file: &os_store::CrawlFile, id: i64, cells: &[(&str, fx::Cell)], fail: bool) {
    for (name, cell) in cells {
        put(file, id, name, cell);
    }
    for (filter, _) in fx::FILTERS {
        let flag = fx::Cell::Num(if fail { 1.0 } else { 0.0 });
        put(file, id, &format!("fail:{filter}"), &flag);
    }
}

/// p1 heavy, p2 fast, p3 Lighthouse-Fehler, p4 Abruffehler der Warteschlange (über alten heavy-Werten),
/// p5 noch nicht abgefragt, p6 Kontingent der Warteschlange erschöpft, p7 Weiterleitung, p8 Bild.
fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let at = Timestamp::UNIX_EPOCH;
    let file = os_store::CrawlFile::create(
        &dir.path().join("pagespeed.ospider"),
        &os_store::CrawlInit {
            config: CrawlConfig::default(),
            crawl_id: SeededIds::new(4).new_crawl_id(at),
            mode: os_model::CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "fixture".into(),
            name: "pagespeed".into(),
            created_at: at,
        },
        os_store::StorageProfile::Balanced,
    )
    .unwrap();
    for (id, class, status) in [
        (1, "HTML", 200),
        (2, "HTML", 200),
        (3, "HTML", 200),
        (4, "HTML", 200),
        (5, "HTML", 200),
        (6, "HTML", 200),
        (7, "HTML", 301),
        (8, "Image", 200),
    ] {
        file.conn()
            .execute(
                "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code) \
                 VALUES(?1,?2,?2,'https://site.test',1,1,'done','link',0,20,?3,?4)",
                (id, url(id), class, status),
            )
            .unwrap();
    }
    let mut heavy = fx::expected_heavy();
    heavy.extend(fx::expected_crux());
    put_all(&file, 1, &heavy, true);
    put_all(&file, 2, &fx::expected_fast(), false);
    put(&file, 3, "PSI Status", &fx::Cell::Text("Error"));
    put(
        &file,
        3,
        "PSI Error",
        &fx::Cell::Text("ERRORED_DOCUMENT_REQUEST The page could not be loaded (status code 401)."),
    );
    put_all(&file, 4, &heavy, true);
    let at = Timestamp::from_db(AT).unwrap();
    os_store::queries::api::record_error(
        file.conn(),
        "psi",
        &url(4),
        "500",
        "API-Abruf fehlgeschlagen",
        at,
    )
    .unwrap();
    os_store::queries::api::record_error(file.conn(), "psi", &url(6), "", "Quota exceeded", at)
        .unwrap();
    Fixture { _dir: dir, file }
}

fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.api.psi.enabled = true;
    cfg
}

fn page(fx: &Fixture, cfg: &CrawlConfig, filter: &str) -> views::ViewPage {
    let mut q = ViewQuery::new("PageSpeed", filter);
    q.columns = ColumnSelection::All;
    views::query_view_in(&tabs::configured(cfg), &fx.file, cfg, &q).unwrap()
}

fn row(page: &views::ViewPage, id: i64) -> &[Value] {
    let i = page
        .row_keys
        .iter()
        .position(|k| k.0 == id)
        .unwrap_or_else(|| panic!("Zeile p{id} fehlt"));
    &page.rows[i]
}

fn cell<'a>(page: &'a views::ViewPage, id: i64, column: &str) -> &'a Value {
    let c = page
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("Spalte {column} fehlt"));
    &row(page, id)[c]
}

fn expect(column: &str, c: &fx::Cell) -> Value {
    let float = pagespeed::COLUMNS
        .iter()
        .any(|(_, n, k)| *n == column && matches!(k, os_analyze::ColumnKind::Float { .. }));
    match c {
        fx::Cell::Num(n) if float => Value::Float(*n),
        fx::Cell::Num(n) => Value::Int(*n as i64),
        fx::Cell::Text(t) => Value::Text((*t).to_string()),
    }
}

// spec: INTEG-133
// spec: INTEG-134
// spec: INTEG-136
#[test]
fn columns_follow_group_order_and_show_the_stored_values() {
    let fx = fixture();
    let cfg = config();
    let all = page(&fx, &cfg, "All");
    assert_eq!(
        all.total, 6,
        "nur interne HTML-URLs mit Status 200 (INTEG-130)"
    );
    let mut expected = vec!["Address", "PSI Status", "PSI Error"];
    expected.extend(pagespeed::COLUMNS.iter().map(|(_, name, _)| *name));
    assert_eq!(all.columns, expected);
    // Spaltenreihenfolge: Overview, CrUX, Lighthouse, Insights, Diagnostics.
    let groups: Vec<&str> = pagespeed::COLUMNS.iter().map(|(g, _, _)| *g).collect();
    let mut order = groups.clone();
    order.dedup();
    assert_eq!(
        order,
        ["overview", "crux", "lighthouse", "insights", "diagnostics"]
    );
    let mut heavy = fx::expected_heavy();
    heavy.extend(fx::expected_crux());
    for (name, value) in &heavy {
        assert_eq!(cell(&all, 1, name), &expect(name, value), "p1 {name}");
    }
    for (name, value) in fx::expected_fast() {
        assert_eq!(cell(&all, 2, name), &expect(name, &value), "p2 {name}");
    }
    assert_eq!(
        cell(&all, 1, "Efficient Cache Policy Savings (Bytes)"),
        &Value::Null
    );
    assert_eq!(cell(&all, 1, "Time to Interactive (sec)"), &Value::Null);
}

// spec: INTEG-139
#[test]
fn each_filter_contains_exactly_the_failing_urls_and_never_errors() {
    let fx = fixture();
    let cfg = config();
    let tabs = tabs::configured(&cfg);
    let counts = views::filter_counts_in(&tabs, &fx.file, &cfg).unwrap();
    for (filter, _) in fx::FILTERS {
        let c = counts
            .iter()
            .find(|c| c.tab == "PageSpeed" && c.filter == filter)
            .unwrap_or_else(|| panic!("{filter}"));
        assert_eq!(c.state, FilterState::Ready, "{filter}");
        // p4 hat dieselben gespeicherten Werte wie p1, aber einen jüngeren Abruffehler.
        assert_eq!(c.count, Some(1), "{filter}");
        let p = page(&fx, &cfg, filter);
        assert_eq!(
            p.row_keys.iter().map(|k| k.0).collect::<Vec<_>>(),
            [1],
            "{filter}"
        );
    }
    // Ohne Anbindung sind die Filter nicht bereit.
    let off = CrawlConfig::default();
    let counts = views::filter_counts_in(&tabs::configured(&off), &fx.file, &off).unwrap();
    assert!(counts
        .iter()
        .filter(|c| c.tab == "PageSpeed" && c.filter != "All")
        .all(|c| matches!(c.state, FilterState::NeedsConfig(_)) && c.count.is_none()));
}

// spec: INTEG-143
#[test]
fn psi_status_and_error_cover_values_queue_errors_and_quota() {
    let fx = fixture();
    let cfg = config();
    let all = page(&fx, &cfg, "All");
    assert_eq!(cell(&all, 1, "PSI Status"), &Value::Text("Success".into()));
    assert_eq!(cell(&all, 1, "PSI Error"), &Value::Null);
    assert_eq!(cell(&all, 3, "PSI Status"), &Value::Text("Error".into()));
    assert_eq!(
        cell(&all, 3, "PSI Error"),
        &Value::Text(
            "ERRORED_DOCUMENT_REQUEST The page could not be loaded (status code 401).".into()
        )
    );
    // Abruffehler der Warteschlange: Status Error, Code und Meldung, alle Spalten leer.
    assert_eq!(cell(&all, 4, "PSI Status"), &Value::Text("Error".into()));
    assert_eq!(
        cell(&all, 4, "PSI Error"),
        &Value::Text("500 API-Abruf fehlgeschlagen".into())
    );
    assert_eq!(cell(&all, 4, "Performance Score"), &Value::Null);
    // Kontingent: kein eigener Statuswert, Fehlertext beginnt mit „Quota exceeded:“.
    assert_eq!(cell(&all, 6, "PSI Status"), &Value::Text("Error".into()));
    match cell(&all, 6, "PSI Error") {
        Value::Text(t) => assert!(t.starts_with("Quota exceeded:"), "{t}"),
        other => panic!("{other:?}"),
    }
    // Noch nicht abgefragt: leer.
    assert_eq!(cell(&all, 5, "PSI Status"), &Value::Null);
    for r in &all.rows {
        let status = &r[1];
        assert!(
            matches!(status, Value::Null)
                || *status == Value::Text("Success".into())
                || *status == Value::Text("Error".into()),
            "{status:?}"
        );
    }
    // Filter PSI Errors (Kennung noch nicht im Register): genau die Status-Error-URLs inkl. Kontingent.
    let mut tabs = tabs::configured(&cfg);
    tabs.iter_mut()
        .find(|t| t.id == "PageSpeed")
        .unwrap()
        .filters
        .push(pagespeed::psi_errors_filter());
    let errors = views::query_view_in(
        &tabs,
        &fx.file,
        &cfg,
        &ViewQuery::new("PageSpeed", "PSI Errors"),
    )
    .unwrap();
    let ids: BTreeSet<i64> = errors.row_keys.iter().map(|k| k.0).collect();
    assert_eq!(ids, BTreeSet::from([3, 4, 6]));
}

const DIAGNOSTICS_FILTERS: [&str; 8] = [
    "Minify CSS",
    "Minify JavaScript",
    "Reduce Unused CSS",
    "Reduce Unused JavaScript",
    "Reduce JavaScript Execution Time",
    "Minimize Main-Thread Work",
    "Optimize DOM Size",
    "Font Display",
];

// spec: INTEG-133
#[test]
fn deselecting_a_group_hides_its_columns_and_filters_without_touching_stored_values() {
    let fx = fixture();
    let mut cfg = config();
    cfg.api.psi.metric_groups = ["overview", "crux", "lighthouse", "insights", "mobile"]
        .map(String::from)
        .to_vec();
    let p = page(&fx, &cfg, "All");
    let diagnostics: Vec<&str> = pagespeed::COLUMNS
        .iter()
        .filter(|(g, _, _)| *g == "diagnostics")
        .map(|(_, n, _)| *n)
        .collect();
    assert_eq!(diagnostics.len(), 16);
    for name in &diagnostics {
        assert!(!p.columns.iter().any(|c| c == name), "{name}");
    }
    assert!(p
        .columns
        .iter()
        .any(|c| c == "Network Payload Size (Bytes)"));
    let tabs = tabs::configured(&cfg);
    let tab = tabs.iter().find(|t| t.id == "PageSpeed").unwrap();
    for f in DIAGNOSTICS_FILTERS {
        assert!(!tab.filters.iter().any(|d| d.id == f), "{f}");
    }
    assert!(tab
        .filters
        .iter()
        .any(|d| d.id == "Avoid Enormous Network Payloads"));
    let counts = views::filter_counts_in(&tabs, &fx.file, &cfg).unwrap();
    assert!(!counts
        .iter()
        .any(|c| c.tab == "PageSpeed" && DIAGNOSTICS_FILTERS.contains(&c.filter.as_str())));
    // Insights abwählen: deren Filter samt Avoid Enormous Network Payloads verschwinden.
    let mut no_insights = config();
    no_insights.api.psi.metric_groups = vec!["diagnostics".into()];
    let tabs = tabs::configured(&no_insights);
    let ids: Vec<&str> = tabs
        .iter()
        .find(|t| t.id == "PageSpeed")
        .unwrap()
        .filters
        .iter()
        .map(|f| f.id)
        .collect();
    let mut expected = vec!["All"];
    let mut diag = DIAGNOSTICS_FILTERS.to_vec();
    diag.sort_unstable();
    expected.extend(diag);
    assert_eq!(ids, expected);
    // Erneutes Anwählen: dieselben Werte wie vorher.
    let again = page(&fx, &config(), "All");
    assert_eq!(cell(&again, 1, "DOM Size"), &Value::Int(2_345));
}

// spec: INTEG-130
#[test]
fn local_source_shows_no_crux_columns() {
    let fx = fixture();
    let mut cfg = config();
    cfg.api.psi.local = true;
    let p = page(&fx, &cfg, "All");
    assert!(!p
        .columns
        .iter()
        .any(|c| c.contains("CrUX") || c == "Core Web Vitals Assessment"));
    assert!(p.columns.iter().any(|c| c == "Performance Score"));
}

// spec: INTEG-133
#[test]
fn tab_columns_and_filters_match_the_fixture_lists() {
    // Jede Spalte hat einen Golden-Wert in den Mock-Fixtures (Efficient Cache Policy nur bei älteren Versionen).
    let mut golden: BTreeSet<&str> = fx::expected_heavy()
        .into_iter()
        .chain(fx::expected_fast())
        .chain(fx::expected_crux())
        .map(|(n, _)| n)
        .filter(|n| *n != "PSI Status")
        .collect();
    golden.insert("Efficient Cache Policy Savings (Bytes)");
    let ours: BTreeSet<&str> = pagespeed::COLUMNS.iter().map(|(_, n, _)| *n).collect();
    assert_eq!(ours, golden);
    assert_eq!(pagespeed::FILTERS.to_vec(), fx::FILTERS.to_vec());
    let tab = pagespeed::tab();
    assert_eq!(
        tab.columns.iter().map(|c| c.key).collect::<Vec<_>>(),
        ["Address", "PSI Status", "PSI Error"]
    );
    assert_eq!(tab.filters.len(), 20);
}
