//! Updater build overrides, shared with the Tauri-free regression tests.

use serde_json::{json, Value};

const DEFAULT_REPO: &str = "domdewom2/openspider";

/// Apply only explicitly requested overrides. Return whether the effective key is empty.
pub fn configure(
    config: &mut Value,
    base: &Value,
    key: Option<&str>,
    beta: Option<&str>,
    repo: Option<&str>,
) -> Result<bool, &'static str> {
    let manifest = match beta {
        Some("1") => "latest-beta.json",
        Some("0") | None => "latest.json",
        _ => return Err("OPENSPIDER_UPDATE_BETA must be 0 or 1"),
    };
    if let Some(key) = key {
        config["plugins"]["updater"]["pubkey"] = json!(key);
    }
    // A key-only build needs the default endpoint unless the caller supplied one.
    // A repo variable alone must not alter incoming updater configuration.
    if beta.is_some() || (key.is_some() && config["plugins"]["updater"].get("endpoints").is_none())
    {
        let repo = repo.unwrap_or(DEFAULT_REPO);
        config["plugins"]["updater"]["endpoints"] = json!([format!(
            "https://github.com/{repo}/releases/latest/download/{manifest}"
        )]);
    }
    let effective_key = config["plugins"]["updater"]
        .get("pubkey")
        .unwrap_or(&base["plugins"]["updater"]["pubkey"]);
    Ok(effective_key
        .as_str()
        .is_none_or(|key| key.trim().is_empty()))
}

pub fn warn_missing_key(profile: Option<&str>, missing_key: bool) -> bool {
    profile == Some("release") && missing_key
}
