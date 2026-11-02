//! M3-K01: Chromium-Bereitstellung nach K3-01 gegen den os-testsite-Mock (`mocks::chromium`): Download, Prüfsumme vor
//! dem Entpacken, atomarer Austausch, Abbruch und Fortsetzung, Entfernen. Der Transport ist hier ein minimaler
//! HTTP/1.1-Client; der App-Transport mit Proxy wird in `os-app` (`render_provision.rs`) geprüft.
use std::{
    io::{Cursor, Write},
    path::Path,
    sync::{Arc, Mutex},
};

use futures_util::{future::BoxFuture, StreamExt};
use os_render::provision::{
    self, download, install, Cancel, Manifest, Phase, Plan, Platform, Progress, ProvisionError,
    Response, Transport,
};
use os_testsite::mocks::{chromium::ChromiumMock, MockServer};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

/// Plattform der Fixture-Archive. Fest gewählt, damit die Tests auf jedem Host dieselbe Ablage prüfen.
const PLATFORM: Platform = Platform::MacArm64;

fn zip_name(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_str().unwrap())
        .collect::<Vec<_>>()
        .join("/")
}

/// Archiv im Aufbau von „Chrome for Testing“: Programm (ausführbar), Ressource und ein Symlink wie in den
/// Framework-Ordnern der macOS-Archive.
fn archive_with(platform: Platform, extra: &[(&str, &[u8])]) -> Vec<u8> {
    use zip::write::SimpleFileOptions;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let key = platform.key();
    zip.start_file(
        zip_name(&platform.executable()),
        SimpleFileOptions::default().unix_permissions(0o755),
    )
    .unwrap();
    zip.write_all(b"fixture chromium executable").unwrap();
    zip.start_file(
        format!("chrome-{key}/resources.pak"),
        SimpleFileOptions::default().unix_permissions(0o644),
    )
    .unwrap();
    // Etwas nicht komprimierbare Masse, damit Fortschritt und Abbruch mehrere Stücke sehen.
    zip.write_all(&noise(256 * 1024)).unwrap();
    zip.add_symlink(
        format!("chrome-{key}/Current"),
        "resources.pak",
        SimpleFileOptions::default(),
    )
    .unwrap();
    for (name, data) in extra {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn archive() -> Vec<u8> {
    archive_with(PLATFORM, &[])
}

/// Deterministisches Rauschen (lineare Kongruenz), damit das Archiv nicht auf wenige Byte schrumpft.
fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x2545_f491_4f6c_dd1du64;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 56) as u8
        })
        .collect()
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn plan(url: String, bytes: &[u8], version: &str) -> Plan {
    let json = serde_json::json!({
        "schema": 1,
        "product": "Chrome for Testing",
        "version": version,
        "source": "fixture",
        "platforms": { PLATFORM.key(): { "url": url, "size": bytes.len(), "sha256": sha256(bytes) } }
    });
    Manifest::parse(&json.to_string())
        .unwrap()
        .plan(PLATFORM)
        .unwrap()
}

/// Minimaler HTTP/1.1-Client (nur `http://`, `Connection: close`, Körper bis `Content-Length`).
struct Raw;
impl Transport for Raw {
    fn get<'a>(&'a self, url: &'a str, offset: u64) -> BoxFuture<'a, Result<Response, String>> {
        Box::pin(async move {
            let rest = url.strip_prefix("http://").ok_or("nur http")?;
            let (host, path) = rest.split_once('/').ok_or("kein Pfad")?;
            let mut stream = tokio::net::TcpStream::connect(host)
                .await
                .map_err(|e| e.to_string())?;
            let range = if offset > 0 {
                format!("Range: bytes={offset}-\r\n")
            } else {
                String::new()
            };
            stream
                .write_all(
                    format!(
                        "GET /{path} HTTP/1.1\r\nHost: {host}\r\n{range}Connection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .map_err(|e| e.to_string())?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .map_err(|e| e.to_string())?;
            let status = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or("Statuszeile")?;
            let (mut content_range, mut length) = (None, None);
            loop {
                line.clear();
                reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| e.to_string())?;
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    match name.to_ascii_lowercase().as_str() {
                        "content-range" => content_range = Some(value.trim().to_string()),
                        "content-length" => length = value.trim().parse::<u64>().ok(),
                        _ => {}
                    }
                }
            }
            let length = length.unwrap_or(0);
            let body =
                futures_util::stream::unfold((reader, 0u64), move |(mut reader, got)| async move {
                    if got >= length {
                        return None;
                    }
                    let mut buf = vec![0u8; 16 * 1024];
                    match reader.read(&mut buf).await {
                        Ok(0) => Some((
                            Err("Verbindung vorzeitig geschlossen".into()),
                            (reader, length),
                        )),
                        Ok(n) => {
                            buf.truncate(n);
                            Some((Ok(buf), (reader, got + n as u64)))
                        }
                        Err(e) => Some((Err(e.to_string()), (reader, length))),
                    }
                })
                .boxed();
            Ok(Response {
                status,
                content_range,
                body,
            })
        })
    }
}

#[derive(Default, Clone)]
struct Seen(Arc<Mutex<Vec<Progress>>>);
impl Seen {
    fn callback(&self) -> impl Fn(Progress) + Send + Sync {
        let seen = self.0.clone();
        move |p| seen.lock().unwrap().push(p)
    }
    fn all(&self) -> Vec<Progress> {
        self.0.lock().unwrap().clone()
    }
}

async fn server(bytes: &[u8]) -> (MockServer, ChromiumMock) {
    let mock = ChromiumMock::new(bytes.to_vec());
    let server = MockServer::start(vec![mock.clone().into_api()])
        .await
        .unwrap();
    (server, mock)
}

fn assert_nothing_installed(root: &Path) {
    assert!(provision::installed(root, PLATFORM).is_none());
    assert!(!root.join(install::CURRENT).exists());
    let leftovers: Vec<_> = std::fs::read_dir(root)
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n != install::PARTIAL_DIR)
                .collect()
        })
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

// spec: OUT-260 [M3]
// spec: OUT-265 [M3]
#[tokio::test]
async fn downloads_verifies_and_publishes_atomically() {
    let bytes = archive();
    let (server, mock) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let plan = plan(
        format!("{}/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    let seen = Seen::default();
    let installed = download::provision(&plan, &root, &Raw, &Cancel::new(), &seen.callback())
        .await
        .unwrap();
    assert!(bytes.len() > 256 * 1024);
    assert_eq!(installed.version, "156.0.8078.80");
    assert_eq!(
        installed.executable,
        root.join("156.0.8078.80").join(PLATFORM.executable())
    );
    assert_eq!(
        provision::installed(&root, PLATFORM),
        Some(installed.clone())
    );
    assert_eq!(
        std::fs::read_to_string(root.join(install::CURRENT)).unwrap(),
        "156.0.8078.80"
    );
    // Archiv nach dem Entpacken verworfen, keine Arbeitsordner übrig.
    assert_eq!(provision::partial_bytes(&root, &plan), 0);
    let names: Vec<_> = std::fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != install::PARTIAL_DIR)
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed.executable)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111, "Programm ausführbar");
        let link = root
            .join("156.0.8078.80")
            .join(format!("chrome-{}", PLATFORM.key()))
            .join("Current");
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
    }
    // Fortschritt: Download bis zur vollen Größe, dann Prüfung, dann Installation.
    let progress = seen.all();
    let phases: Vec<_> = progress.iter().map(|p| p.phase).collect();
    assert_eq!(phases.first(), Some(&Phase::Downloading));
    assert!(progress
        .windows(2)
        .all(|w| w[0].done <= w[1].done || w[1].phase != Phase::Downloading));
    let verifying = phases.iter().position(|p| *p == Phase::Verifying).unwrap();
    assert_eq!(progress[verifying].done, bytes.len() as u64);
    assert_eq!(phases.last(), Some(&Phase::Installing));
    assert_eq!(mock.requests().len(), 1);
    assert_eq!(mock.requests()[0].range, None);
    server.stop().await;
}

// spec: OUT-265 [M3]
#[tokio::test]
async fn wrong_checksum_is_rejected_before_unpacking() {
    let bytes = archive();
    let (server, _) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let mut plan = plan(
        format!("{}/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    plan.archive.sha256 = sha256(b"something else");
    let seen = Seen::default();
    let error = download::provision(&plan, &root, &Raw, &Cancel::new(), &seen.callback())
        .await
        .unwrap_err();
    match &error {
        ProvisionError::Checksum { expected, actual } => {
            assert_eq!(expected, &plan.archive.sha256);
            assert_eq!(actual, &sha256(&bytes));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(error.message_key(), "err-chromium-checksum");
    // Nicht entpackt: weder Version noch Arbeitsordner, und das verworfene Archiv ist weg.
    assert!(!seen.all().iter().any(|p| p.phase == Phase::Installing));
    assert_nothing_installed(&root);
    assert_eq!(provision::partial_bytes(&root, &plan), 0);
    server.stop().await;
}

// spec: OUT-260 [M3]
#[tokio::test]
async fn cancelled_download_resumes_with_range() {
    let bytes = archive();
    let (server, mock) = server(&bytes).await;
    let cut = bytes.len() / 2;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let plan = plan(
        format!("{}/stall/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    let cancel = Cancel::new();
    let seen = Seen::default();
    let task = {
        let (plan, root, cancel, seen) = (plan.clone(), root.clone(), cancel.clone(), seen.clone());
        tokio::spawn(async move {
            download::provision(&plan, &root, &Raw, &cancel, &seen.callback()).await
        })
    };
    // Warten, bis die erste Hälfte da ist; danach stockt der Mock.
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        while !seen.all().iter().any(|p| p.done >= cut as u64) {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    let error = tokio::time::timeout(std::time::Duration::from_secs(10), task)
        .await
        .expect("Abbruch wirkt trotz stockender Übertragung")
        .unwrap()
        .unwrap_err();
    assert_eq!(error, ProvisionError::Cancelled);
    assert_eq!(provision::partial_bytes(&root, &plan), cut as u64);
    assert_nothing_installed(&root);

    let installed = download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap();
    assert!(installed.executable.is_file());
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1].range.as_deref(),
        Some(&*format!("bytes={cut}-"))
    );
    server.stop().await;
}

// spec: OUT-260 [M3]
#[tokio::test]
async fn interrupted_transfer_keeps_partial_and_resumes() {
    let bytes = archive();
    let (server, mock) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let plan = plan(
        format!("{}/cut/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    let error = download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap_err();
    assert!(matches!(error, ProvisionError::Network(_)), "{error:?}");
    let kept = provision::partial_bytes(&root, &plan);
    assert!(kept > 0 && kept < bytes.len() as u64, "{kept}");
    assert_nothing_installed(&root);
    download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap();
    assert_eq!(
        mock.requests()[1].range.as_deref(),
        Some(&*format!("bytes={kept}-"))
    );
    assert!(provision::installed(&root, PLATFORM).is_some());
    server.stop().await;
}

// spec: OUT-260 [M3]
#[tokio::test]
async fn server_without_range_support_restarts_cleanly() {
    let bytes = archive();
    let (server, mock) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let plan = plan(
        format!("{}/norange/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    // Teil-Download mit falschem Inhalt: wird ersetzt, nicht angehängt.
    let part = install::partial_path(&root, &plan);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, vec![0u8; 1000]).unwrap();
    download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap();
    assert_eq!(mock.requests()[0].range.as_deref(), Some("bytes=1000-"));
    assert!(provision::installed(&root, PLATFORM).is_some());
    server.stop().await;
}

// spec: OUT-265 [M3]
#[tokio::test]
async fn oversized_transfer_is_discarded() {
    let bytes = archive();
    let (server, _) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let mut plan = plan(
        format!("{}/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    plan.archive.size = bytes.len() as u64 - 10;
    let error = download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap_err();
    assert!(
        matches!(error, ProvisionError::TooLarge { .. }),
        "{error:?}"
    );
    assert_eq!(provision::partial_bytes(&root, &plan), 0);
    assert_nothing_installed(&root);
    server.stop().await;
}

// spec: OUT-265 [M3]
#[tokio::test]
async fn archive_escaping_the_target_is_rejected() {
    let bytes = archive_with(PLATFORM, &[("../escape.txt", b"outside")]);
    let (server, _) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let plan = plan(
        format!("{}/archive.zip", server.base_url("chromium")),
        &bytes,
        "156.0.8078.80",
    );
    let error = download::provision(&plan, &root, &Raw, &Cancel::new(), &|_| {})
        .await
        .unwrap_err();
    assert!(matches!(error, ProvisionError::Archive(_)), "{error:?}");
    assert!(!dir.path().join("escape.txt").exists());
    assert_nothing_installed(&root);
    server.stop().await;
}

// spec: OUT-260 [M3]
#[tokio::test]
async fn new_version_replaces_old_and_remove_cleans_up() {
    let bytes = archive();
    let (server, _) = server(&bytes).await;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("browser");
    let url = format!("{}/archive.zip", server.base_url("chromium"));
    for version in ["155.0.1.2", "156.0.8078.80"] {
        download::provision(
            &plan(url.clone(), &bytes, version),
            &root,
            &Raw,
            &Cancel::new(),
            &|_| {},
        )
        .await
        .unwrap();
    }
    assert_eq!(
        provision::installed(&root, PLATFORM).unwrap().version,
        "156.0.8078.80"
    );
    assert!(!root.join("155.0.1.2").exists(), "alte Version entfernt");
    // Ein liegengebliebener Teil-Download wird beim Entfernen mit verworfen.
    let next = plan(url, &bytes, "157.0.0.1");
    let part = install::partial_path(&root, &next);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, b"partial").unwrap();
    provision::remove(&root).unwrap();
    assert!(provision::installed(&root, PLATFORM).is_none());
    assert!(!root.exists(), "leere Ablage entfernt");
    // Entfernen ohne Installation ist kein Fehler.
    provision::remove(&root).unwrap();
    server.stop().await;
}

#[test]
fn manifest_rejects_invalid_entries() {
    let base = serde_json::json!({
        "schema": 1, "product": "Chrome for Testing", "version": "156.0.8078.80", "source": "x",
        "platforms": { "win64": { "url": "https://storage.googleapis.com/chrome-for-testing-public/156.0.8078.80/win64/chrome-win64.zip", "size": 1, "sha256": "a".repeat(64) } }
    });
    assert!(Manifest::parse(&base.to_string()).is_ok());
    for (pointer, value) in [
        ("/version", serde_json::json!("../156")),
        ("/schema", serde_json::json!(2)),
        ("/platforms/win64/sha256", serde_json::json!("A".repeat(64))),
        ("/platforms/win64/sha256", serde_json::json!("abc")),
        ("/platforms/win64/size", serde_json::json!(0)),
        (
            "/platforms/win64/url",
            serde_json::json!("file:///tmp/chrome.zip"),
        ),
    ] {
        let mut json = base.clone();
        *json.pointer_mut(pointer).unwrap() = value;
        assert!(Manifest::parse(&json.to_string()).is_err(), "{pointer}");
    }
    let mut unknown = base.clone();
    unknown["platforms"]["win32"] = unknown["platforms"]["win64"].clone();
    assert!(Manifest::parse(&unknown.to_string()).is_err());
    // Nur die offizielle HTTPS-Quelle ist für die eingebettete Versionsdatei zulässig.
    let mut foreign = base.clone();
    foreign["platforms"]["win64"]["url"] = "https://example.com/chrome-win64.zip".into();
    let foreign = Manifest::parse(&foreign.to_string()).unwrap();
    assert!(foreign.check_official().is_err());
}

// spec: OUT-260 [M3]
#[test]
fn bundled_versions_file_pins_one_version_for_each_target() {
    let manifest = Manifest::bundled().unwrap();
    assert_eq!(manifest.product, "Chrome for Testing");
    assert!(provision::valid_version(&manifest.version));
    // macOS arm64/x64 nativ, Windows x64 (auch für Windows on ARM per Emulation); Linux nur über Pfad (OUT-273).
    for (os, arch, key) in [
        ("macos", "aarch64", Some("mac-arm64")),
        ("macos", "x86_64", Some("mac-x64")),
        ("windows", "x86_64", Some("win64")),
        ("windows", "aarch64", Some("win64")),
        ("linux", "x86_64", None),
    ] {
        let platform = Platform::for_target(os, arch).unwrap();
        let plan = manifest.plan(platform);
        assert_eq!(
            plan.as_ref().ok().map(|p| p.platform.key()),
            key,
            "{os}/{arch}"
        );
        if let Ok(plan) = plan {
            assert!(plan.archive.url.starts_with(&format!(
                "https://storage.googleapis.com/chrome-for-testing-public/{}/",
                manifest.version
            )));
            assert!(plan.archive.size > 50_000_000);
        } else {
            assert_eq!(
                manifest.plan(platform).unwrap_err(),
                ProvisionError::Unsupported
            );
        }
    }
}

// spec: INTEG-132 [M3]
#[test]
fn provisioning_delivers_only_chromium_no_lighthouse_runtime() {
    // K3-01: Lighthouse bleibt beim vorhandenen Node (INTEG-132, eigene Chrome-Instanz getrennt vom Render-Browser);
    // die Versionsdatei stellt ausschließlich die Chromium-Archive bereit.
    let manifest = Manifest::bundled().unwrap();
    for (key, archive) in &manifest.platforms {
        assert!(
            archive.url.ends_with(&format!("/chrome-{key}.zip")),
            "{}",
            archive.url
        );
        let lower = archive.url.to_ascii_lowercase();
        assert!(!lower.contains("node") && !lower.contains("lighthouse"));
    }
}
