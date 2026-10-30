//! Build-Skript der Tauri-Hülle.
//!
//! * `tauri::generate_context!` verlangt, dass `frontendDist` existiert. Damit `cargo check/clippy/test --workspace`
//!   auch ohne vorherigen Frontend-Build funktioniert, wird das (gitignorierte) Verzeichnis bei Bedarf angelegt.
//! * Je Command-Domäne aus `../src/ipc/commands.json` entsteht ein internes Tauri-Plugin (`plugin:<domäne>|<command>`,
//!   M2.md 3.7). Tauri prüft jeden Plugin-Aufruf gegen die Capabilities; deshalb erzeugt dieses Skript je Domäne eine
//!   Berechtigungsdatei (`<plugin>:default` erlaubt alle Commands der Domäne; `plugin_name` setzt bei Namen von
//!   Tauri-Kern-Plugins das Präfix `os-`). `capabilities/default.json` nennt die Berechtigungen; der Test
//!   `tests/capabilities.rs` prüft, dass keine Domäne fehlt.
//!
//! Die Command-Namen kommen aus der generierten Datei `commands.json` (Format von `os-app-manifest`: eine Zeile
//! `"name": "<domäne>.<command>",` je Command); das Skript liest sie zeilenweise, ohne JSON-Abhängigkeit.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use tauri_build::{Attributes, InlinedPlugin};

const COMMANDS_JSON: &str = "../src/ipc/commands.json";

// gemeinsame Regel für Plugin-Namen (`plugin_name`, `TAURI_CORE_PLUGINS`)
include!("src/plugin_names.rs");

/// Domäne → Commands in der Reihenfolge der Datei.
fn read_domains(path: &Path) -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} nicht lesbar: {e}", path.display()));
    let mut domains: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix("\"name\": \"") else {
            continue;
        };
        let Some(name) = rest.strip_suffix("\",") else {
            continue;
        };
        let (domain, command) = name
            .split_once('.')
            .unwrap_or_else(|| panic!("Command ohne Domäne: {name}"));
        domains
            .entry(domain.to_string())
            .or_default()
            .push(command.to_string());
    }
    assert!(
        !domains.is_empty(),
        "{} enthält keine Commands (Format von os-app-manifest erwartet)",
        path.display()
    );
    domains
}

fn permission_file(domain: &str, commands: &[String]) -> String {
    let mut list = String::new();
    for (i, c) in commands.iter().enumerate() {
        if i > 0 {
            list.push_str(", ");
        }
        let _ = write!(list, "\"{c}\"");
    }
    format!(
        "# Automatisch aus commands.json erzeugt (build.rs) – nicht von Hand ändern.\n\
         [default]\n\
         description = \"Alle Commands der Domäne {domain}\"\n\
         permissions = [\"allow-all\"]\n\
         \n\
         [[permission]]\n\
         identifier = \"allow-all\"\n\
         description = \"Erlaubt alle Commands der Domäne {domain}\"\n\
         commands.allow = [{list}]\n"
    )
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    // M3-F01: Tauri does not expand environment variables in JSON configuration.
    // Preserve CLI/platform overrides, then inject the public release configuration.
    println!("cargo:rerun-if-env-changed=OPENSPIDER_UPDATER_PUBKEY");
    println!("cargo:rerun-if-env-changed=OPENSPIDER_UPDATE_BETA");
    let mut config: serde_json::Value = std::env::var("TAURI_CONFIG")
        .ok()
        .map(|s| serde_json::from_str(&s).expect("TAURI_CONFIG invalid"))
        .unwrap_or_else(|| serde_json::json!({}));
    let key = std::env::var("OPENSPIDER_UPDATER_PUBKEY").unwrap_or_default();
    let manifest = match std::env::var("OPENSPIDER_UPDATE_BETA").as_deref() {
        Ok("1") => "latest-beta.json",
        Ok("0") | Err(_) => "latest.json",
        _ => panic!("OPENSPIDER_UPDATE_BETA must be 0 or 1"),
    };
    config["plugins"]["updater"] = serde_json::json!({
        "pubkey": key,
        "endpoints": [format!("https://github.com/domdewom2/openspider/releases/latest/download/{manifest}")]
    });
    // cargo check/test do not stage the release sidecar; release builds require it.
    let triple = std::env::var("TARGET").expect("TARGET");
    let suffix = if triple.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let sidecar = manifest_dir.join(format!("binaries/openspider-{triple}{suffix}"));
    println!("cargo:rerun-if-changed={}", sidecar.display());
    if std::env::var("PROFILE").as_deref() == Ok("debug") && !sidecar.exists() {
        config["bundle"]["externalBin"] = serde_json::json!([]);
    }
    let config = serde_json::to_string(&config).expect("config serialization");
    std::env::set_var("TAURI_CONFIG", &config);
    println!("cargo:rustc-env=TAURI_CONFIG={config}");

    let _ = std::fs::create_dir_all(manifest_dir.join("../dist"));

    println!("cargo:rerun-if-changed={COMMANDS_JSON}");
    let domains = read_domains(&manifest_dir.join(COMMANDS_JSON));

    let mut attributes = Attributes::new();
    for (domain, commands) in &domains {
        let plugin = plugin_name(domain);
        let dir = out_dir.join("os-permissions").join(&plugin);
        std::fs::create_dir_all(&dir).expect("Berechtigungsordner nicht anlegbar");
        std::fs::write(dir.join("default.toml"), permission_file(domain, commands))
            .expect("Berechtigungsdatei nicht schreibbar");
        let pattern = format!(
            "{}/*.toml",
            glob_escape(&dir.to_string_lossy().replace('\\', "/"))
        );
        attributes = attributes.plugin(
            leak(plugin),
            InlinedPlugin::new().permissions_path_pattern(leak(pattern)),
        );
    }
    tauri_build::try_build(attributes).expect("tauri-build fehlgeschlagen");
}

/// Maskiert Glob-Sonderzeichen im Pfad.
fn glob_escape(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '[' | ']' | '*' | '?') {
            out.push('[');
            out.push(c);
            out.push(']');
        } else {
            out.push(c);
        }
    }
    out
}
