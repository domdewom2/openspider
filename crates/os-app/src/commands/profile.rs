//! Benannte Konfigurationsprofile (OUT-220/221); Laden wendet noch keine Werte an.
use super::config::{decode, dto, valid, warn_loaded};
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, store_error, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;
use os_model::{CrawlConfig, CrawlState};
use os_store::profile::ConfigProfiles;
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

fn store(app: &App) -> ConfigProfiles {
    ConfigProfiles::new(&app.config().paths.settings)
}
fn info(store: &ConfigProfiles, name: &str) -> Result<ConfigProfileInfo, CmdError> {
    let path = store.path(name).map_err(|e| store_error(&e))?;
    let modified = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .map_err(|_| CmdError::new(ErrorCode::Io, "err-profile-metadata"))?;
    let stamp: time::OffsetDateTime = modified.into();
    Ok(ConfigProfileInfo {
        name: name.into(),
        is_default: store
            .default_name()
            .map_err(|e| store_error(&e))?
            .as_deref()
            == Some(name),
        modified_at: os_model::Timestamp(stamp).to_db(),
        path: path.to_string_lossy().into_owned(),
    })
}
fn list<'a>(app: &'a App, _: CallCtx, _: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let store = store(app);
        app.write_queue()
            .submit_admin(move || {
                let entries: Result<Vec<_>, _> = store
                    .names()
                    .map_err(|e| store_error(&e))?
                    .iter()
                    .map(|n| info(&store, n))
                    .collect();
                to_json(&entries?)
            })
            .await
    })
}
fn load<'a>(app: &'a App, ctx: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let NameArgs { name } = parse_args(args)?;
        if app
            .sessions()
            .current(&ctx.window)
            .is_some_and(|s| s.state == CrawlState::Running)
        {
            let mut e = CmdError::new(ErrorCode::CrawlRunning, "err-config-pause-required");
            e.field = Some("profile".into());
            return Err(e);
        }
        let store = store(app);
        let loaded = app
            .write_queue()
            .submit_admin(move || {
                let loaded = store.load(&name).map_err(|e| store_error(&e))?;
                valid(&loaded.config)?;
                store.remember_loaded(&name).map_err(|e| store_error(&e))?;
                Ok(loaded)
            })
            .await?;
        warn_loaded(app, &loaded);
        to_json(&dto(&loaded.config)?)
    })
}
fn save<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ProfileSaveArgs = parse_args(args)?;
        let cfg = decode(args.config)?;
        valid(&cfg)?;
        let store = store(app);
        app.write_queue()
            .submit_admin(move || {
                store
                    .save(&args.name, &cfg, args.overwrite)
                    .map_err(|e| store_error(&e))?;
                to_json(&info(&store, &args.name)?)
            })
            .await
    })
}
fn delete<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let NameArgs { name } = parse_args(args)?;
        let store = store(app);
        app.write_queue()
            .submit_admin(move || {
                store.delete(&name).map_err(|e| store_error(&e))?;
                Ok(Value::Null)
            })
            .await
    })
}
fn set_default<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let SetDefaultProfileArgs { name } = parse_args(args)?;
        let store = store(app);
        app.write_queue()
            .submit_admin(move || {
                if let Some(name) = &name {
                    valid(&store.load(name).map_err(|e| store_error(&e))?.config)?;
                }
                store
                    .set_default(name.as_deref())
                    .map_err(|e| store_error(&e))?;
                Ok(Value::Null)
            })
            .await
    })
}
fn import<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let PathArgs { path } = parse_args(args)?;
        let store = store(app);
        let loaded = app
            .write_queue()
            .submit_admin(move || {
                let path = Path::new(&path);
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| CmdError::invalid_input("err-profile-name", "path"))?;
                let loaded = CrawlConfig::load_file(path).map_err(|e| {
                    let mut error = CmdError::invalid_input(
                        "err-profile-format",
                        e.field.unwrap_or_else(|| "path".into()),
                    );
                    if let Some(line) = e.line {
                        error = error.with_arg("line", line);
                    }
                    if let Some(col) = e.column {
                        error = error.with_arg("column", col);
                    }
                    error
                })?;
                valid(&loaded.config)?;
                store
                    .save(name, &loaded.config, false)
                    .map_err(|e| store_error(&e))?;
                Ok((loaded, info(&store, name)?))
            })
            .await?;
        warn_loaded(app, &loaded.0);
        to_json(&loaded.1)
    })
}
fn export<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let args: ProfileExportArgs = parse_args(args)?;
        if args.include_secrets == Some(true) && args.password.as_ref().is_none_or(|p| p.is_empty())
        {
            return Err(CmdError::invalid_input(
                "err-export-password-required",
                "password",
            ));
        }
        let store = store(app);
        let secrets = app.config().secrets.clone();
        app.write_queue()
            .submit_admin(move || export_file(&store, secrets.as_ref(), args))
            .await
    })
}

fn export_file(
    store: &ConfigProfiles,
    secrets: &dyn os_fetch::SecretStoreMut,
    args: ProfileExportArgs,
) -> CmdResult {
    let cfg = store.load(&args.name).map_err(|e| store_error(&e))?.config;
    valid(&cfg)?;
    let path = Path::new(&args.path);
    // Keine Ausgabe darf ein verwaltetes Profil ersetzen: dort bleiben nur Verweise.
    if store.is_managed_path(path).map_err(|e| store_error(&e))? {
        return Err(CmdError::invalid_input(
            "err-export-profile-destination",
            "path",
        ));
    }
    if args.include_secrets == Some(true) {
        let mut values = BTreeMap::new();
        for kind in os_model::ApiKind::ALL {
            let id = format!("api/{kind}");
            if let Some(value) = secrets
                .try_get(&id)
                .map_err(|_| CmdError::new(ErrorCode::Permission, "err-secret-store"))?
            {
                values.insert(id, value);
            }
        }
        let payload = serde_json::json!({"config":cfg.to_osconfig_string().map_err(|_|CmdError::new(ErrorCode::Io,"err-profile-serialize"))?,"secrets":values});
        let encrypted = os_integrations::core::encrypt_config_export(
            payload.to_string().as_bytes(),
            args.password.as_deref().unwrap_or_default(),
        )
        .map_err(|_| CmdError::new(ErrorCode::Internal, "err-export-encrypt"))?;
        os_store::profile::write_export(path, &encrypted).map_err(|e| store_error(&e))?;
    } else {
        cfg.save_file(path).map_err(|_| {
            CmdError::new(ErrorCode::Io, "err-profile-export").with_arg("path", args.path)
        })?;
    }
    Ok(Value::Null)
}

/// Unveränderter IPC-Katalog.
pub fn register(r: &mut Registry) {
    for (name, args, ret, handler) in [
        (
            "profile.list",
            "void",
            "ConfigProfileInfo[]",
            list as crate::registry::Handler,
        ),
        ("profile.load", "NameArgs", "CrawlConfigDto", load),
        ("profile.save", "ProfileSaveArgs", "ConfigProfileInfo", save),
        ("profile.delete", "NameArgs", "void", delete),
        (
            "profile.setDefault",
            "SetDefaultProfileArgs",
            "void",
            set_default,
        ),
        ("profile.import", "PathArgs", "ConfigProfileInfo", import),
        ("profile.export", "ProfileExportArgs", "void", export),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            handler,
        );
    }
}
