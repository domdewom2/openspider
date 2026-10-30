//! Rechte, Fenster und Konfiguration der Hülle (M2.md 3.7): jede Command-Domäne hat ihr Plugin und ihre Berechtigung,
//! `tauri.conf.json` hält Mindestgrößen, Systemversion, CSP und Dateizuordnungen ein (OUT-272).

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use openspider_desktop_lib::commands::domains;
use openspider_desktop_lib::plugin_names::{plugin_name, TAURI_CORE_PLUGINS};
use os_app::{App, AppConfig};
use serde_json::Value;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn json(rel: &str) -> Value {
    serde_json::from_str(&fs::read_to_string(manifest_dir().join(rel)).unwrap()).unwrap()
}

fn commands_json_domains() -> Vec<String> {
    let v: Value = serde_json::from_str(
        &fs::read_to_string(manifest_dir().join("../src/ipc/commands.json")).unwrap(),
    )
    .unwrap();
    let mut out: Vec<String> = Vec::new();
    for c in v.as_array().unwrap() {
        let d = c["name"]
            .as_str()
            .unwrap()
            .split('.')
            .next()
            .unwrap()
            .to_string();
        if !out.contains(&d) {
            out.push(d);
        }
    }
    out
}

// spec: UI-140
#[tokio::test]
async fn one_plugin_per_registry_domain_matching_commands_json() {
    let dir = std::env::temp_dir().join(format!("os-desktop-domains-{}", std::process::id()));
    let app = App::new(AppConfig::new(&dir)).unwrap();
    let from_registry: Vec<String> = domains(&app).iter().map(|d| (*d).to_string()).collect();
    assert_eq!(from_registry, commands_json_domains());
    assert!(from_registry.len() >= 26, "{from_registry:?}");
    let _ = fs::remove_dir_all(dir);
}

// spec: UI-140
#[test]
fn domains_named_like_tauri_core_plugins_get_the_os_prefix() {
    assert_eq!(plugin_name("view"), "view");
    assert_eq!(plugin_name("events"), "events");
    assert_eq!(plugin_name("app"), "os-app");
    for core in TAURI_CORE_PLUGINS {
        assert_eq!(plugin_name(core), format!("os-{core}"));
    }
    // keine Domäne der Commands trägt sonst einen Kern-Plugin-Namen
    let clashes: Vec<_> = commands_json_domains()
        .into_iter()
        .filter(|d| TAURI_CORE_PLUGINS.contains(&d.as_str()))
        .collect();
    assert_eq!(clashes, vec!["app".to_string()]);
}

// spec: UI-140
#[test]
fn the_capability_grants_every_domain_to_the_app_windows() {
    let cap = json("capabilities/default.json");
    let granted: BTreeSet<&str> = cap["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    for domain in commands_json_domains() {
        let want = format!("{}:default", plugin_name(&domain));
        assert!(
            granted.contains(want.as_str()),
            "capabilities/default.json fehlt {want} (neue Domäne? Eintrag ergänzen)"
        );
    }
    for plugin in [
        "dialog:default",
        "notification:default",
        "os:default",
        "core:default",
    ] {
        assert!(granted.contains(plugin), "{plugin} fehlt");
    }
    // Fenster main, settings und die dynamischen Visualisierungsfenster
    let windows: Vec<&str> = cap["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w.as_str().unwrap())
        .collect();
    assert_eq!(windows, vec!["main", "settings", "viz-*"]);
    // keine Berechtigung ohne Domäne oder Plugin (Tippfehler)
    let domains: BTreeSet<String> = commands_json_domains()
        .iter()
        .map(|d| plugin_name(d))
        .collect();
    for p in &granted {
        let prefix = p.split(':').next().unwrap();
        let known = domains.contains(prefix)
            || [
                "core",
                "dialog",
                "notification",
                "os",
                "clipboard-manager",
                "deep-link",
                "updater",
                "process",
            ]
            .contains(&prefix);
        assert!(known, "unbekannte Berechtigung {p}");
    }
}

// spec: UI-140
#[test]
fn the_main_window_has_the_minimum_size_and_a_settings_window_is_declared() {
    let conf = json("tauri.conf.json");
    let windows = conf["app"]["windows"].as_array().unwrap();
    let main = windows.iter().find(|w| w["label"] == "main").unwrap();
    assert_eq!(
        (main["minWidth"].as_f64(), main["minHeight"].as_f64()),
        (Some(1024.0), Some(640.0))
    );
    let settings = windows
        .iter()
        .find(|w| w["label"] == "settings")
        .expect("Einstellungsfenster");
    assert_eq!(
        settings["create"], false,
        "das Einstellungsfenster entsteht erst auf Anforderung"
    );
    assert_eq!(conf["bundle"]["macOS"]["minimumSystemVersion"], "13.0");
}

// spec: UI-140
#[test]
fn the_csp_names_no_external_source() {
    let conf = json("tauri.conf.json");
    for key in ["csp", "devCsp"] {
        let csp = conf["app"]["security"][key]
            .as_str()
            .unwrap_or_else(|| panic!("{key} fehlt"));
        for part in csp.split_whitespace().map(|p| p.trim_end_matches(';')) {
            if part.starts_with("http://")
                || part.starts_with("https://")
                || part.starts_with("ws://")
                || part.starts_with("wss://")
            {
                let host = part.split("://").nth(1).unwrap();
                assert!(
                    host.starts_with("localhost")
                        || host == "ipc.localhost"
                        || host == "ospider.localhost",
                    "{key}: externe Quelle {part}"
                );
            }
            assert_ne!(part, "*", "{key}: Platzhalterquelle");
        }
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("object-src 'none'"));
    }
    // das Produktionsprofil kennt gar keine Entwicklungsserver
    let prod = conf["app"]["security"]["csp"].as_str().unwrap();
    assert!(!prod.contains("localhost:1420") && !prod.contains("unsafe-eval"));
    // das Schema für Screenshots und Bilder ist erlaubt
    assert!(prod.contains("ospider:"));
}

// spec: OUT-272
#[test]
fn the_five_file_types_are_associated_with_the_app() {
    let conf = json("tauri.conf.json");
    let assoc = conf["bundle"]["fileAssociations"].as_array().unwrap();
    let exts: BTreeSet<&str> = assoc
        .iter()
        .flat_map(|a| a["ext"].as_array().unwrap())
        .map(|e| e.as_str().unwrap())
        .collect();
    assert_eq!(
        exts,
        BTreeSet::from(["ospider", "osconfig", "osauth", "ospreset", "ostask"])
    );
    for a in assoc {
        assert!(a["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert_eq!(a["role"], "Editor");
    }
    assert_eq!(conf["bundle"]["active"], true);
}
