//! B3-DIFF (Nutzerentscheidung 04.11.2026): `compare_crawls` mit `changes` liefert den Report „Crawl Changes“
//! (AGENT-015, OUT-154) paginiert und gleich der Datei von `--save-report "Crawl Changes"` (Golden der Fixture
//! `out-b3-diff`).
#[path = "tools_support.rs"]
mod support;
use os_mcp::{tools::compare_crawls::CompareCrawls, Tool, ToolContext, ToolError};
use os_model::CrawlConfig;
use serde_json::{json, Value};
use support::*;

fn golden_rows() -> Vec<Vec<String>> {
    let raw = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/golden/out-b3-diff/crawl_changes.csv"),
    )
    .unwrap();
    let body = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    // Die Golden-Datei enthält keine maskierten Felder (keine Kommas oder Anführungszeichen in den Werten).
    assert!(!body.contains('"'));
    body.lines()
        .map(|l| {
            l.trim_end_matches('\r')
                .split(',')
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// spec: AGENT-015
// spec: OUT-154
#[tokio::test]
async fn changes_return_the_crawl_changes_report_paginated_and_per_section() {
    let world = World::new();
    let b = world
        .add(
            "out-b3-diff/v1",
            "http://www.site.test/",
            CrawlConfig::default(),
            61,
        )
        .await;
    let a = world
        .add(
            "out-b3-diff/v2",
            "http://www.site.test/",
            CrawlConfig::default(),
            62,
        )
        .await;
    let ctx = ToolContext::new(world.dir.path().into());
    let golden = golden_rows();
    let header = &golden[0];

    let mut rows = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut args = json!({"a":a.id,"b":b.id,"changes":true,"limit":5});
        if let Some(c) = &cursor {
            args["cursor"] = json!(c);
        }
        let out = CompareCrawls.call(&ctx, args).unwrap();
        pages += 1;
        assert_eq!(out.total, Some((golden.len() - 1) as u64));
        assert_eq!(out.summary["report"], "Crawl Changes");
        let counts = out.summary["crawl_changes"].as_array().unwrap();
        assert_eq!(counts.len(), 8, "alle Abschnitte, auch mit 0");
        assert_eq!(counts[0], json!({"change":"Page Gone","count":2}));
        for item in out.untrusted.as_ref().unwrap()["rows"].as_array().unwrap() {
            rows.push(header.iter().map(|h| cell(&item[h])).collect::<Vec<_>>());
        }
        match out.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    assert_eq!(pages, 4, "18 Zeilen in Seiten zu 5");
    assert_eq!(
        rows,
        golden[1..],
        "identisch mit --save-report \"Crawl Changes\""
    );

    // Ein Abschnitt.
    let out = CompareCrawls
        .call(
            &ctx,
            json!({"a":a.id,"b":b.id,"changes":true,"change":"longer redirect chain"}),
        )
        .unwrap();
    let only = out.untrusted.unwrap()["rows"].as_array().unwrap().clone();
    assert_eq!(only.len(), 1);
    assert_eq!(only[0]["Address"], "http://www.site.test/c1");
    assert_eq!(only[0]["Redirect Chain Length (Previous)"], 2);
    assert_eq!(only[0]["Redirect Chain Length (Current)"], 3);

    // Ungültige Kombinationen.
    for args in [
        json!({"a":a.id,"b":b.id,"changes":true,"element":"page_title"}),
        json!({"a":a.id,"b":b.id,"changes":true,"change":"Gone"}),
        json!({"a":a.id,"b":b.id,"change":"Page Gone"}),
        json!({"a":a.id,"b":b.id,"changes":"yes"}),
    ] {
        assert!(
            matches!(
                CompareCrawls.call(&ctx, args.clone()),
                Err(ToolError::InvalidArguments(_))
            ),
            "{args}"
        );
    }
    // Ohne `changes` bleibt die Antwort wie bisher (Filterzähler), mit den neuen Elementen der Change Detection.
    let plain = CompareCrawls
        .call(&ctx, json!({"a":a.id,"b":b.id}))
        .unwrap();
    assert!(plain.summary["filters"].is_array());
    let detection = plain.summary["change_detection"].as_array().unwrap();
    assert!(detection
        .iter()
        .any(|d| d["element"] == "Canonical" && d["count"] == 1));
    assert!(detection
        .iter()
        .any(|d| d["element"] == "Redirect Target" && d["count"] == 4));
}
