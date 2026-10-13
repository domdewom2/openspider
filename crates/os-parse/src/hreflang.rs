//! hreflang-Annotationen (TECH-070 ff.) und Validierung der Sprach-/Regionscodes (TECH-072).
//!
//! Annotationen kommen aus `<link rel=alternate hreflang href>` (hier), aus dem `Link`-Header (`headers`) und
//! aus `xhtml:link` in XML-Sitemaps (`sitemap`). Alle drei erzeugen ihre [`HreflangRef`] über [`make_ref`].
//!
//! # Gültige Codes (TECH-072)
//!
//! Ohne Beachtung der Schreibweise: `x-default`, `ll`, `ll-RR` und mit Script-Option zusätzlich `ll-Ssss` und
//! `ll-Ssss-RR`. `ll` steht in ISO 639-1, `RR` in ISO 3166-1 alpha-2, `Ssss` in ISO 15924 (eingecheckte
//! Tabellen unter `resources/iso/`, erzeugt aus den Unicode-CLDR-Validitätsdaten; zurückgezogene Sprachcodes wie
//! `iw`, `in`, `ji` stehen nicht darin). Fehlergründe: `invalid_language`, `invalid_region`, `invalid_format`
//! (ein unbekannter Skriptcode und ein nicht erlaubtes Script-Subtag gelten als `invalid_format`).
//!
//! Festlegung zu `EU`: Der Code wird großgeschrieben und allein stehend als Regionscode der Europäischen Union
//! gelesen (TECH-072 führt `EU` ausdrücklich als ungültig), obwohl `eu` (klein) das baskische Sprachkürzel ist,
//! das als `eu` gültig bleibt.

use std::collections::HashSet;
use std::sync::OnceLock;

use os_model::{HintSource, HreflangOverLimit, HreflangRef};

use crate::canonical::{has_rel_token, link_elements, raw_and_resolved};
use crate::dom::{self, Document};
use crate::ParseConfig;

/// Höchstzahl gespeicherter Annotationen je URL (TECH-004).
pub const MAX_ANNOTATIONS: usize = 500;

/// Fehlergrund: Sprachcode unbekannt oder zurückgezogen.
pub const INVALID_LANGUAGE: &str = "invalid_language";
/// Fehlergrund: Regionscode unbekannt (auch numerische Regionen wie `419`).
pub const INVALID_REGION: &str = "invalid_region";
/// Fehlergrund: Form des Codes verletzt (Trenner, Länge, Reihenfolge, Script nicht erlaubt/unbekannt).
pub const INVALID_FORMAT: &str = "invalid_format";

const ISO_639_1: &str = include_str!("../resources/iso/iso639-1.txt");
const ISO_3166_1: &str = include_str!("../resources/iso/iso3166-1-alpha2.txt");
const ISO_15924: &str = include_str!("../resources/iso/iso15924.txt");

/// Allein stehend großgeschrieben als Regionscode gelesen (siehe Moduldoku).
const UPPERCASE_REGION_ONLY: &[&str] = &["EU"];

fn table(text: &'static str) -> HashSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_ascii_lowercase())
        .collect()
}

fn languages() -> &'static HashSet<String> {
    static T: OnceLock<HashSet<String>> = OnceLock::new();
    T.get_or_init(|| table(ISO_639_1))
}

fn regions() -> &'static HashSet<String> {
    static T: OnceLock<HashSet<String>> = OnceLock::new();
    T.get_or_init(|| table(ISO_3166_1))
}

fn scripts() -> &'static HashSet<String> {
    static T: OnceLock<HashSet<String>> = OnceLock::new();
    T.get_or_init(|| table(ISO_15924))
}

/// Prüft einen hreflang-Wert (TECH-072). `Ok(())` = gültig, sonst der Fehlergrund.
pub fn validate_code(code: &str, allow_script: bool) -> Result<(), &'static str> {
    if code.is_empty() {
        return Err(INVALID_FORMAT);
    }
    if code.eq_ignore_ascii_case("x-default") {
        return Ok(());
    }
    let parts: Vec<&str> = code.split('-').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return Err(INVALID_FORMAT);
    }
    if parts.len() > 3 {
        return Err(INVALID_FORMAT);
    }

    let lang = parts[0];
    if !lang.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(INVALID_FORMAT);
    }
    match lang.len() {
        2 => {
            let region_reading = parts.len() == 1 && UPPERCASE_REGION_ONLY.contains(&lang);
            if region_reading || !languages().contains(&lang.to_ascii_lowercase()) {
                return Err(INVALID_LANGUAGE);
            }
        }
        3 => return Err(INVALID_LANGUAGE),
        _ => return Err(INVALID_FORMAT),
    }

    let mut rest = &parts[1..];
    if let Some(&second) = rest.first() {
        let is_script_shape = second.len() == 4 && second.bytes().all(|b| b.is_ascii_alphabetic());
        if is_script_shape {
            if !allow_script || !scripts().contains(&second.to_ascii_lowercase()) {
                return Err(INVALID_FORMAT);
            }
            rest = &rest[1..];
        }
    }
    match rest {
        [] => Ok(()),
        [region] => validate_region(region),
        _ => Err(INVALID_FORMAT),
    }
}

fn validate_region(region: &str) -> Result<(), &'static str> {
    if region.len() == 2 && region.bytes().all(|b| b.is_ascii_alphabetic()) {
        if regions().contains(&region.to_ascii_lowercase()) {
            Ok(())
        } else {
            Err(INVALID_REGION)
        }
    } else if region.len() == 3 && region.bytes().all(|b| b.is_ascii_digit()) {
        Err(INVALID_REGION)
    } else {
        Err(INVALID_FORMAT)
    }
}

/// Baut eine Annotation: Code getrimmt (roh) und kleingeschrieben (normalisiert), Gültigkeit nach TECH-072.
pub(crate) fn make_ref(
    source: HintSource,
    code: &str,
    href_raw: String,
    resolved: Option<String>,
    in_head: bool,
    order: u32,
    allow_script: bool,
) -> HreflangRef {
    let code_raw = code.trim().to_string();
    let verdict = validate_code(&code_raw, allow_script);
    HreflangRef {
        source,
        code_norm: code_raw.to_ascii_lowercase(),
        code_raw,
        code_valid: verdict.is_ok(),
        invalid_reason: verdict.err().map(str::to_string),
        href_raw,
        resolved,
        in_head,
        order,
    }
}

/// Wendet das 500er-Limit an (TECH-004): `Truncate` behält die ersten 500, `Skip` verwirft alle. Zweiter Wert =
/// verworfene Annotationen.
pub(crate) fn apply_limit(
    mut refs: Vec<HreflangRef>,
    mode: HreflangOverLimit,
) -> (Vec<HreflangRef>, u32) {
    if refs.len() <= MAX_ANNOTATIONS {
        return (refs, 0);
    }
    let total = refs.len();
    match mode {
        HreflangOverLimit::Truncate => {
            refs.truncate(MAX_ANNOTATIONS);
            (refs, (total - MAX_ANNOTATIONS) as u32)
        }
        HreflangOverLimit::Skip => (Vec::new(), total as u32),
    }
}

/// Extrahiert die Annotationen; zweiter Wert = wegen des 500er-Limits verworfene Annotationen.
pub fn extract(doc: &Document, base: &url::Url, cfg: &ParseConfig) -> (Vec<HreflangRef>, u32) {
    let mut refs = Vec::new();
    for el in link_elements(doc) {
        let Some(rel) = el.value().attr("rel") else {
            continue;
        };
        if !has_rel_token(rel, "alternate") {
            continue;
        }
        let Some(code) = el.value().attr("hreflang") else {
            continue;
        };
        if code.trim().is_empty() {
            continue;
        }
        let Some(href) = el.value().attr("href") else {
            continue;
        };
        let (raw, resolved) = raw_and_resolved(base, href);
        refs.push(make_ref(
            HintSource::Html,
            code,
            raw,
            resolved,
            dom::in_head(el),
            refs.len() as u32,
            cfg.allow_script_subtag,
        ));
    }
    apply_limit(refs, cfg.hreflang_over_limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_the_expected_size() {
        assert_eq!(languages().len(), 184);
        assert!(regions().len() >= 249);
        assert!(scripts().len() >= 180);
    }

    #[test]
    fn limit_modes() {
        let make = |n: usize| -> Vec<HreflangRef> {
            (0..n)
                .map(|i| {
                    make_ref(
                        HintSource::Html,
                        "de",
                        format!("/{i}"),
                        None,
                        true,
                        i as u32,
                        true,
                    )
                })
                .collect()
        };
        let (kept, dropped) = apply_limit(make(510), HreflangOverLimit::Truncate);
        assert_eq!((kept.len(), dropped), (500, 10));
        let (kept, dropped) = apply_limit(make(510), HreflangOverLimit::Skip);
        assert_eq!((kept.len(), dropped), (0, 510));
        let (kept, dropped) = apply_limit(make(500), HreflangOverLimit::Skip);
        assert_eq!((kept.len(), dropped), (500, 0));
    }
}
