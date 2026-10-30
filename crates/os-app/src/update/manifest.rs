use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tauri-v2 update manifest; validation is STUB(M3-U01).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: semver::Version,
    pub notes: String,
    pub pub_date: Option<String>,
    pub platforms: BTreeMap<String, PlatformEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub url: String,
    pub signature: String,
}
/// Platform lookup contract (STUB(M3-U01): supported-platform policy).
pub fn platform_key(os: &str, arch: &str, installer: Option<&str>) -> String {
    match installer {
        Some(kind) => format!("{os}-{arch}-{kind}"),
        None => format!("{os}-{arch}"),
    }
}
