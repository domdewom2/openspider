//! App-Einstellungen (OUT-222): `settings.get`/`settings.set`, Merge-Patch, atomares Schreiben von `settings.toml`,
//! Ereignis `settingsChanged`, Neustart-Kennzeichnung, Exportordner `<Dokumente>/OpenSpider/Exports`.

#[path = "runtime_common.rs"]
mod common;

use std::collections::BTreeSet;
use std::sync::Arc;

use common::*;
use os_app::dto::{AppInfo, AppSettingsDto, UiEvent};
use os_app::events::CollectingSink;
use os_app::runtime::info::effective_export_dir;
use os_app::{App, AppConfig, ErrorCode, WindowId};
use os_model::settings::AppSettings;
use serde_json::{json, Value};

async fn get(app: &App) -> AppSettingsDto {
    serde_json::from_value(
        app.dispatch(ctx(), "settings.get", Value::Null)
            .await
            .unwrap(),
    )
    .unwrap()
}

async fn set(app: &App, patch: Value) -> Result<AppSettingsDto, os_app::CmdError> {
    app.dispatch(ctx(), "settings.set", json!({ "patch": patch }))
        .await
        .map(|v| serde_json::from_value(v).unwrap())
}

fn read_file(dir: &std::path::Path) -> String {
    std::fs::read_to_string(dir.join("settings.toml")).unwrap()
}

// spec: OUT-222
#[tokio::test]
async fn without_a_file_the_defaults_apply_and_nothing_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let s = get(&app).await;
    assert_eq!(s.general.language, "system");
    assert_eq!(s.appearance.live_interval_ms, 500);
    assert!(s.restart_required.is_empty());
    assert!(
        !dir.path().join("settings.toml").exists(),
        "Lesen schreibt nichts"
    );
}

// spec: OUT-222
#[tokio::test]
async fn set_applies_a_merge_patch_and_writes_settings_toml_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let s = set(
        &app,
        json!({ "general": { "language": "de" }, "appearance": { "zoomPct": 125, "theme": "dark" } }),
    )
    .await
    .unwrap();
    assert_eq!(s.general.language, "de");
    assert_eq!(
        (s.appearance.zoom_pct, s.appearance.theme.as_str()),
        (125, "dark")
    );
    // nicht genannte Werte bleiben
    assert!(s.general.notifications);
    assert_eq!(s.appearance.live_interval_ms, 500);

    // die Datei trägt genau diese Werte in TOML-Schreibweise (snake_case), kein temporärer Rest bleibt liegen
    let text = read_file(dir.path());
    assert!(text.contains("language = \"de\""), "{text}");
    assert!(text.contains("zoom_pct = 125"), "{text}");
    let leftovers: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    // ein neuer Start liest sie wieder
    drop(app);
    let again = new_app(dir.path());
    let s = get(&again).await;
    assert_eq!(
        (s.general.language.as_str(), s.appearance.zoom_pct),
        ("de", 125)
    );
}

// spec: OUT-222
#[tokio::test]
async fn null_resets_a_value_to_its_default() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    set(
        &app,
        json!({ "export": { "directory": "/tmp/out" }, "appearance": { "zoomPct": 150 } }),
    )
    .await
    .unwrap();
    assert_eq!(
        get(&app).await.export.directory.as_deref(),
        Some("/tmp/out")
    );
    let s = set(
        &app,
        json!({ "export": { "directory": null }, "appearance": { "zoomPct": null } }),
    )
    .await
    .unwrap();
    assert_eq!(s.export.directory, None);
    assert_eq!(s.appearance.zoom_pct, 100);
}

// spec: OUT-222
#[tokio::test]
async fn invalid_values_and_unknown_keys_are_rejected_without_touching_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    set(&app, json!({ "appearance": { "zoomPct": 110 } }))
        .await
        .unwrap();
    let before = read_file(dir.path());

    // Wertebereich (AppSettings::validate)
    let e = set(&app, json!({ "appearance": { "zoomPct": 77 } }))
        .await
        .unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);
    assert_eq!(e.field.as_deref(), Some("appearance.zoom_pct"));
    let e = set(&app, json!({ "appearance": { "liveIntervalMs": 100 } }))
        .await
        .unwrap_err();
    assert_eq!(e.field.as_deref(), Some("appearance.live_interval_ms"));
    assert_eq!(e.message_key, "err-invalid-setting");

    // falscher Typ und unbekannter Aufzählungswert: das Blatt wird genannt
    let e = set(&app, json!({ "general": { "language": "klingonisch" } }))
        .await
        .unwrap_err();
    assert_eq!(e.field.as_deref(), Some("general.language"));
    let e = set(&app, json!({ "appearance": { "zebra": "ja" } }))
        .await
        .unwrap_err();
    assert_eq!(e.field.as_deref(), Some("appearance.zebra"));

    // unbekannte Schlüssel
    let e = set(&app, json!({ "appearance": { "farbe": 1 } }))
        .await
        .unwrap_err();
    assert_eq!(
        (e.message_key.as_str(), e.field.as_deref()),
        ("err-unknown-setting", Some("appearance.farbe"))
    );
    let e = set(&app, json!({ "nirgendwo": {} })).await.unwrap_err();
    assert_eq!(e.field.as_deref(), Some("nirgendwo"));

    // kein Objekt
    let e = set(&app, json!(5)).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidInput);

    assert_eq!(
        read_file(dir.path()),
        before,
        "abgelehnte Patches ändern die Datei nicht"
    );
    assert_eq!(get(&app).await.appearance.zoom_pct, 110);
}

// spec: OUT-222
#[tokio::test]
async fn a_whole_dto_can_be_sent_back_as_a_patch() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let mut dto = serde_json::to_value(get(&app).await).unwrap();
    dto["appearance"]["theme"] = json!("light");
    // restartRequired und schemaVersion sind Meta-Felder und werden ignoriert
    dto["schemaVersion"] = json!(99);
    let s = set(&app, dto).await.unwrap();
    assert_eq!(s.appearance.theme, "light");
}

// spec: OUT-222
#[tokio::test]
async fn changes_requiring_a_restart_are_marked_as_such() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let s = set(&app, json!({ "storage": { "crawlsDir": "/data/crawls" } }))
        .await
        .unwrap();
    assert_eq!(s.restart_required, vec!["storage.crawls_dir".to_string()]);
    // eine Einstellung ohne Neustart nennt nichts
    let s = set(&app, json!({ "general": { "notifications": false } }))
        .await
        .unwrap();
    assert!(s.restart_required.is_empty());
    // derselbe Wert noch einmal: keine Änderung, kein Eintrag
    let s = set(&app, json!({ "storage": { "crawlsDir": "/data/crawls" } }))
        .await
        .unwrap();
    assert!(s.restart_required.is_empty());
}

// spec: OUT-222
#[tokio::test]
async fn settings_changed_goes_to_every_window_but_only_for_real_changes() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let main = Arc::new(CollectingSink::new());
    let settings = Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), main.clone());
    app.subscribe(WindowId("settings".into()), settings.clone());

    set(
        &app,
        json!({ "appearance": { "accent": "#aa00ff", "zebra": false } }),
    )
    .await
    .unwrap();
    for sink in [&main, &settings] {
        match sink.take().as_slice() {
            [UiEvent::SettingsChanged { keys }] => {
                assert_eq!(
                    keys,
                    &vec![
                        "appearance.accent".to_string(),
                        "appearance.zebra".to_string()
                    ]
                );
            }
            other => panic!("{other:?}"),
        }
    }
    // unverändert → kein Ereignis, keine Schreibung
    let before = std::fs::metadata(dir.path().join("settings.toml"))
        .unwrap()
        .modified()
        .unwrap();
    set(&app, json!({ "appearance": { "zebra": false } }))
        .await
        .unwrap();
    assert!(main.is_empty() && settings.is_empty());
    assert_eq!(
        std::fs::metadata(dir.path().join("settings.toml"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
    // abgelehnt → kein Ereignis
    set(&app, json!({ "appearance": { "zoomPct": 1 } }))
        .await
        .unwrap_err();
    assert!(main.is_empty());
}

// spec: OUT-222
#[tokio::test]
async fn the_live_interval_setting_drives_the_tick_period_without_restart() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    assert_eq!(app.live_interval_ms(), 500);
    set(&app, json!({ "appearance": { "liveIntervalMs": 2000 } }))
        .await
        .unwrap();
    assert_eq!(app.live_interval_ms(), 2000);
}

// spec: OUT-222
#[tokio::test]
async fn an_unreadable_settings_file_is_kept_aside_and_defaults_apply() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.toml"), "das ist [ kein TOML").unwrap();
    let app = new_app(dir.path());
    assert_eq!(get(&app).await.appearance.zoom_pct, 100);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("settings.toml.broken")).unwrap(),
        "das ist [ kein TOML",
        "die Originaldatei bleibt erhalten"
    );
    assert!(!dir.path().join("settings.toml").exists());
    // der Hinweis erscheint im ersten Tick
    let t = app.assemble_tick().unwrap();
    assert_eq!(t.notices[0].code, "settings-reset");
}

// spec: OUT-222
#[tokio::test]
async fn external_changes_are_picked_up_by_reload() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), sink.clone());
    let mut s = AppSettings::default();
    s.appearance.zoom_pct = 90;
    s.save(dir.path()).unwrap();
    let changed = app.reload_settings().unwrap();
    assert_eq!(changed, vec!["appearance.zoom_pct".to_string()]);
    assert_eq!(get(&app).await.appearance.zoom_pct, 90);
    assert!(matches!(
        sink.take().as_slice(),
        [UiEvent::SettingsChanged { .. }]
    ));
    assert!(app.reload_settings().unwrap().is_empty());
}

// spec: OUT-222
#[test]
fn the_dto_mirrors_the_model_field_for_field() {
    let mut s = AppSettings::default();
    s.storage.ram_limit_gb = Some(4.5);
    s.retention.keep_last = Some(3);
    s.network.trusted_certificates = vec!["a.pem".into()];
    s.accounts.auto_connect = vec!["gsc".into()];
    let dto = os_app::settings_store::settings_to_dto(&s, &[]);
    let back = serde_json::to_value(&dto).unwrap();
    // alle Modellschlüssel (camelCase) kommen im DTO vor
    fn keys(prefix: &str, v: &Value, out: &mut BTreeSet<String>) {
        if let Value::Object(m) = v {
            for (k, c) in m {
                let camel = {
                    let mut o = String::new();
                    let mut up = false;
                    for ch in k.chars() {
                        if ch == '_' {
                            up = true;
                        } else if up {
                            o.extend(ch.to_uppercase());
                            up = false;
                        } else {
                            o.push(ch);
                        }
                    }
                    o
                };
                let p = if prefix.is_empty() {
                    camel
                } else {
                    format!("{prefix}.{camel}")
                };
                out.insert(p.clone());
                keys(&p, c, out);
            }
        }
    }
    let (mut model, mut dto_keys) = (BTreeSet::new(), BTreeSet::new());
    keys("", &serde_json::to_value(&s).unwrap(), &mut model);
    keys("", &back, &mut dto_keys);
    model.remove("schemaVersion");
    dto_keys.remove("restartRequired");
    assert_eq!(model, dto_keys);
    assert_eq!(dto.storage.ram_limit_gb, Some(4.5));
}

// --- Exportordner (OUT-222) -------------------------------------------------------------------------------------

// spec: OUT-222
#[test]
fn without_an_entry_the_export_folder_is_documents_openspider_exports() {
    let s = AppSettings::default();
    // gemockter OS-Ordnerdienst: macOS-Stil und Windows mit umgeleitetem Dokumente-Ordner
    let mac = std::path::Path::new("/Users/anna/Documents");
    let win = std::path::Path::new("D:\\OneDrive\\Dokumente");
    assert_eq!(
        effective_export_dir(&s, Some(mac)),
        mac.join("OpenSpider").join("Exports")
    );
    assert_eq!(
        effective_export_dir(&s, Some(win)),
        win.join("OpenSpider").join("Exports")
    );
    // ohne Angabe fragt die Funktion das Betriebssystem; das Ergebnis endet immer auf OpenSpider/Exports
    let os_default = effective_export_dir(&s, None);
    assert!(
        os_default.ends_with(std::path::Path::new("OpenSpider").join("Exports")),
        "{os_default:?}"
    );
}

// spec: OUT-222
#[test]
fn a_configured_export_folder_wins_and_a_blank_one_counts_as_unset() {
    let mut s = AppSettings::default();
    s.export.directory = Some("/mnt/berichte".into());
    assert_eq!(
        effective_export_dir(&s, None),
        std::path::PathBuf::from("/mnt/berichte")
    );
    s.export.directory = Some("  ".into());
    assert!(effective_export_dir(&s, Some(std::path::Path::new("/d"))).ends_with("Exports"));
}

// spec: OUT-222
#[tokio::test]
async fn a_changed_export_folder_applies_to_the_next_call_without_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let docs = dir.path().join("Docs");
    let mut config = AppConfig::new(dir.path());
    config.documents_dir = Some(docs.clone());
    let app = App::new(config).unwrap();
    let info = |app: &App| {
        let app = app.clone();
        async move {
            let v = app.dispatch(ctx(), "app.info", Value::Null).await.unwrap();
            serde_json::from_value::<AppInfo>(v).unwrap().paths.exports
        }
    };
    assert_eq!(
        info(&app).await,
        docs.join("OpenSpider")
            .join("Exports")
            .display()
            .to_string()
    );
    // der Ordner wird erst beim ersten Export angelegt, nicht beim Lesen der Einstellung
    assert!(!docs.exists());

    let target = dir.path().join("Berichte");
    let s = set(
        &app,
        json!({ "export": { "directory": target.display().to_string() } }),
    )
    .await
    .unwrap();
    assert!(
        s.restart_required.is_empty(),
        "der Exportordner braucht keinen Neustart"
    );
    assert_eq!(info(&app).await, target.display().to_string());
    assert_eq!(
        get(&app).await.export.directory,
        Some(target.display().to_string())
    );
}

// spec: OUT-222
#[test]
fn settings_and_crawl_configuration_share_no_leaf_keys() {
    // Eine .osconfig enthält keine Werte aus settings.toml: die Schlüsselpfade beider Formate sind disjunkt
    fn leaves(prefix: &str, v: &Value, out: &mut BTreeSet<String>) {
        match v {
            Value::Object(m) if !m.is_empty() => {
                for (k, c) in m {
                    let p = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    leaves(&p, c, out);
                }
            }
            _ => {
                out.insert(prefix.to_string());
            }
        }
    }
    let (mut a, mut b) = (BTreeSet::new(), BTreeSet::new());
    leaves(
        "",
        &serde_json::to_value(AppSettings::default()).unwrap(),
        &mut a,
    );
    leaves(
        "",
        &serde_json::to_value(os_model::CrawlConfig::default()).unwrap(),
        &mut b,
    );
    let shared: Vec<_> = a
        .intersection(&b)
        .filter(|k| !matches!(k.as_str(), "schema_version" | "generator"))
        .cloned()
        .collect();
    assert!(shared.is_empty(), "gemeinsame Schlüssel: {shared:?}");
}
