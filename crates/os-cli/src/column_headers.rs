//! OUT-196: presentation headers follow the saved UI language; canonical keys stay stable.
use crate::exit::CliError;
use os_model::UiLanguage;
use os_store::paths::AppPaths;
use std::collections::BTreeMap;

// Generated from the same Clean-Room Fluent catalogs the desktop uses.
include!(concat!(env!("OUT_DIR"), "/column_labels.rs"));

pub fn labels(mode: Option<&str>, paths: &AppPaths) -> Result<BTreeMap<String, String>, CliError> {
    if mode != Some("ui") {
        return Ok(BTreeMap::new());
    }
    let settings = os_model::settings::AppSettings::load(&paths.settings)
        .map_err(|e| CliError::usage(e.to_string()))?;
    Ok(labels_for_language(
        settings.general.language,
        sys_locale::get_locale().as_deref(),
    ))
}

fn labels_for_language(language: UiLanguage, system: Option<&str>) -> BTreeMap<String, String> {
    let german = match language {
        UiLanguage::De => true,
        UiLanguage::En => false,
        UiLanguage::System => system.is_some_and(|lang| {
            lang.split(['-', '_', '.'])
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("de"))
        }),
    };
    if german {
        GERMAN
            .iter()
            .map(|(key, label)| ((*key).into(), (*label).into()))
            .collect()
    } else {
        BTreeMap::new()
    }
}

/// Target adapters infer cell types using canonical metadata even for localized tables.
pub fn canonical(name: &str) -> &str {
    GERMAN
        .iter()
        .find(|(_, label)| *label == name)
        .map_or(name, |(key, _)| key)
}

#[cfg(test)]
mod tests {
    use super::*;
    // spec: OUT-198
    // spec: OUT-196
    #[test]
    fn injected_system_language_uses_generated_ui_labels() {
        for locale in ["de-DE", "de_DE.UTF-8", "DE-at"] {
            let labels = labels_for_language(UiLanguage::System, Some(locale));
            assert_eq!(labels["Address"], "Adresse");
            assert_eq!(labels["Title 1"], "Titel 1");
            assert!(labels.len() > 100, "all UI column labels are generated");
        }
        assert!(labels_for_language(UiLanguage::System, Some("en-US")).is_empty());
        assert!(labels_for_language(UiLanguage::System, None).is_empty());
        assert!(labels_for_language(UiLanguage::En, Some("de-DE")).is_empty());
        assert_eq!(
            labels_for_language(UiLanguage::De, None)["Status Code"],
            "Statuscode"
        );
        assert_eq!(canonical("Statuscode"), "Status Code");
    }
}
