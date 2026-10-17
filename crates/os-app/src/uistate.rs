//! `ui-state.json`: versionierter UI-Zustand, atomar mit `.bak` (UI-125).
//!
//! Der Kern speichert den Inhalt unverändert (Fenster, Trenner, Tabs, Spalten, Verlauf). Schreiben: temporäre Datei,
//! dann wird die bisherige Datei zu `ui-state.json.bak` und die neue zur Datei – nie eine halb geschriebene Datei
//! unter dem Zielnamen. Laden: ist die Datei unlesbar oder fehlt sie nach einem Absturz zwischen den beiden
//! Umbenennungen, gilt die Sicherung; gibt es auch sie nicht, ein leerer Zustand.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::dto::{CmdError, ErrorCode, UiStateJson};

/// Dateiname im Einstellungsordner.
pub const UI_STATE_FILE: &str = "ui-state.json";
/// Name der Sicherung.
pub const UI_STATE_BACKUP: &str = "ui-state.json.bak";
/// Größte zulässige Datei (Schutz vor ausufernden Verläufen).
pub const MAX_UI_STATE_BYTES: usize = 8 * 1024 * 1024;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Der UI-Zustand auf der Platte.
pub struct UiStateStore {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl std::fmt::Debug for UiStateStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UiStateStore")
            .field("dir", &self.dir)
            .finish()
    }
}

fn io_error(path: &Path, e: &std::io::Error) -> CmdError {
    let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
        ErrorCode::Permission
    } else {
        ErrorCode::Io
    };
    CmdError::new(code, "err-uistate-io")
        .with_arg("path", path.display().to_string())
        .with_detail(e.to_string())
}

fn empty_state() -> UiStateJson {
    UiStateJson {
        version: 1,
        state: serde_json::json!({}),
    }
}

fn read_state(path: &Path) -> Option<UiStateJson> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice::<UiStateJson>(&bytes).ok()
}

impl UiStateStore {
    /// Zustand im Einstellungsordner `dir`.
    pub fn new(dir: impl Into<PathBuf>) -> UiStateStore {
        UiStateStore {
            dir: dir.into(),
            lock: Mutex::new(()),
        }
    }

    /// Pfad der Datei.
    pub fn path(&self) -> PathBuf {
        self.dir.join(UI_STATE_FILE)
    }

    /// Pfad der Sicherung.
    pub fn backup_path(&self) -> PathBuf {
        self.dir.join(UI_STATE_BACKUP)
    }

    /// Lädt den Zustand (Datei, sonst Sicherung, sonst leerer Zustand).
    pub fn load(&self) -> UiStateJson {
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        read_state(&self.path())
            .or_else(|| read_state(&self.backup_path()))
            .unwrap_or_else(empty_state)
    }

    /// Speichert den Zustand unverändert und liefert ihn zurück.
    pub fn save(&self, state: &UiStateJson) -> Result<UiStateJson, CmdError> {
        if state.version == 0 {
            return Err(CmdError::invalid_input("err-invalid-uistate", "version"));
        }
        if !state.state.is_object() {
            return Err(CmdError::invalid_input("err-invalid-uistate", "state"));
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(|e| {
            CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
        })?;
        if bytes.len() > MAX_UI_STATE_BYTES {
            return Err(
                CmdError::new(ErrorCode::InvalidInput, "err-uistate-too-large")
                    .with_arg("bytes", bytes.len())
                    .with_arg("max", MAX_UI_STATE_BYTES),
            );
        }
        let _g = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        std::fs::create_dir_all(&self.dir).map_err(|e| io_error(&self.dir, &e))?;
        let path = self.path();
        let tmp = self.dir.join(format!(
            "{UI_STATE_FILE}.tmp{}-{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        if let Err(e) = write_synced(&tmp, &bytes) {
            let _ = std::fs::remove_file(&tmp);
            return Err(io_error(&tmp, &e));
        }
        if path.exists() {
            if let Err(e) = std::fs::rename(&path, self.backup_path()) {
                let _ = std::fs::remove_file(&tmp);
                return Err(io_error(&path, &e));
            }
        }
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            io_error(&path, &e)
        })?;
        Ok(state.clone())
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(bytes)?;
    f.sync_all()
}
