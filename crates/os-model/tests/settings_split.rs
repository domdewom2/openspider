//! App-Einstellungen (`settings.toml`, OUT-222): Standardwerte, Rundreise, Validierung, Trennung von der
//! Crawl-Konfiguration (disjunkte Schlüsselmengen).

use std::collections::BTreeSet;
use std::fs;

use os_model::settings::*;
use os_model::*;
use serde_json::Value as Json;

fn leaves(prefix: &str, v: &Json, out: &mut BTreeSet<String>) {
    match v {
        Json::Object(map) if !map.is_empty() => {
            for (k, child) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                leaves(&p, child, out);
            }
        }
        _ => {
            out.insert(prefix.to_string());
        }
    }
}

fn dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("os-model-settings-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

// spec: OUT-222
#[test]
fn settings_and_crawl_config_share_no_key() {
    // Schlüssel, die nur bei gesetztem Optional erscheinen, tauchen im JSON als null auf: alle Blätter zählen.
    let mut s = BTreeSet::new();
    leaves(
        "",
        &serde_json::to_value(AppSettings::default()).unwrap(),
        &mut s,
    );
    let mut c = BTreeSet::new();
    leaves(
        "",
        &serde_json::to_value(CrawlConfig::default()).unwrap(),
        &mut c,
    );
    // schema_version gibt es in beiden Dateien als Formatkennung; sie gehört zum jeweiligen Format
    s.remove("schema_version");
    c.remove("schema_version");
    let common: Vec<_> = s.intersection(&c).collect();
    assert!(
        common.is_empty(),
        "Schlüssel in beiden Formaten: {common:?}"
    );
    // Der Exportordner steht ausschließlich in den App-Einstellungen
    assert!(s.contains("export.directory"));
    assert!(!c.contains("export.directory"));
    // eine .osconfig enthält keine Abschnitte aus settings.toml (auch nicht in der Textform)
    let cfg_text = CrawlConfig::default().to_osconfig_string().unwrap();
    for section in [
        "[appearance]",
        "[storage]",
        "[retention]",
        "[accounts]",
        "[mcp]",
        "[general]",
        "[network.proxy]",
    ] {
        assert!(
            !cfg_text.contains(section),
            "{section} gehört in settings.toml"
        );
    }
    for key in [
        "theme =",
        "accent =",
        "zoom_pct",
        "live_interval_ms",
        "ram_limit_gb",
        "notifications =",
    ] {
        assert!(!cfg_text.contains(key), "{key} gehört in settings.toml");
    }
}

// spec: OUT-222, UI-120, UI-121, UI-018, CRAWL-124, OUT-229
#[test]
fn defaults_follow_the_spec() {
    let s = AppSettings::default();
    assert_eq!(s.general.language, UiLanguage::System);
    assert_eq!(s.appearance.theme, ThemeMode::System);
    assert_eq!(s.appearance.zoom_pct, 100);
    assert_eq!(s.appearance.row_density, RowDensity::Compact);
    assert!(s.appearance.zebra);
    assert_eq!(s.appearance.live_interval_ms, 500);
    assert_eq!(s.storage.performance_profile, PerformanceProfile::Balanced);
    assert_eq!(s.retention.policy, RetentionPolicy::Never);
    assert_eq!(s.network.proxy.mode, ProxyMode::System);
    assert_eq!(s.export.directory, None);
    assert!(!s.mcp.autostart);
    assert!(s.validate().is_empty());
}

// spec: OUT-222
#[test]
fn settings_round_trip_byte_identically_and_tolerate_missing_keys() {
    let mut s = AppSettings::default();
    s.general.language = UiLanguage::De;
    s.appearance.theme = ThemeMode::Dark;
    s.appearance.accent = "#112233".into();
    s.export.directory = Some("/tmp/exports".into());
    s.network.proxy = ProxySettings {
        mode: ProxyMode::Manual,
        kind: ProxyKind::Socks5,
        host: "proxy.test".into(),
        port: 1080,
        username: Some("u".into()),
        bypass: vec!["localhost".into()],
    };
    s.network.trusted_certificates = vec!["/etc/ca.pem".into()];
    s.storage.ram_limit_gb = Some(6.5);
    let text = s.to_toml_string().unwrap();
    let back = AppSettings::from_toml_str(&text).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.to_toml_string().unwrap(), text);
    assert!(!text.to_lowercase().contains("password"));

    let partial = AppSettings::from_toml_str("[appearance]\ntheme = \"light\"\n").unwrap();
    assert_eq!(partial.appearance.theme, ThemeMode::Light);
    assert_eq!(partial.appearance.zoom_pct, 100);
}

// spec: OUT-222
#[test]
fn settings_are_saved_atomically_and_loaded_from_the_settings_dir() {
    let d = dir("save");
    assert_eq!(
        AppSettings::load(&d).unwrap(),
        AppSettings::default(),
        "ohne Datei: Standardwerte"
    );
    let mut s = AppSettings::default();
    s.general.notifications = false;
    s.save(&d).unwrap();
    assert!(d.join("settings.toml").is_file());
    assert_eq!(
        fs::read_dir(&d).unwrap().count(),
        1,
        "keine temporäre Datei bleibt liegen"
    );
    assert_eq!(AppSettings::load(&d).unwrap(), s);
    fs::write(d.join("settings.toml"), "appearance = 5").unwrap();
    assert!(AppSettings::load(&d).is_err());
}

// spec: UI-018, UI-120, CRAWL-129, OUT-229, CRAWL-122
#[test]
fn validation_checks_ranges() {
    let mut s = AppSettings::default();
    s.appearance.live_interval_ms = 100;
    s.appearance.zoom_pct = 111;
    s.appearance.accent = "gruen".into();
    s.storage.ram_limit_gb = Some(0.5);
    s.retention.policy = RetentionPolicy::Custom;
    s.retention.custom_days = 0;
    s.retention.keep_last = Some(0);
    s.network.proxy.mode = ProxyMode::Manual;
    s.network.proxy.host = String::new();
    s.network.proxy.port = 0;
    let paths: Vec<_> = s.validate().into_iter().map(|i| i.path).collect();
    for p in [
        "appearance.live_interval_ms",
        "appearance.zoom_pct",
        "appearance.accent",
        "storage.ram_limit_gb",
        "retention.custom_days",
        "retention.keep_last",
        "network.proxy.host",
        "network.proxy.port",
    ] {
        assert!(paths.contains(&p.to_string()), "{p} fehlt in {paths:?}");
    }
    let mut ok = AppSettings::default();
    ok.appearance.live_interval_ms = 250;
    assert!(ok.validate().is_empty());
    ok.appearance.live_interval_ms = 5000;
    assert!(ok.validate().is_empty());
}

// spec: OUT-222
#[test]
fn restart_required_is_flagged_per_key() {
    let old = AppSettings::default();
    let mut new = old.clone();
    new.appearance.theme = ThemeMode::Dark;
    new.storage.crawls_dir = Some("/data/crawls".into());
    let changed = old.changed_keys(&new);
    assert_eq!(changed, ["appearance.theme", "storage.crawls_dir"]);
    let restart = AppSettings::restart_required(changed.iter().map(String::as_str));
    assert_eq!(restart, ["storage.crawls_dir"]);
    assert!(old.changed_keys(&old).is_empty());
}
