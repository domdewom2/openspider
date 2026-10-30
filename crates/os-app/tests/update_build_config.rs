// Test the actual build helper without linking Tauri or its Linux plugins.
#[path = "../../../apps/desktop/src-tauri/build_update.rs"]
mod build_update;

use build_update::{configure, warn_missing_key};
use serde_json::{json, Value};

fn incoming() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/update/build-config.json"
    ))
    .unwrap()
}

fn base() -> Value {
    serde_json::from_str(include_str!(
        "../../../apps/desktop/src-tauri/tauri.conf.json"
    ))
    .unwrap()
}

// spec: OUT-250 [M3]
#[test]
fn no_build_variables_preserve_the_entire_incoming_config() {
    for mut config in [incoming(), json!({})] {
        let original = config.clone();
        configure(&mut config, &base(), None, None, None).unwrap();
        assert_eq!(config, original);
        configure(&mut config, &base(), None, None, Some("example/alternate")).unwrap();
        assert_eq!(config, original, "repo alone must not enable overrides");
    }
}

// spec: OUT-250 [M3]
#[test]
fn key_override_preserves_custom_endpoints_and_other_plugin_options() {
    let mut config = incoming();
    let mut expected = config.clone();
    expected["plugins"]["updater"]["pubkey"] = json!("");
    assert!(configure(&mut config, &base(), Some(""), None, None).unwrap());
    assert_eq!(config, expected);
}

// spec: OUT-250 [M3]
#[test]
fn beta_selects_the_manifest_without_replacing_the_incoming_key() {
    for (beta, manifest) in [("0", "latest.json"), ("1", "latest-beta.json")] {
        let mut config = incoming();
        let mut expected = config.clone();
        expected["plugins"]["updater"]["endpoints"] = json!([format!(
            "https://github.com/example/alternate/releases/latest/download/{manifest}"
        )]);
        assert!(!configure(
            &mut config,
            &base(),
            None,
            Some(beta),
            Some("example/alternate")
        )
        .unwrap());
        assert_eq!(config, expected);
    }
}

// spec: OUT-250 [M3]
#[test]
fn key_only_builds_supply_the_default_endpoint_when_no_override_exists() {
    let fixture = incoming();
    let mut config = json!({});
    assert!(!configure(
        &mut config,
        &base(),
        fixture["plugins"]["updater"]["pubkey"].as_str(),
        None,
        None
    )
    .unwrap());
    let endpoint = config["plugins"]["updater"]["endpoints"][0]
        .as_str()
        .unwrap();
    assert!(endpoint.starts_with("https://github.com/"));
    assert!(endpoint.ends_with("/releases/latest/download/latest.json"));
    assert!(!base().to_string().contains("github.com"));
}

// spec: OUT-250 [M3]
#[test]
fn release_warning_uses_the_effective_key_including_base_config() {
    for (mut config, base, missing) in [
        (json!({}), base(), true),
        (incoming(), base(), false),
        (json!({}), incoming(), false),
        (
            json!({"plugins":{"updater":{"pubkey":""}}}),
            incoming(),
            true,
        ),
    ] {
        let missing_key = configure(&mut config, &base, None, None, None).unwrap();
        assert_eq!(missing_key, missing);
        assert_eq!(warn_missing_key(Some("release"), missing_key), missing);
        assert!(!warn_missing_key(Some("debug"), missing_key));
        assert!(!warn_missing_key(None, missing_key));
    }
}

// spec: OUT-250 [M3]
#[test]
fn invalid_beta_value_fails_without_mutating_configuration() {
    let mut config = incoming();
    let original = config.clone();
    assert!(configure(&mut config, &base(), Some(""), Some("true"), None).is_err());
    assert_eq!(config, original);
}
