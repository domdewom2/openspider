//! Das Aufgabenmodell (OUT-160 … OUT-165). Eine Aufgabe liegt als TOML-Datei `schedules/<task-id>.ostask`
//! im Einstellungsordner (OS-Festlegung); sie enthält nie Geheimnisse, nur Verweise (Konto-Kennungen,
//! Profilnamen).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Version des Dateiformats.
pub const FORMAT_VERSION: u32 = 1;

fn yes() -> bool {
    true
}

/// Zeitplan (OUT-161). Zeiten sind lokale Zeit des Rechners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    /// Einmalig zu einem Zeitpunkt (RFC 3339 mit Offset).
    Once {
        /// Zeitpunkt.
        at: String,
    },
    /// [P2] Stündlich ab einem lokalen Startzeitpunkt `YYYY-MM-DDTHH:MM`.
    Hourly {
        /// Startzeitpunkt (lokal).
        start: String,
    },
    /// [P2] Alle `every` Tage ab einem lokalen Startzeitpunkt `YYYY-MM-DDTHH:MM`.
    EveryDays {
        /// Abstand in Tagen (≥ 1).
        every: u32,
        /// Startzeitpunkt (lokal).
        start: String,
    },
    /// Täglich.
    Daily {
        /// Uhrzeit `HH:MM`.
        time: String,
        /// Frühestes Datum `YYYY-MM-DD` (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// Wöchentlich an den gewählten Wochentagen (1 = Montag … 7 = Sonntag).
    Weekly {
        /// Wochentage.
        days: Vec<u8>,
        /// Uhrzeit `HH:MM`.
        time: String,
        /// Frühestes Datum `YYYY-MM-DD` (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
    /// Monatlich; kurze Monate laufen am letzten Tag.
    Monthly {
        /// Tag des Monats 1 – 31.
        day: u8,
        /// Uhrzeit `HH:MM`.
        time: String,
        /// Frühestes Datum `YYYY-MM-DD` (optional).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<String>,
    },
}

/// Quelle des Crawls (OUT-162).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// Spider ab Start-URL.
    Spider {
        /// Start-URL.
        url: String,
    },
    /// Listenmodus: Datei, Sitemap-URL oder Google-Tabelle; mehrere Quellen durch Zeilenumbruch getrennt.
    List {
        /// Quelle(n).
        path: String,
    },
}

impl Source {
    /// Die einzelnen Listenquellen (leer bei Spider).
    pub fn list_sources(&self) -> Vec<String> {
        match self {
            Source::List { path } => path
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
            Source::Spider { .. } => Vec::new(),
        }
    }
}

/// Crawl-Einstellungen (OUT-162).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrawlSpec {
    /// Quelle.
    pub source: Source,
    /// Konfiguration: Profilname oder Pfad einer `.osconfig`; `None` = Standardprofil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// Authentifizierungsprofil: Name oder Pfad einer `.osauth`-Datei.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_profile: Option<String>,
    /// Überschreibungen einzelner Konfigurationsschlüssel (`--set schlüssel=wert`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<String, String>,
}

/// Umgang mit überlappenden Läufen derselben Aufgabe (OUT-167).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlap {
    /// Parallel starten (Standard).
    #[default]
    Parallel,
    /// Überspringen, wenn der Vorlauf noch aktiv ist.
    Skip,
}

/// Ausführung (OUT-163, OUT-166, OUT-167, OUT-171).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Execution {
    /// Überlappung.
    #[serde(default)]
    pub overlap: Overlap,
    /// Verpasste Läufe nachholen (höchstens einer).
    #[serde(default = "yes")]
    pub run_if_missed: bool,
    /// Auch im Akkubetrieb ausführen.
    #[serde(default = "yes")]
    pub on_battery: bool,
    /// [P2] Rechner zum Ausführen aufwecken (Windows).
    #[serde(default)]
    pub wake: bool,
    /// [P2] Höchstlaufzeit in Minuten.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_min: Option<u32>,
    /// Mit dem vorherigen Crawl des Projekts vergleichen (OUT-152).
    #[serde(default)]
    pub auto_compare: bool,
}

impl Default for Execution {
    fn default() -> Self {
        Execution {
            overlap: Overlap::Parallel,
            run_if_missed: true,
            on_battery: true,
            wake: false,
            timeout_min: None,
            auto_compare: false,
        }
    }
}

/// Format der Exportdateien.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// CSV.
    #[default]
    Csv,
    /// XLSX.
    Xlsx,
}

/// Ein Export der Aufgabe (`kind`: `tab`, `bulk`, `report`, `sitemap`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportItem {
    /// Art.
    pub kind: String,
    /// Name aus dem Katalog bzw. `pages`/`images` bei Sitemaps.
    pub name: String,
    /// Format.
    #[serde(default)]
    pub format: ExportFormat,
    /// Zielordner; `None` = Standard-Exportordner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

/// Exportoptionen aus dem Mehrfachexport (OUT-075, OUT-165).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportOptions {
    /// Auswahl `(kind, name)` in Reihenfolge.
    pub items: Vec<(String, String)>,
    /// Format.
    #[serde(default)]
    pub format: ExportFormat,
    /// Zielordner (leer = Standard).
    #[serde(default)]
    pub dir: String,
    /// In einer Arbeitsmappe zusammenfassen (OUT-007).
    #[serde(default)]
    pub single_workbook: bool,
    /// Leere Exporte überspringen (OUT-012).
    #[serde(default)]
    pub skip_empty: bool,
}

/// Ausgabeordner-Modus (OUT-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    /// Je Lauf ein Unterordner `YYYY-MM-DD_HH-MM-SS` (Standard).
    #[default]
    Timestamped,
    /// Gleichnamige Dateien ersetzen.
    Overwrite,
}

/// Ausgabe-Einstellungen, die das Dialogmodell nicht ausdrücklich führt (OS-Festlegungen).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Output {
    /// Ordner-Modus.
    #[serde(default)]
    pub mode: OutputMode,
    /// Crawl-Datei zusätzlich als `crawl.ospider` in den Exportordner kopieren (OUT-165).
    #[serde(default)]
    pub save_crawl: bool,
    /// Exporte auf ein Segment beschränken (OUT-011).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segment: Option<String>,
}

/// Looker-Studio-Zeitreihe (OUT-210).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomSummary {
    /// Google-Konto.
    pub account: String,
    /// Ziel (Tabellenname unter `OpenSpider/<Projekt>`).
    pub target: String,
    /// Gewählte Metriken in Spaltenreihenfolge.
    pub metrics: Vec<String>,
}

/// Benachrichtigung (OUT-281).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Notify {
    /// Bei Erfolg senden.
    #[serde(default)]
    pub on_success: bool,
    /// Bei Fehler senden.
    #[serde(default)]
    pub on_failure: bool,
    /// Mail-Konto.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Empfänger.
    #[serde(default)]
    pub to: Vec<String>,
}

/// Eine geplante Aufgabe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Kennung (Dateiname, OS-Planer, `--run-task`).
    pub id: String,
    /// Name (eindeutig).
    pub name: String,
    /// Projekt (leer = Name).
    #[serde(default)]
    pub project: String,
    /// Beschreibung.
    #[serde(default)]
    pub description: String,
    /// Eingeschaltet.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Zeitplan.
    pub schedule: Schedule,
    /// Crawl.
    pub crawl: CrawlSpec,
    /// Ausführung.
    #[serde(default)]
    pub execution: Execution,
    /// API-Quellen (`ga4`, `gsc`, `psi`, `majestic`, `ahrefs`, `moz`).
    #[serde(default)]
    pub apis: Vec<String>,
    /// Ausgabe.
    #[serde(default)]
    pub output: Output,
    /// Exporte (Dialogmodell).
    #[serde(default)]
    pub exports: Vec<ExportItem>,
    /// Exportoptionen des Mehrfachexports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_options: Option<ExportOptions>,
    /// Name einer gespeicherten Exportvorlage (OUT-075).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Zeitreihe für Looker Studio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_summary: Option<CustomSummary>,
    /// Benachrichtigung.
    #[serde(default)]
    pub notify: Notify,
}

/// Wirksame Exporte einer Aufgabe nach den Vorrangregeln des Dialogs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveExports {
    /// Tabellen, Bulk-Exporte und Berichte `(kind, name)` in Reihenfolge.
    pub items: Vec<(String, String)>,
    /// Format.
    pub format: ExportFormat,
    /// Zielordner (`None` = Standard-Exportordner).
    pub dir: Option<String>,
    /// Arbeitsmappe.
    pub single_workbook: bool,
    /// Leere überspringen.
    pub skip_empty: bool,
    /// XML-Sitemap erzeugen.
    pub sitemap_pages: bool,
    /// Bild-Sitemap erzeugen.
    pub sitemap_images: bool,
}

impl Task {
    /// Projektname; ohne Angabe der Aufgabenname (OUT-161).
    pub fn project_name(&self) -> &str {
        if self.project.trim().is_empty() {
            &self.name
        } else {
            &self.project
        }
    }

    /// Die Exporte, wie ein Lauf sie ausführt: Die Optionen des Mehrfachexports gelten, wenn sie vorhanden
    /// sind, sonst die Einzelliste; Sitemaps stammen immer aus der Einzelliste.
    pub fn effective_exports(&self) -> EffectiveExports {
        let tables = |e: &&ExportItem| matches!(e.kind.as_str(), "tab" | "bulk" | "report");
        let first = self.exports.iter().find(tables);
        let (items, format, dir, single_workbook, skip_empty) = match &self.export_options {
            Some(o) => (
                o.items.clone(),
                o.format,
                (!o.dir.trim().is_empty()).then(|| o.dir.clone()),
                o.single_workbook,
                o.skip_empty,
            ),
            None => (
                self.exports
                    .iter()
                    .filter(tables)
                    .map(|e| (e.kind.clone(), e.name.clone()))
                    .collect(),
                first.map(|e| e.format).unwrap_or_default(),
                first.and_then(|e| e.dir.clone()),
                false,
                false,
            ),
        };
        let has = |name: &str| {
            self.exports
                .iter()
                .any(|e| e.kind == "sitemap" && e.name == name)
        };
        let dir = dir.or_else(|| {
            self.exports
                .iter()
                .find(|e| e.kind == "sitemap")
                .and_then(|e| e.dir.clone())
        });
        EffectiveExports {
            items,
            format,
            dir,
            single_workbook,
            skip_empty,
            sitemap_pages: has("pages"),
            sitemap_images: has("images"),
        }
    }
}

/// Dateiinhalt einer einzelnen Aufgabe (`schedules/<id>.ostask`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFile {
    /// Formatversion.
    pub version: u32,
    /// Die Aufgabe.
    pub task: Task,
}

/// Dateiinhalt eines Exports mehrerer Aufgaben.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskBundle {
    /// Formatversion.
    pub version: u32,
    /// Aufgaben.
    pub tasks: Vec<Task>,
}
