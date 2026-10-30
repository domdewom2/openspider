use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tauri-v2 update metadata (artifact signatures are verified after download).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    #[serde(deserialize_with = "version")]
    pub version: semver::Version,
    #[serde(default)]
    pub notes: String,
    pub pub_date: Option<String>,
    pub platforms: BTreeMap<String, PlatformEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub url: String,
    pub signature: String,
}
/// Includes installer type for custom Windows targets.
pub fn platform_key(os: &str, arch: &str, installer: Option<&str>) -> String {
    match installer {
        Some(kind) => format!("{os}-{arch}-{kind}"),
        None => format!("{os}-{arch}"),
    }
}

fn version<'de, D: serde::Deserializer<'de>>(d: D) -> Result<semver::Version, D::Error> {
    let value = String::deserialize(d)?;
    semver::Version::parse(value.strip_prefix('v').unwrap_or(&value))
        .map_err(serde::de::Error::custom)
}
/// Reject credentials, fragments and non-HTTPS endpoints, including download URLs.
pub fn https_url(value: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Update-URL ungültig")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        tracing::warn!("Update abgelehnt: HTTPS ohne Zugangsdaten erforderlich");
        return Err("Update erfordert HTTPS ohne Zugangsdaten".into());
    }
    Ok(url)
}
impl UpdateManifest {
    pub fn validate(&self, beta: bool) -> Result<(), String> {
        if !beta && !self.version.pre.is_empty() {
            return Err("Vorabversion am stabilen Update-Endpunkt".into());
        }
        if let Some(date) = &self.pub_date {
            time::OffsetDateTime::parse(date, &time::format_description::well_known::Rfc3339)
                .map_err(|_| "Update-Datum ungültig")?;
        }
        if self.platforms.is_empty() {
            return Err("Update ohne Plattformen".into());
        }
        for entry in self.platforms.values() {
            https_url(&entry.url)?;
            super::verify::signature(&entry.signature).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn is_newer_than(&self, current: &semver::Version) -> bool {
        self.version.cmp_precedence(current).is_gt()
    }
}
