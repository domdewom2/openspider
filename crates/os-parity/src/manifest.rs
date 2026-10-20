//! Manifest einer Referenz-Site (`tests/golden/parity/<site>/manifest.toml`, Abschnitt 5.2).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::ParityError;

/// Stand eines Referenz-Exports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportStatus {
    /// Noch nicht vom Nutzer abgelegt.
    #[default]
    Pending,
    /// Liegt unter `ref/`.
    Present,
    /// In der Free-Version des Referenzprodukts nicht exportierbar.
    Unavailable,
}

/// Ein Eintrag `[[export]]`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExportEntry {
    /// Konfigurationsvariante: `--set`-Werte, getrennt vom Standardlauf.
    #[serde(default)]
    pub config: BTreeMap<String, String>,
    /// Zielauswahl nach dem Export, z. B. Address beim Crawl Path Report.
    #[serde(default)]
    pub select: BTreeMap<String, String>,
    /// Dateiname unter `ref/`.
    pub ref_file: String,
    /// OpenSpider-Seite: `tab:<Tab>:<Filter>` | `bulk:<Name>` | `report:<Name>`.
    pub os: String,
    /// Name der Mapping-Datei (ohne `.toml`).
    pub mapping: String,
    /// Stand.
    #[serde(default)]
    pub status: ExportStatus,
    /// Optional: Dateiname der OpenSpider-Ausgabe, wenn er von der Namensregel (OUT-008) abweicht.
    #[serde(default)]
    pub os_file: Option<String>,
    /// Optional: Grund, warum der Export nicht verglichen wird, obwohl die Referenz vorliegt (z. B. Funktion
    /// außerhalb von M1). Der Eintrag erscheint im Bericht als übersprungen mit diesem Grund.
    #[serde(default)]
    pub skip: Option<String>,
}

/// Manifest einer Site.
#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    /// Liste relativ zum Fixture-Verzeichnis; ohne Angabe Spider-Modus.
    #[serde(default)]
    pub list_source: Option<String>,
    /// Name der Site (Fixture-Verzeichnis).
    pub site: String,
    /// Fester Port; `0`/fehlend = freier Port.
    #[serde(default)]
    pub port: u16,
    /// Version der Referenz (vom Nutzer eingetragen).
    #[serde(default)]
    pub ref_version: Option<String>,
    /// Exportdatum.
    #[serde(default)]
    pub exported_at: Option<String>,
    /// Einstellungen der Referenz beim Export.
    #[serde(default)]
    pub ref_settings: Option<String>,
    /// Exportliste.
    #[serde(default)]
    pub export: Vec<ExportEntry>,
}

impl Manifest {
    /// Lädt das Manifest.
    pub fn load(path: &Path) -> Result<Self, ParityError> {
        let text = std::fs::read_to_string(path).map_err(|e| {
            ParityError::Invalid(format!("Manifest {} nicht lesbar: {e}", path.display()))
        })?;
        Self::parse(&text)
            .map_err(|e| ParityError::Invalid(format!("Manifest {}: {e}", path.display())))
    }

    /// Parst den TOML-Text.
    pub fn parse(text: &str) -> Result<Self, ParityError> {
        let manifest: Self =
            toml::from_str(text).map_err(|e| ParityError::Invalid(e.to_string()))?;
        if let Some(list) = &manifest.list_source {
            let path = Path::new(list);
            if list.is_empty()
                || path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(ParityError::Invalid(
                    "list_source muss relativ zum Fixture-Verzeichnis sein".into(),
                ));
            }
        }
        let mut files = std::collections::BTreeSet::new();
        for e in &manifest.export {
            if !files.insert(&e.ref_file) {
                return Err(ParityError::Invalid(format!(
                    "doppelter Referenzdateiname {}",
                    e.ref_file
                )));
            }
        }
        Ok(manifest)
    }

    /// Einträge mit Status `present`.
    pub fn present(&self) -> impl Iterator<Item = &ExportEntry> {
        self.export
            .iter()
            .filter(|e| e.status == ExportStatus::Present)
    }
}

/// Art des OpenSpider-Exports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OsExport {
    /// `--export-tabs "<Tab>:<Filter>"`.
    Tab {
        /// Tab.
        tab: String,
        /// Filter.
        filter: String,
    },
    /// `--bulk-export "<Menü>:<Eintrag>"` (Name kann weitere `:` enthalten).
    Bulk(String),
    /// `--save-report "<Menü>:<Report>"` bzw. `"<Report>"`.
    Report(String),
    /// XML- oder Bilder-Sitemap: `sitemap:default` bzw. `sitemap:images`.
    Sitemap(String),
}

impl OsExport {
    /// Parst `tab:Internal:All`, `bulk:Links:All Inlinks`, `report:Crawl Overview`.
    pub fn parse(spec: &str) -> Result<Self, ParityError> {
        let bad = || ParityError::Invalid(format!("ungültige OpenSpider-Angabe {spec:?}"));
        let (kind, rest) = spec.split_once(':').ok_or_else(bad)?;
        let rest = rest.trim();
        if rest.is_empty() {
            return Err(bad());
        }
        match kind.trim() {
            "tab" => {
                let (tab, filter) = rest.split_once(':').ok_or_else(bad)?;
                if tab.trim().is_empty() || filter.trim().is_empty() {
                    return Err(bad());
                }
                Ok(Self::Tab {
                    tab: tab.trim().to_string(),
                    filter: filter.trim().to_string(),
                })
            }
            "bulk" => Ok(Self::Bulk(rest.to_string())),
            "report" => Ok(Self::Report(rest.to_string())),
            "sitemap" if matches!(rest, "default" | "images") => {
                Ok(Self::Sitemap(rest.to_string()))
            }
            _ => Err(bad()),
        }
    }

    /// Wert für die Kommandozeilenoption (Kommas im Namen maskiert, OUT-192).
    pub fn cli_value(&self) -> String {
        let raw = match self {
            Self::Tab { tab, filter } => format!("{tab}:{filter}"),
            Self::Bulk(n) | Self::Report(n) | Self::Sitemap(n) => n.clone(),
        };
        raw.replace(',', "\\,")
    }

    /// Mögliche Dateinamen der CSV-Ausgabe nach OUT-008 (wahrscheinlichster zuerst).
    pub fn file_candidates(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let mut push = |s: String| {
            if !s.is_empty() && !names.contains(&s) {
                names.push(s);
            }
        };
        match self {
            Self::Sitemap(kind) => {
                return vec![if kind == "images" {
                    "image-sitemap.xml"
                } else {
                    "sitemap.xml"
                }
                .into()]
            }
            Self::Tab { tab, filter } => push(slug(&format!("{tab} {filter}"))),
            Self::Bulk(name) | Self::Report(name) => {
                let parts: Vec<&str> = name.split(':').map(str::trim).collect();
                push(slug(&parts.join(" ")));
                push(slug(parts.last().copied().unwrap_or("")));
                if parts.len() > 2 {
                    push(slug(&parts[parts.len() - 2..].join(" ")));
                }
            }
        }
        names.into_iter().map(|n| format!("{n}.csv")).collect()
    }
}

/// Dateinamen-Regel OUT-008: Kleinbuchstaben, alles außer `a-z0-9` wird `_`, Wiederholungen
/// zusammengefasst, führende/abschließende `_` entfallen.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
site = "parity-onpage"
port = 8902
ref_version = "24.3"
exported_at = "2026-10-20"
ref_settings = "Standard"
[[export]]
ref_file = "internal_all.csv"
os = "tab:Internal:All"
mapping = "internal_all"
status = "present"
[[export]]
ref_file = "x.csv"
os = "bulk:Links:All Inlinks"
mapping = "all_inlinks"
[[export]]
ref_file = "y.csv"
os = "report:Crawl Overview"
mapping = "crawl_overview"
status = "unavailable"
os_file = "overview.csv"
"#;

    #[test]
    fn parses_manifest_with_defaults() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.site, "parity-onpage");
        assert_eq!(m.port, 8902);
        assert_eq!(m.export.len(), 3);
        assert_eq!(m.export[1].status, ExportStatus::Pending);
        assert_eq!(m.present().count(), 1);
        assert_eq!(m.export[2].os_file.as_deref(), Some("overview.csv"));
    }

    #[test]
    fn unknown_status_is_an_error() {
        assert!(Manifest::parse(
            "site=\"a\"\n[[export]]\nref_file=\"a\"\nos=\"tab:A:B\"\nmapping=\"m\"\nstatus=\"vielleicht\"\n"
        )
        .is_err());
    }

    #[test]
    fn legacy_abbreviated_keys_are_not_accepted() {
        // M2-H01: kein Alias auf die alten Schlüssel; ein Manifest mit dem alten Dateischlüssel ist ungültig.
        let legacy = format!(
            "site=\"a\"\n[[export]]\n{}_file=\"a.csv\"\nos=\"tab:A:B\"\nmapping=\"m\"\nstatus=\"present\"\n",
            ["s", "f"].concat()
        );
        assert!(Manifest::parse(&legacy).is_err());
    }

    #[test]
    fn slug_follows_file_name_rule() {
        assert_eq!(slug("Internal All"), "internal_all");
        assert_eq!(
            slug("Response Codes Client Error (4xx)"),
            "response_codes_client_error_4xx"
        );
        assert_eq!(
            slug("Links Non-Indexable Page Inlinks Only"),
            "links_non_indexable_page_inlinks_only"
        );
        assert_eq!(slug("  --A//B--  "), "a_b");
    }

    #[test]
    fn os_export_parsing_and_cli_values() {
        let t = OsExport::parse("tab:Response Codes:Client Error (4xx)").unwrap();
        assert_eq!(
            t,
            OsExport::Tab {
                tab: "Response Codes".into(),
                filter: "Client Error (4xx)".into()
            }
        );
        assert_eq!(t.cli_value(), "Response Codes:Client Error (4xx)");
        assert_eq!(t.file_candidates(), ["response_codes_client_error_4xx.csv"]);

        let b = OsExport::parse("bulk:Response Codes:Internal:Client Error (4xx) Inlinks").unwrap();
        assert_eq!(
            b.cli_value(),
            "Response Codes:Internal:Client Error (4xx) Inlinks"
        );
        let c = b.file_candidates();
        assert_eq!(c[0], "response_codes_internal_client_error_4xx_inlinks.csv");
        assert!(c.contains(&"client_error_4xx_inlinks.csv".to_string()));

        let l = OsExport::parse("bulk:Links:All Inlinks").unwrap();
        assert_eq!(
            l.file_candidates(),
            ["links_all_inlinks.csv", "all_inlinks.csv"]
        );
        let r = OsExport::parse("report:Crawl Overview").unwrap();
        assert_eq!(r.file_candidates(), ["crawl_overview.csv"]);
        assert_eq!(
            OsExport::parse("report:Redirects:Redirect Chains")
                .unwrap()
                .file_candidates(),
            ["redirects_redirect_chains.csv", "redirect_chains.csv"]
        );
        assert_eq!(
            OsExport::Bulk("A,B:C".into()).cli_value(),
            "A\\,B:C",
            "Kommas werden maskiert"
        );
    }

    #[test]
    fn invalid_os_specs_are_rejected() {
        assert!(OsExport::parse("tab:NurTab").is_err());
        assert!(OsExport::parse("foo:Bar").is_err());
        assert!(OsExport::parse("tab:").is_err());
        assert!(OsExport::parse("ohne-doppelpunkt").is_err());
    }
}

#[cfg(test)]
mod m2_tests {
    use super::*;

    // spec: OUT-127
    #[test]
    fn image_sitemap_output_name_matches_the_spec() {
        assert_eq!(
            OsExport::parse("sitemap:images").unwrap().file_candidates(),
            ["image-sitemap.xml"]
        );
    }

    // spec: CRAWL-029
    #[test]
    fn invalid_list_sources_and_duplicate_export_files_are_rejected() {
        for path in ["", "../list.txt", "/list.txt"] {
            assert!(Manifest::parse(&format!("site = \"s\"\nlist_source = {path:?}\n")).is_err());
        }
        let entry = "[[export]]\nref_file = \"same.csv\"\nos = \"tab:Internal:All\"\nmapping = \"internal_all\"\n";
        assert!(Manifest::parse(&format!("site = \"s\"\n{entry}{entry}")).is_err());
        assert!(OsExport::parse("tab:Internal:").is_err());
        assert!(OsExport::parse("sitemap:unknown").is_err());
    }
}
