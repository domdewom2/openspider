//! Laden und Speichern von `.osconfig`-Dateien sowie das Standard-Profil (CRAWL-132, OUT-220, OUT-221).
//!
//! * Laden: fehlende Schlüssel = Standardwert, Kommentare sind erlaubt, unbekannte Schlüssel erzeugen eine
//!   Warnung mit Feldpfad (Laden geht weiter), Typfehler sind Fehler mit Zeile/Spalte, eine neuere
//!   `schema_version` lädt mit Warnung.
//! * Schreiben: alle Werte, in Felddeklarationsreihenfolge (stabil), `generator`-Zeile; Schreiben →
//!   Laden → Schreiben ist byte-identisch.
//! * Passwörter stehen nie im Profil: `auth.basic` kennt nur `secret_ref`. Ein trotzdem vorhandenes Feld
//!   wie `password` wird als unbekannt gemeldet und nie übernommen.
//! * Standard-Profil: `<settings>/default.osconfig`; das Einstellungsverzeichnis kommt als Parameter, damit
//!   `os-model` keine Benutzerpfade (`dirs`) kennen muss.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::ops::Range;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use toml::de::{DeTable, DeValue};

use crate::config::schema::{array_items, json_schema, object_properties};
use crate::config::validate::{ConfigIssue, ConfigIssueKind, ValidationReport};
use crate::config::{CrawlConfig, CONFIG_SCHEMA_VERSION};
use crate::error::ModelError;

/// Dateiname des Standard-Profils im Einstellungsordner (OUT-220/OUT-227).
pub const DEFAULT_PROFILE_FILE: &str = "default.osconfig";

/// Eine geladene Konfiguration samt Warnungen.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedConfig {
    /// Die Konfiguration (fehlende Schlüssel mit Standardwerten).
    pub config: CrawlConfig,
    /// Warnungen: unbekannte Schlüssel (mit Feldpfad und Zeile), neuere `schema_version`.
    pub warnings: Vec<ConfigIssue>,
    locations: BTreeMap<String, (u32, u32)>,
}

impl LoadedConfig {
    /// Eine Konfiguration, die nicht aus einer Datei stammt (keine Warnungen, keine Positionen).
    pub fn from_config(config: CrawlConfig) -> Self {
        LoadedConfig {
            config,
            warnings: Vec::new(),
            locations: BTreeMap::new(),
        }
    }

    /// Zeile und Spalte (1-basiert) des Schlüssels `path` in der Quelldatei.
    pub fn location(&self, path: &str) -> Option<(u32, u32)> {
        self.locations.get(path).copied()
    }

    /// Wie [`CrawlConfig::validate`], aber mit Zeile/Spalte der betroffenen Schlüssel in den Meldungen.
    pub fn validate(&self) -> ValidationReport {
        let mut rep = self.config.validate();
        rep.locate(&self.locations);
        rep
    }
}

/// Fehler beim Laden einer Konfiguration: Syntaxfehler, Typfehler oder nicht lesbare Datei.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLoadError {
    /// Datei, falls aus einer Datei geladen.
    pub file: Option<PathBuf>,
    /// Zeile (1-basiert).
    pub line: Option<u32>,
    /// Spalte (1-basiert).
    pub column: Option<u32>,
    /// Feldpfad des fehlerhaften Wertes, z. B. `limits.max_depth`.
    pub field: Option<String>,
    /// Fehlerbeschreibung.
    pub message: String,
}

impl fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(f, "{}: ", file.display())?;
        }
        if let (Some(l), Some(c)) = (self.line, self.column) {
            write!(f, "Zeile {l}, Spalte {c}: ")?;
        }
        if let Some(field) = &self.field {
            write!(f, "{field}: ")?;
        }
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<ConfigLoadError> for ModelError {
    fn from(e: ConfigLoadError) -> Self {
        ModelError::Config(e.to_string())
    }
}

impl CrawlConfig {
    /// Lädt eine `.osconfig` aus Text (TOML, Kommentare erlaubt). Siehe Moduldokumentation.
    pub fn load_str(src: &str) -> Result<LoadedConfig, ConfigLoadError> {
        let src = src.strip_prefix('\u{feff}').unwrap_or(src);

        let doc = DeTable::parse(src).map_err(|e| load_error(src, None, &e))?;

        let mut warnings = Vec::new();
        let file_version = doc
            .get_ref()
            .get("schema_version")
            .and_then(|v| v.get_ref().as_integer())
            .and_then(|i| i.as_str().parse::<i64>().ok());
        let newer = file_version.filter(|v| *v > i64::from(CONFIG_SCHEMA_VERSION));
        if let Some(v) = newer {
            warnings.push(ConfigIssue::new(
                ConfigIssueKind::NewerSchemaVersion,
                "schema_version",
                format!(
                    "Datei hat schema_version {v}, unterstützt wird {CONFIG_SCHEMA_VERSION}; \
                     unbekannte Schlüssel werden ignoriert"
                ),
            ));
        }

        let config = toml::de::Deserializer::parse(src)
            .and_then(CrawlConfig::deserialize)
            .map_err(|e| {
                let mut err = load_error(src, Some(&doc), &e);
                if let Some(v) = newer {
                    err.message.push_str(&format!(
                        " (die Datei hat schema_version {v}, unterstützt wird {CONFIG_SCHEMA_VERSION})"
                    ));
                }
                err
            })?;

        let mut walk = Walk {
            src,
            root: json_schema(),
            unknown: Vec::new(),
            locations: BTreeMap::new(),
        };
        walk.table(doc.get_ref(), Some(walk.root), "");
        if let Some(v) = warnings.first_mut() {
            v.line = walk.locations.get("schema_version").map(|l| l.0);
            v.column = walk.locations.get("schema_version").map(|l| l.1);
        }
        walk.unknown.sort_by_key(|w| (w.line, w.column));
        warnings.append(&mut walk.unknown);

        Ok(LoadedConfig {
            config,
            warnings,
            locations: walk.locations,
        })
    }

    /// Lädt eine `.osconfig`-Datei; Fehler tragen den Dateipfad.
    pub fn load_file(path: &Path) -> Result<LoadedConfig, ConfigLoadError> {
        let bytes = fs::read(path).map_err(|e| ConfigLoadError {
            file: Some(path.to_path_buf()),
            line: None,
            column: None,
            field: None,
            message: format!("Datei nicht lesbar: {e}"),
        })?;
        let text = String::from_utf8(bytes).map_err(|e| ConfigLoadError {
            file: Some(path.to_path_buf()),
            line: None,
            column: None,
            field: None,
            message: format!("keine gültige UTF-8-Datei: {e}"),
        })?;
        Self::load_str(&text).map_err(|mut e| {
            e.file = Some(path.to_path_buf());
            e
        })
    }

    /// Serialisiert die Konfiguration als `.osconfig`: alle Werte, stabile Reihenfolge, `schema_version`
    /// und `generator` des schreibenden Programms. Dadurch ist Schreiben → Laden → Schreiben byte-identisch.
    pub fn to_osconfig_string(&self) -> Result<String, ModelError> {
        let mut stamped = self.clone();
        stamped.schema_version = CONFIG_SCHEMA_VERSION;
        stamped.generator = CrawlConfig::default().generator;
        stamped.to_toml_string()
    }

    /// Schreibt die Konfiguration atomar (temporäre Datei, dann Umbenennen); fehlende Ordner werden angelegt.
    pub fn save_file(&self, path: &Path) -> Result<(), ModelError> {
        let text = self.to_osconfig_string()?;
        write_atomic(path, text.as_bytes())
    }

    /// Pfad des Standard-Profils im Einstellungsordner.
    pub fn default_profile_path(settings_dir: &Path) -> PathBuf {
        settings_dir.join(DEFAULT_PROFILE_FILE)
    }

    /// Lädt das Standard-Profil; ohne Datei die Werkseinstellungen (OUT-220).
    pub fn load_default(settings_dir: &Path) -> Result<LoadedConfig, ConfigLoadError> {
        let path = Self::default_profile_path(settings_dir);
        if path.exists() {
            Self::load_file(&path)
        } else {
            Ok(LoadedConfig::from_config(CrawlConfig::default()))
        }
    }

    /// „Als Standard speichern“: schreibt `<settings>/default.osconfig`.
    pub fn save_default(&self, settings_dir: &Path) -> Result<(), ModelError> {
        self.save_file(&Self::default_profile_path(settings_dir))
    }

    /// „Standard zurücksetzen“: löscht `<settings>/default.osconfig`. `Ok(true)`, wenn es existierte.
    /// Die Bestätigungsabfrage ist Sache der Oberfläche.
    pub fn reset_default(settings_dir: &Path) -> Result<bool, ModelError> {
        let path = Self::default_profile_path(settings_dir);
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(ModelError::Config(format!(
                "{}: Standard-Profil nicht löschbar: {e}",
                path.display()
            ))),
        }
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ModelError> {
    let fail = |what: &str, e: std::io::Error| {
        ModelError::Config(format!("{}: {what}: {e}", path.display()))
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| fail("Ordner nicht anlegbar", e))?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".tmp{}", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    fs::write(&tmp, bytes).map_err(|e| fail("nicht schreibbar", e))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        fail("nicht ersetzbar", e)
    })
}

/// Zeile und Spalte (1-basiert, Spalte in Zeichen) eines Byte-Offsets.
fn line_col(src: &str, offset: usize) -> (u32, u32) {
    let offset = offset.min(src.len());
    let before = src.get(..offset).unwrap_or_default();
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line as u32, col as u32)
}

fn load_error(
    src: &str,
    doc: Option<&toml::Spanned<DeTable<'_>>>,
    e: &toml::de::Error,
) -> ConfigLoadError {
    let (line, column) = match e.span() {
        Some(span) => {
            let (l, c) = line_col(src, span.start);
            (Some(l), Some(c))
        }
        None => (None, None),
    };
    let field = match (doc, e.span()) {
        (Some(doc), Some(span)) => path_for_span(doc.get_ref(), &span, ""),
        _ => None,
    };
    ConfigLoadError {
        file: None,
        line,
        column,
        field,
        message: e.message().to_string(),
    }
}

fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Feldpfad des tiefsten Schlüssels/Wertes, dessen Textbereich `span` enthält. Der Bereich einer Tabelle
/// umfasst nur ihre Kopfzeile; deshalb wird immer erst in die Inhalte abgestiegen.
fn path_for_span(table: &DeTable<'_>, span: &Range<usize>, prefix: &str) -> Option<String> {
    for (k, v) in table.iter() {
        let path = join(prefix, k.get_ref());
        match v.get_ref() {
            DeValue::Table(t) => {
                if let Some(p) = path_for_span(t, span, &path) {
                    return Some(p);
                }
            }
            DeValue::Array(a) => {
                for (i, item) in a.iter().enumerate() {
                    let item_path = format!("{path}[{i}]");
                    if let DeValue::Table(t) = item.get_ref() {
                        if let Some(p) = path_for_span(t, span, &item_path) {
                            return Some(p);
                        }
                    }
                    if contains(&item.span(), span) {
                        return Some(item_path);
                    }
                }
            }
            _ => {}
        }
        if contains(&v.span(), span) || contains(&k.span(), span) {
            return Some(path);
        }
    }
    None
}

fn join(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Läuft parallel durch Dokument und Schema: sammelt unbekannte Schlüssel und alle Schlüsselpositionen.
struct Walk<'a> {
    src: &'a str,
    root: &'a Value,
    unknown: Vec<ConfigIssue>,
    locations: BTreeMap<String, (u32, u32)>,
}

impl<'a> Walk<'a> {
    fn table(&mut self, table: &DeTable<'_>, schema: Option<&'a Value>, prefix: &str) {
        let props = schema.and_then(|s| object_properties(self.root, s));
        for (k, v) in table.iter() {
            let key = k.get_ref();
            let path = join(prefix, key);
            let (line, column) = line_col(self.src, k.span().start);
            self.locations.insert(path.clone(), (line, column));

            let Some(props) = props else { continue };
            let Some(child) = props.get(key.as_ref()) else {
                let mut issue = ConfigIssue::new(
                    ConfigIssueKind::UnknownField,
                    path.clone(),
                    unknown_message(&path, v.get_ref()),
                );
                issue.line = Some(line);
                issue.column = Some(column);
                self.unknown.push(issue);
                continue;
            };
            match v.get_ref() {
                DeValue::Table(t) => self.table(t, Some(child), &path),
                DeValue::Array(a) => {
                    let items = array_items(self.root, child);
                    for (i, item) in a.iter().enumerate() {
                        if let DeValue::Table(t) = item.get_ref() {
                            let item_path = format!("{path}[{i}]");
                            let (l, c) = line_col(self.src, item.span().start);
                            self.locations.insert(item_path.clone(), (l, c));
                            self.table(t, items, &item_path);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

fn unknown_message(path: &str, value: &DeValue<'_>) -> String {
    const SECRET_WORDS: [&str; 6] = ["password", "passwort", "pass", "secret", "token", "api_key"];
    let last = path.rsplit('.').next().unwrap_or(path).to_lowercase();
    if path.starts_with("auth") && SECRET_WORDS.contains(&last.as_str()) {
        return "Passwörter gehören nicht ins Profil (nur secret_ref, OUT-225); \
                der Wert wird ignoriert und nie gespeichert"
            .to_string();
    }
    let what = if matches!(value, DeValue::Table(_)) {
        "unbekannter Abschnitt"
    } else {
        "unbekannter Schlüssel"
    };
    format!("{what}, wird ignoriert")
}
