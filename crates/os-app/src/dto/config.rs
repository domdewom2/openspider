//! Konfiguration, Profile und Segmente (`config.*`, `profile.*`, `segment.*`).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::view::RuleNode;

/// Konfiguration eines Crawls samt Version (`config.get`, `config.set`, Profile).
///
/// `config` hat die Struktur von `os_model::CrawlConfig` als JSON mit den TOML-Schlüsseln (snake_case, z. B.
/// `speed.threads`); `config.schema` liefert das zugehörige JSON-Schema, aus dem die Oberfläche Typen, Wertebereiche
/// und Aufzählungswerte liest. Passwörter und Tokens enthält sie nie (OUT-225).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CrawlConfigDto {
    /// Version des Formats.
    pub schema_version: u32,
    /// Erzeuger, z. B. `openspider 0.1.0`.
    pub generator: String,
    /// Inhalt.
    pub config: serde_json::Value,
}

/// Antwort von `config.set` (K-5, K-20).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSetResult {
    /// Analyse-Teilschritte, die durch die Änderung veralten.
    pub invalidated_steps: Vec<String>,
    /// Felder, die erst nach einer Pause wirken.
    pub deferred: Vec<String>,
}

/// Vergleichsbasis von `config.diff` (OUT-224, P2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConfigDiffBase {
    /// Werkseinstellungen.
    Defaults,
    /// Gespeichertes Profil.
    #[serde(rename_all = "camelCase")]
    Profile {
        /// Profilname.
        name: String,
    },
    /// Konfiguration eines Crawls.
    #[serde(rename_all = "camelCase")]
    Crawl {
        /// Crawl.
        crawl_id: String,
    },
}

/// Argument von `config.diff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiffArgs {
    /// Basis.
    pub base: ConfigDiffBase,
}

/// Eine Abweichung (OUT-224): `Setting`, `Current`, `Default`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    /// Stabiler Schlüssel `bereich.option`.
    pub path: String,
    /// i18n-Schlüssel des Anzeigenamens.
    pub label_key: String,
    /// Aktueller Wert.
    pub current: serde_json::Value,
    /// Wert der Basis.
    pub base: serde_json::Value,
}

/// Art der Regelliste, die `config.testRules` auswertet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum RuleListKind {
    /// Include-/Exclude-Regeln (CRAWL-109/110/111).
    IncludeExclude,
    /// Umschreibung (CRAWL-103/104/107).
    Rewrite,
    /// URL-Mapping des Vergleichs (OUT-146).
    Mapping,
}

/// Eine Regel der Testeingabe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct RuleDef {
    /// Suchmuster (Regex).
    pub pattern: String,
    /// Ersetzung mit `$1`-Gruppen (bei Include/Exclude `null`).
    pub replacement: Option<String>,
    /// Bei Include/Exclude: `true` = Include, `false` = Exclude.
    pub include: Option<bool>,
}

/// Argument von `config.testRules` und `compare.testMapping`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TestRulesArgs {
    /// Art.
    pub kind: RuleListKind,
    /// Regeln.
    pub rules: Vec<RuleDef>,
    /// Eingabe-URL.
    pub input: String,
    /// Vertragserweiterung M2-F05b: remove_parameters.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub remove_parameters: Option<Vec<String>>,

    /// Vertragserweiterung M2-F05b: remove_all_parameters.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub remove_all_parameters: Option<bool>,

    /// Vertragserweiterung M2-F05b: regex_partial_match.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub regex_partial_match: Option<bool>,
}

/// Wirkung einer Regel auf die Eingabe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct RuleStep {
    /// Index der Regel.
    pub index: u32,
    /// Regel hat getroffen.
    pub matched: bool,
    /// Wert vor der Regel.
    pub before: String,
    /// Wert nach der Regel.
    pub after: String,
    /// Fehler (ungültiger Ausdruck, Schrittlimit).
    pub error: Option<String>,
}

/// Ergebnis von `config.testRules` (Regel-Trace).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct RuleTrace {
    /// Ergebnis nach allen Regeln.
    pub output: String,
    /// Bei Include/Exclude: wird die URL gecrawlt?
    pub included: Option<bool>,
    /// Wirkung je Regel.
    pub steps: Vec<RuleStep>,
}

/// Ein gespeichertes Konfigurationsprofil (OUT-220).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConfigProfileInfo {
    /// Name.
    pub name: String,
    /// Ist das Standardprofil.
    pub is_default: bool,
    /// Geändert (ISO 8601 UTC).
    pub modified_at: String,
    /// Datei.
    pub path: String,
}

/// Argument von `profile.save`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSaveArgs {
    /// Name.
    pub name: String,
    /// Konfiguration.
    pub config: CrawlConfigDto,
    /// Vorhandenes Profil überschreiben.
    pub overwrite: bool,
}

/// Argument von `profile.setDefault` (`null` = Werkseinstellungen).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(optional_fields = nullable)]
pub struct SetDefaultProfileArgs {
    /// Profilname.
    pub name: Option<String>,
}

/// Argument von `profile.export` (OUT-221).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProfileExportArgs {
    /// Profilname.
    pub name: String,
    /// Zieldatei (`.osconfig`).
    pub path: String,
    /// Vertragserweiterung M2-F05b: include_secrets.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub include_secrets: Option<bool>,

    /// Vertragserweiterung M2-F05b: password.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub password: Option<String>,
}

/// Ein Segment (CRAWL-134, UI-068); `rules` ist die Oberfläche des Regelmodells der Konfiguration
/// (`segments.list`): Gruppen ODER, Bedingungen UND.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentDef {
    /// Kennung.
    pub id: String,
    /// Name (1 – 64 Zeichen).
    pub name: String,
    /// Farbe `#rrggbb`.
    pub color: String,
    /// Regelbaum.
    pub rules: RuleNode,
    /// Position (Priorität, 0 = zuerst).
    pub order: u32,
}

/// Argument von `segment.save` (vollständige Liste, Reihenfolge = Priorität).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentSaveArgs {
    /// Segmente.
    pub segments: Vec<SegmentDef>,
}

/// Antwort von `segment.save`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentSaveResult {
    /// Analyse-Teilschritte, die veralten.
    pub invalidated_steps: Vec<String>,
}

/// Argument von `segment.validate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentValidateArgs {
    /// Segment.
    pub segment: SegmentDef,
}

/// Argument von `segment.preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentPreviewArgs {
    /// Segmente.
    pub segments: Vec<SegmentDef>,
}

/// Zähler eines Segments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentCount {
    /// Segment.
    pub id: String,
    /// URLs.
    pub urls: u64,
}

/// Ergebnis von `segment.preview` (Ergebnis der Operation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SegmentPreview {
    /// Zähler je Segment.
    pub counts: Vec<SegmentCount>,
    /// URLs ohne Segment.
    pub unassigned: u64,
}

/// Command-Vertrag RobotsTestArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RobotsTestArgs {
    /// Vertragserweiterung M2-F05b: url.
    pub url: String,

    /// Vertragserweiterung M2-F05b: content.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub content: Option<String>,

    /// Vertragserweiterung M2-F05b: subdomain.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub subdomain: Option<String>,
}

/// Command-Vertrag RobotsTestResult (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RobotsTestResult {
    /// Vertragserweiterung M2-F05b: allowed.
    pub allowed: bool,

    /// Vertragserweiterung M2-F05b: matched_line.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub matched_line: Option<u32>,

    /// Vertragserweiterung M2-F05b: user_agent.
    pub user_agent: String,
}

/// Command-Vertrag RobotsLoadArgs (M2.md 3.14.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RobotsLoadArgs {
    /// Vertragserweiterung M2-F05b: host.
    pub host: String,
}
