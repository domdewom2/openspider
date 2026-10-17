//! Lint 2.3 Regel 1 für die Hülle (M2-ui-concept 2.3, `docs/plan/M2.md` 3.6): in `apps/desktop/src-tauri/**` kommt weder
//! `rusqlite` noch eine Funktion aus `os_store`/`os_analyze` (und den anderen Kern-Crates) vor. Die Hülle spricht nur
//! mit `os-app` (`App::dispatch`).

use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Kern-Crates, auf die die Hülle nicht zugreifen darf (Bindestrich- und Unterstrichschreibweise).
const FORBIDDEN: &[&str] = &[
    "rusqlite",
    "os_store",
    "os-store",
    "os_analyze",
    "os-analyze",
    "os_engine",
    "os-engine",
    "os_export",
    "os-export",
    "os_fetch",
    "os-fetch",
    "os_parse",
    "os-parse",
    "os_render",
    "os-render",
    "os_integrations",
    "os-integrations",
    "os_mcp",
    "os-mcp",
    "os_sched",
    "os-sched",
];

// spec: UI-140
#[test]
fn the_shell_sources_never_name_the_core_or_the_database() {
    let mut files = Vec::new();
    rs_files(&manifest_dir().join("src"), &mut files);
    assert!(files.len() >= 8, "{files:?}");
    let this = Path::new(file!()).file_name().unwrap().to_owned();
    // auch die Tests der Hülle (außer dieser Datei, die die Namen als Daten enthält)
    rs_files(&manifest_dir().join("tests"), &mut files);
    for f in files {
        if f.file_name() == Some(this.as_os_str()) {
            continue;
        }
        let text = fs::read_to_string(&f).unwrap();
        for word in FORBIDDEN {
            assert!(
                !text.contains(word),
                "{} nennt {word}: die Hülle greift nur über os-app auf den Kern zu",
                f.display()
            );
        }
    }
}

// spec: UI-140
#[test]
fn the_shell_manifest_depends_only_on_os_app_and_os_model_from_the_core() {
    let text = fs::read_to_string(manifest_dir().join("Cargo.toml")).unwrap();
    for word in FORBIDDEN {
        assert!(!text.contains(word), "Cargo.toml der Hülle nennt {word}");
    }
    assert!(text.contains("os-app.workspace = true"));
}
