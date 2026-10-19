//! settings.get/set (OUT-222).
//!
//! `settings.get` liefert die App-Einstellungen (`settings.toml`) als `AppSettingsDto`; `settings.set` wendet einen
//! JSON-Merge-Patch an, schreibt die Datei atomar, meldet `settingsChanged` an alle Fenster und nennt in
//! `restartRequired` die Schlüssel, deren Änderung einen Neustart erfordert.

use futures_util::future::BoxFuture;
use os_model::settings::AppSettings;
use std::{collections::BTreeSet, sync::Mutex};

use crate::dto::{CmdError, ErrorCode, SettingsPatch};
use crate::registry::{CmdResult, CommandMeta, Registry};
use crate::runtime::error::{parse_args, to_json};
use crate::runtime::{App, CallCtx};

#[derive(Default)]
struct PendingRestarts {
    previous: Option<AppSettings>,
    keys: BTreeSet<String>,
}

/// Retains pending requirements for the lifetime of the app, across settings windows.
pub fn on_startup(app: &App) {
    app.ext::<Mutex<PendingRestarts>>()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .previous = Some(app.settings_snapshot());
}

/// Additive service hook; existing settings.get/set response contracts stay unchanged.
pub fn on_settings_changed(app: &App) {
    let current = app.settings_snapshot();
    let state = app.ext::<Mutex<PendingRestarts>>();
    let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(previous) = state.previous.replace(current.clone()) {
        let changed = previous.changed_keys(&current);
        state.keys.extend(
            AppSettings::restart_required(changed.iter().map(String::as_str))
                .into_iter()
                .map(String::from),
        );
    }
}

fn get<'a>(app: &'a App, _ctx: CallCtx, _args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move { to_json(&app.settings().dto(&[])) })
}

fn set<'a>(app: &'a App, _ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let SettingsPatch { patch } = parse_args(args)?;
        let app = app.clone();
        // Dateizugriff nicht auf einem Tokio-Worker
        let dto = tokio::task::spawn_blocking(move || app.apply_settings_patch(&patch))
            .await
            .map_err(|e| {
                CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
            })??;
        to_json(&dto)
    })
}

/// Read-only hardware snapshot for the settings UI (UI-123, OUT-267).
fn resources<'a>(app: &'a App, _: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
    let storage = app.settings_snapshot().storage;
    let restart_required: Vec<_> = app
        .ext::<Mutex<PendingRestarts>>()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .keys
        .iter()
        .cloned()
        .collect();
    Box::pin(async move {
        tokio::task::spawn_blocking(move || {
            let sample = os_engine::memory::sample();
            let cores = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
            let (ram, cache, renderers) =
                os_engine::memory::limits(&storage, sample.physical_bytes, cores);
            Ok(serde_json::json!({
                "physicalBytes": sample.physical_bytes,
                "usedBytes": sample.engine_bytes.saturating_add(sample.renderer_bytes),
                "ramLimitBytes": ram,
                "cacheLimitMb": cache >> 20,
                "renderProcessLimit": renderers,
                "restartRequired": restart_required,
            }))
        })
        .await
        .map_err(|e| {
            CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
        })?
    })
}

/// Persists explicitly accepted PEM certificates, after validating the entire chain.
fn import_certificates<'a>(
    app: &'a App,
    _: CallCtx,
    args: serde_json::Value,
) -> BoxFuture<'a, CmdResult> {
    let directory = app.config().home.join("certificates");
    let id = app.config().ids.new_crawl_id(app.config().clock.now());
    Box::pin(async move {
        let certificates: Vec<String> =
            parse_args(args.get("certificates").cloned().unwrap_or(args))?;
        tokio::task::spawn_blocking(move || {
            if certificates.is_empty()
                || certificates.len() > 16
                || certificates.iter().any(|c| c.len() > 1024 * 1024)
            {
                return Err(CmdError::invalid_input(
                    "err-invalid-certificate",
                    "network.trustedCertificates",
                ));
            }
            let io = |e: std::io::Error| {
                CmdError::new(ErrorCode::Io, "err-io").with_detail(e.to_string())
            };
            std::fs::create_dir_all(&directory).map_err(io)?;
            let path = directory.join(format!("{id}.pem"));
            std::fs::write(&path, certificates.join("\n")).map_err(io)?;
            let network = os_model::settings::NetworkSettings {
                trusted_certificates: vec![path.to_string_lossy().into_owned()],
                ..Default::default()
            };
            if let Err(detail) = os_fetch::net::certificates::roots(&network) {
                let _ = std::fs::remove_file(&path);
                return Err(CmdError::invalid_input(
                    "err-invalid-certificate",
                    "network.trustedCertificates",
                )
                .with_detail(detail));
            }
            to_json(&network.trusted_certificates)
        })
        .await
        .map_err(|e| {
            CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
        })?
    })
}

/// Trägt die Commands dieser Domäne in die Registry ein.
pub fn register(r: &mut Registry) {
    r.register(
        CommandMeta {
            name: "settings.resources",
            args: "void",
            ret: "JsonValue",
            long_running: false,
        },
        resources,
    );
    r.register(
        CommandMeta {
            name: "settings.importCertificates",
            args: "JsonValue",
            ret: "string[]",
            long_running: false,
        },
        import_certificates,
    );
    r.register(
        CommandMeta {
            name: "settings.get",
            args: "void",
            ret: "AppSettingsDto",
            long_running: false,
        },
        get,
    );
    r.register(
        CommandMeta {
            name: "settings.set",
            args: "SettingsPatch",
            ret: "AppSettingsDto",
            long_running: false,
        },
        set,
    );
}

#[cfg(test)]
mod u14_tests {
    use super::*;
    use os_fetch::secrets::MemorySecretStore;
    use std::sync::Arc;

    fn app(dir: &std::path::Path) -> App {
        let mut config = crate::AppConfig::new(dir);
        config.secrets = Arc::new(MemorySecretStore::new());
        App::new(config).unwrap()
    }

    // spec: OUT-222
    #[tokio::test]
    async fn pending_restart_survives_get_profile_changes_and_windows_until_core_restart() {
        let dir = tempfile::tempdir().unwrap();
        let running = app(dir.path());
        let ctx = || CallCtx::for_window("reopened-settings");
        let snapshot = |app: App| async move {
            app.dispatch(ctx(), "settings.resources", serde_json::Value::Null)
                .await
                .unwrap()["restartRequired"]
                .clone()
        };
        assert_eq!(snapshot(running.clone()).await, serde_json::json!([]));
        running
            .apply_settings_patch(
                &serde_json::json!({"storage": {"crawlsDir": "/fixture/changed"}}),
            )
            .unwrap();
        let get = running
            .dispatch(ctx(), "settings.get", serde_json::Value::Null)
            .await
            .unwrap();
        assert_eq!(get["restartRequired"], serde_json::json!([]));
        running
            .apply_settings_patch(
                &serde_json::json!({"storage": {"performanceProfile": "min_memory"}}),
            )
            .unwrap();
        assert_eq!(
            snapshot(running.clone()).await,
            serde_json::json!(["storage.crawls_dir"])
        );
        drop(running);
        let restarted = app(dir.path());
        assert_eq!(
            restarted.settings_snapshot().storage.crawls_dir.as_deref(),
            Some("/fixture/changed")
        );
        assert_eq!(snapshot(restarted).await, serde_json::json!([]));
    }

    // spec: UI-123
    #[tokio::test]
    async fn settings_resources_reports_live_limits_without_changing_settings() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let before = app.settings_snapshot();
        let result = app
            .dispatch(
                CallCtx::for_window("settings"),
                "settings.resources",
                serde_json::Value::Null,
            )
            .await
            .unwrap();
        assert!(result["physicalBytes"].as_u64().unwrap() > 0);
        assert!(result["ramLimitBytes"].as_u64().unwrap() >= 1 << 30);
        assert!(result["cacheLimitMb"].as_u64().unwrap() >= 512);
        assert!(result["renderProcessLimit"].as_u64().unwrap() >= 1);
        assert_eq!(before, app.settings_snapshot());
    }

    // spec: OUT-270
    #[tokio::test]
    async fn explicitly_imported_fixture_ca_is_valid_and_invalid_input_leaves_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let app = app(dir.path());
        let pem = include_str!(
            "../../../../apps/desktop/src/features/settings/sections/network/fixtures/ca.pem.txt"
        );
        let paths = app
            .dispatch(
                CallCtx::for_window("settings"),
                "settings.importCertificates",
                serde_json::json!({ "certificates": [pem] }),
            )
            .await
            .unwrap();
        let path = paths[0].as_str().unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), pem);
        // Discovering/importing a chain never silently changes trust settings.
        assert!(app
            .settings_snapshot()
            .network
            .trusted_certificates
            .is_empty());
        let error = app
            .dispatch(
                CallCtx::for_window("settings"),
                "settings.importCertificates",
                serde_json::json!({ "certificates": ["invalid"] }),
            )
            .await
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(
            std::fs::read_dir(dir.path().join("certificates"))
                .unwrap()
                .count(),
            1
        );
    }
}
