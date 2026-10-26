//! OUT-196: presentation headers follow the saved UI language; canonical keys stay stable.
use crate::exit::CliError;
use os_model::UiLanguage;
use os_store::paths::AppPaths;
use std::collections::BTreeMap;

const GERMAN: &[(&str, &str)] = &[
    ("Address", "Adresse"),
    ("Content Type", "Inhaltstyp"),
    ("Status Code", "Statuscode"),
    ("Status", "Status"),
    ("Indexability", "Indexierbarkeit"),
    ("Indexability Status", "Indexierbarkeitsstatus"),
    ("Title", "Titel"),
    ("Title 1", "Titel 1"),
    ("Title 1 Length", "Titel 1 Länge"),
    ("Title 1 Pixel Width", "Titel 1 Pixelbreite"),
    ("Meta Description 1", "Meta-Beschreibung 1"),
    ("Meta Description 1 Length", "Meta-Beschreibung 1 Länge"),
    (
        "Meta Description 1 Pixel Width",
        "Meta-Beschreibung 1 Pixelbreite",
    ),
    ("Word Count", "Wortanzahl"),
    ("Crawl Depth", "Crawltiefe"),
    ("Folder Depth", "Ordnertiefe"),
    ("Size (bytes)", "Größe (Bytes)"),
    ("Transferred (bytes)", "Übertragen (Bytes)"),
    ("Response Time", "Antwortzeit"),
    ("Inlinks", "Eingehende Links"),
    ("Unique Inlinks", "Eindeutige eingehende Links"),
    ("Outlinks", "Ausgehende Links"),
    ("Unique Outlinks", "Eindeutige ausgehende Links"),
    ("External Outlinks", "Externe ausgehende Links"),
    ("Redirect URL", "Weiterleitungs-URL"),
    ("Redirect Type", "Weiterleitungstyp"),
    ("Last Modified", "Zuletzt geändert"),
    ("Crawl Timestamp", "Crawl-Zeitstempel"),
    ("Source", "Quelle"),
    ("Destination", "Ziel"),
    ("Type", "Typ"),
    ("Occurrences", "Vorkommen"),
];

pub fn labels(mode: Option<&str>, paths: &AppPaths) -> Result<BTreeMap<String, String>, CliError> {
    if mode != Some("ui") {
        return Ok(BTreeMap::new());
    }
    let settings = os_model::settings::AppSettings::load(&paths.settings)
        .map_err(|e| CliError::usage(e.to_string()))?;
    let german = match settings.general.language {
        UiLanguage::De => true,
        UiLanguage::En => false,
        UiLanguage::System => ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
            .is_some_and(|lang| lang.to_ascii_lowercase().starts_with("de")),
    };
    Ok(if german {
        GERMAN
            .iter()
            .map(|(key, label)| ((*key).into(), (*label).into()))
            .collect()
    } else {
        BTreeMap::new()
    })
}

/// Target adapters infer cell types using canonical metadata even for localized tables.
pub fn canonical(name: &str) -> &str {
    GERMAN
        .iter()
        .find(|(_, label)| *label == name)
        .map_or(name, |(key, _)| key)
}
