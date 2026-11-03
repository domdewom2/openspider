//! Ablage des bereitgestellten Chromium: Prüfsumme, Entpacken in einen privaten Ordner, atomarer Austausch über die
//! Zeigerdatei `current`, Entfernen.
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use sha2::{Digest, Sha256};

use super::{valid_version, Plan, Platform, ProvisionError};

/// Zeigerdatei mit dem Namen der aktiven Version.
pub const CURRENT: &str = "current";
/// Ordner der unvollständigen Downloads (fortsetzbar).
pub const PARTIAL_DIR: &str = ".partial";
const STAGING_PREFIX: &str = ".staging-";
const TRASH_PREFIX: &str = ".trash-";
/// Obergrenzen gegen manipulierte Archive (die echten Archive haben < 1 000 Einträge und < 500 MB).
const MAX_ENTRIES: usize = 50_000;
const MAX_UNPACKED: u64 = 4 * 1024 * 1024 * 1024;

/// Eine bereitgestellte Installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub version: String,
    pub executable: PathBuf,
}

/// Aktive Installation unter `root` für `platform`, falls vollständig vorhanden.
pub fn installed(root: &Path, platform: Platform) -> Option<Installed> {
    let version = fs::read_to_string(root.join(CURRENT)).ok()?;
    let version = version.trim();
    if !valid_version(version) {
        return None;
    }
    let executable = root.join(version).join(platform.executable());
    executable.is_file().then(|| Installed {
        version: version.to_string(),
        executable,
    })
}

/// Datei des (unvollständigen) Downloads eines Plans.
pub fn partial_path(root: &Path, plan: &Plan) -> PathBuf {
    root.join(PARTIAL_DIR).join(format!(
        "chrome-{}-{}.zip",
        plan.platform.key(),
        plan.version
    ))
}

/// Bereits geladene Bytes eines abgebrochenen Downloads (0 = keiner).
pub fn partial_bytes(root: &Path, plan: &Plan) -> u64 {
    fs::metadata(partial_path(root, plan)).map_or(0, |m| m.len())
}

/// Verwirft alle unvollständigen Downloads.
pub fn discard_partials(root: &Path) -> io::Result<()> {
    match fs::remove_dir_all(root.join(PARTIAL_DIR)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// SHA-256 einer Datei als Hex (klein).
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// SHA-256 von Bytes als Hex (klein).
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn unique(root: &Path, prefix: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    root.join(format!(
        "{prefix}{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Entfernt den Arbeitsordner, falls die Installation nicht veröffentlicht wurde.
struct Stage(Option<PathBuf>);
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(path) = self.0.take() {
            let _ = fs::remove_dir_all(path);
        }
    }
}

/// Prüft das (bereits per SHA-256 bestätigte) Archiv, entpackt es privat und veröffentlicht es atomar.
pub fn unpack_and_publish(
    archive: &Path,
    root: &Path,
    plan: &Plan,
) -> Result<Installed, ProvisionError> {
    if !valid_version(&plan.version) {
        return Err(ProvisionError::Manifest(format!(
            "ungültige Version {:?}",
            plan.version
        )));
    }
    let file = fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(io::BufReader::new(file))
        .map_err(|e| ProvisionError::Archive(e.to_string()))?;
    if zip.len() > MAX_ENTRIES {
        return Err(ProvisionError::Archive("zu viele Einträge".into()));
    }
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip
            .by_index(i)
            .map_err(|e| ProvisionError::Archive(e.to_string()))?;
        total = total.saturating_add(entry.size());
        if entry.enclosed_name().is_none() {
            return Err(ProvisionError::Archive(format!(
                "Eintrag außerhalb des Zielordners: {:?}",
                entry.name()
            )));
        }
        if total > MAX_UNPACKED {
            return Err(ProvisionError::Archive("entpackt zu groß".into()));
        }
    }
    fs::create_dir_all(root)?;
    let stage = Stage(Some(unique(root, STAGING_PREFIX)));
    let stage_path = stage.0.clone().expect("stage");
    fs::create_dir(&stage_path)?;
    zip.extract(&stage_path)
        .map_err(|e| ProvisionError::Archive(e.to_string()))?;
    let relative = plan.platform.executable();
    let executable = stage_path.join(&relative);
    if !executable.is_file() {
        return Err(ProvisionError::Archive(format!(
            "Programm {} fehlt im Archiv",
            relative.display()
        )));
    }
    ensure_executable(&executable)?;
    publish(stage, root, &plan.version, &relative)
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    if permissions.mode() & 0o111 != 0o111 {
        permissions.set_mode(permissions.mode() | 0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn ensure_executable(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Verschiebt den fertigen Arbeitsordner an `root/<version>` und setzt danach `current` per Umbenennen. Ein laufender
/// Renderer behält seinen Pfad; alte Versionen werden bestmöglich entfernt (unter Windows ggf. erst beim nächsten Mal).
fn publish(
    mut stage: Stage,
    root: &Path,
    version: &str,
    relative: &Path,
) -> Result<Installed, ProvisionError> {
    let stage_path = stage.0.clone().expect("stage");
    let destination = root.join(version);
    if fs::symlink_metadata(&destination).is_ok() {
        let trash = unique(root, TRASH_PREFIX);
        fs::rename(&destination, &trash)
            .map_err(|e| ProvisionError::InUse(format!("{}: {e}", destination.display())))?;
        let _ = fs::remove_dir_all(&trash);
    }
    fs::rename(&stage_path, &destination)?;
    stage.0 = None;
    let pointer = unique(root, ".current-");
    fs::write(&pointer, version)?;
    if let Err(e) = fs::rename(&pointer, root.join(CURRENT)) {
        let _ = fs::remove_file(&pointer);
        return Err(e.into());
    }
    cleanup(root, Some(version));
    Ok(Installed {
        version: version.to_string(),
        executable: destination.join(relative),
    })
}

/// Entfernt alte Versionen und Papierkorb-Ordner; Fehler werden gesammelt, nicht abgebrochen.
fn cleanup(root: &Path, keep: Option<&str>) -> Vec<String> {
    let mut failed = Vec::new();
    let Ok(entries) = fs::read_dir(root) else {
        return failed;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let stale = (valid_version(&name) && Some(name.as_str()) != keep)
            || name.starts_with(TRASH_PREFIX)
            || (keep.is_none() && name.starts_with(STAGING_PREFIX));
        if stale && entry.path().is_dir() {
            if let Err(e) = fs::remove_dir_all(entry.path()) {
                failed.push(format!("{name}: {e}"));
            }
        }
    }
    failed
}

/// Entfernt die bereitgestellte Installation samt unvollständigen Downloads. Zuerst verschwindet `current`, damit
/// die Auswahl sofort auf System-Chrome zurückfällt; ein noch laufendes Chromium (Windows) meldet [`ProvisionError::InUse`].
pub fn remove(root: &Path) -> Result<(), ProvisionError> {
    match fs::remove_file(root.join(CURRENT)) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
        _ => {}
    }
    discard_partials(root)?;
    let failed = cleanup(root, None);
    if !failed.is_empty() {
        return Err(ProvisionError::InUse(failed.join("; ")));
    }
    // Leere Ablage aufräumen (bleibt bestehen, wenn sie fremde Dateien enthält).
    let _ = fs::remove_dir(root);
    Ok(())
}
