//! Ablage der Aufgabendateien `schedules/<task-id>.ostask` (OUT-160). Schreiben ist atomar (Datei
//! daneben schreiben, dann umbenennen); der Name einer Aufgabe ist eindeutig (Groß-/Kleinschreibung egal).

use std::path::{Path, PathBuf};

use super::model::{Task, TaskFile, FORMAT_VERSION};

/// Dateiendung.
pub const EXTENSION: &str = "ostask";

/// Fehler der Ablage.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Dateisystemfehler.
    #[error("{path}: {source}")]
    Io {
        /// Betroffener Pfad.
        path: PathBuf,
        /// Ursache.
        source: std::io::Error,
    },
    /// Aufgabe unbekannt.
    #[error("Aufgabe '{0}' nicht gefunden")]
    NotFound(String),
    /// Ungültige Kennung.
    #[error("ungültige Aufgabenkennung '{0}'")]
    InvalidId(String),
    /// Name schon vergeben.
    #[error("Aufgabenname '{0}' ist schon vergeben")]
    NameTaken(String),
    /// Name leer.
    #[error("Aufgabenname fehlt")]
    NameEmpty,
    /// Datei nicht lesbar.
    #[error("{path}: {message}")]
    Parse {
        /// Betroffener Pfad.
        path: PathBuf,
        /// Meldung.
        message: String,
    },
}

fn io(path: &Path, source: std::io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Ist `id` eine gültige Kennung (`[a-z0-9][a-z0-9_-]{0,63}`)? Verhindert Pfadausbrüche und
/// Sonderzeichen im Planer-Aufruf.
pub fn valid_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    id.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Neue Kennung aus dem Namen: Kleinbuchstaben-Anteil plus 8 Zufallszeichen.
pub fn new_id(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(24).collect();
    let slug = if slug.is_empty() { "task" } else { &slug };
    let random = uuid::Uuid::now_v7().simple().to_string();
    format!("{slug}-{}", &random[random.len() - 8..])
}

/// Ordnername aus einem Anzeigenamen (OUT-008): unzulässige Zeichen werden `_`, Windows-reservierte Namen
/// und abschließende Punkte/Leerzeichen vermieden. Die Anzeige bleibt unverändert.
pub fn folder_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    out = out.trim().trim_end_matches(['.', ' ']).to_string();
    if out.chars().count() > 80 {
        out = out
            .chars()
            .take(80)
            .collect::<String>()
            .trim_end()
            .to_string();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit());
    if out.is_empty() {
        "task".to_string()
    } else if reserved {
        format!("_{out}")
    } else {
        out
    }
}

/// Aufgabenordner im Einstellungsordner.
#[derive(Debug, Clone)]
pub struct TaskStore {
    dir: PathBuf,
}

impl TaskStore {
    /// Ablage in `dir` (wird bei Bedarf angelegt).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        TaskStore { dir: dir.into() }
    }

    /// Ablage `<settings>/schedules`.
    pub fn in_settings(settings: &Path) -> Self {
        Self::new(settings.join("schedules"))
    }

    /// Ordner.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Pfad der Datei zu `id`.
    pub fn path(&self, id: &str) -> Result<PathBuf, StoreError> {
        if !valid_id(id) {
            return Err(StoreError::InvalidId(id.to_string()));
        }
        Ok(self.dir.join(format!("{id}.{EXTENSION}")))
    }

    /// Liest eine Datei.
    pub fn read_file(path: &Path) -> Result<Task, StoreError> {
        let text = std::fs::read_to_string(path).map_err(|e| io(path, e))?;
        let file: TaskFile = toml::from_str(&text).map_err(|e| StoreError::Parse {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;
        if file.version > FORMAT_VERSION {
            return Err(StoreError::Parse {
                path: path.to_path_buf(),
                message: format!(
                    "Aufgabendatei der Version {} wird nicht unterstützt (bekannt: {FORMAT_VERSION})",
                    file.version
                ),
            });
        }
        Ok(file.task)
    }

    /// Eine Aufgabe.
    pub fn get(&self, id: &str) -> Result<Task, StoreError> {
        let path = self.path(id)?;
        if !path.is_file() {
            return Err(StoreError::NotFound(id.to_string()));
        }
        let mut task = Self::read_file(&path)?;
        // Die Kennung folgt dem Dateinamen.
        task.id = id.to_string();
        Ok(task)
    }

    /// Alle lesbaren Aufgaben (nach Name sortiert) und die Fehler der unlesbaren Dateien.
    pub fn list_with_errors(&self) -> (Vec<Task>, Vec<StoreError>) {
        let mut tasks = Vec::new();
        let mut errors = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return (tasks, errors);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != EXTENSION) {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if !valid_id(&id) {
                continue;
            }
            match Self::read_file(&path) {
                Ok(mut task) => {
                    task.id = id;
                    tasks.push(task);
                }
                Err(e) => errors.push(e),
            }
        }
        tasks.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.cmp(&b.id))
        });
        (tasks, errors)
    }

    /// Alle lesbaren Aufgaben.
    pub fn list(&self) -> Vec<Task> {
        self.list_with_errors().0
    }

    /// Prüft, ob `name` frei ist (andere Aufgaben als `except`).
    pub fn name_taken(&self, name: &str, except: Option<&str>) -> bool {
        let wanted = name.trim().to_lowercase();
        self.list()
            .iter()
            .any(|t| Some(t.id.as_str()) != except && t.name.trim().to_lowercase() == wanted)
    }

    /// Speichert die Aufgabe (neue Kennung, wenn `id` leer ist). Der Name muss eindeutig sein.
    pub fn save(&self, mut task: Task) -> Result<Task, StoreError> {
        task.name = task.name.trim().to_string();
        if task.name.is_empty() {
            return Err(StoreError::NameEmpty);
        }
        if task.id.is_empty() {
            task.id = new_id(&task.name);
        }
        let path = self.path(&task.id)?;
        if self.name_taken(&task.name, Some(&task.id)) {
            return Err(StoreError::NameTaken(task.name));
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| io(&self.dir, e))?;
        let text = toml::to_string_pretty(&TaskFile {
            version: FORMAT_VERSION,
            task: task.clone(),
        })
        .map_err(|e| StoreError::Parse {
            path: path.clone(),
            message: e.to_string(),
        })?;
        let tmp = self
            .dir
            .join(format!(".{}.{}.tmp", task.id, std::process::id()));
        std::fs::write(&tmp, text).map_err(|e| io(&tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io(&path, e)
        })?;
        Ok(task)
    }

    /// Entfernt die Aufgabendatei. Erzeugte Crawls und Exporte bleiben (OUT-160).
    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        let path = self.path(id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(StoreError::NotFound(id.to_string()))
            }
            Err(e) => Err(io(&path, e)),
        }
    }
}
