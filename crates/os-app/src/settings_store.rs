//! Laden und atomares Schreiben der App-Einstellungen (`settings.toml`) mit Merge-Patch (OUT-222).
//!
//! * Das Modell ist `os_model::settings::AppSettings` (TOML-Pfade in `snake_case`); die Oberfläche spricht
//!   `AppSettingsDto` in `camelCase`. [`SettingsStore::dto`] und [`SettingsStore::apply_patch`] übersetzen die
//!   Schlüssel.
//! * `settings.set` ist ein JSON-Merge-Patch (RFC 7396) über den DTO: Objekte werden zusammengeführt, `null` setzt
//!   einen Wert auf den Standard zurück. Unbekannte Schlüssel und Wertefehler sind `InvalidInput` mit dem TOML-Pfad
//!   in `field`; Wertebereiche prüft `AppSettings::validate`.
//! * Geschrieben wird nur bei einer tatsächlichen Änderung, atomar (temporäre Datei, dann Umbenennen). Eine
//!   unlesbare Datei wird nie überschrieben: sie wird nach `settings.toml.broken` verschoben, die App startet mit
//!   den Standardwerten und meldet das.
//! * Geheimnisse stehen nie hier (OUT-225).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use os_model::settings::{AppSettings, SETTINGS_FILE};
use serde_json::{Map, Value};

use crate::dto::{AppSettingsDto, CmdError, ErrorCode, Notice, NoticeLevel};

/// Ergebnis eines angewendeten Patches.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsChange {
    /// Neue Einstellungen.
    pub settings: AppSettings,
    /// Geänderte Schlüssel (TOML-Pfade, sortiert); leer = nichts geändert, nichts geschrieben.
    pub changed: Vec<String>,
    /// Davon die Schlüssel, die einen Neustart erfordern.
    pub restart_required: Vec<String>,
}

/// Die Einstellungen im Speicher und ihre Datei.
type SettingsValidator = fn(&AppSettings) -> Result<(), CmdError>;

pub struct SettingsStore {
    validators: Mutex<Vec<SettingsValidator>>,
    dir: PathBuf,
    current: RwLock<AppSettings>,
    write: Mutex<()>,
}

impl std::fmt::Debug for SettingsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsStore")
            .field("dir", &self.dir)
            .finish_non_exhaustive()
    }
}

fn notice_with(code: &str, path: &Path, detail: &str) -> Notice {
    let mut args = std::collections::BTreeMap::new();
    args.insert(
        "path".to_string(),
        Value::String(path.display().to_string()),
    );
    args.insert("detail".to_string(), Value::String(detail.to_string()));
    Notice {
        level: NoticeLevel::Warning,
        code: code.to_string(),
        args,
    }
}

fn io_error(path: &Path, e: &std::io::Error) -> CmdError {
    let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
        ErrorCode::Permission
    } else {
        ErrorCode::Io
    };
    CmdError::new(code, "err-settings-io")
        .with_arg("path", path.display().to_string())
        .with_detail(e.to_string())
}

impl SettingsStore {
    /// Lädt `<dir>/settings.toml`. Der zweite Rückgabewert ist eine Meldung für den ersten Tick, wenn die Datei
    /// unlesbar war: Eine fehlerhafte Datei wird nach `settings.toml.broken` verschoben (`settings-reset`), eine nicht
    /// lesbare bleibt liegen (`settings-unreadable`); in beiden Fällen gelten die Standardwerte, und die App startet.
    pub fn open(dir: impl Into<PathBuf>) -> (SettingsStore, Option<Notice>) {
        let dir = dir.into();
        let path = AppSettings::path_in(&dir);
        let mut notice = None;
        let settings = match std::fs::read_to_string(&path) {
            Ok(text) => match AppSettings::from_toml_str(&text) {
                Ok(s) => s,
                Err(parse) => {
                    let mut target = path.clone().into_os_string();
                    target.push(".broken");
                    let target = PathBuf::from(target);
                    notice = Some(match std::fs::rename(&path, &target) {
                        Ok(()) => notice_with("settings-reset", &target, &parse.to_string()),
                        Err(e) => notice_with("settings-unreadable", &path, &e.to_string()),
                    });
                    AppSettings::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(e) => {
                notice = Some(notice_with("settings-unreadable", &path, &e.to_string()));
                AppSettings::default()
            }
        };
        (
            SettingsStore {
                validators: Mutex::new(Vec::new()),
                dir,
                current: RwLock::new(settings),
                write: Mutex::new(()),
            },
            notice,
        )
    }

    /// Additive Fachprüfung vor dem Schreiben; ohne Registrierung unverändertes Verhalten.
    pub fn add_validator(&self, validator: SettingsValidator) {
        self.validators
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(validator);
    }

    /// Ordner der Datei.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Pfad von `settings.toml`.
    pub fn path(&self) -> PathBuf {
        self.dir.join(SETTINGS_FILE)
    }

    /// Aktuelle Einstellungen.
    pub fn get(&self) -> AppSettings {
        self.current
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Aktuelle Einstellungen als DTO der Oberfläche.
    pub fn dto(&self, restart_required: &[String]) -> AppSettingsDto {
        let mut dto = settings_to_dto(&self.get(), restart_required);
        crate::mcp::enrich_settings(&self.dir, &mut dto);
        dto
    }

    /// Wendet einen Merge-Patch (camelCase, über `AppSettingsDto`) an und schreibt bei einer Änderung atomar.
    pub fn apply_patch(&self, patch: &Value) -> Result<SettingsChange, CmdError> {
        let _guard = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let old = self.get();
        let old_json = serde_json::to_value(&old).map_err(internal_json)?;

        let Value::Object(raw) = camel_to_snake_keys(patch) else {
            return Err(CmdError::invalid_input(
                "err-invalid-settings-patch",
                "patch",
            ));
        };
        let mut pmap = raw;
        // Meta-Felder des DTO, die ein Client unverändert zurückschicken darf
        pmap.remove("schema_version");
        pmap.remove("restart_required");
        let patch = Value::Object(pmap);

        check_known(&old_json, &patch, "")?;
        let merged = merge_patch(old_json.clone(), &patch);
        let new: AppSettings = match serde_json::from_value(merged) {
            Ok(s) => s,
            Err(e) => return Err(locate_bad_field(&old_json, &patch, &e)),
        };

        let issues = new.validate();
        if let Some(first) = issues.first() {
            let list: Vec<Value> = issues
                .iter()
                .map(|i| serde_json::json!({ "path": i.path, "message": i.message }))
                .collect();
            return Err(
                CmdError::invalid_input("err-invalid-setting", first.path.clone())
                    .with_arg("issues", Value::Array(list))
                    .with_detail(first.message.clone()),
            );
        }

        for validate in self
            .validators
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
        {
            validate(&new)?;
        }
        let changed = old.changed_keys(&new);
        if !changed.is_empty() {
            new.save(&self.dir).map_err(|e| {
                CmdError::new(ErrorCode::Io, "err-settings-write")
                    .with_arg("path", self.path().display().to_string())
                    .with_detail(e.to_string())
            })?;
            *self.current.write().unwrap_or_else(|p| p.into_inner()) = new.clone();
        }
        let restart_required = AppSettings::restart_required(changed.iter().map(String::as_str))
            .into_iter()
            .map(str::to_string)
            .collect();
        Ok(SettingsChange {
            settings: new,
            changed,
            restart_required,
        })
    }

    /// Liest die Datei erneut (Änderung von außen). Liefert die geänderten Schlüssel; ein Lesefehler lässt den
    /// Stand im Speicher unverändert.
    pub fn reload(&self) -> Result<Vec<String>, CmdError> {
        let _guard = self.write.lock().unwrap_or_else(|p| p.into_inner());
        let path = self.path();
        let new = match std::fs::read_to_string(&path) {
            Ok(text) => AppSettings::from_toml_str(&text).map_err(|e| {
                CmdError::new(ErrorCode::InvalidInput, "err-settings-invalid")
                    .with_arg("path", path.display().to_string())
                    .with_detail(e.to_string())
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
            Err(e) => return Err(io_error(&path, &e)),
        };
        let old = self.get();
        let changed = old.changed_keys(&new);
        if !changed.is_empty() {
            *self.current.write().unwrap_or_else(|p| p.into_inner()) = new;
        }
        Ok(changed)
    }
}

fn internal_json(e: serde_json::Error) -> CmdError {
    CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
}

/// Einstellungen → DTO (Schlüssel in `camelCase`, `restartRequired` angehängt).
pub fn settings_to_dto(settings: &AppSettings, restart_required: &[String]) -> AppSettingsDto {
    let mut v = snake_to_camel_keys(&serde_json::to_value(settings).unwrap_or(Value::Null));
    if let Value::Object(m) = &mut v {
        m.remove("schemaVersion");
        m.insert(
            "restartRequired".to_string(),
            Value::Array(
                restart_required
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
    }
    // Modell und DTO sind feldgleich (Test `settings_dto_mirrors_model`); ein Abweichen wäre ein Programmierfehler.
    serde_json::from_value(v)
        .unwrap_or_else(|e| panic!("AppSettingsDto passt nicht zum Modell: {e}"))
}

fn to_camel(key: &str) -> String {
    let mut out = String::with_capacity(key.len());
    let mut upper = false;
    for c in key.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn to_snake(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + 4);
    for c in key.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn map_keys(v: &Value, f: &dyn Fn(&str) -> String) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, child)| (f(k), map_keys(child, f)))
                .collect::<Map<String, Value>>(),
        ),
        other => other.clone(),
    }
}

fn snake_to_camel_keys(v: &Value) -> Value {
    map_keys(v, &to_camel)
}

fn camel_to_snake_keys(v: &Value) -> Value {
    map_keys(v, &to_snake)
}

/// RFC 7396 über `snake_case`-Objekte. `null` entfernt den Schlüssel (das Modell liefert dann den Standardwert).
fn merge_patch(target: Value, patch: &Value) -> Value {
    let Value::Object(pmap) = patch else {
        return patch.clone();
    };
    let mut out = match target {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    for (k, pv) in pmap {
        if pv.is_null() {
            out.remove(k);
        } else {
            let base = out.remove(k).unwrap_or(Value::Null);
            out.insert(k.clone(), merge_patch(base, pv));
        }
    }
    Value::Object(out)
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Jeder Schlüssel des Patches muss im Modell vorkommen.
fn check_known(target: &Value, patch: &Value, prefix: &str) -> Result<(), CmdError> {
    let (Value::Object(t), Value::Object(p)) = (target, patch) else {
        return Ok(());
    };
    for (k, pv) in p {
        let path = join(prefix, k);
        match t.get(k) {
            None => {
                return Err(CmdError::invalid_input("err-unknown-setting", path));
            }
            Some(tv) => check_known(tv, pv, &path)?,
        }
    }
    Ok(())
}

fn leaves(prefix: &str, patch: &Value, out: &mut Vec<(String, Value)>) {
    match patch {
        Value::Object(m) if !m.is_empty() => {
            for (k, v) in m {
                leaves(&join(prefix, k), v, out);
            }
        }
        other => out.push((prefix.to_string(), other.clone())),
    }
}

fn single_leaf_patch(path: &str, value: &Value) -> Value {
    let mut v = value.clone();
    for part in path.rsplit('.') {
        let mut m = Map::new();
        m.insert(part.to_string(), v);
        v = Value::Object(m);
    }
    v
}

/// Findet das Blatt des Patches, das die Deserialisierung scheitern lässt.
fn locate_bad_field(old_json: &Value, patch: &Value, err: &serde_json::Error) -> CmdError {
    let mut all = Vec::new();
    leaves("", patch, &mut all);
    for (path, value) in all {
        let merged = merge_patch(old_json.clone(), &single_leaf_patch(&path, &value));
        if let Err(e) = serde_json::from_value::<AppSettings>(merged) {
            return CmdError::invalid_input("err-invalid-setting", path).with_detail(e.to_string());
        }
    }
    CmdError::new(ErrorCode::InvalidInput, "err-invalid-setting").with_detail(err.to_string())
}
