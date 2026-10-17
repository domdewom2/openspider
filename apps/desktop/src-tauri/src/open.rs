//! Dateien öffnen: Einzelinstanz und Dateizuordnung (OUT-232, OUT-272).
//!
//! Doppelklick auf `.ospider`, `.osconfig`, `.osauth`, `.ospreset` oder `.ostask` startet höchstens eine Instanz:
//! eine zweite Instanz übergibt ihre Argumente an die laufende (single-instance), macOS meldet geöffnete Dateien als
//! Deep-Link-/Open-Ereignis. Die Hülle entscheidet nichts selbst: Jede Datei wird als Meldung `open-file` (Art und Pfad)
//! an die Oberfläche gegeben; die Rückfrage („laufenden Crawl pausieren?“, „Konfiguration laden?“) und der Aufruf von
//! `crawl.open` bzw. `config.*` gehören ins Frontend.
//!
//! Die Meldung reist im nächsten Tick und geht deshalb auch dann nicht verloren, wenn die Datei den Start auslöst und das
//! Fenster noch nicht abonniert hat.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use os_app::dto::{Notice, NoticeLevel};
use os_app::App;

/// Art der Datei.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenKind {
    /// `.ospider`: Crawl-Datei.
    Crawl,
    /// `.osconfig`: Konfiguration.
    Config,
    /// `.osauth`: Authentifizierungsprofile.
    Auth,
    /// `.ospreset`: Vorgabe.
    Preset,
    /// `.ostask`: Aufgabe.
    Task,
}

impl OpenKind {
    /// Kennung in der Meldung (`args.kind`).
    pub fn as_str(self) -> &'static str {
        match self {
            OpenKind::Crawl => "crawl",
            OpenKind::Config => "config",
            OpenKind::Auth => "auth",
            OpenKind::Preset => "preset",
            OpenKind::Task => "task",
        }
    }

    /// Art zu einer Dateiendung (ohne Punkt, Groß-/Kleinschreibung egal).
    pub fn from_extension(ext: &str) -> Option<OpenKind> {
        match ext.to_ascii_lowercase().as_str() {
            "ospider" => Some(OpenKind::Crawl),
            "osconfig" => Some(OpenKind::Config),
            "osauth" => Some(OpenKind::Auth),
            "ospreset" => Some(OpenKind::Preset),
            "ostask" => Some(OpenKind::Task),
            _ => None,
        }
    }
}

/// Eine zu öffnende Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    /// Art.
    pub kind: OpenKind,
    /// Absoluter Pfad (relative Angaben sind gegen das Arbeitsverzeichnis des Aufrufers aufgelöst).
    pub path: PathBuf,
}

fn classify(path: PathBuf) -> Option<OpenRequest> {
    let kind = OpenKind::from_extension(path.extension()?.to_str()?)?;
    Some(OpenRequest { kind, path })
}

/// Dateien aus Kommandozeilenargumenten (`args[0]` ist das Programm). Optionen (`-…`) und unbekannte Endungen werden
/// übergangen; relative Pfade löst `cwd` auf.
pub fn requests_from_args(args: &[String], cwd: Option<&Path>) -> Vec<OpenRequest> {
    args.iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter_map(|a| {
            let p = PathBuf::from(a);
            let p = if p.is_relative() {
                cwd.map_or(p.clone(), |c| c.join(&p))
            } else {
                p
            };
            classify(p)
        })
        .collect()
}

/// Dateien aus `file://`-URLs (macOS meldet geöffnete Dateien so).
pub fn requests_from_urls(urls: &[tauri::Url]) -> Vec<OpenRequest> {
    urls.iter()
        .filter(|u| u.scheme() == "file")
        .filter_map(|u| u.to_file_path().ok())
        .filter_map(classify)
        .collect()
}

/// Die Meldung, die die Oberfläche zum Öffnen auffordert (i18n-Schlüssel `notice-open-file`).
pub fn notice_for(req: &OpenRequest) -> Notice {
    let mut args = BTreeMap::new();
    args.insert(
        "kind".to_string(),
        serde_json::Value::String(req.kind.as_str().to_string()),
    );
    args.insert(
        "path".to_string(),
        serde_json::Value::String(req.path.display().to_string()),
    );
    Notice {
        level: NoticeLevel::Info,
        code: "open-file".to_string(),
        args,
    }
}

/// Gibt die Dateien an die Oberfläche weiter; Anzahl der weitergegebenen Dateien.
pub fn forward(app: &App, requests: &[OpenRequest]) -> usize {
    for r in requests {
        app.push_notice(notice_for(r));
    }
    if !requests.is_empty() {
        app.flush_tick();
    }
    requests.len()
}
