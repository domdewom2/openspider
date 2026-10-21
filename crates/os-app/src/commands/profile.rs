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
use std::path::Path;

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
        recent_rank: store
            .recent()
            .map_err(|e| store_error(&e))?
            .iter()
            .position(|n| n == name)
            .map(|i| i as u32),
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
        #[derive(serde::Deserialize)]
        struct ImportArgs {
            path: String,
            password: Option<String>,
        }
        let ImportArgs { path, password } = parse_args(args)?;
        let secrets = app.config().secrets.clone();
        let store = store(app);
        let loaded = app
            .write_queue()
            .submit_admin(move || {
                let path = Path::new(&path);
                let name = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| CmdError::invalid_input("err-profile-name", "path"))?;
                let (loaded, credentials) = read_export(path, password.as_deref())?;
                valid(&loaded.config)?;
                // Read every previous value before making any changes.
                let previous = credentials
                    .keys()
                    .map(|key| {
                        secrets
                            .try_get(key)
                            .map(|value| (key.clone(), value))
                            .map_err(|_| CmdError::new(ErrorCode::Io, "err-profile-secrets"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                store
                    .save(name, &loaded.config, false)
                    .map_err(|e| match e {
                        // profile.import hat keine Überschreiboption (3.6a).
                        os_store::StoreError::AlreadyExists(_) => {
                            CmdError::invalid_input("err-file-exists", "path")
                                .with_arg("reason", "exists")
                        }
                        _ => store_error(&e),
                    })?;
                let mut attempted = 0;
                let result = (|| {
                    for (key, value) in &credentials {
                        attempted += 1;
                        secrets
                            .set(key, value)
                            .map_err(|_| CmdError::new(ErrorCode::Io, "err-profile-secrets"))?;
                    }
                    Ok((loaded, info(&store, name)?))
                })();
                if result.is_err() {
                    let mut rollback_failed = false;
                    // Include the failed write: a backend may mutate before returning an error.
                    for (key, value) in previous[..attempted].iter().rev() {
                        let restored = match value {
                            Some(value) => secrets.set(key, value),
                            None => secrets.delete(key),
                        };
                        rollback_failed |= restored.is_err();
                    }
                    rollback_failed |= store.delete(name).is_err();
                    if rollback_failed {
                        return Err(CmdError::new(ErrorCode::Io, "err-profile-import-rollback"));
                    }
                }
                result
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
        if !Path::new(&args.path).is_absolute() {
            return Err(CmdError::invalid_input("err-export-absolute-path", "path"));
        }
        let store = store(app);
        let secrets = app.config().secrets.clone();
        app.write_queue()
            .submit_admin(move || export_file(&store, secrets.as_ref(), args))
            .await
    })
}

/// File marker of a password-protected profile export (INTEG-002); followed by the `OSAPIP1` envelope.
const SECRET_EXPORT_MAGIC: &[u8] = b"OSCFGSEC1\n";

fn export_file(
    store: &ConfigProfiles,
    secrets: &dyn os_fetch::SecretStore,
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
        // INTEG-002: the explicit consent is `includeSecrets`; configuration and API credentials are sealed together
        // with Argon2id + AES-256-GCM, so the file never contains a plaintext secret.
        let io = || {
            CmdError::new(ErrorCode::Io, "err-profile-export").with_arg("path", args.path.as_str())
        };
        let mut stored = serde_json::Map::new();
        for kind in os_model::ApiKind::ALL {
            let id = format!("api/{kind}");
            if let Some(value) = secrets.try_get(&id).map_err(|_| io())? {
                stored.insert(id, Value::String(value));
            }
        }
        let plain = serde_json::to_vec(&serde_json::json!({
            "config": serde_json::to_value(&cfg).map_err(|_| io())?,
            "secrets": stored,
        }))
        .map_err(|_| io())?;
        let envelope = os_integrations::secret_export::encrypt_with_password(
            &plain,
            args.password.as_deref().unwrap_or_default(),
        )
        .map_err(|_| io())?;
        let mut bytes = SECRET_EXPORT_MAGIC.to_vec();
        bytes.extend_from_slice(&envelope);
        std::fs::write(path, bytes).map_err(|_| io())?;
        return Ok(Value::Null);
    }
    cfg.save_file(path).map_err(|_| {
        CmdError::new(ErrorCode::Io, "err-profile-export").with_arg("path", args.path)
    })?;
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

/// Password-protected profile reader for import before CLI/scheduling use.
/// Credentials are returned separately for installation in the caller's SecretStore.
pub fn read_export(
    path: &Path,
    password: Option<&str>,
) -> Result<
    (
        os_model::LoadedConfig,
        std::collections::BTreeMap<String, String>,
    ),
    CmdError,
> {
    let bytes =
        std::fs::read(path).map_err(|_| CmdError::new(ErrorCode::Io, "err-profile-read"))?;
    let invalid = || CmdError::invalid_input("err-profile-format", "path");
    if let Some(envelope) = bytes.strip_prefix(SECRET_EXPORT_MAGIC) {
        let password = password
            .filter(|p| !p.is_empty())
            .ok_or_else(|| CmdError::invalid_input("err-export-password-required", "password"))?;
        let plain = os_integrations::secret_export::decrypt_with_password(envelope, password)
            .map_err(|_| {
                CmdError::invalid_input("err-profile-password-or-integrity", "password")
            })?;
        #[derive(serde::Deserialize)]
        struct Payload {
            config: Value,
            secrets: std::collections::BTreeMap<String, String>,
        }
        let payload: Payload = serde_json::from_slice(&plain).map_err(|_| invalid())?;
        let loaded = os_model::LoadedConfig::from_config(
            serde_json::from_value(payload.config).map_err(|_| invalid())?,
        );
        valid(&loaded.config)?;
        if payload.secrets.keys().any(|key| {
            !os_model::ApiKind::ALL
                .iter()
                .any(|kind| key == &format!("api/{kind}"))
        }) {
            return Err(invalid());
        }
        Ok((loaded, payload.secrets))
    } else {
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
        Ok((loaded, Default::default()))
    }
}
