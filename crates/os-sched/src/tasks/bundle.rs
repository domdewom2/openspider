//! Export und Import von Aufgaben (OUT-170): `.ostask`-Dateien (TOML) ohne Geheimnisse.
//!
//! Die Aufgabendatei enthält nur Verweise (Konto-Kennungen, Profilnamen). Zusätzlich werden Werte entfernt,
//! die wie Geheimnisse aussehen: Konfigurationsüberschreibungen mit Schlüsseln wie `password` oder `token`
//! und Zugangsdaten in URLs (`https://nutzer:kennwort@host/`).

use super::model::{Task, TaskBundle, TaskFile, FORMAT_VERSION};
use super::store::{new_id, valid_id, StoreError, TaskStore};

/// Fehler beim Lesen oder Schreiben.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    /// Datei nicht lesbar.
    #[error("Aufgabendatei nicht lesbar: {0}")]
    Parse(String),
    /// Neuere Version.
    #[error("Aufgabendatei der Version {0} wird nicht unterstützt (bekannt: {FORMAT_VERSION})")]
    Version(u32),
    /// Ablage.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Sieht der Schlüsselname nach einem Geheimnis aus?
pub fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "passphrase",
        "secret",
        "token",
        "api_key",
        "apikey",
        "credential",
        "authorization",
        "cookie",
    ]
    .iter()
    .any(|s| lower.contains(s))
}

/// Entfernt `nutzer:kennwort@` aus einer URL.
pub fn strip_userinfo(text: &str) -> String {
    let Some((scheme, rest)) = text.split_once("://") else {
        return text.to_string();
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    match authority.rsplit_once('@') {
        Some((_, host)) => format!("{scheme}://{host}{tail}"),
        None => text.to_string(),
    }
}

/// Entfernt Geheimnisse aus einer Aufgabe.
pub fn scrub(task: &mut Task) {
    task.crawl.overrides.retain(|k, _| !is_secret_key(k));
    for value in task.crawl.overrides.values_mut() {
        *value = strip_userinfo(value);
    }
    match &mut task.crawl.source {
        super::model::Source::Spider { url } => *url = strip_userinfo(url),
        super::model::Source::List { path } => {
            *path = path
                .lines()
                .map(strip_userinfo)
                .collect::<Vec<_>>()
                .join("\n");
        }
    }
    if let Some(config) = &mut task.crawl.config {
        *config = strip_userinfo(config);
    }
}

/// Schreibt Aufgaben als Bündel (`[[tasks]]`), ohne Geheimnisse.
pub fn export_text(tasks: &[Task]) -> Result<String, BundleError> {
    let mut tasks = tasks.to_vec();
    tasks.iter_mut().for_each(scrub);
    toml::to_string_pretty(&TaskBundle {
        version: FORMAT_VERSION,
        tasks,
    })
    .map_err(|e| BundleError::Parse(e.to_string()))
}

/// Liest ein Bündel oder eine einzelne Aufgabendatei.
pub fn import_text(text: &str) -> Result<Vec<Task>, BundleError> {
    if let Ok(bundle) = toml::from_str::<TaskBundle>(text) {
        if bundle.version > FORMAT_VERSION {
            return Err(BundleError::Version(bundle.version));
        }
        return Ok(bundle.tasks);
    }
    match toml::from_str::<TaskFile>(text) {
        Ok(file) if file.version > FORMAT_VERSION => Err(BundleError::Version(file.version)),
        Ok(file) => Ok(vec![file.task]),
        Err(e) => Err(BundleError::Parse(e.to_string())),
    }
}

/// Importiert Aufgaben in die Ablage. Kennung und Name bleiben erhalten, sofern sie frei sind; sonst
/// bekommt die Aufgabe eine neue Kennung bzw. ein Namensanhang `(2)`, `(3)` …
pub fn import_into(store: &TaskStore, text: &str) -> Result<Vec<Task>, BundleError> {
    let mut saved = Vec::new();
    for mut task in import_text(text)? {
        scrub(&mut task);
        if !valid_id(&task.id) || store.get(&task.id).is_ok() {
            task.id = new_id(&task.name);
        }
        let base = task.name.trim().to_string();
        let mut n = 1;
        while store.name_taken(&task.name, Some(&task.id)) {
            n += 1;
            task.name = format!("{base} ({n})");
        }
        saved.push(store.save(task)?);
    }
    Ok(saved)
}
