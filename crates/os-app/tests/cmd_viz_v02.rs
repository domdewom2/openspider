//! M3-V02 über die Commands: Verzeichnisgraph (UI-094), Links einblenden (UI-097), 3D-Arten (UI-098),
//! Body-Text-Wortwolke (UI-100) auf echten Crawls von `fx-ui-dirs` und `fx-ui-words`.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
#[path = "runtime_common.rs"]
mod common;

use os_app::{
    dto::*,
    pool::CrawlKey,
    runtime::{hooks::WindowHost, sessions::SessionInfo},
    App, CallCtx, WindowId,
};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::{CrawlConfig, CrawlState};
use serde_json::{json, Value};
use std::sync::Arc;

struct Host;
impl WindowHost for Host {
    fn open_viz(&self, _id: &str) -> Result<(), CmdError> {
        Ok(())
    }
    fn focus_main(&self) {}
    fn exit(&self) {}
}

fn expected(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../tests/fixtures/ui/{name}/expected.json"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn app(fx: &FixtureCrawl) -> App {
    let app = common::new_app(&fx.dir.path().join("app"));
    let key = CrawlKey("fixture".into());
    app.register_crawl(key.clone(), fx.file.path());
    app.sessions().set_current(
        WindowId("main".into()),
        SessionInfo {
            key,
            path: fx.file.path().to_owned(),
            config: Arc::new(CrawlConfig::default()),
            state: CrawlState::Completed,
            writable: true,
            engine: None,
        },
    );
    app.set_window_host(Arc::new(Host));
    app
}

async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap_or_else(|e| panic!("{name}: {e:?}"))
}

async fn fail(app: &App, name: &str, args: Value) -> CmdError {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap_err()
}

fn id_of(fx: &FixtureCrawl, path: &str) -> i64 {
    fx.file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address LIKE '%' || ?1 AND in_tables = 1",
            [path],
            |r| r.get(0),
        )
        .unwrap()
}

fn graph_args(id: &Value, kind: &str, budget: u32) -> Value {
    json!({"snapshotId": id, "kind": kind, "budget": budget, "metric": "crawl_depth", "expandDepth": 1000000})
}

fn edges_of(graph: &Value, kind: Option<&str>) -> Vec<(i64, i64)> {
    graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["edgeType"].as_str() == kind)
        .map(|e| (e["source"].as_i64().unwrap(), e["target"].as_i64().unwrap()))
        .collect()
}

// spec: UI-094
// spec: UI-090 [M3]
#[tokio::test]
async fn directory_graph_has_the_nodes_and_edges_of_the_directory_tree() {
    let exp = expected("fx-ui-dirs");
    let fx = crawl_fixture("ui/fx-ui-dirs", CrawlConfig::default()).await;
    let app = app(&fx);
    let opened = call(&app, "viz.open", json!({"kind": "directoryGraph"})).await;
    let id = &opened["snapshotId"];
    assert!(opened["window"].as_str().unwrap().starts_with("viz-"));
    let graph = call(&app, "viz.graph", graph_args(id, "directoryGraph", 10_000)).await;
    let tree = call(&app, "viz.graph", graph_args(id, "directoryTree", 10_000)).await;
    assert_eq!(graph, tree);
    assert_eq!(
        graph["nodes"].as_array().unwrap().len() as u64,
        exp["directoryNodes"].as_u64().unwrap()
    );
    assert_eq!(
        graph["edges"].as_array().unwrap().len() as u64,
        exp["directoryEdges"].as_u64().unwrap()
    );
    let hollow = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["address"].is_null())
        .count() as u64;
    assert_eq!(hollow, exp["hollowNodes"].as_u64().unwrap());
    // Die Optionen aus UI-092/UI-095 wirken wie im Crawl-Graphen (Metrik, Farbmetrik, Filter).
    let mut args = graph_args(id, "directoryGraph", 10_000);
    args["metric"] = json!("word_count");
    args["colorMetric"] = json!("response_time");
    args["includeNonIndexable"] = json!(false);
    let styled = call(&app, "viz.graph", args).await;
    assert!(styled["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["metric"].is_number() && n["colorValue"].is_number()));
    assert!(styled["availableMetrics"]
        .as_array()
        .unwrap()
        .contains(&json!("word_count")));
    // Wurzel per Kontextmenü: eine URL des Verzeichnisbaums.
    let post = id_of(&fx, "/blog/post");
    let rooted = call(
        &app,
        "viz.open",
        json!({"kind": "directoryGraph", "rootUrlId": post}),
    )
    .await;
    let sub = call(
        &app,
        "viz.graph",
        graph_args(&rooted["snapshotId"], "directoryGraph", 10_000),
    )
    .await;
    assert_eq!(sub["nodes"].as_array().unwrap().len(), 2);
    for w in [&opened, &rooted] {
        app.window_closed(&WindowId(w["window"].as_str().unwrap().into()));
    }
}

// spec: UI-098
#[tokio::test]
async fn three_d_kinds_open_and_accept_one_hundred_thousand_as_node_limit() {
    let fx = crawl_fixture("ui/fx-ui-dirs", CrawlConfig::default()).await;
    let app = app(&fx);
    for (kind, tree) in [
        ("crawlGraph3d", "crawlTree"),
        ("directoryGraph3d", "directoryTree"),
    ] {
        let opened = call(&app, "viz.open", json!({"kind": kind})).await;
        let id = &opened["snapshotId"];
        let three = call(&app, "viz.graph", graph_args(id, kind, 100_000)).await;
        let flat = call(&app, "viz.graph", graph_args(id, tree, 10_000)).await;
        assert_eq!(three["nodes"], flat["nodes"], "{kind}");
        assert_eq!(three["edges"], flat["edges"], "{kind}");
        // 2D bleibt bei 50 000, 3D endet bei 100 000.
        assert_eq!(
            fail(&app, "viz.graph", graph_args(id, tree, 100_000))
                .await
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(
            fail(&app, "viz.graph", graph_args(id, kind, 100_001))
                .await
                .code,
            ErrorCode::InvalidInput
        );
        app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
    }
}

// spec: UI-097
// spec: UI-090 [M3]
#[tokio::test]
async fn link_overlay_adds_typed_edges_counts_unique_links_resets_and_exports() {
    let exp = expected("fx-ui-words");
    let fx = crawl_fixture("ui/fx-ui-words", CrawlConfig::default()).await;
    let app = app(&fx);
    let target = id_of(&fx, "/target.html");
    for kind in [
        "crawlGraph",
        "directoryGraph",
        "crawlGraph3d",
        "directoryGraph3d",
    ] {
        let opened = call(&app, "viz.open", json!({"kind": kind})).await;
        let id = opened["snapshotId"].clone();
        let plain = call(&app, "viz.graph", graph_args(&id, kind, 10_000)).await;
        assert!(plain.get("linkTotal").is_none());
        let node = plain["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!(target))
            .unwrap()
            .clone();
        let mut args = graph_args(&id, kind, 10_000);
        args["linksNode"] = json!(target);
        args["linksMode"] = json!("inlinks");
        let shown = call(&app, "viz.graph", args.clone()).await;
        let inlinks = edges_of(&shown, Some("inlink"));
        assert_eq!(
            inlinks.len() as u64,
            exp["uniqueInlinks"].as_u64().unwrap(),
            "{kind}"
        );
        assert_eq!(shown["linkTotal"], json!(inlinks.len()));
        // Anzahl = Spalte Unique Inlinks des Knotens.
        assert_eq!(node["uniqueInlinks"], json!(inlinks.len()));
        // Baumkanten bleiben unverändert.
        assert_eq!(edges_of(&shown, None), edges_of(&plain, None));
        assert_eq!(shown["nodes"], plain["nodes"]);
        for (nofollow, nav, key) in [
            (false, false, "uniqueInlinksWithoutNofollow"),
            (true, true, "uniqueInlinksWithoutNavFooter"),
            (false, true, "uniqueInlinksWithoutNofollowAndNavFooter"),
        ] {
            let mut a = args.clone();
            a["linksIncludeNofollow"] = json!(nofollow);
            a["linksExcludeNavFooter"] = json!(nav);
            let g = call(&app, "viz.graph", a).await;
            assert_eq!(
                edges_of(&g, Some("inlink")).len() as u64,
                exp[key].as_u64().unwrap(),
                "{kind} {key}"
            );
        }
        let mut out = args.clone();
        out["linksMode"] = json!("outlinks");
        let g = call(&app, "viz.graph", out).await;
        assert_eq!(
            edges_of(&g, Some("outlink")).len() as u64,
            exp["uniqueOutlinks"].as_u64().unwrap()
        );
        for (mode, key, edge) in [
            ("childInlinks", "childInlinks", "inlink"),
            ("childOutlinks", "childOutlinks", "outlink"),
        ] {
            let mut a = args.clone();
            a["linksMode"] = json!(mode);
            let g = call(&app, "viz.graph", a).await;
            if kind.starts_with("crawl") {
                assert_eq!(
                    edges_of(&g, Some(edge)).len() as u64,
                    exp[key].as_u64().unwrap(),
                    "{kind} {mode}"
                );
            } else {
                // Im Verzeichnisbaum hat die Zielseite keine Kinder.
                assert_eq!(g["linkTotal"], json!(0), "{kind} {mode}");
            }
        }
        let mut bad = args.clone();
        bad["linksMode"] = json!("sideways");
        assert_eq!(
            fail(&app, "viz.graph", bad).await.code,
            ErrorCode::InvalidInput
        );

        // CSV mit eingeblendeten Links: Edge Type inlink neben tree.
        let shown = call(&app, "viz.graph", args.clone()).await;
        let dir = fx.dir.path().join(format!("csv-{kind}"));
        let op = call(
            &app,
            "viz.exportCsv",
            json!({"snapshotId": id, "dir": dir, "nodes": shown["nodes"], "edges": shown["edges"]}),
        )
        .await;
        let status = app.ops().wait(op["opId"].as_str().unwrap()).await.unwrap();
        assert_eq!(status.state, OpState::Done, "{:?}", status.error);
        let csv = std::fs::read_to_string(dir.join("edges.csv")).unwrap();
        assert_eq!(
            csv.lines()
                .filter(|l| l.trim_end().ends_with(",inlink"))
                .count() as u64,
            exp["uniqueInlinks"].as_u64().unwrap()
        );
        assert_eq!(
            csv.lines()
                .filter(|l| l.trim_end().ends_with(",tree"))
                .count(),
            edges_of(&plain, None).len()
        );
        // Zurücksetzen = Graph ohne Linkangabe; danach passt nur noch der Stand ohne Zusatzkanten.
        let reset = call(&app, "viz.graph", graph_args(&id, kind, 10_000)).await;
        assert_eq!(reset, plain);
        assert_eq!(
            fail(
                &app,
                "viz.exportCsv",
                json!({"snapshotId": id, "dir": dir, "nodes": shown["nodes"], "edges": shown["edges"]})
            )
            .await
            .code,
            ErrorCode::InvalidInput
        );
        app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
    }
}

fn words_args(id: &Value, url: i64) -> Value {
    json!({"snapshotId": id, "kind": "body", "urlId": url, "language": "en"})
}

// spec: UI-100
#[tokio::test]
async fn body_words_count_the_visible_text_of_word_count_with_default_options() {
    let exp = expected("fx-ui-words");
    let mut cfg = CrawlConfig::default();
    cfg.extraction.store_html = true;
    let fx = crawl_fixture("ui/fx-ui-words", cfg).await;
    let app = app(&fx);
    let target = id_of(&fx, "/target.html");
    let opened = call(
        &app,
        "viz.open",
        json!({"kind": "bodyWords", "rootUrlId": target}),
    )
    .await;
    let id = opened["snapshotId"].clone();
    let terms = call(&app, "viz.words", words_args(&id, target)).await;
    let actual: Vec<(String, u64)> = terms
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["word"].as_str().unwrap().to_owned(),
                t["count"].as_u64().unwrap(),
            )
        })
        .collect();
    let wanted: Vec<(String, u64)> = exp["bodyTerms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| (t[0].as_str().unwrap().to_owned(), t[1].as_u64().unwrap()))
        .collect();
    assert_eq!(actual, wanted);
    // Gleiche Textbasis wie Word Count: ohne Stoppwörter und Mindestlänge ergibt die Summe die Wortzahl.
    let mut all = words_args(&id, target);
    all["removeStopwords"] = json!(false);
    all["minLength"] = json!(1);
    all["maxTerms"] = json!(10_000);
    let every = call(&app, "viz.words", all).await;
    let sum: u64 = every
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["count"].as_u64().unwrap())
        .sum();
    let word_count: u64 = fx
        .file
        .conn()
        .query_row("SELECT word_count FROM urls WHERE id = ?1", [target], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(word_count, exp["wordCount"].as_u64().unwrap());
    assert_eq!(sum, word_count);
    // Navigation, Header und Footer (Inhaltsbereich der Wortzahl) fehlen.
    assert!(!every
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["word"] == "footer" || t["word"] == "navigation" || t["word"] == "header"));
    // Max. Begriffe und Mindestlänge wirken.
    let mut few = words_args(&id, target);
    few["maxTerms"] = json!(2);
    few["minLength"] = json!(5);
    assert_eq!(
        call(&app, "viz.words", few).await,
        json!([{"word": "pricing", "count": 6}, {"word": "plans", "count": 3}])
    );
    // Schnappschuss: späteres Überschreiben des HTML ändert die geöffnete Wolke nicht.
    fx.file
        .conn()
        .execute("DELETE FROM page_html WHERE url_id = ?1", [target])
        .unwrap();
    assert_eq!(
        call(&app, "viz.words", words_args(&id, target)).await,
        terms
    );
    // Andere URLs des Schnappschusses werden beim Aufruf gelesen.
    let home = call(&app, "viz.words", words_args(&id, id_of(&fx, "/"))).await;
    assert!(home
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["word"] == "start"));
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}

// spec: UI-100
#[tokio::test]
async fn body_words_without_stored_html_return_the_html_not_stored_hint() {
    let fx = crawl_fixture("ui/fx-ui-words", CrawlConfig::default()).await;
    let app = app(&fx);
    let target = id_of(&fx, "/target.html");
    let opened = call(
        &app,
        "viz.open",
        json!({"kind": "bodyWords", "rootUrlId": target}),
    )
    .await;
    let err = fail(&app, "viz.words", words_args(&opened["snapshotId"], target)).await;
    assert_eq!(err.code, ErrorCode::Unsupported);
    assert_eq!(err.message_key, "err-viz-html-not-stored");
    assert_eq!(err.args["state"]["code"], "HTML_NOT_STORED");
    assert_eq!(
        err.args["state"]["action"]["anchor"],
        "extraction.store_html"
    );
    // Unbekannte URL → Eingabefehler.
    let mut bad = words_args(&opened["snapshotId"], 999_999);
    bad["urlId"] = json!(999_999);
    assert_eq!(
        fail(&app, "viz.words", bad).await.code,
        ErrorCode::InvalidInput
    );
    app.window_closed(&WindowId(opened["window"].as_str().unwrap().into()));
}
