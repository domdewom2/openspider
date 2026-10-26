//! Offline Chromium manifest and archive fixtures.
use std::path::{Path, PathBuf};
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/out-m2-cli")
        .join(name)
}

// spec: OUT-207
#[test]
fn browser_installation_uses_vendor_metadata_and_publishes_complete_archives() {
    use std::io::Write;
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture("browser.json")).unwrap()).unwrap();
    let (version, url) = os_cli::browser::download(&manifest, "mac-arm64").unwrap();
    assert_eq!(version, "123.0.0.1");
    assert!(url.ends_with("chrome-mac-arm64.zip"));
    manifest["channels"]["Stable"]["downloads"]["chrome"][0]["url"] =
        serde_json::json!("https://untrusted.test/chrome.zip");
    assert!(os_cli::browser::download(&manifest, "mac-arm64").is_err());
    let root = tempfile::tempdir().unwrap();
    let path =
        "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file(
        path,
        zip::write::SimpleFileOptions::default().unix_permissions(0o755),
    )
    .unwrap();
    zip.write_all(b"browser fixture executable").unwrap();
    let archive = zip.finish().unwrap().into_inner();
    let executable = os_cli::browser::unpack(&archive, root.path(), &version, "mac-arm64").unwrap();
    assert_eq!(
        std::fs::read(executable).unwrap(),
        b"browser fixture executable"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("current")).unwrap(),
        version
    );
    assert!(os_cli::browser::unpack(b"truncated", root.path(), "124.0.0.0", "mac-arm64").is_err());
    assert!(!root.path().join("124.0.0.0").exists());
    assert!(!std::fs::read_dir(root.path()).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".openspider")));
}
