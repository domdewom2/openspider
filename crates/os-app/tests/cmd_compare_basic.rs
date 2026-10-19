//! Commands `compare.*` (M2-C01): Auswahl und Rollen (OUT-140), Tausch (OUT-141), Test-Reiter und Fehlerposition des
//! URL-Mappings (OUT-146, CRAWL-034, TECH-306), Schließen. Fixtures: `out-m2-compare/v1`, `v2`, `mapping` und die
//! älteren Crawl-Dateien `crawl-files/schema-v{1,2}.ospider`.
// Testcode legt Crawl-Dateien an und liest sie: die Ausnahme von `clippy.toml` gilt hier, nicht in Quellcode.
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

#[path = "../../os-analyze/tests/compare_common.rs"]
mod common;

use std::path::Path;

use common::*;
use os_analyze::compare::CompareClass;
use os_app::dto::*;
use os_app::{App, AppConfig, CallCtx, WindowId};
use serde_json::json;

fn app(home: &Path) -> App {
    App::new(AppConfig::new(home)).unwrap()
}

fn ctx() -> CallCtx {
    CallCtx::for_window("main")
}

async fn call(
    app: &App,
    name: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, CmdError> {
    app.dispatch(ctx(), name, args).await
}

async fn open(app: &App, a: &Path, b: &Path, mapping: serde_json::Value) -> CompareSession {
    let v = call(
        app,
        "compare.open",
        json!({"current": a, "previous": b, "mapping": mapping, "elements": [], "contentThresholdPct": null}),
    )
    .await
    .unwrap_or_else(|e| panic!("compare.open: {e:?}"));
    serde_json::from_value(v).unwrap()
}

// spec: OUT-140
#[tokio::test(flavor = "multi_thread")]
async fn open_orders_the_crawls_by_start_time() {
    let (old, new) = (v1().await, v2().await);
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    // Auswahl „verkehrt herum“: der neuere Crawl wird trotzdem Current.
    let s = open(&app, old.file.path(), new.file.path(), json!([])).await;
    assert_eq!(s.current.file, new.file.path().to_string_lossy());
    assert_eq!(s.previous.file, old.file.path().to_string_lossy());
    assert!(s.current.started_at > s.previous.started_at);
    assert_eq!(s.version, 1);
    // Standard der Change Detection aus der Konfiguration des Current-Crawls (M2-C02 wertet aus).
    assert_eq!(s.content_threshold_pct, 10);
    assert!(!s.elements.is_empty());
    assert!(!s.elements.iter().any(|e| e == "content"));
    // Existierende (gecrawlte) URLs je Crawl.
    assert!(s.current.urls > s.previous.urls);
}

// spec: OUT-140
#[tokio::test(flavor = "multi_thread")]
async fn crawls_are_selected_by_their_management_id() {
    let (old, new) = (v1().await, v2().await);
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let crawls = &app.config().paths.crawls;
    std::fs::create_dir_all(crawls).unwrap();
    let mut ids = Vec::new();
    // Die Crawl-Verwaltung findet Dateien `<Kennung>.ospider` im Crawl-Ordner.
    for fx in [&old, &new] {
        let id = fx.file.meta().unwrap().crawl_id.to_string();
        fx.file
            .save_as(&crawls.join(format!("{id}.ospider")))
            .unwrap();
        ids.push(id);
    }
    let v = call(
        &app,
        "compare.open",
        json!({"current": ids[0], "previous": ids[1], "mapping": [], "elements": []}),
    )
    .await
    .unwrap();
    let s: CompareSession = serde_json::from_value(v).unwrap();
    assert_eq!(s.current.crawl_id, ids[1]);
    assert_eq!(s.previous.crawl_id, ids[0]);

    let err = call(
        &app,
        "compare.open",
        json!({"current": "0190f5f0-0000-7000-8000-000000000000", "previous": ids[1], "mapping": [], "elements": []}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    assert_eq!(err.args["field"], "current");
}

// spec: OUT-141
#[tokio::test(flavor = "multi_thread")]
async fn swap_exchanges_the_roles_and_recomputes_the_classes() {
    let (old, new) = (v1().await, v2().await);
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    open(&app, new.file.path(), old.file.path(), json!([])).await;
    let window = WindowId("main".into());
    let before = os_app::commands::compare::with_active(&app, &window, |a| {
        Ok(a.comparison.filter("Internal", "HTML").unwrap())
    })
    .await
    .unwrap();
    let s: CompareSession =
        serde_json::from_value(call(&app, "compare.swap", json!(null)).await.unwrap()).unwrap();
    assert_eq!(s.current.file, old.file.path().to_string_lossy());
    assert_eq!(s.version, 2);
    assert_eq!(
        os_app::commands::compare::active_version(&app, &window),
        Some(2)
    );
    let after = os_app::commands::compare::with_active(&app, &window, |a| {
        Ok(a.comparison.filter("Internal", "HTML").unwrap())
    })
    .await
    .unwrap();
    assert_eq!(
        after.class(CompareClass::Added),
        before.class(CompareClass::Removed)
    );
    assert_eq!(
        after.class(CompareClass::New),
        before.class(CompareClass::Missing)
    );
    assert_eq!(
        after.class(CompareClass::Missing),
        before.class(CompareClass::New)
    );
    assert_eq!(
        after.class(CompareClass::Removed),
        before.class(CompareClass::Added)
    );
}

// spec: OUT-146
// spec: CRAWL-034
// spec: TECH-306
#[tokio::test(flavor = "multi_thread")]
async fn test_mapping_shows_each_rule_and_the_result() {
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let v = call(
        &app,
        "compare.testMapping",
        json!({"rules": [
            {"pattern": r"^http://staging\.site\.test/(.*)$", "replacement": "http://www.site.test/$1"},
            {"pattern": r"(\.test/.+)/$", "replacement": "$1"},
            {"pattern": "x(", "replacement": ""},
        ], "input": "http://staging.site.test/shop/a/"}),
    )
    .await
    .unwrap();
    let t: RuleTrace = serde_json::from_value(v).unwrap();
    assert_eq!(t.output, "http://www.site.test/shop/a");
    assert_eq!(t.included, None);
    assert_eq!(t.steps.len(), 3);
    assert_eq!(t.steps[0].after, "http://www.site.test/shop/a/");
    assert!(t.steps[0].matched && t.steps[1].matched);
    assert!(!t.steps[2].matched);
    assert!(t.steps[2].error.as_deref().unwrap().contains("position"));
}

// spec: OUT-146
#[tokio::test(flavor = "multi_thread")]
async fn invalid_mapping_is_rejected_with_its_position() {
    let (old, new) = (staging().await, live().await);
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    let err = call(
        &app,
        "compare.open",
        json!({"current": new.file.path(), "previous": old.file.path(),
               "mapping": [{"pattern": "ok", "replacement": ""}, {"pattern": "a[b", "replacement": ""}],
               "elements": []}),
    )
    .await
    .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert_eq!(err.field.as_deref(), Some("mapping[1].pattern"));
    assert_eq!(err.args["rule"], 1);
    assert!(err.args["position"].is_u64());
    // Gültiges Mapping: Staging-URLs gelten als Live-URLs.
    let s = open(
        &app,
        new.file.path(),
        old.file.path(),
        json!([{"pattern": r"^http://staging\.site\.test", "replacement": "http://www.site.test"},
               {"pattern": r"(\.test/.+)/$", "replacement": "$1"}]),
    )
    .await;
    assert_eq!(s.mapping.len(), 2);
    let window = WindowId("main".into());
    let new_pages = os_app::commands::compare::with_active(&app, &window, |a| {
        Ok(a.comparison
            .filter("Internal", "HTML")
            .unwrap()
            .class(CompareClass::New))
    })
    .await
    .unwrap();
    assert_eq!(new_pages, Some(0));
}

// spec: OUT-140
#[tokio::test(flavor = "multi_thread")]
async fn invalid_options_are_rejected() {
    let (old, new) = (v1().await, v2().await);
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    for (args, field) in [
        (json!({"contentThresholdPct": 101}), "contentThresholdPct"),
        (json!({"elements": ["nope"]}), "elements"),
    ] {
        let mut full = json!({"current": new.file.path(), "previous": old.file.path(), "mapping": [], "elements": []});
        for (k, v) in args.as_object().unwrap() {
            full[k] = v.clone();
        }
        let err = call(&app, "compare.open", full).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert_eq!(err.field.as_deref(), Some(field));
    }
}

// spec: OUT-140
// Ältere Dateien werden als Kopie im Cache migriert; „Schließen“ entfernt die Kopien, die Originale bleiben unverändert.
#[tokio::test(flavor = "multi_thread")]
async fn close_ends_the_session_and_removes_migrated_copies() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/crawl-files");
    let dir = tempfile::tempdir().unwrap();
    let mut originals = Vec::new();
    for name in ["schema-v1.ospider", "schema-v2.ospider"] {
        let p = dir.path().join(name);
        std::fs::copy(fixtures.join(name), &p).unwrap();
        originals.push((p.clone(), std::fs::read(&p).unwrap()));
    }
    let home = tempfile::tempdir().unwrap();
    let app = app(home.path());
    open(&app, &originals[0].0, &originals[1].0, json!([])).await;
    let work = app.config().paths.cache.join("compare");
    let copies = |dir: &Path| {
        std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "ospider")
            })
            .count()
    };
    assert_eq!(copies(&work), 2);
    call(&app, "compare.close", json!(null)).await.unwrap();
    let left: Vec<_> = std::fs::read_dir(&work).unwrap().collect();
    assert!(left.is_empty(), "{left:?}");
    for (p, bytes) in &originals {
        assert_eq!(&std::fs::read(p).unwrap(), bytes);
    }
    let err = call(&app, "compare.swap", json!(null)).await.unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    // Schließen ohne Sitzung ist kein Fehler.
    call(&app, "compare.close", json!(null)).await.unwrap();
}
