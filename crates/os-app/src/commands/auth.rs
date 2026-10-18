//! Encrypted authentication profiles. Replies expose metadata only (OUT-223, CRAWL-121).
use crate::{
    dto::*,
    registry::{CmdResult, CommandMeta, Registry},
    runtime::{
        error::{parse_args, to_json},
        App, CallCtx,
    },
};
use futures_util::future::BoxFuture;
use os_store::auth_profiles::{self, Protection};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, sync::Mutex};

#[derive(Default)]
struct Profiles(Mutex<()>);
fn error(e: impl std::fmt::Display) -> CmdError {
    CmdError::new(ErrorCode::InvalidInput, "err-auth-profile").with_detail(e.to_string())
}
fn dir(app: &App) -> PathBuf {
    app.config().paths.settings.join("auth")
}
fn path(app: &App, name: &str) -> Result<PathBuf, CmdError> {
    if name.trim().is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
        return Err(error("Profilname muss 1 bis 120 Byte lang sein"));
    }
    let encoded: String = name.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    Ok(dir(app).join(format!("{encoded}.osauth")))
}
fn read(app: &App, name: &str) -> Result<Value, CmdError> {
    let bytes = fs::read(path(app, name)?).map_err(error)?;
    auth_profiles::open(&bytes, None, |id| app.config().secrets.get(id)).map_err(error)
}
fn metadata(v: &Value) -> Result<AuthProfileInfo, CmdError> {
    let input: AuthProfileInput = serde_json::from_value(v["profile"].clone())
        .map_err(|_| error("Ungültiger Profilinhalt"))?;
    Ok(AuthProfileInfo {
        name: input.name,
        kind: input.kind,
        host: input.host,
        created_at: v["createdAt"]
            .as_str()
            .ok_or_else(|| error("Erstellungszeit fehlt"))?
            .into(),
        has_secret: input.secret.is_some(),
    })
}
fn save(
    app: &App,
    input: AuthProfileInput,
    created: Option<String>,
) -> Result<AuthProfileInfo, CmdError> {
    let dest = path(app, &input.name)?;
    if input.host.trim().is_empty()
        || input.host.contains('@')
        || input.host.contains('?')
        || input.host.chars().any(char::is_control)
    {
        return Err(error("Host ohne Zugangsdaten und Query erforderlich"));
    }
    let id = format!("auth/{}", input.name);
    let secrets = &app.config().secrets;
    let key = match secrets.try_get(&id).map_err(error)? {
        Some(key) => key,
        None => {
            let key = auth_profiles::new_key();
            secrets.set(&id, &key).map_err(error)?;
            key
        }
    };
    let mut input = input;
    let previous = if dest.exists() {
        Some(read(app, &input.name)?)
    } else {
        None
    };
    if input.secret.is_none() {
        input.secret = previous
            .as_ref()
            .and_then(|p| p["profile"]["secret"].as_str().map(str::to_owned));
    }
    let created = previous
        .as_ref()
        .and_then(|p| p["createdAt"].as_str().map(str::to_owned))
        .or(created)
        .unwrap_or_else(|| app.config().clock.now().to_db());
    let value = json!({"profile":input,"createdAt":created});
    let bytes = auth_profiles::seal(
        &value,
        Protection::Machine {
            key_ref: &id,
            key: &key,
        },
    )
    .map_err(error)?;
    fs::create_dir_all(dir(app)).map_err(error)?;
    // Same-directory atomic replacement; a failed write never truncates the previous profile.
    let tmp = dest.with_file_name(format!(".profile-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &dest)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result.map_err(error)?;
    metadata(&value)
}

fn list<'a>(app: &'a App, _: CallCtx, _: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let state = app.ext::<Profiles>();
        let _guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        let mut infos = Vec::new();
        if dir(app).exists() {
            for entry in fs::read_dir(dir(app)).map_err(error)? {
                let entry = entry.map_err(error)?;
                if entry.file_type().map_err(error)?.is_file()
                    && entry.path().extension().is_some_and(|e| e == "osauth")
                {
                    let bytes = fs::read(entry.path()).map_err(error)?;
                    let v = auth_profiles::open(&bytes, None, |id| app.config().secrets.get(id))
                        .map_err(error)?;
                    infos.push(metadata(&v)?);
                }
            }
        }
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        to_json(&infos)
    })
}
fn save_command<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: AuthSaveProfileArgs = parse_args(args)?;
        let state = app.ext::<Profiles>();
        let _guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        to_json(&save(app, a.profile, None)?)
    })
}
fn delete<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: NameArgs = parse_args(args)?;
        let state = app.ext::<Profiles>();
        let _guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        let dest = path(app, &a.name)?;
        match fs::remove_file(dest) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(error(e)),
        }
        app.config()
            .secrets
            .delete(&format!("auth/{}", a.name))
            .map_err(error)?;
        Ok(Value::Null)
    })
}
fn export<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: AuthExportArgs = parse_args(args)?;
        let state = app.ext::<Profiles>();
        let _guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        let v = read(app, &a.name)?;
        let id = format!("auth/{}", a.name);
        let key = app
            .config()
            .secrets
            .try_get(&id)
            .map_err(error)?
            .ok_or_else(|| error("Rechnerschlüssel fehlt"))?;
        let protection = if a.bind_to_machine {
            Protection::Machine {
                key_ref: &id,
                key: &key,
            }
        } else {
            Protection::Passphrase(
                a.passphrase
                    .as_deref()
                    .ok_or_else(|| error("Passphrase erforderlich"))?,
            )
        };
        let bytes = auth_profiles::seal(&v, protection).map_err(error)?;
        if PathBuf::from(&a.path).starts_with(dir(app)) {
            return Err(error("Exportziel liegt im Profilordner"));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&a.path)
            .map_err(error)?;
        use std::io::Write;
        file.write_all(&bytes).map_err(error)?;
        to_json(&FileResult {
            path: a.path,
            bytes: bytes.len() as u64,
        })
    })
}
fn import<'a>(app: &'a App, _: CallCtx, args: Value) -> BoxFuture<'a, CmdResult> {
    Box::pin(async move {
        let a: AuthImportArgs = parse_args(args)?;
        let state = app.ext::<Profiles>();
        let _guard = state.0.lock().unwrap_or_else(|p| p.into_inner());
        let bytes = fs::read(&a.path).map_err(error)?;
        let v = auth_profiles::open(&bytes, a.passphrase.as_deref(), |id| {
            app.config().secrets.get(id)
        })
        .map_err(error)?;
        metadata(&v)?;
        let input: AuthProfileInput =
            serde_json::from_value(v["profile"].clone()).map_err(|_| error("Ungültiges Profil"))?;
        to_json(&save(
            app,
            input,
            v["createdAt"].as_str().map(str::to_owned),
        )?)
    })
}
pub fn register(r: &mut Registry) {
    for (name, args, ret, h) in [
        (
            "auth.importProfile",
            "AuthImportArgs",
            "AuthProfileInfo",
            import as crate::registry::Handler,
        ),
        ("auth.exportProfile", "AuthExportArgs", "FileResult", export),
        ("auth.listProfiles", "void", "AuthProfileInfo[]", list),
        (
            "auth.saveProfile",
            "AuthSaveProfileArgs",
            "AuthProfileInfo",
            save_command,
        ),
        ("auth.deleteProfile", "NameArgs", "void", delete),
    ] {
        r.register(
            CommandMeta {
                name,
                args,
                ret,
                long_running: false,
            },
            h,
        );
    }
}
