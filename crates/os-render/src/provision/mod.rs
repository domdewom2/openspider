//! Chromium-Bereitstellung nach K3-01 (OUT-260/OUT-265, CRAWL-077/078): Chromium wird nicht gebündelt, sondern auf
//! Anfrage als feste Version „Chrome for Testing“ in den App-Datenordner geladen.
//!
//! - [`Manifest`]: Versionsdatei `resources/chromium/versions.json` (feste Version, URL, Größe und SHA-256 je Plattform).
//! - [`download::provision`]: Download über einen vom Aufrufer gelieferten [`download::Transport`] (die App nutzt den
//!   app-weiten Proxy), fortsetzbar per HTTP-Range, Prüfsumme **vor** dem Entpacken, atomarer Austausch über die
//!   Zeigerdatei `current`.
//! - [`resolve`]: Auswahlreihenfolge konfigurierter Pfad (Konfiguration, dann `OPENSPIDER_CHROMIUM`) → bereitgestelltes
//!   Chromium → System-Chrome; ohne Treffer [`RenderError::ChromiumMissing`] mit Download-/Pfad-Hinweis.
//!
//! Ablage: `<wurzel>/<version>/chrome-<plattform>/…` und `<wurzel>/current` (Versionsname). Das Format entspricht dem
//! der CLI-Installation (`openspider --install-browser`, OUT-207), sodass beide dieselbe Ablage lesen können.
//!
//! macOS: Der Prozess lädt selbst über HTTP und entpackt selbst; dabei entsteht kein Quarantäne-Attribut (das setzen nur
//! Programme mit `LSFileQuarantineEnabled` bzw. Sandbox). Ausführungsrechte kommen aus den Unix-Modi des Archivs; für das
//! Hauptprogramm werden sie zusätzlich sichergestellt ([`install`]).

pub mod download;
pub mod install;
mod system;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use serde::{Deserialize, Serialize};

use crate::RenderError;

pub use download::{Cancel, Phase, Progress, Response, Transport};
pub use install::{installed, partial_bytes, remove, Installed};

/// Versionsdatei, beim Bauen eingebettet (Quelle: `resources/chromium/versions.json`).
pub const VERSIONS_JSON: &str = include_str!("../../../../resources/chromium/versions.json");

/// Umgebungsvariable des App-Basisordners (wie `os_store::paths::HOME_ENV`).
pub const HOME_ENV: &str = "OPENSPIDER_HOME";
/// Umgebungsvariable für einen fest vorgegebenen Chromium-Pfad (Entwicklung, CI, Linux-Server).
pub const CHROMIUM_ENV: &str = "OPENSPIDER_CHROMIUM";

/// Offizielle Quelle der Archive (nur diese ist für die eingebettete Versionsdatei zulässig).
const OFFICIAL_HOST: &str = "storage.googleapis.com";
const OFFICIAL_PREFIX: &str = "/chrome-for-testing-public/";

/// Plattformschlüssel von „Chrome for Testing“.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "mac-arm64")]
    MacArm64,
    #[serde(rename = "mac-x64")]
    MacX64,
    #[serde(rename = "win64")]
    Win64,
    /// Nur zur Auflösung einer vorhandenen Ablage (Linux: Kern und CLI, OUT-273); kein Download in der App.
    #[serde(rename = "linux64")]
    Linux64,
}

impl Platform {
    /// Alle Schlüssel.
    pub const ALL: [Platform; 4] = [
        Platform::MacArm64,
        Platform::MacX64,
        Platform::Win64,
        Platform::Linux64,
    ];

    /// Schlüssel wie in der Versionsdatei.
    pub fn key(self) -> &'static str {
        match self {
            Platform::MacArm64 => "mac-arm64",
            Platform::MacX64 => "mac-x64",
            Platform::Win64 => "win64",
            Platform::Linux64 => "linux64",
        }
    }

    /// Plattform zu einem Schlüssel.
    pub fn from_key(key: &str) -> Option<Platform> {
        Platform::ALL.into_iter().find(|p| p.key() == key)
    }

    /// Plattform zu Betriebssystem und Architektur (`std::env::consts`). Windows auf arm64 nutzt den x64-Build unter
    /// der Emulation (OUT-263); macOS läuft nativ je Architektur (OUT-260/261).
    pub fn for_target(os: &str, arch: &str) -> Option<Platform> {
        match (os, arch) {
            ("macos", "aarch64") => Some(Platform::MacArm64),
            ("macos", "x86_64") => Some(Platform::MacX64),
            ("windows", "x86_64" | "aarch64") => Some(Platform::Win64),
            ("linux", "x86_64") => Some(Platform::Linux64),
            _ => None,
        }
    }

    /// Plattform dieses Prozesses.
    pub fn current() -> Option<Platform> {
        Platform::for_target(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Pfad des Programms relativ zum Versionsordner (plattformneutral aus Komponenten gebaut).
    pub fn executable(self) -> PathBuf {
        let root = PathBuf::from(format!("chrome-{}", self.key()));
        match self {
            Platform::MacArm64 | Platform::MacX64 => root
                .join("Google Chrome for Testing.app")
                .join("Contents")
                .join("MacOS")
                .join("Google Chrome for Testing"),
            Platform::Win64 => root.join("chrome.exe"),
            Platform::Linux64 => root.join("chrome"),
        }
    }
}

/// Fehler der Bereitstellung. Der Text beginnt mit dem Meldungsschlüssel (`err-chromium-…`), danach folgt eine
/// deutsche Beschreibung für Logs; die Oberfläche übersetzt anhand des Schlüssels.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProvisionError {
    #[error("err-chromium-unsupported: Für diese Plattform gibt es keinen Chromium-Download; Pfad angeben oder OPENSPIDER_CHROMIUM setzen.")]
    Unsupported,
    #[error("err-chromium-manifest: Ungültige Chromium-Versionsdatei: {0}")]
    Manifest(String),
    #[error("err-chromium-network: Chromium-Download fehlgeschlagen: {0}")]
    Network(String),
    #[error("err-chromium-http: Chromium-Download fehlgeschlagen (HTTP {0})")]
    Http(u16),
    #[error("err-chromium-size: Das Chromium-Archiv ist größer als erwartet ({actual} > {expected} Byte).")]
    TooLarge { expected: u64, actual: u64 },
    #[error("err-chromium-checksum: Prüfsumme des Chromium-Archivs stimmt nicht (erwartet {expected}, erhalten {actual}); das Archiv wurde verworfen.")]
    Checksum { expected: String, actual: String },
    #[error("err-chromium-archive: Ungültiges Chromium-Archiv: {0}")]
    Archive(String),
    #[error("err-chromium-io: Chromium-Ablage nicht beschreibbar: {0}")]
    Io(String),
    #[error("err-chromium-cancelled: Chromium-Download abgebrochen; er kann fortgesetzt werden.")]
    Cancelled,
    #[error("err-chromium-in-use: Chromium wird noch verwendet und konnte nicht vollständig entfernt werden: {0}")]
    InUse(String),
}

impl ProvisionError {
    /// Meldungsschlüssel (Teil vor dem Doppelpunkt).
    pub fn message_key(&self) -> String {
        let text = self.to_string();
        text.split_once(':')
            .map_or(text.as_str(), |(key, _)| key)
            .to_string()
    }
}

impl From<std::io::Error> for ProvisionError {
    fn from(e: std::io::Error) -> Self {
        ProvisionError::Io(e.to_string())
    }
}

/// Ein Archiv der Versionsdatei.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Archive {
    pub url: String,
    /// Erwartete Größe in Byte (Obergrenze beim Download).
    pub size: u64,
    /// SHA-256 des Archivs, 64 Hexadezimalziffern in Kleinbuchstaben.
    pub sha256: String,
}

/// Inhalt von `resources/chromium/versions.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub product: String,
    pub version: String,
    /// Herkunft von Version und Prüfsummen (Dokumentation).
    pub source: String,
    /// Datum der Ermittlung (Dokumentation).
    #[serde(default)]
    pub determined: Option<String>,
    pub platforms: BTreeMap<String, Archive>,
}

/// Ein auszuführender Download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub version: String,
    pub platform: Platform,
    pub archive: Archive,
}

/// Versionsnamen bestehen nur aus Ziffern und Punkten (zugleich Ordnername der Ablage).
pub fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 32
        && version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && version.bytes().any(|b| b.is_ascii_digit())
        && !version.starts_with('.')
        && !version.contains("..")
}

fn valid_sha256(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Manifest {
    /// Liest und prüft eine Versionsdatei (Schema, Version, Plattformschlüssel, Prüfsummen, URL-Schema).
    pub fn parse(json: &str) -> Result<Manifest, ProvisionError> {
        let manifest: Manifest =
            serde_json::from_str(json).map_err(|e| ProvisionError::Manifest(e.to_string()))?;
        if manifest.schema != 1 {
            return Err(ProvisionError::Manifest(format!(
                "Schema {} wird nicht unterstützt",
                manifest.schema
            )));
        }
        if !valid_version(&manifest.version) {
            return Err(ProvisionError::Manifest(format!(
                "ungültige Version {:?}",
                manifest.version
            )));
        }
        for (key, archive) in &manifest.platforms {
            if Platform::from_key(key).is_none() {
                return Err(ProvisionError::Manifest(format!(
                    "unbekannte Plattform {key:?}"
                )));
            }
            if !valid_sha256(&archive.sha256) {
                return Err(ProvisionError::Manifest(format!(
                    "{key}: SHA-256 muss aus 64 Hexadezimalziffern (klein) bestehen"
                )));
            }
            if archive.size == 0 {
                return Err(ProvisionError::Manifest(format!("{key}: Größe fehlt")));
            }
            if !(archive.url.starts_with("https://") || archive.url.starts_with("http://")) {
                return Err(ProvisionError::Manifest(format!(
                    "{key}: URL muss HTTP(S) sein"
                )));
            }
        }
        Ok(manifest)
    }

    /// Die eingebettete Versionsdatei, zusätzlich auf die offizielle HTTPS-Quelle geprüft ([`Manifest::check_official`]).
    pub fn bundled() -> Result<&'static Manifest, ProvisionError> {
        static BUNDLED: OnceLock<Result<Manifest, ProvisionError>> = OnceLock::new();
        BUNDLED
            .get_or_init(|| {
                let manifest = Manifest::parse(VERSIONS_JSON)?;
                manifest.check_official()?;
                Ok(manifest)
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    /// Alle URLs zeigen per HTTPS auf die offizielle Ablage dieser Version.
    pub fn check_official(&self) -> Result<(), ProvisionError> {
        let prefix = format!("https://{OFFICIAL_HOST}{OFFICIAL_PREFIX}{}/", self.version);
        for (key, archive) in &self.platforms {
            let expected = format!("{prefix}{key}/chrome-{key}.zip");
            if archive.url != expected {
                return Err(ProvisionError::Manifest(format!(
                    "{key}: URL {:?} ist nicht die offizielle Quelle {expected:?}",
                    archive.url
                )));
            }
        }
        Ok(())
    }

    /// Archiv für eine Plattform.
    pub fn archive(&self, platform: Platform) -> Option<&Archive> {
        self.platforms.get(platform.key())
    }

    /// Download-Plan für eine Plattform; ohne Eintrag [`ProvisionError::Unsupported`].
    pub fn plan(&self, platform: Platform) -> Result<Plan, ProvisionError> {
        let archive = self
            .archive(platform)
            .ok_or(ProvisionError::Unsupported)?
            .clone();
        Ok(Plan {
            version: self.version.clone(),
            platform,
            archive,
        })
    }
}

/// Standardablage: `$OPENSPIDER_HOME/browser`, sonst `<lokaler Datenordner>/OpenSpider/browser` (macOS:
/// `~/Library/Application Support`, Windows: `%LOCALAPPDATA%` – nicht im servergespeicherten Profil).
pub fn default_root() -> Option<PathBuf> {
    root_for_home(std::env::var_os(HOME_ENV).as_deref())
}

/// Wie [`default_root`] mit ausdrücklich gegebenem Wert von `OPENSPIDER_HOME` (`None` oder leer = Standardort).
pub fn root_for_home(home: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    match home.filter(|h| !h.is_empty()) {
        Some(home) => Some(PathBuf::from(home).join("browser")),
        None => dirs::data_local_dir().map(|d| d.join("OpenSpider").join("browser")),
    }
}

/// Woher das verwendete Chromium stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// Pfad aus der Konfiguration (`rendering.chromium_path`).
    Configured,
    /// Pfad aus `OPENSPIDER_CHROMIUM`.
    Environment,
    /// Von OpenSpider bereitgestellt (Ablage unter [`default_root`]).
    Provisioned,
    /// Auf dem Rechner installiertes Chrome/Chromium.
    System,
}

/// Ergebnis der Auswahl.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    pub source: Source,
}

/// Eingaben der Auswahl (für Tests einzeln vorgebbar).
#[derive(Debug, Clone, Default)]
pub struct Lookup {
    pub configured: Option<PathBuf>,
    pub environment: Option<PathBuf>,
    pub root: Option<PathBuf>,
    pub platform: Option<Platform>,
}

impl Lookup {
    /// Werte dieses Prozesses: `OPENSPIDER_CHROMIUM`, [`default_root`], [`Platform::current`].
    pub fn from_env(configured: Option<&Path>) -> Lookup {
        Lookup {
            configured: configured.map(Path::to_path_buf),
            environment: std::env::var_os(CHROMIUM_ENV)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            root: default_root(),
            platform: Platform::current(),
        }
    }
}

/// Auswahl mit den Werten dieses Prozesses und der Systemsuche ([`system::find`]).
pub fn resolve(configured: Option<&Path>) -> Result<Resolved, RenderError> {
    resolve_with(&Lookup::from_env(configured), system::find)
}

/// Auswahlreihenfolge (K3-01): konfigurierter Pfad → `OPENSPIDER_CHROMIUM` → bereitgestelltes Chromium →
/// System-Chrome. Ein ausdrücklich vorgegebener, aber fehlender Pfad ist ein Fehler und fällt **nie** still auf eine
/// andere Installation zurück (CRAWL-077: kein stiller Rückfall).
pub fn resolve_with(
    lookup: &Lookup,
    system: impl FnOnce() -> Option<PathBuf>,
) -> Result<Resolved, RenderError> {
    for (path, source) in [
        (&lookup.configured, Source::Configured),
        (&lookup.environment, Source::Environment),
    ] {
        if let Some(path) = path {
            return if path.is_file() {
                Ok(Resolved {
                    path: path.clone(),
                    source,
                })
            } else {
                Err(RenderError::ChromiumMissing {
                    path: path.display().to_string(),
                })
            };
        }
    }
    if let (Some(root), Some(platform)) = (&lookup.root, lookup.platform) {
        if let Some(found) = installed(root, platform) {
            return Ok(Resolved {
                path: found.executable,
                source: Source::Provisioned,
            });
        }
    }
    system()
        .map(|path| Resolved {
            path,
            source: Source::System,
        })
        .ok_or_else(|| RenderError::ChromiumMissing {
            path: "OPENSPIDER_CHROMIUM / bereitgestelltes Chromium / Systeminstallation".into(),
        })
}

/// Auf dem Rechner installiertes Chrome/Chromium (ohne bereitgestelltes).
pub fn system_chromium() -> Option<PathBuf> {
    system::find()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifest_is_official_and_complete() {
        let manifest = Manifest::bundled().expect("bundled versions.json");
        for platform in [Platform::MacArm64, Platform::MacX64, Platform::Win64] {
            assert!(manifest.archive(platform).is_some(), "{platform:?}");
        }
        assert!(manifest.archive(Platform::Linux64).is_none());
    }

    #[test]
    fn windows_arm_uses_x64_build() {
        assert_eq!(
            Platform::for_target("windows", "aarch64"),
            Some(Platform::Win64)
        );
        assert_eq!(Platform::for_target("linux", "aarch64"), None);
    }

    #[test]
    fn message_key_is_prefix() {
        assert_eq!(
            ProvisionError::Cancelled.message_key(),
            "err-chromium-cancelled"
        );
    }
}
