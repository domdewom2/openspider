//! M2-N28: Der Filter „Non-200 Pagination URLs“ hatte dieselbe Abfrageform wie die Hreflang-Beziehungsfilter vor
//! M2-N26 (`pagination p JOIN urls t ON t.id = p.target_id WHERE p.url_id = u.id AND t.state = 'done'`). SQLite
//! durfte die Unterabfrage über `urls t` (Index `urls_state`) treiben und für jede Seite der Grundmenge alle
//! gecrawlten URLs durchsuchen – quadratisch in Overview, Issues und Filterzählern (UI-145, PERF-004). Mit
//! `CROSS JOIN` ist die Reihenfolge fest: zuerst die Pagination-Zeilen der Seite, dann das Ziel per Rowid.

use os_analyze::views::{plan, ViewQuery, URLS_FROM};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, Timestamp, Value};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

const FILTER: &str = "Non-200 Pagination URLs";

fn file(dir: &std::path::Path, sql: Option<&str>) -> CrawlFile {
    let at = Timestamp::from_db("2026-11-02T12:00:00.000Z").unwrap();
    let init = CrawlInit {
        crawl_id: SeededIds::new(28).new_crawl_id(at),
        mode: CrawlMode::Spider,
        start_url: Some("https://site.test/".into()),
        list_source: None,
        project: "Projekt".into(),
        name: "N28".into(),
        config: CrawlConfig::default(),
        created_at: at,
    };
    let file =
        CrawlFile::create(&dir.join("n28.ospider"), &init, StorageProfile::Balanced).unwrap();
    if let Some(sql) = sql {
        file.conn().execute_batch(sql).unwrap();
    }
    file
}

/// `pages` gecrawlte interne HTML-Seiten (Status 200).
fn crawl(file: &CrawlFile, pages: i64) {
    let tx = file.conn().unchecked_transaction().unwrap();
    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO urls (id, url, address, origin, is_internal, state, discovered_via, folder_depth, \
                 url_length, status_code, content_class) \
                 VALUES (?1, ?2, ?2, 'https://site.test', 1, 'done', 'link', 1, 30, 200, 'HTML')",
            )
            .unwrap();
        for id in 1..=pages {
            stmt.execute(rusqlite::params![id, format!("https://site.test/{id}")])
                .unwrap();
        }
    }
    tx.commit().unwrap();
}

fn bind(stmt: &mut rusqlite::Statement<'_>, params: &[(String, Value)]) {
    for (name, value) in params {
        if let Some(idx) = stmt.parameter_index(name).unwrap() {
            match value {
                Value::Int(i) => stmt.raw_bind_parameter(idx, *i).unwrap(),
                Value::Bool(b) => stmt.raw_bind_parameter(idx, i64::from(*b)).unwrap(),
                Value::Float(x) => stmt.raw_bind_parameter(idx, *x).unwrap(),
                Value::Text(t) => stmt.raw_bind_parameter(idx, t.as_str()).unwrap(),
                Value::Null => stmt.raw_bind_parameter(idx, rusqlite::types::Null).unwrap(),
            }
        }
    }
}

/// `COUNT(*)` von Grundmenge und Filter, wie Overview/Issues zählen; `rewrite` ändert das Filterprädikat.
fn count_sql(
    cfg: &CrawlConfig,
    rewrite: impl Fn(&str) -> String,
) -> (String, Vec<(String, Value)>) {
    let p = plan(&ViewQuery::new("Pagination", FILTER), cfg).unwrap();
    (
        format!(
            "SELECT COUNT(*) FROM {URLS_FROM} WHERE ({}) AND ({})",
            p.population_sql,
            rewrite(&p.filter_sql)
        ),
        p.params,
    )
}

fn explain(file: &CrawlFile, cfg: &CrawlConfig) -> Vec<String> {
    let (sql, params) = count_sql(cfg, str::to_string);
    let mut stmt = file
        .conn()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap();
    bind(&mut stmt, &params);
    let mut rows = stmt.raw_query();
    let mut plan = Vec::new();
    while let Some(r) = rows.next().unwrap() {
        plan.push(r.get::<_, String>(3).unwrap());
    }
    if std::env::var_os("N28_PLAN").is_some() {
        eprintln!("{FILTER}: {plan:#?}");
    }
    plan
}

/// Das Ziel `t` ist eine Suche per Primärschlüssel je Pagination-Zeile, nie eine Suche über alle URLs.
fn assert_target_by_rowid(plan: &[String]) {
    for line in plan
        .iter()
        .filter(|l| l.starts_with("SEARCH t ") || l.starts_with("SCAN t"))
    {
        assert!(line.contains("INTEGER PRIMARY KEY"), "{line}\n{plan:#?}");
    }
    assert!(plan.iter().any(|l| l.starts_with("SEARCH p ")), "{plan:#?}");
}

// spec: UI-145
// spec: PERF-004
// spec: TECH-031
#[test]
fn non_200_pagination_filter_looks_up_the_target_row_of_each_pagination_link() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), None);
    crawl(&file, 20);
    assert_target_by_rowid(&explain(&file, &CrawlConfig::default()));
}

// spec: UI-145
// spec: PERF-004
// spec: TECH-031
#[test]
fn the_join_order_does_not_depend_on_table_statistics() {
    // Ohne Statistik wählt SQLite heute schon `p` zuerst; die freie Reihenfolge hängt aber an den Schätzungen.
    // Mit Statistiken, nach denen ein Zustand von `urls` selten ist und eine Seite viele Pagination-Zeilen hat
    // (z. B. nach `ANALYZE` auf einer Datei mit großen Pagination-Serien), sucht der Plan ohne `CROSS JOIN` über
    // `urls_state` alle gecrawlten URLs je Seite ab (Form der Hreflang-Filter vor M2-N26).
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path(), None);
    crawl(&file, 20);
    file.conn()
        .execute_batch(
            "ANALYZE sqlite_schema;
             DELETE FROM sqlite_stat1;
             INSERT INTO sqlite_stat1 VALUES ('urls', 'urls_state', '1000000 1');
             INSERT INTO sqlite_stat1 VALUES ('pagination', 'pagination', '1000000 500000 500000 500000 1');
             INSERT INTO sqlite_stat1 VALUES ('pagination', 'pagination_target', '1000000 1');
             ANALYZE sqlite_schema;",
        )
        .unwrap();
    assert_target_by_rowid(&explain(&file, &CrawlConfig::default()));
}

// spec: TECH-031
// spec: UI-145
#[test]
fn fixed_join_order_keeps_the_rows_of_the_non_200_pagination_filter() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(
        dir.path(),
        Some(include_str!(
            "../../../tests/fixtures/tech-m2-pagination/crawl.sql"
        )),
    );
    let cfg = CrawlConfig::default();
    let count = |rewrite: &dyn Fn(&str) -> String| -> i64 {
        let (sql, params) = count_sql(&cfg, rewrite);
        let mut stmt = file.conn().prepare(&sql).unwrap();
        bind(&mut stmt, &params);
        let mut rows = stmt.raw_query();
        rows.next().unwrap().unwrap().get(0).unwrap()
    };
    let fixed = count(&|f| f.to_string());
    // Gleiche Treffermenge wie mit freier Join-Reihenfolge (Inner Join in beiden Fällen).
    let free = count(&|f| f.replace("CROSS JOIN", "JOIN"));
    assert!(fixed > 0, "Fixture enthält Non-200-Ziele");
    assert_eq!(fixed, free);
}
