//! Tests der lesenden Werkzeuge (M1-P27) mit dem rmcp-Client gegen gespeicherte Crawls von `out-basic-site`
//! (und `out-issues`, `crawl-robots`, `synthetic-small` für Sonderfälle): `list_crawls`, `crawl_overview`,
//! `query_urls`, `url_details`, `list_issues`.
//!
//! Die Zahlen werden gegen die Exporte derselben Crawls geprüft (Report „Crawl Overview“, Tab-Export,
//! Issues-Export) – das ist der Kern von AGENT-010, -011 und -013: dieselbe Logik, dieselben Zahlen.

#[path = "tools_support.rs"]
mod support;

use std::collections::BTreeSet;

use os_analyze::views::ViewQuery;
use os_analyze::SortKey;
use os_export::{save_report, Format};
use os_model::CrawlConfig;
use os_store::{registry, CrawlFile, OpenMode};
use serde_json::{json, Value};
use support::*;

// ---------------------------------------------------------------------------------------------------------
// AGENT-010 list_crawls

// spec: AGENT-010
#[tokio::test]
async fn list_crawls_shows_both_crawls_and_marks_the_paused_one() {
    let world = World::new();
    let running = world.add("out-basic-site", "/", base_config(), 1).await;
    let paused = world
        .add_with_state("out-basic-site", "/", base_config(), 2, Some("Paused"))
        .await;
    assert_ne!(running.id, paused.id);
    let client = world.client().await;

    let reply = client.call("list_crawls", json!({})).await;
    let summary = reply.summary();
    assert_eq!(reply.total(), Some(2));
    assert_eq!(reply.next_cursor(), None);
    let crawls = summary["crawls"].as_array().unwrap();
    assert_eq!(crawls.len(), 2);
    // Neueste zuerst = Kennungen absteigend (UUID v7 ist zeitlich geordnet).
    let ids: Vec<&str> = crawls
        .iter()
        .map(|c| c["crawl_id"].as_str().unwrap())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(ids, sorted);

    let entry = |id: &str| crawls.iter().find(|c| c["crawl_id"] == id).unwrap();
    let done = entry(&running.id);
    assert_eq!(done["state"], "Completed");
    assert_eq!(done["mode"], "spider");
    assert!(done["start_url"].as_str().unwrap().starts_with("http://"));
    assert!(done["started_at"].is_string());
    let on_disk = registry::list_crawls(world.dir.path()).unwrap();
    for listing in &on_disk {
        let e = entry(&listing.meta.crawl_id.to_string());
        assert_eq!(e["urls"], listing.url_count, "URL-Anzahl");
        assert!(listing.url_count > 5);
    }
    let pausiert = entry(&paused.id);
    assert_eq!(pausiert["state"], "Paused", "pausierter Crawl sichtbar");
    assert_eq!(pausiert["ended_at"], Value::Null);
    client.cancel().await;
}

// spec: AGENT-010, AGENT-005
#[tokio::test]
async fn list_crawls_is_paginated_with_an_opaque_cursor() {
    let world = World::new();
    for seed in 1..=3 {
        world.add("out-basic-site", "/", base_config(), seed).await;
    }
    let client = world.client().await;
    let first = client.call("list_crawls", json!({"limit": 2})).await;
    assert_eq!(first.total(), Some(3));
    let cursor = first.next_cursor().expect("zweite Seite");
    let second = client
        .call("list_crawls", json!({"limit": 2, "cursor": cursor}))
        .await;
    assert_eq!(second.next_cursor(), None);
    let mut seen = BTreeSet::new();
    for r in [&first, &second] {
        for c in r.summary()["crawls"].as_array().unwrap() {
            assert!(
                seen.insert(c["crawl_id"].as_str().unwrap().to_string()),
                "Dublette"
            );
        }
    }
    assert_eq!(seen.len(), 3);

    // Ein Cursor einer anderen Abfrage wird abgelehnt.
    let foreign = client
        .call(
            "query_urls",
            json!({"crawl_id": seen.iter().next().unwrap(), "tab": "Internal",
            "cursor": first.next_cursor().unwrap()}),
        )
        .await;
    assert!(foreign.is_error);
    assert!(
        foreign.error_text().contains("Cursor"),
        "{}",
        foreign.error_text()
    );
    client.cancel().await;
}

// spec: AGENT-010
#[tokio::test]
async fn an_empty_or_missing_crawl_folder_lists_nothing() {
    let world = World::new();
    let client = world.client().await;
    let reply = client.call("list_crawls", json!({})).await;
    assert_eq!(reply.total(), Some(0));
    assert_eq!(reply.summary()["crawls"], json!([]));
    client.cancel().await;
}

// spec: AGENT-010
#[tokio::test]
async fn an_unknown_crawl_id_is_a_tool_error_that_names_the_known_crawls() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let reply = client
        .call(
            "crawl_overview",
            json!({"crawl_id": "01900000-0000-7000-8000-000000000000"}),
        )
        .await;
    assert!(reply.is_error);
    assert!(
        reply.error_text().contains(&stored.id),
        "{}",
        reply.error_text()
    );
    assert!(reply.error_text().contains("list_crawls"));
    let reply = client
        .call("crawl_overview", json!({"crawl_id": "kein-uuid"}))
        .await;
    assert!(
        reply.is_error && reply.error_text().contains("UUID"),
        "{}",
        reply.error_text()
    );
    let reply = client.call("crawl_overview", json!({})).await;
    assert!(reply.is_error && reply.error_text().contains("crawl_id"));
    client.cancel().await;
}

// ---------------------------------------------------------------------------------------------------------
// AGENT-010 crawl_overview

/// Zelle der Spalte `URLs` bzw. `% of Total` in der Textform des CSV-Reports.
fn report_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Number(n) if n.is_f64() => format!("{:.2}", n.as_f64().unwrap()),
        other => other.to_string(),
    }
}

// spec: AGENT-010
#[tokio::test]
async fn crawl_overview_has_exactly_the_numbers_of_the_crawl_overview_report() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;

    let tmp = tempfile::tempdir().unwrap();
    let result = save_report(
        &stored.fx.file,
        &stored.cfg,
        "Crawl Overview",
        tmp.path(),
        Format::Csv,
    )
    .unwrap();
    let report = Table::read(&result.files[0].path);
    let expected: Vec<(String, String, String, String)> = report
        .rows
        .iter()
        .map(|r| {
            (
                r[report.col("Section")].clone(),
                r[report.col("Item")].clone(),
                r[report.col("URLs")].clone(),
                r[report.col("% of Total")].clone(),
            )
        })
        .collect();
    assert!(expected.len() > 50, "der Report ist nicht leer");

    let reply = client
        .call("crawl_overview", json!({"crawl_id": stored.id}))
        .await;
    let mut actual = Vec::new();
    for section in reply.summary()["sections"].as_array().unwrap() {
        for item in section["items"].as_array().unwrap() {
            actual.push((
                section["section"].as_str().unwrap().to_string(),
                item["item"].as_str().unwrap().to_string(),
                report_cell(&item["urls"]),
                report_cell(&item["percent_of_total"]),
            ));
        }
    }
    assert_eq!(actual, expected, "Zeile für Zeile wie der Report");
    assert_eq!(reply.summary()["crawl_id"], stored.id.as_str());
    assert_eq!(reply.summary()["state"], "Completed");
    assert!(reply.untrusted.is_none(), "nur Zahlen, keine Seiteninhalte");
    client.cancel().await;
}

// spec: AGENT-010
#[tokio::test]
async fn crawl_overview_names_the_same_state_as_list_crawls_also_for_a_crashed_crawl() {
    let world = World::new();
    // Gespeichert als „Running“, aber ohne Lebenszeichen der letzten zehn Sekunden: abgestürzt.
    let crashed = world
        .add_with_state("out-basic-site", "/", base_config(), 1, Some("Running"))
        .await;
    let done = world.add("out-basic-site", "/", base_config(), 2).await;
    let client = world.client().await;

    let listing = client.call("list_crawls", json!({})).await;
    let listed = |id: &str| -> Value {
        listing.summary()["crawls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["crawl_id"] == id)
            .unwrap()
            .clone()
    };
    for stored in [&crashed, &done] {
        let overview = client
            .call("crawl_overview", json!({"crawl_id": stored.id}))
            .await;
        assert_eq!(
            overview.summary()["state"],
            listed(&stored.id)["state"],
            "beide Werkzeuge nennen denselben Zustand"
        );
    }
    let overview = client
        .call("crawl_overview", json!({"crawl_id": crashed.id}))
        .await;
    assert_eq!(overview.summary()["state"], "Incomplete");
    assert_eq!(overview.summary()["stored_state"], "Running");
    let overview = client
        .call("crawl_overview", json!({"crawl_id": done.id}))
        .await;
    assert_eq!(overview.summary()["state"], "Completed");
    assert!(overview.summary().get("stored_state").is_none());
    client.cancel().await;
}

/// Crawl-Ordner-Erweiterung: speichert das Fixture ohne die Crawl-Analyse (wie die Golden-Tests in `os-export`).
trait AddWithoutAnalysis {
    async fn add_without_analysis(
        &self,
        scenario: &str,
        start: &str,
        cfg: CrawlConfig,
        seed: u64,
    ) -> Stored;
}

impl AddWithoutAnalysis for World {
    async fn add_without_analysis(
        &self,
        scenario: &str,
        start: &str,
        cfg: CrawlConfig,
        seed: u64,
    ) -> Stored {
        let mut deps = os_engine::testing::test_deps();
        deps.ids = std::sync::Arc::new(os_model::SeededIds::new(seed));
        let fx = os_engine::testing::crawl_fixture_with(scenario, start, cfg, deps).await;
        let id = fx.file.meta().unwrap().crawl_id.to_string();
        fx.file
            .save_as(&self.dir.path().join(format!("{id}.ospider")))
            .expect("Crawl speichern");
        let cfg = fx.file.config().unwrap();
        Stored { id, fx, cfg }
    }
}

// spec: OUT-080, AGENT-010
#[tokio::test]
async fn crawl_overview_equals_the_golden_of_the_overview_report() {
    // Das Golden entsteht wie in `os-export` (`reports_overview.rs`) aus dem frisch gecrawlten Fixture OHNE
    // Crawl-Analyse: Zähler, die sie brauchen, stehen dort leer. `World::add` führt sie wie die CLI aus
    // (dann stünde dort 0), also wird der Crawl hier ohne Analyse gespeichert.
    let world = World::new();
    let stored = world
        .add_without_analysis("out-issues", "/", base_config(), 1)
        .await;
    let client = world.client().await;
    let reply = client
        .call("crawl_overview", json!({"crawl_id": stored.id}))
        .await;

    let bases = stored.fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    // Die Durchschnitte hängen an der Laufzeit des Crawls und sind im Golden maskiert.
    let mask = |item: &str, text: String| {
        if item == "Average Response Time (ms)" || item == "Average URLs/s" {
            "<N>".to_string()
        } else {
            os_testsite::golden::normalize(&text, &bases)
        }
    };
    let mut actual = Vec::new();
    for section in reply.summary()["sections"].as_array().unwrap() {
        for item in section["items"].as_array().unwrap() {
            let name = item["item"].as_str().unwrap();
            actual.push((
                section["section"].as_str().unwrap().to_string(),
                name.to_string(),
                mask(name, report_cell(&item["urls"])),
                report_cell(&item["percent_of_total"]),
            ));
        }
    }
    let mut csv = String::from("Section,Item,URLs,% of Total\n");
    for (section, item, urls, pct) in actual {
        let cells = [section, item, urls, pct].map(|s| format!("\"{}\"", s.replace('"', "\"\"")));
        csv.push_str(&cells.join(","));
        csv.push('\n');
    }
    os_testsite::golden::assert_m1_overview_golden(
        fixtures_dir().join("../golden/out-issues/crawl_overview.csv"),
        &csv,
    );
    client.cancel().await;
}

// spec: AGENT-010
#[tokio::test]
async fn crawl_overview_names_the_top_issues_like_the_issues_tab() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let overview = client
        .call("crawl_overview", json!({"crawl_id": stored.id}))
        .await;
    let issues = client
        .call("list_issues", json!({"crawl_id": stored.id}))
        .await;
    let top = overview.summary()["top_issues"].as_array().unwrap();
    assert!(!top.is_empty());
    let all = issues.summary()["issues"].as_array().unwrap();
    for (t, i) in top.iter().zip(all) {
        assert_eq!(t["id"], i["id"]);
        assert_eq!(t["type"], i["type"]);
        assert_eq!(t["priority"], i["priority"]);
        assert_eq!(t["urls"], i["urls"]);
    }
    client.cancel().await;
}

// ---------------------------------------------------------------------------------------------------------
// AGENT-011 query_urls

// spec: AGENT-011, AGENT-005
#[tokio::test]
async fn internal_all_paged_with_limit_5_equals_the_tab_export_without_duplicates() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;

    let export = export_view(&stored, &ViewQuery::new("Internal", "All"));
    let (rows, columns, total) = query_all(
        &client,
        json!({"crawl_id": stored.id, "tab": "Internal", "filter": "All"}),
        5,
    )
    .await;
    assert_eq!(columns, export.header, "gleiche Spalten");
    assert_eq!(total as usize, export.rows.len());
    assert_eq!(
        rows.len(),
        export.rows.len(),
        "Vereinigung der Seiten = Export"
    );
    let addresses: Vec<String> = rows.iter().map(|r| cell_text(&r[0])).collect();
    assert_eq!(
        addresses,
        export.column("Address"),
        "gleiche URLs, gleiche Reihenfolge"
    );
    let distinct: BTreeSet<&String> = addresses.iter().collect();
    assert_eq!(distinct.len(), addresses.len(), "keine Dubletten");
    for (mcp, csv) in rows.iter().zip(&export.rows) {
        for (c, (m, e)) in mcp.iter().zip(csv).enumerate() {
            // Zeitstempel liefert MCP in UTC (ISO 8601 mit Z), der Export mit lokalem Offset.
            if export.header[c].contains("Timestamp") {
                continue;
            }
            assert!(
                cells_equal(m, e),
                "Spalte {}: {m} <> {e:?}",
                export.header[c]
            );
        }
    }
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn client_errors_4xx_with_a_sort_equal_the_tab_export() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;

    for descending in [false, true] {
        let mut q = ViewQuery::new("Response Codes", "Client Error (4xx)");
        q.sort = vec![SortKey {
            column: "Address".to_string(),
            descending,
        }];
        let export = export_view(&stored, &q);
        assert!(export.rows.len() >= 2, "die Fixture hat 404-Seiten");
        let reply = client
            .call(
                "query_urls",
                json!({"crawl_id": stored.id, "tab": "Response Codes", "filter": "Client Error (4xx)",
                       "sort": [{"column": "Address", "descending": descending}], "limit": 500}),
            )
            .await;
        assert_eq!(reply.total(), Some(export.rows.len() as u64));
        let addresses: Vec<String> = reply.rows().iter().map(|r| cell_text(&r[0])).collect();
        assert_eq!(
            addresses,
            export.column("Address"),
            "absteigend: {descending}"
        );
        assert_eq!(reply.summary()["filter_state"], "ready");
    }
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn sort_columns_and_search_work_like_in_the_export() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;

    let mut q = ViewQuery::new("Internal", "HTML");
    q.sort = vec![
        SortKey {
            column: "Status Code".to_string(),
            descending: true,
        },
        SortKey {
            column: "Address".to_string(),
            descending: false,
        },
    ];
    let export = export_view(&stored, &q);
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal:HTML",
                   "sort": ["Status Code", "Address"], "limit": 500}),
        )
        .await;
    // "Status Code" aufsteigend im Aufruf: die Reihenfolge ist die umgekehrte Primärsortierung des Exports.
    let asc: Vec<String> = reply.rows().iter().map(|r| cell_text(&r[0])).collect();
    let mut q2 = ViewQuery::new("Internal", "HTML");
    q2.sort = vec![
        SortKey {
            column: "Status Code".to_string(),
            descending: false,
        },
        SortKey {
            column: "Address".to_string(),
            descending: false,
        },
    ];
    assert_eq!(asc, export_view(&stored, &q2).column("Address"));
    assert!(!export.rows.is_empty());

    // Suche über die Adresse.
    let found = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal", "search": "ABOUT", "columns": ["Address"]}),
        )
        .await;
    let rows = found.rows();
    assert!(!rows.is_empty());
    assert!(rows
        .iter()
        .all(|r| cell_text(&r[0]).to_lowercase().contains("about")));
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn response_time_is_in_seconds_and_transferred_is_the_body_size() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;

    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal", "filter": "HTML",
                   "columns": ["Address", "Response Time", "Transferred (bytes)"], "limit": 500}),
        )
        .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    let columns = reply.summary()["columns"].clone();
    assert_eq!(
        columns,
        json!(["Address", "Response Time", "Transferred (bytes)"])
    );
    let conn = stored.fx.file.conn();
    let mut checked = 0;
    for row in reply.rows() {
        let address = cell_text(&row[0]);
        let (ms, transferred): (Option<i64>, Option<i64>) = conn
            .query_row(
                "SELECT response_time_ms, transferred_bytes FROM urls WHERE address = ?1",
                [address.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        match ms {
            Some(ms) => {
                let seconds = row[1].as_f64().expect("Sekunden als Zahl");
                assert!(
                    (seconds - ms as f64 / 1000.0).abs() < 0.0006,
                    "{address}: {seconds} s <> {ms} ms"
                );
                assert!(seconds < 60.0, "Sekunden, nicht Millisekunden: {seconds}");
                checked += 1;
            }
            None => assert!(row[1].is_null()),
        }
        assert_eq!(
            row[2].as_i64(),
            transferred,
            "Transferred = gespeicherte Body-Bytes ({address})"
        );
    }
    assert!(checked > 3);
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn invalid_names_are_errors_that_list_the_valid_ones() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let id = stored.id.as_str();

    let bad_filter = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Response Codes", "filter": "Foo"}),
        )
        .await;
    assert!(bad_filter.is_error);
    let text = bad_filter.error_text();
    assert!(text.contains("Foo"), "{text}");
    for valid in [
        "All",
        "Client Error (4XX)",
        "Redirection (3XX)",
        "Server Error (5XX)",
    ] {
        assert!(text.contains(valid), "{valid} fehlt in: {text}");
    }
    let combined = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Response Codes:Foo"}),
        )
        .await;
    assert!(combined.is_error && combined.error_text().contains("Client Error (4XX)"));

    let bad_tab = client
        .call("query_urls", json!({"crawl_id": id, "tab": "Antwortcodes"}))
        .await;
    assert!(bad_tab.is_error);
    for valid in [
        "Internal",
        "External",
        "Response Codes",
        "Page Titles",
        "Hreflang",
    ] {
        assert!(
            bad_tab.error_text().contains(valid),
            "{valid}: {}",
            bad_tab.error_text()
        );
    }

    let bad_column = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Internal", "columns": ["Adresse"]}),
        )
        .await;
    assert!(bad_column.is_error);
    assert!(
        bad_column.error_text().contains("Address"),
        "{}",
        bad_column.error_text()
    );
    let bad_sort = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Internal", "sort": ["Nope"]}),
        )
        .await;
    assert!(bad_sort.is_error && bad_sort.error_text().contains("sort"));

    for limit in [0, 501] {
        let r = client
            .call(
                "query_urls",
                json!({"crawl_id": id, "tab": "Internal", "limit": limit}),
            )
            .await;
        assert!(r.is_error, "limit {limit}");
        assert!(r.error_text().contains("500"), "{}", r.error_text());
    }
    client.cancel().await;
}

// spec: AGENT-011, AGENT-005
#[tokio::test]
async fn a_cursor_of_another_query_is_rejected() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let id = stored.id.as_str();
    let first = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Internal", "limit": 2}),
        )
        .await;
    let cursor = first.next_cursor().unwrap();
    // gleicher Cursor, anderer Filter
    let other = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Internal", "filter": "HTML", "cursor": cursor}),
        )
        .await;
    assert!(other.is_error);
    assert!(
        other.error_text().contains("Cursor"),
        "{}",
        other.error_text()
    );
    // gleicher Cursor, gleiche Abfrage: weiter
    let next = client
        .call(
            "query_urls",
            json!({"crawl_id": id, "tab": "Internal", "limit": 2, "cursor": cursor}),
        )
        .await;
    assert!(!next.is_error);
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn multi_columns_can_be_sorted_by_their_numbered_name_and_listed_with_all_columns() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal", "columns": "all",
                   "sort": ["Meta Robots 1", "Address"], "limit": 500}),
        )
        .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    let columns: Vec<String> = reply.summary()["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert!(
        columns.iter().any(|c| c.starts_with("Meta Robots")),
        "{columns:?}"
    );
    assert!(
        columns.contains(&"Response Time".to_string()),
        "OS-Erweiterung bei all"
    );
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn a_numbered_name_with_a_suffix_is_accepted_and_an_unknown_suffix_lists_the_valid_names() {
    // `HTML hreflang 1 URL` ist die URL-Teilspalte der verschränkten Mehrfachspalte (`ColumnSql::MultiPaired`)
    // im Tab Hreflang; die Vorabprüfung muss denselben Namen akzeptieren wie die Ansicht in os-analyze.
    let world = World::new();
    let stored = world.add("tech/hreflang", "/en/", base_config(), 1).await;
    let client = world.client().await;
    let page = stored.fx.site.url("/en/");
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Hreflang",
                   "columns": ["Address", "HTML hreflang 1 URL"],
                   "search": "/en/", "limit": 500}),
        )
        .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    assert_eq!(
        reply.summary()["columns"],
        json!(["Address", "HTML hreflang 1 URL"])
    );
    let row = reply
        .rows()
        .into_iter()
        .find(|r| cell_text(&r[0]) == page)
        .expect("die Seite steht im Hreflang-Tab");
    // Erste Annotation der Fixture (`tech/hreflang`, `/en/`): `en` zeigt auf `/en/` selbst.
    assert_eq!(cell_text(&row[1]), page);

    // Ein unbekannter Suffix bleibt ein Fehler; die Meldung nennt die gültigen Formen inkl. Suffix.
    for bad in [
        "HTML hreflang 1 Foo",
        "HTML hreflang 0 URL",
        "HTML hreflang URL",
    ] {
        let reply = client
            .call(
                "query_urls",
                json!({"crawl_id": stored.id, "tab": "Hreflang", "columns": [bad]}),
            )
            .await;
        assert!(reply.is_error, "{bad}");
        let text = reply.error_text();
        assert!(text.contains(bad), "{text}");
        assert!(text.contains("HTML hreflang 1…n URL"), "{text}");
        assert!(text.contains("HTML hreflang 1…n,"), "{text}");
    }
    // Auch beim Sortieren gilt dieselbe Regel.
    let sorted = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Hreflang",
                   "sort": ["HTML hreflang 1 URL"], "limit": 5}),
        )
        .await;
    assert!(!sorted.is_error, "{:?}", sorted.blocks);
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn a_numbered_multi_column_name_can_be_requested_as_a_column() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal",
                   "columns": ["Address", "Meta Robots 1"], "limit": 500}),
        )
        .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    assert_eq!(
        reply.summary()["columns"],
        json!(["Address", "Meta Robots 1"])
    );
    client.cancel().await;
}

// spec: AGENT-011
#[tokio::test]
async fn filters_that_need_the_crawl_analysis_report_their_state_instead_of_zero() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.analysis.auto_at_end = false;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Sitemaps", "filter": "Orphan URLs"}),
        )
        .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    assert_eq!(reply.summary()["filter_state"], "needs_crawl_analysis");
    assert!(reply.summary()["filter_state_note"]
        .as_str()
        .unwrap()
        .contains("Crawl-Analyse"));
    assert_eq!(reply.summary()["rows"], 0);
    client.cancel().await;
}

// spec: AGENT-005
#[tokio::test]
async fn oversized_responses_are_truncated_with_an_export_offer() {
    let world = World::new();
    let stored = world
        .add(
            "synthetic-small",
            "/",
            {
                let mut cfg = base_config();
                cfg.limits.max_urls = 520;
                cfg
            },
            1,
        )
        .await;
    let client = world.client().await;
    let reply = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Internal", "columns": "all", "limit": 500}),
        )
        .await;
    assert!(!reply.is_error);
    assert_eq!(
        reply.blocks.len(),
        3,
        "Zusammenfassung, Datenblock, Hinweis"
    );
    let chars: usize = reply.blocks.iter().map(|b| b.chars().count()).sum();
    assert!(chars <= 50_000, "{chars}");
    assert!(reply.blocks[2].contains("gekürzt") && reply.blocks[2].contains("write"));
    // Die Zusammenfassung bleibt gültiges JSON mit total und next_cursor.
    assert!(reply.total().unwrap() >= 500);
    assert!(reply.next_cursor().is_some());
    client.cancel().await;
}

// ---------------------------------------------------------------------------------------------------------
// AGENT-012 url_details

async fn details(client: &Client, id: &str, url: &str, extra: Value) -> Reply {
    let mut args = json!({"crawl_id": id, "url": url});
    if let Value::Object(map) = extra {
        for (k, v) in map {
            args[k] = v;
        }
    }
    client.call("url_details", args).await
}

fn count(stored: &Stored, sql: &str, param: i64) -> u64 {
    stored
        .fx
        .file
        .conn()
        .query_row(sql, [param], |r| r.get::<_, i64>(0))
        .unwrap()
        .try_into()
        .unwrap()
}

// spec: AGENT-012
#[tokio::test]
async fn the_start_page_comes_with_all_sections() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let start = stored.fx.site.url("/");
    let reply = details(&client, &stored.id, &start, json!({})).await;
    let s = reply.summary();
    assert_eq!(s["found"], true);
    assert_eq!(s["crawled"], true);
    assert_eq!(s["state"], "done");
    assert!(s.get("reason").is_none());
    let sections: Vec<&str> = s["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(
        sections,
        [
            "overview",
            "inlinks",
            "outlinks",
            "headers",
            "issues",
            "directives",
            "canonicals",
            "hreflang",
            "structured_data",
            "rendering"
        ]
    );
    // strukturierte Daten und Rendering: in M1 nicht erhoben
    for name in ["structured_data", "rendering"] {
        assert_eq!(s[name]["status"], "not_collected", "{name}");
        assert!(s[name]["message"]
            .as_str()
            .unwrap()
            .contains("nicht erhoben"));
    }
    assert!(s["issues"].is_array());

    let data = reply.untrusted.as_ref().expect("Datenblock");
    assert_eq!(data["overview"]["Address"], start.as_str());
    assert_eq!(data["overview"]["Status Code"], 200);
    assert!(
        data["overview"]["Response Time"].as_f64().unwrap() < 60.0,
        "Sekunden"
    );
    assert!(data["headers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|h| h["direction"] == "response"
            && h["name"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case("content-type")));
    for key in [
        "directives",
        "canonicals",
        "hreflang",
        "inlinks",
        "outlinks",
    ] {
        assert!(data.get(key).is_some(), "{key}");
    }
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn hreflang_annotations_come_per_part_column_with_language_and_url_in_fixture_order() {
    // Die Hreflang-Spalten sind verschränkte Mehrfachspalten (`ColumnSql::MultiPaired`, TECH-071): je Index
    // `HTML hreflang n` (Sprache) und `HTML hreflang n URL`. `url_details` nennt je Annotation ein Objekt mit
    // Sprache und URL, in der Reihenfolge des Quelltextes (`tech/hreflang`, `/en/`), und stimmt mit den
    // Teilspalten des Hreflang-Tabs überein.
    let world = World::new();
    let stored = world.add("tech/hreflang", "/en/", base_config(), 1).await;
    let client = world.client().await;
    let page = stored.fx.site.url("/en/");
    let reply = details(
        &client,
        &stored.id,
        &page,
        json!({"sections": ["hreflang"]}),
    )
    .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    let entries = reply.untrusted.as_ref().expect("Datenblock")["hreflang"]
        .as_array()
        .expect("Liste der Annotationen")
        .clone();

    // Reihenfolge und Sprache wie im `<head>` der Fixture; die URL ist die aufgelöste Adresse des Ziels
    // (relative `href` gegen den Host der Testsite, absolute unverändert).
    let fixture: [(&str, &str); 7] = [
        ("en", "/en/"),
        ("de", "/de/"),
        ("fr", "/fr/"),
        ("fr-CA", "/fr-old/"),
        ("es", "http://site.test/es/"),
        ("it", "/it/"),
        ("nl", "/nl/"),
    ];
    assert_eq!(entries.len(), fixture.len(), "{entries:?}");
    for (i, (entry, (code, href))) in entries.iter().zip(fixture).enumerate() {
        let expected_url = if href.starts_with("http") {
            href.to_string()
        } else {
            stored.fx.site.url(href)
        };
        assert_eq!(entry["source"], "html", "{i}");
        assert_eq!(entry["position"], i as u64, "{i}");
        assert_eq!(entry["code"], code, "{i}");
        assert_eq!(entry["href"], href, "{i}");
        assert_eq!(entry["url"], expected_url.as_str(), "{i}");
    }

    // Dieselben Werte stehen in den Teilspalten des Hreflang-Tabs (MultiPaired): je Index Sprache + URL.
    // Die Teilspalten werden ausdrücklich angefordert (`<Basis> <n>` und `<Basis> <n> URL`), nicht über `all`.
    let mut wanted = vec!["Address".to_string()];
    for n in 1..=entries.len() {
        wanted.push(format!("HTML hreflang {n}"));
        wanted.push(format!("HTML hreflang {n} URL"));
    }
    let tab = client
        .call(
            "query_urls",
            json!({"crawl_id": stored.id, "tab": "Hreflang", "columns": wanted,
                   "search": "/en/", "limit": 500}),
        )
        .await;
    assert!(!tab.is_error, "{:?}", tab.blocks);
    let names: Vec<String> = tab.summary()["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        names, wanted,
        "genau die angeforderten Teilspalten, in dieser Reihenfolge"
    );
    let row = tab
        .rows()
        .into_iter()
        .find(|r| cell_text(&r[0]) == page)
        .expect("die Seite steht im Hreflang-Tab");
    let cell = |name: &str| -> String {
        let at = names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("Spalte {name} fehlt in {names:?}"));
        cell_text(&row[at])
    };
    for (i, entry) in entries.iter().enumerate() {
        let n = i + 1;
        assert_eq!(
            cell(&format!("HTML hreflang {n}")),
            entry["code"].as_str().unwrap(),
            "Sprache {n}"
        );
        assert_eq!(
            cell(&format!("HTML hreflang {n} URL")),
            entry["url"].as_str().unwrap(),
            "URL {n}"
        );
    }

    // Eine Seite ohne Annotation liefert eine leere Liste, keinen Fehler.
    let home = stored.fx.site.url("/");
    let reply = details(
        &client,
        &stored.id,
        &home,
        json!({"sections": ["hreflang"]}),
    )
    .await;
    assert!(!reply.is_error, "{:?}", reply.blocks);
    assert_eq!(
        reply.untrusted.as_ref().expect("Datenblock")["hreflang"],
        json!([])
    );
    client.cancel().await;
}

// spec: AGENT-012, AGENT-005
#[tokio::test]
async fn inlinks_and_outlinks_are_paged_with_limit_2_and_their_own_cursors() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let start = stored.fx.site.url("/");
    let url_id: i64 = stored
        .fx
        .file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address = ?1",
            [start.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let expected_in = count(
        &stored,
        "SELECT COUNT(*) FROM links WHERE target_id = ?1",
        url_id,
    );
    let expected_out = count(
        &stored,
        "SELECT COUNT(*) FROM links WHERE source_id = ?1",
        url_id,
    );
    assert!(
        expected_in > 2 && expected_out > 2,
        "{expected_in}/{expected_out}"
    );

    // Beide Abschnitte in einer Anfrage: je Abschnitt total/returned/next_cursor in der Zusammenfassung.
    let first = details(
        &client,
        &stored.id,
        &start,
        json!({"sections": ["inlinks", "outlinks"], "limit": 2}),
    )
    .await;
    assert_eq!(first.summary()["inlinks"]["total"], expected_in);
    assert_eq!(first.summary()["outlinks"]["total"], expected_out);
    assert_eq!(first.summary()["inlinks"]["returned"], 2);
    assert!(
        first.total().is_none(),
        "bei zwei Linkabschnitten gibt es kein gemeinsames total"
    );

    // Ein Abschnitt nach dem anderen durchblättern; Gesamtzahl und Reihenfolge wie bei einer großen Seite.
    for (section, expected) in [("inlinks", expected_in), ("outlinks", expected_out)] {
        let all = details(
            &client,
            &stored.id,
            &start,
            json!({"sections": [section], "limit": 500}),
        )
        .await;
        let reference = all.untrusted.as_ref().unwrap()[section]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(reference.len() as u64, expected);
        let mut collected: Vec<Value> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut extra = json!({"sections": [section], "limit": 2});
            if let Some(c) = &cursor {
                extra["cursor"] = json!(c);
            }
            let page = details(&client, &stored.id, &start, extra).await;
            assert!(!page.is_error, "{:?}", page.blocks);
            assert_eq!(page.total(), Some(expected));
            collected.extend(
                page.untrusted.as_ref().unwrap()[section]
                    .as_array()
                    .unwrap()
                    .clone(),
            );
            cursor = page.next_cursor();
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(
            collected, reference,
            "{section}: Vereinigung der Seiten = vollständige Liste"
        );
    }

    // Der Cursor des einen Abschnitts passt nicht zum anderen.
    let page = details(
        &client,
        &stored.id,
        &start,
        json!({"sections": ["inlinks"], "limit": 2}),
    )
    .await;
    let cursor = page.next_cursor().unwrap();
    let wrong = details(
        &client,
        &stored.id,
        &start,
        json!({"sections": ["outlinks"], "limit": 2, "cursor": cursor}),
    )
    .await;
    assert!(wrong.is_error && wrong.error_text().contains("Cursor"));
    // `cursor` ist mehrdeutig, wenn beide Linkabschnitte angefragt sind.
    let ambiguous = details(
        &client,
        &stored.id,
        &start,
        json!({"limit": 2, "cursor": cursor}),
    )
    .await;
    assert!(ambiguous.is_error && ambiguous.error_text().contains("inlinks_cursor"));
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn a_page_with_issues_lists_them_and_unknown_sections_are_rejected() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let url = stored.fx.site.url("/no-title.html");
    let reply = details(&client, &stored.id, &url, json!({"sections": ["issues"]})).await;
    let issues = reply.summary()["issues"].as_array().unwrap();
    assert!(
        issues
            .iter()
            .any(|i| i["tab"] == "Page Titles" && i["filter"] == "Missing" && i["type"] == "Issue"),
        "{issues:?}"
    );
    assert!(
        reply.untrusted.is_none(),
        "nur Katalogdaten, keine Seiteninhalte"
    );

    let bad = details(&client, &stored.id, &url, json!({"sections": ["kopf"]})).await;
    assert!(bad.is_error);
    assert!(bad.error_text().contains("hreflang") && bad.error_text().contains("kopf"));
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn a_url_that_is_not_in_the_crawl_is_reported_as_not_found() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let url = stored.fx.site.url("/nie-verlinkt.html");
    let reply = details(&client, &stored.id, &url, json!({})).await;
    assert!(!reply.is_error, "eine Auskunft, kein Fehler");
    let s = reply.summary();
    assert_eq!(s["found"], false);
    assert_eq!(s["crawled"], false);
    assert_eq!(s["reason"], "not_found");
    assert!(s["message"].as_str().unwrap().contains("nicht"));
    assert!(reply.untrusted.is_none());
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn an_excluded_url_reports_the_exclude_rule_as_reason() {
    let world = World::new();
    let text = std::fs::read_to_string(
        fixtures_dir()
            .join("out-basic-site")
            .join("exclude.osconfig"),
    )
    .unwrap();
    let cfg = CrawlConfig::from_toml_str(&text).unwrap();
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let excluded: String = stored
        .fx
        .file
        .conn()
        .query_row(
            "SELECT address FROM urls WHERE not_crawled_reason = 'excluded' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("die Exclude-Regel hat eine URL ausgeschlossen");
    let reply = details(
        &client,
        &stored.id,
        &excluded,
        json!({"sections": ["overview"]}),
    )
    .await;
    let s = reply.summary();
    assert_eq!(s["found"], true);
    assert_eq!(s["crawled"], false);
    assert_eq!(s["reason"], "excluded");
    assert!(s["message"].as_str().unwrap().contains("Exclude"));
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn a_url_blocked_by_robots_txt_reports_robots_txt_as_reason() {
    let world = World::new();
    let stored = world.add("crawl-robots", "/", base_config(), 1).await;
    let client = world.client().await;
    let blocked: String = stored
        .fx
        .file
        .conn()
        .query_row(
            "SELECT address FROM urls WHERE robots_blocked = 1 AND is_internal = 1 AND in_tables = 1 \
             AND no_response_reason = 'blocked_by_robots' ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("robots.txt sperrt URLs");
    let reply = details(
        &client,
        &stored.id,
        &blocked,
        json!({"sections": ["overview"]}),
    )
    .await;
    let s = reply.summary();
    assert_eq!(s["found"], true);
    assert_eq!(s["crawled"], false);
    assert_eq!(s["reason"], "robots_txt");
    assert!(s["message"].as_str().unwrap().contains("robots.txt"));
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn a_url_beyond_max_urls_reports_the_limit_as_reason() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.limits.max_urls = 5;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    let client = world.client().await;
    let waiting: String = stored
        .fx
        .file
        .conn()
        .query_row(
            "SELECT address FROM urls WHERE state <> 'done' AND is_internal = 1 AND in_tables = 1 \
             AND not_crawled_reason IS NULL ORDER BY id LIMIT 1",
            [],
            |r| r.get(0),
        )
        .expect("das Limit lässt URLs liegen");
    let reply = details(
        &client,
        &stored.id,
        &waiting,
        json!({"sections": ["overview"]}),
    )
    .await;
    let s = reply.summary();
    assert_eq!(s["found"], true);
    assert_eq!(s["crawled"], false);
    assert_eq!(s["reason"], "url_limit");
    assert!(s["message"].as_str().unwrap().contains("Limit"));
    client.cancel().await;
}

// spec: AGENT-012
#[tokio::test]
async fn every_stored_reason_is_reported_with_its_own_text() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.extraction.http_headers = true;
    let stored = world.add("out-basic-site", "/", cfg, 1).await;
    // Gründe, die der Crawl dieser Fixture nicht von selbst erzeugt (Listenmodus, Tiefenlimit, …), direkt in
    // einer Kopie der Datei gesetzt: Die Werkzeugschicht reicht jeden gespeicherten Grund mit eigenem Text durch.
    let path = world.dir.path().join(format!("{}.ospider", stored.id));
    let rw = CrawlFile::open(&path, OpenMode::ReadWrite).unwrap();
    let reasons = [
        "not_included",
        "outside_scope",
        "store_only",
        "crawl_disabled",
        "nofollow",
        "depth_limit",
        "redirect_limit",
        "external_not_checked",
        "list_mode_depth",
    ];
    let mut stmt = rw
        .conn()
        .prepare("SELECT id, address FROM urls WHERE state = 'done' AND is_internal = 1 AND in_tables = 1 ORDER BY id")
        .unwrap();
    let urls: Vec<(i64, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    drop(stmt);
    assert!(urls.len() >= reasons.len());
    for (reason, (id, _)) in reasons.iter().zip(&urls) {
        rw.conn()
            .execute(
                "UPDATE urls SET state = 'discovered', not_crawled_reason = ?1 WHERE id = ?2",
                (*reason, *id),
            )
            .unwrap();
    }
    drop(rw);

    let client = world.client().await;
    let mut messages = BTreeSet::new();
    for (reason, (_, address)) in reasons.iter().zip(&urls) {
        let reply = details(
            &client,
            &stored.id,
            address,
            json!({"sections": ["overview"]}),
        )
        .await;
        let s = reply.summary();
        assert_eq!(s["crawled"], false, "{reason}");
        assert_eq!(s["reason"], *reason);
        messages.insert(s["message"].as_str().unwrap().to_string());
    }
    assert_eq!(
        messages.len(),
        reasons.len(),
        "jeder Grund hat einen eigenen Text"
    );
    client.cancel().await;
}

// ---------------------------------------------------------------------------------------------------------
// AGENT-013 list_issues

fn issue_key(i: &Value) -> (String, String, String, u64) {
    (
        i["name"].as_str().unwrap().to_string(),
        i["type"].as_str().unwrap().to_string(),
        i["priority"].as_str().unwrap().to_string(),
        i["urls"].as_u64().unwrap(),
    )
}

// spec: AGENT-013
#[tokio::test]
async fn issues_equal_the_issues_export_including_the_texts() {
    for scenario in ["out-issues", "out-basic-site"] {
        let world = World::new();
        let stored = world.add(scenario, "/", base_config(), 1).await;
        let client = world.client().await;

        let tmp = tempfile::tempdir().unwrap();
        os_export::catalog::bulk_issues::export(
            &stored.fx.file,
            &stored.cfg,
            tmp.path(),
            Format::Csv,
        )
        .unwrap();
        let overview = Table::read(&tmp.path().join("issues").join("issues_overview.csv"));

        let reply = client
            .call("list_issues", json!({"crawl_id": stored.id, "limit": 500}))
            .await;
        let issues = reply.summary()["issues"].as_array().unwrap();
        assert_eq!(reply.total(), Some(issues.len() as u64));
        assert_eq!(
            issues.len(),
            overview.rows.len(),
            "{scenario}: gleiche Menge an Issues"
        );
        for (issue, row) in issues.iter().zip(&overview.rows) {
            let cell = |name: &str| row[overview.col(name)].clone();
            assert_eq!(issue["name"], cell("Issue Name").as_str(), "{scenario}");
            assert_eq!(issue["type"], cell("Issue Type").as_str());
            assert_eq!(issue["priority"], cell("Issue Priority").as_str());
            assert_eq!(issue["urls"].to_string(), cell("URLs"));
            // Textgleichheit (AGENT-013): what/why/fix sind die Katalogtexte des Exports.
            assert_eq!(
                issue["what"],
                cell("Description").as_str(),
                "{}",
                issue["id"]
            );
            assert_eq!(issue["fix"], cell("How To Fix").as_str(), "{}", issue["id"]);
            assert_eq!(
                issue["why"],
                cell("Why It Matters").as_str(),
                "{}",
                issue["id"]
            );
        }
        if scenario == "out-issues" {
            let names: Vec<&str> = issues.iter().map(|i| i["name"].as_str().unwrap()).collect();
            for expected in [
                "Page Titles: Missing",
                "Page Titles: Duplicate",
                "Page Titles: Over 60 Characters",
                "H1: Missing",
            ] {
                assert!(names.contains(&expected), "{expected} fehlt in {names:?}");
            }
        }
        client.cancel().await;
    }
}

// spec: AGENT-013
#[tokio::test]
async fn the_german_texts_are_the_german_catalog_texts() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let en = client
        .call("list_issues", json!({"crawl_id": stored.id}))
        .await;
    let de = client
        .call("list_issues", json!({"crawl_id": stored.id, "lang": "de"}))
        .await;
    assert_eq!(de.summary()["lang"], "de");
    let (en, de) = (
        en.summary()["issues"].as_array().unwrap().clone(),
        de.summary()["issues"].as_array().unwrap().clone(),
    );
    assert_eq!(en.len(), de.len());
    for (e, d) in en.iter().zip(&de) {
        assert_eq!(e["id"], d["id"]);
        assert_ne!(e["what"], d["what"], "{}", e["id"]);
        assert!(!d["fix"].as_str().unwrap().is_empty());
    }
    let bad = client
        .call("list_issues", json!({"crawl_id": stored.id, "lang": "fr"}))
        .await;
    assert!(bad.is_error && bad.error_text().contains("de, en"));
    client.cancel().await;
}

// spec: AGENT-013
#[tokio::test]
async fn priority_and_type_filter_the_issues_and_invalid_values_list_the_valid_ones() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let id = stored.id.as_str();
    let all = client.call("list_issues", json!({"crawl_id": id})).await;
    let all: Vec<Value> = all.summary()["issues"].as_array().unwrap().clone();

    for priority in ["high", "medium", "low"] {
        let reply = client
            .call("list_issues", json!({"crawl_id": id, "priority": priority}))
            .await;
        let got: Vec<(String, String, String, u64)> = reply.summary()["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(issue_key)
            .collect();
        let want: Vec<(String, String, String, u64)> = all
            .iter()
            .filter(|i| {
                i["priority"]
                    .as_str()
                    .unwrap()
                    .eq_ignore_ascii_case(priority)
            })
            .map(issue_key)
            .collect();
        assert_eq!(got, want, "priority={priority}");
        assert_eq!(reply.total(), Some(want.len() as u64));
    }
    let errors = client
        .call("list_issues", json!({"crawl_id": id, "type": "error"}))
        .await;
    let got = errors.summary()["issues"].as_array().unwrap();
    assert!(!got.is_empty());
    assert!(got.iter().all(|i| i["type"] == "Issue"), "error = Issue");
    let both = client
        .call(
            "list_issues",
            json!({"crawl_id": id, "type": "error", "priority": "high"}),
        )
        .await;
    assert!(both.summary()["issues"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["type"] == "Issue" && i["priority"] == "High"));

    let bad = client
        .call(
            "list_issues",
            json!({"crawl_id": id, "priority": "dringend"}),
        )
        .await;
    assert!(bad.is_error);
    assert!(
        bad.error_text().contains("high, medium, low"),
        "{}",
        bad.error_text()
    );
    let bad = client
        .call("list_issues", json!({"crawl_id": id, "type": "fatal"}))
        .await;
    assert!(bad.is_error);
    for valid in ["issue", "warning", "opportunity"] {
        assert!(bad.error_text().contains(valid), "{}", bad.error_text());
    }
    client.cancel().await;
}

// spec: AGENT-013
#[tokio::test]
async fn each_issue_has_at_most_5_examples_and_query_urls_returns_exactly_its_count() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let reply = client
        .call("list_issues", json!({"crawl_id": stored.id}))
        .await;
    let issues = reply.summary()["issues"].as_array().unwrap().clone();
    let examples = reply.untrusted.as_ref().unwrap()["examples"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(issues.len(), examples.len());
    for (issue, ex) in issues.iter().zip(&examples) {
        assert_eq!(ex["id"], issue["id"]);
        let urls = ex["urls"].as_array().unwrap();
        assert!(
            !urls.is_empty() && urls.len() <= 5,
            "{}: {} Beispiele",
            issue["id"],
            urls.len()
        );
        assert_eq!(urls.len() as u64, issue["urls"].as_u64().unwrap().min(5));
        // Der genannte Aufruf von query_urls liefert genau die gemeldete Anzahl.
        let q = &issue["query"];
        assert_eq!(q["tool"], "query_urls");
        let list = client
            .call(
                "query_urls",
                json!({"crawl_id": stored.id, "tab": q["tab"], "filter": q["filter"], "columns": ["Address"], "limit": 500}),
            )
            .await;
        assert!(!list.is_error, "{:?}", list.blocks);
        assert_eq!(list.total(), issue["urls"].as_u64(), "{}", issue["id"]);
        // Die Beispiele stammen aus dieser Liste.
        let listed: BTreeSet<String> = list.rows().iter().map(|r| cell_text(&r[0])).collect();
        for u in urls {
            assert!(listed.contains(u.as_str().unwrap()));
        }
    }
    // Seitenstämmige Adressen nur im Datenblock.
    assert!(
        !reply.trusted_text().contains("http://"),
        "{}",
        reply.trusted_text()
    );
    client.cancel().await;
}

// spec: AGENT-013, AGENT-005
#[tokio::test]
async fn issues_are_paged_with_an_opaque_cursor() {
    let world = World::new();
    let stored = world.add("out-issues", "/", base_config(), 1).await;
    let client = world.client().await;
    let id = stored.id.as_str();
    let all = client.call("list_issues", json!({"crawl_id": id})).await;
    let all_ids: Vec<Value> = all.summary()["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone())
        .collect();
    let mut seen: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({"crawl_id": id, "limit": 2});
        if let Some(c) = &cursor {
            args["cursor"] = json!(c);
        }
        let page = client.call("list_issues", args).await;
        assert_eq!(page.total(), Some(all_ids.len() as u64));
        seen.extend(
            page.summary()["issues"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["id"].clone()),
        );
        cursor = page.next_cursor();
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, all_ids);
    // Cursor mit anderem Filter: abgelehnt.
    let first = client
        .call("list_issues", json!({"crawl_id": id, "limit": 1}))
        .await;
    let foreign = client
        .call("list_issues", json!({"crawl_id": id, "limit": 1, "priority": "low", "cursor": first.next_cursor().unwrap()}))
        .await;
    assert!(foreign.is_error && foreign.error_text().contains("Cursor"));
    client.cancel().await;
}

// spec: AGENT-013
#[tokio::test]
async fn issues_that_need_the_crawl_analysis_are_named_instead_of_counted() {
    let world = World::new();
    let mut cfg = base_config();
    cfg.analysis.auto_at_end = false;
    let stored = world.add("out-issues", "/", cfg, 1).await;
    let client = world.client().await;
    let reply = client
        .call("list_issues", json!({"crawl_id": stored.id}))
        .await;
    let pending = reply.summary()["not_evaluated"]
        .as_array()
        .expect("nicht auswertbare Issues");
    assert!(!pending.is_empty());
    // spec: ONPAGE-007, ONPAGE-178, ONPAGE-179, ONPAGE-180
    assert!(pending.iter().all(|p| {
        let own_image_filter = p["tab"] == "Images"
            && matches!(
                p["filter"].as_str(),
                Some("Background Images" | "Missing Size Attributes" | "Incorrectly Sized Images")
            );
        p["state"]
            == if own_image_filter {
                "needs_rendering"
            } else {
                "needs_crawl_analysis"
            }
    }));
    client.cancel().await;
}
