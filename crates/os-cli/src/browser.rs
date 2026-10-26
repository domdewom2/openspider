//! Managed headless Chromium installation for OUT-207. No shell or browser UI is needed.
//! Vendor manifest: https://github.com/GoogleChromeLabs/chrome-for-testing#json-api-endpoints
use crate::exit::CliError;
use os_fetch::{BodyLimit, Fetch, FetchConfig, FetchOutcome, FetchRequest, HttpFetcher};
use os_model::{CrawlConfig, FetchPurpose, SystemClock};
use os_store::paths::AppPaths;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const MANIFEST: &str = "https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json";
fn platform() -> Result<&'static str, CliError> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        ("windows", "x86_64") => Ok("win64"),
        ("linux", "x86_64") => Ok("linux64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        _ => Err(CliError::usage(
            "Kein Chromium-Download für diese Plattform verfügbar",
        )),
    }
}
fn executable(platform: &str) -> PathBuf {
    let root = PathBuf::from(format!("chrome-{platform}"));
    if platform.starts_with("mac-") {
        root.join("Google Chrome for Testing.app")
            .join("Contents")
            .join("MacOS")
            .join("Google Chrome for Testing")
    } else if platform.starts_with("win") {
        root.join("chrome.exe")
    } else {
        root.join("chrome")
    }
}
pub fn installed(paths: &AppPaths) -> Option<PathBuf> {
    let root = paths.settings.join("browser");
    let version = std::fs::read_to_string(root.join("current")).ok()?;
    if !valid_version(version.trim()) {
        return None;
    }
    let path = root.join(version.trim()).join(executable(platform().ok()?));
    path.is_file().then_some(path)
}
fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && version.bytes().any(|b| b.is_ascii_digit())
}

/// Select the platform's stable download; reject unrelated or insecure URLs.
pub fn download(
    manifest: &serde_json::Value,
    platform: &str,
) -> Result<(String, String), CliError> {
    let stable = &manifest["channels"]["Stable"];
    let version = stable["version"]
        .as_str()
        .filter(|v| valid_version(v))
        .ok_or_else(|| CliError::usage("Ungültiges Chromium-Manifest"))?;
    let url = stable["downloads"]["chrome"]
        .as_array()
        .and_then(|entries| entries.iter().find(|e| e["platform"] == platform))
        .and_then(|e| e["url"].as_str())
        .ok_or_else(|| CliError::usage("Chromium-Download für diese Plattform fehlt"))?;
    let parsed = os_url::parse_start_url(url)
        .map_err(|_| CliError::usage("Ungültige Chromium-Download-URL"))?
        .url;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("storage.googleapis.com")
        || !parsed.path().starts_with("/chrome-for-testing-public/")
    {
        return Err(CliError::usage("Ungültige Chromium-Download-Quelle"));
    }
    Ok((version.into(), url.into()))
}
async fn fetch(fetcher: &HttpFetcher, url: &str, limit: u64) -> Result<Vec<u8>, CliError> {
    let url = os_url::parse_start_url(url)
        .map_err(|e| CliError::usage(e.to_string()))?
        .url;
    let response = fetcher
        .fetch(FetchRequest {
            url,
            purpose: FetchPurpose::Resource,
            cookies: None,
            max_body_bytes: limit,
            attempt: 1,
            body_limit: BodyLimit {
                max_decoded_bytes: limit,
                abort_media_after: None,
            },
        })
        .await;
    match response.outcome {
        FetchOutcome::Response(r)
            if r.status == 200 && !r.truncated && r.decode_error.is_none() =>
        {
            Ok(r.body.to_vec())
        }
        _ => Err(CliError::usage(
            "Chromium-Download fehlgeschlagen oder unvollständig; bitte erneut ausführen",
        )),
    }
}
/// Extract into a private directory, validate the executable, then publish a versioned installation.
pub fn unpack(
    bytes: &[u8],
    root: &Path,
    version: &str,
    platform: &str,
) -> Result<PathBuf, CliError> {
    let error = |e: std::io::Error| CliError::usage(format!("Chromium-Installation: {e}"));
    if !valid_version(version) {
        return Err(CliError::usage("Ungültige Chromium-Version"));
    }
    std::fs::create_dir_all(root).map_err(error)?;
    let stage = crate::staging::Staging::new(root).map_err(error)?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| CliError::usage(e.to_string()))?;
    let mut total = 0u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| CliError::usage(e.to_string()))?;
        total = total.saturating_add(entry.size());
        if entry.enclosed_name().is_none() || total > 2 * 1024 * 1024 * 1024 {
            return Err(CliError::usage("Ungültiges Chromium-Archiv"));
        }
    }
    archive
        .extract(&stage.path)
        .map_err(|e| CliError::usage(e.to_string()))?;
    let relative = executable(platform);
    if !stage.path.join(&relative).is_file() {
        return Err(CliError::usage("Chromium-Programm fehlt im Archiv"));
    }
    let destination = root.join(version);
    if !destination.exists() {
        std::fs::rename(&stage.path, &destination).map_err(error)?;
    }
    if !destination.join(&relative).is_file() {
        return Err(CliError::usage(
            "Vorhandene Chromium-Installation ist unvollständig",
        ));
    }
    // A versioned executable remains valid while another process is rendering with it.
    let pointer = stage.path.with_extension("current");
    std::fs::write(&pointer, version).map_err(error)?;
    #[cfg(windows)]
    if root.join("current").exists() {
        std::fs::remove_file(root.join("current")).map_err(error)?;
    }
    std::fs::rename(&pointer, root.join("current")).map_err(error)?;
    Ok(destination.join(relative))
}
pub async fn install(paths: &AppPaths) -> Result<PathBuf, CliError> {
    let secrets = os_fetch::secret_store_from_env();
    let mut cfg = FetchConfig::from_crawl_config(
        &CrawlConfig::default(),
        secrets.as_ref(),
        Arc::new(SystemClock),
    )
    .map_err(|e| CliError::usage(e.to_string()))?;
    cfg.network = os_model::settings::AppSettings::load(&paths.settings)
        .map_err(|e| CliError::usage(e.to_string()))?
        .network;
    cfg.response_timeout = std::time::Duration::from_secs(600);
    let fetcher = HttpFetcher::new(cfg).map_err(|e| CliError::usage(e.to_string()))?;
    let manifest = fetch(&fetcher, MANIFEST, 4 * 1024 * 1024).await?;
    let manifest = serde_json::from_slice(&manifest)
        .map_err(|_| CliError::usage("Ungültiges Chromium-Manifest"))?;
    let platform = platform()?;
    let (version, url) = download(&manifest, platform)?;
    let bytes = fetch(&fetcher, &url, 500 * 1024 * 1024).await?;
    unpack(&bytes, &paths.settings.join("browser"), &version, platform)
}
