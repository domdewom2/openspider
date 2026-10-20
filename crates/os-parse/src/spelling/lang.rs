//! Sprachwahl der Rechtschreib- und Grammatikprüfung (ONPAGE-153).
//!
//! Modus „Auto“: Sprache aus `<html lang>` (BCP 47, Groß-/Kleinschreibung egal; `xml:lang` nur ohne `lang`). Fehlt die
//! Region, gilt die Variante aus `spelling.default_variants`, sonst die eingebaute Variante der Spec (`en` → `en-GB`,
//! `de` → `de-DE`, `pt` → `pt-PT`, `fa` → `fa-IR`); Sprachen ohne Varianten bleiben ohne Region (`fr`). Fehlt `lang`
//! oder ist es leer, gilt `spelling.default_language` (ebenso aufgelöst). Modus „Manuell“: `spelling.manual_language`
//! für jede Seite.
//!
//! Unterstützt sind die Sprachen der Zielmenge (Stufen 1 und 2) und die Erweiterungen `nb`, `fi`, `cs`; die Sprachen
//! ohne Leerzeichen-Wortgrenzen (`ja`, `zh`, `km`, Stufe 3) und unbekannte Codes gelten als „unsupported“.

use os_model::{SpellingConfig, SpellingMode};

/// Basissprachen der Stufen 1 und 2 sowie die Erweiterungen `nb`, `fi`, `cs`.
pub const SUPPORTED: &[&str] = &[
    "ar", "ast", "be", "br", "ca", "cs", "da", "de", "el", "en", "es", "fa", "fi", "fr", "ga",
    "gl", "it", "nb", "nl", "pl", "pt", "ro", "ru", "sk", "sl", "sv", "ta", "tl", "uk",
];

/// Sprachen ohne Leerzeichen-Wortgrenzen (Stufe 3, P2): bis zur Wortsegmentierung „unsupported“.
pub const WITHOUT_WORD_SPACES: &[&str] = &["ja", "zh", "km"];

/// Eingebaute Varianten je Sprache ohne Region (Defaults der Spec, ONPAGE-153).
pub const BUILTIN_VARIANTS: &[(&str, &str)] = &[
    ("en", "en-GB"),
    ("de", "de-DE"),
    ("pt", "pt-PT"),
    ("fa", "fa-IR"),
];

/// Ergebnis der Sprachwahl einer Seite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    /// Code in kanonischer Schreibweise (`de-DE`, `fr`, `xx`).
    pub code: String,
    /// Ist die Sprache unterstützt?
    pub supported: bool,
}

impl Language {
    /// Basissprache (`de` für `de-AT`).
    pub fn base(&self) -> &str {
        base_of(&self.code)
    }

    /// Wert der Spalte `Language`: der Code, bei nicht unterstützten Sprachen mit Zusatz „(unsupported)“.
    pub fn label(&self) -> String {
        if self.supported {
            self.code.clone()
        } else {
            format!("{} (unsupported)", self.code)
        }
    }
}

/// Basissprache eines kanonischen Codes.
pub fn base_of(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

/// Zerlegt einen BCP-47-Code in Sprache (klein) und Region (groß); Schrift-Subtags und Varianten entfallen.
/// `None` bei leerem oder ungültigem Code.
pub fn parse_tag(raw: &str) -> Option<(String, Option<String>)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.split(['-', '_']);
    let lang = parts.next()?.to_ascii_lowercase();
    if !(2..=8).contains(&lang.len()) || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let region = parts.find_map(|p| {
        let alpha = p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic());
        let digits = p.len() == 3 && p.chars().all(|c| c.is_ascii_digit());
        (alpha || digits).then(|| p.to_ascii_uppercase())
    });
    Some((lang, region))
}

/// Kanonischer Code mit Default-Variante, wenn die Region fehlt.
pub fn canonical(raw: &str, cfg: &SpellingConfig) -> Option<String> {
    let (lang, region) = parse_tag(raw)?;
    if let Some(region) = region {
        return Some(format!("{lang}-{region}"));
    }
    let configured = cfg
        .default_variants
        .iter()
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(&lang))
        .and_then(|(_, v)| parse_tag(v))
        .filter(|(l, _)| *l == lang);
    if let Some((l, r)) = configured {
        return Some(match r {
            Some(r) => format!("{l}-{r}"),
            None => l,
        });
    }
    Some(
        BUILTIN_VARIANTS
            .iter()
            .find(|(l, _)| *l == lang)
            .map_or(lang, |(_, v)| (*v).to_string()),
    )
}

/// Ist die Basissprache unterstützt?
pub fn is_supported(code: &str) -> bool {
    let base = base_of(code);
    SUPPORTED.contains(&base) && !WITHOUT_WORD_SPACES.contains(&base)
}

/// Sprache einer Seite aus der Sprachangabe des Dokuments (`lang`, sonst `xml:lang`; bei PDFs `/Lang`).
pub fn resolve(declared: Option<&str>, cfg: &SpellingConfig) -> Language {
    let raw = match cfg.mode {
        SpellingMode::Manual => cfg.manual_language.as_str(),
        SpellingMode::Auto => declared
            .map(str::trim)
            .filter(|d| !d.is_empty())
            .unwrap_or(cfg.default_language.as_str()),
    };
    match canonical(raw, cfg) {
        Some(code) => Language {
            supported: is_supported(&code),
            code,
        },
        None => Language {
            code: raw.trim().to_string(),
            supported: false,
        },
    }
}

/// Sprachangabe von `<html>`: `lang`, nur ohne `lang` das Attribut `xml:lang`.
pub fn declared_html(doc: &crate::dom::Document) -> Option<String> {
    let root = doc.html().root_element();
    let attr = |name: &str| root.value().attr(name).map(str::to_string);
    attr("lang").or_else(|| attr("xml:lang"))
}
