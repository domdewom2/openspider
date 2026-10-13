//! Schwellen für den Paritätslauf (A3 aus M1.md Abschnitt 6).
//!
//! Standardwerte stehen im Code ([`Thresholds::builtin`]); `tests/parity/thresholds.toml`
//! (Besitz P10/P29) überschreibt einzelne Einträge:
//!
//! ```toml
//! default_agreement_pct = 97.0     # Standard für alle Exporte ohne passendes Muster
//! url_set_identical = true         # URL-Menge muss (abzüglich Abweichungen) identisch sein
//! min_sites_present = 4            # bei `run --all`: mindestens so viele Sites mit Status present
//! [agreement_pct]                  # Muster (Mapping-Name, optional mit abschließendem `*`) -> Prozent
//! "internal_*" = 99.0
//! "links_*" = 97.0
//! ```

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::ParityError;

/// Schwellen eines Laufs.
#[derive(Debug, Clone, PartialEq)]
pub struct Thresholds {
    /// Zellübereinstimmung für Exporte ohne passendes Muster.
    pub default_agreement_pct: f64,
    /// URL-Menge muss identisch sein.
    pub url_set_identical: bool,
    /// Mindestzahl Sites mit Status `present` bei `run --all`.
    pub min_sites_present: usize,
    /// Muster → Mindest-Übereinstimmung in Prozent.
    pub agreement_pct: BTreeMap<String, f64>,
}

#[derive(Deserialize, Default)]
struct Raw {
    default_agreement_pct: Option<f64>,
    url_set_identical: Option<bool>,
    min_sites_present: Option<usize>,
    #[serde(default)]
    agreement_pct: BTreeMap<String, f64>,
}

impl Thresholds {
    /// Standardwerte nach A3: ≥ 99 % für Internal, Response Codes, Page Titles, Meta Description,
    /// H1, H2, Canonicals, Directives; ≥ 97 % für Images, Hreflang, Links, Inlinks, Redirects;
    /// Crawl-Overview-Zähler identisch (100 %).
    pub fn builtin() -> Self {
        let mut m = BTreeMap::new();
        for p in [
            "internal_*",
            "response_codes_*",
            "page_titles_*",
            "meta_description_*",
            "h1_*",
            "h2_*",
            "canonicals_*",
            "directives_*",
        ] {
            m.insert(p.to_string(), 99.0);
        }
        for p in [
            "images_*",
            "hreflang_*",
            "links_*",
            "all_inlinks*",
            "redirect*",
        ] {
            m.insert(p.to_string(), 97.0);
        }
        m.insert("crawl_overview*".to_string(), 100.0);
        Self {
            default_agreement_pct: 97.0,
            url_set_identical: true,
            min_sites_present: 4,
            agreement_pct: m,
        }
    }

    /// Lädt die Datei über den Standardwerten; eine fehlende Datei ergibt die Standardwerte.
    pub fn load(path: &Path) -> Result<Self, ParityError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text)
                .map_err(|e| ParityError::Invalid(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::builtin()),
            Err(e) => Err(e.into()),
        }
    }

    /// Parst TOML und legt es über die Standardwerte.
    pub fn parse(text: &str) -> Result<Self, ParityError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ParityError::Invalid(e.to_string()))?;
        let mut t = Self::builtin();
        if let Some(v) = raw.default_agreement_pct {
            t.default_agreement_pct = v;
        }
        if let Some(v) = raw.url_set_identical {
            t.url_set_identical = v;
        }
        if let Some(v) = raw.min_sites_present {
            t.min_sites_present = v;
        }
        t.agreement_pct.extend(raw.agreement_pct);
        Ok(t)
    }

    /// Mindest-Übereinstimmung für einen Export (Mapping-Name); das längste passende Muster gewinnt.
    pub fn agreement_for(&self, export: &str) -> f64 {
        self.agreement_pct
            .iter()
            .filter(|(pat, _)| match pat.strip_suffix('*') {
                Some(prefix) => export.starts_with(prefix),
                None => export == pat.as_str(),
            })
            .max_by_key(|(pat, _)| pat.len())
            .map(|(_, v)| *v)
            .unwrap_or(self.default_agreement_pct)
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self::builtin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_values_follow_acceptance_criteria() {
        let t = Thresholds::builtin();
        assert_eq!(t.agreement_for("internal_all"), 99.0);
        assert_eq!(t.agreement_for("response_codes_client_error_4xx"), 99.0);
        assert_eq!(t.agreement_for("page_titles_over_60_characters"), 99.0);
        assert_eq!(t.agreement_for("canonicals_missing"), 99.0);
        assert_eq!(t.agreement_for("images_missing_alt_text"), 97.0);
        assert_eq!(t.agreement_for("all_inlinks"), 97.0);
        assert_eq!(t.agreement_for("redirect_chains"), 97.0);
        assert_eq!(t.agreement_for("crawl_overview"), 100.0);
        assert_eq!(t.agreement_for("irgendwas_anderes"), 97.0);
        assert!(t.url_set_identical);
        assert_eq!(t.min_sites_present, 4);
    }

    #[test]
    fn file_overrides_builtin_and_longest_pattern_wins() {
        let t = Thresholds::parse(
            "default_agreement_pct = 90.0\nmin_sites_present = 2\n[agreement_pct]\n\"internal_all\" = 100.0\n\"neu_*\" = 50.0\n",
        )
        .unwrap();
        assert_eq!(t.agreement_for("internal_all"), 100.0);
        assert_eq!(
            t.agreement_for("internal_html"),
            99.0,
            "Standardmuster bleibt"
        );
        assert_eq!(t.agreement_for("neu_x"), 50.0);
        assert_eq!(t.agreement_for("unbekannt"), 90.0);
        assert_eq!(t.min_sites_present, 2);
    }

    #[test]
    fn missing_file_and_bad_toml() {
        assert_eq!(
            Thresholds::load(Path::new("/nicht/da/thresholds.toml")).unwrap(),
            Thresholds::builtin()
        );
        assert!(Thresholds::parse("default_agreement_pct = [").is_err());
    }
}
