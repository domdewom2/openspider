//! UI-Zustand (UI-125): `uistate.load`/`uistate.save`, unveränderter Inhalt, atomar mit `.bak`.

#[path = "runtime_common.rs"]
mod common;

use common::*;
use os_app::dto::UiStateJson;
use os_app::{App, ErrorCode};
use serde_json::{json, Value};

async fn load(app: &App) -> UiStateJson {
    serde_json::from_value(
        app.dispatch(ctx(), "uistate.load", Value::Null)
            .await
            .unwrap(),
    )
    .unwrap()
}

async fn save(app: &App, v: Value) -> Result<UiStateJson, os_app::CmdError> {
    app.dispatch(ctx(), "uistate.save", v)
        .await
        .map(|v| serde_json::from_value(v).unwrap())
}

// spec: UI-125
#[tokio::test]
async fn without_a_file_the_state_is_empty_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let s = load(&app).await;
    assert_eq!((s.version, s.state), (1, json!({})));
    assert!(!dir.path().join("ui-state.json").exists());
}

// spec: UI-125
#[tokio::test]
async fn save_stores_the_content_unchanged_and_load_returns_it() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let state = json!({
        "windows": { "main": { "x": 10, "y": 20, "w": 1280, "h": 800 } },
        "tabs": ["internal", "response-codes"],
        "history": [{ "url": "https://example.com/ä?x=1" }],
        "unbekannt": { "bleibt": [1, 2.5, null, true] }
    });
    let saved = save(&app, json!({ "version": 3, "state": state }))
        .await
        .unwrap();
    assert_eq!((saved.version, &saved.state), (3, &state));
    drop(app);
    // nach einem Neustart
    let app = new_app(dir.path());
    let s = load(&app).await;
    assert_eq!((s.version, s.state), (3, state));
}

// spec: UI-125
#[tokio::test]
async fn the_previous_version_becomes_the_backup() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    save(&app, json!({ "version": 1, "state": { "n": 1 } }))
        .await
        .unwrap();
    assert!(
        !dir.path().join("ui-state.json.bak").exists(),
        "erste Speicherung: noch keine Sicherung"
    );
    save(&app, json!({ "version": 1, "state": { "n": 2 } }))
        .await
        .unwrap();
    save(&app, json!({ "version": 1, "state": { "n": 3 } }))
        .await
        .unwrap();
    let bak: UiStateJson =
        serde_json::from_slice(&std::fs::read(dir.path().join("ui-state.json.bak")).unwrap())
            .unwrap();
    assert_eq!(bak.state, json!({ "n": 2 }));
    assert_eq!(load(&app).await.state, json!({ "n": 3 }));
    // kein temporärer Rest
    assert!(std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().contains(".tmp")));
}

// spec: UI-125
#[tokio::test]
async fn a_damaged_file_falls_back_to_the_backup_then_to_an_empty_state() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    save(&app, json!({ "version": 2, "state": { "ok": true } }))
        .await
        .unwrap();
    save(&app, json!({ "version": 2, "state": { "ok": "neu" } }))
        .await
        .unwrap();
    // Datei kaputt (z. B. Absturz mitten im Schreiben eines anderen Werkzeugs)
    std::fs::write(dir.path().join("ui-state.json"), b"{ \"version\": 2, \"sta").unwrap();
    assert_eq!(load(&app).await.state, json!({ "ok": true }));
    // auch die Sicherung kaputt
    std::fs::write(dir.path().join("ui-state.json.bak"), b"nope").unwrap();
    let s = load(&app).await;
    assert_eq!((s.version, s.state), (1, json!({})));
    // Datei fehlt, Sicherung vorhanden (Absturz zwischen den beiden Umbenennungen)
    std::fs::remove_file(dir.path().join("ui-state.json")).unwrap();
    std::fs::write(
        dir.path().join("ui-state.json.bak"),
        br#"{"version":2,"state":{"aus":"bak"}}"#,
    )
    .unwrap();
    assert_eq!(load(&app).await.state, json!({ "aus": "bak" }));
}

// spec: UI-125
#[tokio::test]
async fn invalid_input_is_rejected_and_leaves_the_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    save(&app, json!({ "version": 1, "state": { "keep": 1 } }))
        .await
        .unwrap();
    for bad in [
        json!({ "version": 0, "state": {} }),
        json!({ "version": 1, "state": [1, 2] }),
        json!({ "version": 1, "state": "text" }),
        json!({ "state": {} }),
        json!(null),
    ] {
        let e = save(&app, bad.clone()).await.unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidInput, "{bad}");
    }
    let huge =
        json!({ "version": 1, "state": { "x": "a".repeat(os_app::uistate::MAX_UI_STATE_BYTES) } });
    let e = save(&app, huge).await.unwrap_err();
    assert_eq!(e.message_key, "err-uistate-too-large");
    assert_eq!(load(&app).await.state, json!({ "keep": 1 }));
}

// spec: UI-125
#[tokio::test]
async fn an_unwritable_folder_is_an_io_or_permission_error() {
    let dir = tempfile::tempdir().unwrap();
    // der Einstellungsordner ist eine Datei → create_dir_all schlägt fehl
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"x").unwrap();
    let mut config = os_app::AppConfig::new(dir.path());
    config.paths.settings = blocker.join("sub");
    let app = App::new(config).unwrap();
    // der Start gelingt trotzdem (Standardwerte), die Meldung kommt im ersten Tick
    if cfg!(unix) {
        // (unter Windows meldet das Betriebssystem „Pfad nicht gefunden“, das als fehlende Datei gilt)
        assert_eq!(
            app.assemble_tick().unwrap().notices[0].code,
            "settings-unreadable"
        );
    }
    let e = save(&app, json!({ "version": 1, "state": {} }))
        .await
        .unwrap_err();
    assert!(
        matches!(e.code, ErrorCode::Io | ErrorCode::Permission),
        "{e:?}"
    );
    assert_eq!(e.message_key, "err-uistate-io");
}
