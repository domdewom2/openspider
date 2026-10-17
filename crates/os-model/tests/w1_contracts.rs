//! Infrastruktur (M2-F05a); fachliche Auswertung folgt in den Welle-1-Paketen.
use os_model::{CancelToken, CrawlConfig, LinkPosition};

// spec: CRAWL-080
// Infrastruktur (M2-F05a) für CRAWL-080; Fachprüfung M2-R01.
#[test]
fn fractional_ajax_timeouts_and_legacy_integer_are_validated() {
    let fixture = include_str!("../../../tests/fixtures/config/default.osconfig");
    for (value, valid) in [(0.1, true), (59.9, true), (60.1, false), (0.15, false)] {
        let mut cfg: CrawlConfig = toml::from_str(fixture).unwrap();
        cfg.rendering.ajax_timeout_s = value;
        assert_eq!(cfg.validate().is_ok(), valid, "{value}");
    }
    let cfg: CrawlConfig =
        toml::from_str(&fixture.replace("ajax_timeout_s = 5.0", "ajax_timeout_s = 5")).unwrap();
    assert_eq!(cfg.rendering.ajax_timeout_s, 5.0);
}

// spec: INTEG-082
// Infrastruktur (M2-F05a) für INTEG-082; Fachprüfung M2-D16.
#[test]
fn custom_link_positions_round_trip_without_losing_the_name() {
    let position = LinkPosition::Custom("Product recommendations".into());
    let json = serde_json::to_string(&position).unwrap();
    assert_eq!(json, "\"Product recommendations\"");
    assert_eq!(
        serde_json::from_str::<LinkPosition>(&json).unwrap(),
        position
    );
    assert_eq!(
        "Sidebar".parse::<LinkPosition>().unwrap(),
        LinkPosition::Sidebar
    );
}

// spec: CRAWL-141
// Infrastruktur (M2-F05a) für CRAWL-141; Fachprüfung M2-K03.
#[test]
fn cancellation_releases_a_paused_checkpoint() {
    let token = CancelToken::default();
    token.pause();
    let worker = token.clone();
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        worker.checkpoint()
    });
    started_rx.recv().unwrap();
    token.set_progress(0.25);
    assert_eq!(token.progress(), 0.25);
    token.cancel();
    assert!(thread.join().unwrap().is_err());
}

// spec: INTEG-005
// Infrastruktur (M2-F05a) für INTEG-005; Fachprüfung M2-I01.
#[test]
fn new_numeric_contracts_enforce_boundaries_after_fixture_roundtrip() {
    let fixture = include_str!("../../../tests/fixtures/config/default.osconfig");
    let cfg: CrawlConfig = toml::from_str(fixture).unwrap();
    for (path, min, max) in [
        ("/images/incorrect_size_min_saving_kb", 0, 1_000_000),
        ("/custom_search/regex_timeout_ms", 1, u32::MAX as u64),
        ("/api/ga4/limits/retries", 0, 20),
        ("/api/ga4/limits/concurrency", 1, 50),
        ("/api/ga4/limits/window_s", 1, u32::MAX as u64),
        ("/serp/mobile/title_max_px", 1, 4_000),
        ("/serp/tablet/description_max_px", 1, 4_000),
    ] {
        for (value, valid) in [(min, true), (max, true), (max + 1, false)] {
            let mut json = serde_json::to_value(&cfg).unwrap();
            *json.pointer_mut(path).unwrap() = value.into();
            let parsed = serde_json::from_value::<CrawlConfig>(json);
            let ok = parsed.is_ok_and(|c| c.validate().is_ok());
            assert_eq!(ok, valid, "{path}={value}");
        }
        if min > 0 {
            let mut json = serde_json::to_value(&cfg).unwrap();
            *json.pointer_mut(path).unwrap() = (min - 1).into();
            assert!(
                !serde_json::from_value::<CrawlConfig>(json)
                    .unwrap()
                    .validate()
                    .is_ok(),
                "{path}"
            );
        }
    }
}

// spec: INTEG-081
// Infrastruktur (M2-F05a) für INTEG-081; Fachprüfung M2-D16.
#[test]
fn link_position_rules_require_a_final_content_fallback() {
    let mut cfg: CrawlConfig = toml::from_str(include_str!(
        "../../../tests/fixtures/config/default.osconfig"
    ))
    .unwrap();
    assert!(cfg.validate().is_ok());
    cfg.link_positions.rules.pop();
    assert!(!cfg.validate().is_ok());
}

// spec: CRAWL-140
// Infrastruktur (M2-F05a) für CRAWL-140; Fachprüfung M2-K07.
#[test]
fn all_device_fonts_and_api_limits_validate_loaded_and_cli_values() {
    let fixture = include_str!("../../../tests/fixtures/config/default.osconfig");
    let cfg: CrawlConfig = toml::from_str(fixture).unwrap();
    for device in ["desktop", "mobile", "tablet"] {
        for field in ["title_font_px", "description_font_px"] {
            let path = format!("serp.{device}.{field}");
            for (value, valid) in [(5.9, false), (6.0, true), (64.0, true), (64.1, false)] {
                let mut json = serde_json::to_value(&cfg).unwrap();
                *json
                    .pointer_mut(&format!("/{}", path.replace('.', "/")))
                    .unwrap() = value.into();
                let loaded: CrawlConfig = serde_json::from_value(json).unwrap();
                assert_eq!(loaded.validate().is_ok(), valid, "{path}={value}");
                let mut cli = cfg.clone();
                assert_eq!(
                    cli.set_value(&path, &value.to_string()).is_ok(),
                    valid,
                    "{path}"
                );
            }
        }
    }
    let json = serde_json::to_value(&cfg).unwrap();
    for source in json["api"].as_object().unwrap().keys() {
        if json["api"][source].get("limits").is_none() {
            continue;
        }
        for (field, invalid) in [("retries", 21), ("concurrency", 0), ("window_s", 0)] {
            let path = format!("api.{source}.limits.{field}");
            let mut cli = cfg.clone();
            assert!(
                cli.set_value(&path, &invalid.to_string()).is_err(),
                "{path}"
            );
        }
    }
}

// spec: CRAWL-073, CRAWL-074, CRAWL-075, CRAWL-102
// Infrastruktur (M2-F05a); Fachprüfung M2-E01/E02.
#[test]
fn unlimited_limits_and_custom_robot_files_validate_against_fixture() {
    let mut cfg: CrawlConfig = toml::from_str(include_str!(
        "../../../tests/fixtures/config/default.osconfig"
    ))
    .unwrap();
    for path in [
        "limits.max_url_length",
        "limits.max_links_per_url",
        "limits.max_page_kb",
    ] {
        cfg.set_value(path, "0").unwrap();
    }
    assert!(cfg.validate().is_ok());
    cfg.robots
        .custom_files
        .insert("site.test".into(), "x".repeat(512 * 1024));
    assert!(cfg.validate().is_ok());
    cfg.robots
        .custom_files
        .get_mut("site.test")
        .unwrap()
        .push('x');
    assert!(!cfg.validate().is_ok());
    cfg.robots.custom_files.clear();
    for host in [
        "https://site.test",
        "-bad.test",
        "bad..test",
        "site.test/path",
    ] {
        cfg.robots.custom_files.insert(host.into(), String::new());
        assert!(!cfg.validate().is_ok(), "{host}");
        cfg.robots.custom_files.clear();
    }
}
