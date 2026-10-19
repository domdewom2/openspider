//! Rechtschreib- und Grammatikprüfung (ONPAGE-153 … ONPAGE-157, ONPAGE-166): Sprache wählen, Bereiche prüfen,
//! Zähler nach `page.ext`, Befunde über die Engine nach `WriteBatch.spelling` (`spelling_findings`).
//!
//! **Bereiche:** bei HTML `Page Title` (erster Titel), `Meta Description` (erste Meta-Description) und `Body` (Text des
//! Inhaltsbereichs nach ONPAGE-140 mit Blockgrenzen); bei PDFs `Page Title` (Dokumenttitel) und `Body` (Textlayer).
//! Befunde stehen in dieser Reihenfolge, innerhalb eines Bereichs nach Position.
//!
//! **Erweiterungsschlüssel** (Namensraum `spelling`, M2-D19):
//!
//! | Schlüssel | Wert |
//! |---|---|
//! | `spelling.language` | Spalte `Language`: `de-DE`, `xx (unsupported)`, `de-DE (no dictionary)` |
//! | `spelling.lang_source` | Sprachangabe des Dokuments (`lang`, `xml:lang`, PDF `/Lang`), leer ohne Angabe |
//! | `spelling.spelling_errors` | Anzahl Rechtschreibfehler; leer bei ausgeschalteter Prüfung, nicht unterstützter Sprache oder ohne Wörterbuch |
//! | `spelling.grammar_errors` | Anzahl Grammatikfehler; leer bei ausgeschalteter Prüfung oder nicht unterstützter Sprache |
//! | `spelling.findings` | nur Transport: Befunde als JSON; die Engine nimmt sie mit [`take_findings`] heraus |
//!
//! **Befunde** (`spelling_findings`): `kind` = `Spelling` | `Grammar`, `section` = `Page Title` | `Meta Description` |
//! `Body`, `text` = Fundstelle, `detail` = Beschreibung, `suggestion` = bis zu 3 Vorschläge durch „, “ getrennt,
//! `rule_id` nur bei Grammatik.

pub mod dictionary;
pub mod grammar;
pub mod lang;
pub mod tokens;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use os_model::{ExtKey, GrammarConfig, ParsedPage, SpellingConfig, SpellingWrite, UrlId, Value};
use serde::{Deserialize, Serialize};

use crate::PageCtx;

pub use dictionary::Resources;
pub use lang::Language;

/// Bereich „Page Title“.
pub const SECTION_TITLE: &str = "Page Title";
/// Bereich „Meta Description“.
pub const SECTION_DESCRIPTION: &str = "Meta Description";
/// Bereich „Body“.
pub const SECTION_BODY: &str = "Body";
/// Befundart Rechtschreibung.
pub const KIND_SPELLING: &str = "Spelling";
/// Befundart Grammatik.
pub const KIND_GRAMMAR: &str = "Grammar";
/// Beschreibung eines Rechtschreibbefunds.
pub const SPELLING_DETAIL: &str = "Word not found in the dictionary.";

/// Schlüssel `spelling.language`.
pub const KEY_LANGUAGE: &str = "spelling.language";
/// Schlüssel `spelling.lang_source`.
pub const KEY_LANG_SOURCE: &str = "spelling.lang_source";
/// Schlüssel `spelling.spelling_errors`.
pub const KEY_SPELLING_ERRORS: &str = "spelling.spelling_errors";
/// Schlüssel `spelling.grammar_errors`.
pub const KEY_GRAMMAR_ERRORS: &str = "spelling.grammar_errors";
/// Schlüssel `spelling.findings` (nur Transport).
pub const KEY_FINDINGS: &str = "spelling.findings";

/// Ein Befund der Prüfung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// [`KIND_SPELLING`] oder [`KIND_GRAMMAR`].
    pub kind: String,
    /// Bereich ([`SECTION_TITLE`], [`SECTION_DESCRIPTION`], [`SECTION_BODY`]).
    pub section: String,
    /// Fundstelle.
    pub text: String,
    /// Beschreibung.
    pub detail: String,
    /// Vorschläge (höchstens 3).
    pub suggestions: Vec<String>,
    /// Regelkennung (nur Grammatik).
    pub rule_id: Option<String>,
    /// Erste Zeichenposition im Bereich.
    pub start: usize,
    /// Zeichenposition hinter der Fundstelle.
    pub end: usize,
}

impl Finding {
    /// Vorschläge als Spaltenwert (`a, b, c`), `None` ohne Vorschlag.
    pub fn suggestion(&self) -> Option<String> {
        (!self.suggestions.is_empty()).then(|| self.suggestions.join(", "))
    }
}

/// Die geprüften Bereiche einer Seite.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input<'a> {
    /// Sprachangabe des Dokuments.
    pub declared: Option<&'a str>,
    /// Titel.
    pub title: Option<&'a str>,
    /// Meta-Description.
    pub description: Option<&'a str>,
    /// Text des Inhaltsbereichs bzw. des PDFs (Zeilenumbruch = Blockgrenze).
    pub body: &'a str,
}

impl<'a> Input<'a> {
    /// Bereiche in Ausgabereihenfolge.
    pub fn sections(&self) -> Vec<(&'static str, &'a str)> {
        let mut out = Vec::new();
        if let Some(t) = self.title.filter(|t| !t.trim().is_empty()) {
            out.push((SECTION_TITLE, t));
        }
        if let Some(d) = self.description.filter(|d| !d.trim().is_empty()) {
            out.push((SECTION_DESCRIPTION, d));
        }
        out.push((SECTION_BODY, self.body));
        out
    }
}

/// Ergebnis der Prüfung einer Seite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Gewählte Sprache.
    pub language: Language,
    /// Wert der Spalte `Language`.
    pub label: String,
    /// Anzahl Rechtschreibfehler (`None` = nicht geprüft).
    pub spelling: Option<u32>,
    /// Anzahl Grammatikfehler (`None` = nicht geprüft).
    pub grammar: Option<u32>,
    /// Befunde.
    pub findings: Vec<Finding>,
}

impl Outcome {
    /// Erweiterungswerte der Seite (einschließlich der Befunde zum Transport).
    pub fn facts(&self, declared: Option<&str>) -> Vec<(ExtKey, Value)> {
        let count = |n: Option<u32>| n.map_or(Value::Null, |n| Value::Int(i64::from(n)));
        vec![
            (
                ExtKey::from_static(KEY_LANGUAGE),
                Value::Text(self.label.clone()),
            ),
            (
                ExtKey::from_static(KEY_LANG_SOURCE),
                Value::Text(declared.unwrap_or_default().trim().to_string()),
            ),
            (
                ExtKey::from_static(KEY_SPELLING_ERRORS),
                count(self.spelling),
            ),
            (ExtKey::from_static(KEY_GRAMMAR_ERRORS), count(self.grammar)),
            (
                ExtKey::from_static(KEY_FINDINGS),
                Value::Text(serde_json::to_string(&self.findings).unwrap_or_else(|_| "[]".into())),
            ),
        ]
    }
}

/// Prüft die Bereiche einer Seite. `ignored` ist die Ignorierliste des Crawls (klein geschrieben).
pub fn check(
    input: &Input<'_>,
    spelling: &SpellingConfig,
    grammar: &GrammarConfig,
    ignored: &HashSet<String>,
    resources: &Resources,
) -> Outcome {
    let language = lang::resolve(input.declared, spelling);
    let mut outcome = Outcome {
        label: language.label(),
        language: language.clone(),
        spelling: None,
        grammar: None,
        findings: Vec::new(),
    };
    if !language.supported || !(spelling.enabled || grammar.enabled) {
        return outcome;
    }
    let dict = spelling
        .enabled
        .then(|| dictionary::find(&language.code, resources))
        .flatten();
    if spelling.enabled && dict.is_none() {
        outcome.label = format!("{} (no dictionary)", language.code);
    }
    let personal = if dict.is_some() {
        dictionary::personal_words(&language.code, resources)
    } else {
        HashSet::new()
    };
    let mut suggestions: HashMap<String, Vec<String>> = HashMap::new();
    let (mut n_spelling, mut n_grammar) = (0u32, 0u32);
    for (section, text) in input.sections() {
        let mut found = Vec::new();
        if let Some(dict) = &dict {
            for f in spell(text, dict, &personal, ignored, &mut suggestions) {
                found.push(Finding {
                    section: section.to_string(),
                    ..f
                });
            }
        }
        if grammar.enabled {
            for m in grammar::check(text, &language.code, &grammar.disabled_rules) {
                found.push(Finding {
                    kind: KIND_GRAMMAR.to_string(),
                    section: section.to_string(),
                    text: m.text,
                    detail: m.description,
                    suggestions: m.suggestions,
                    rule_id: Some(m.rule_id),
                    start: m.start,
                    end: m.end,
                });
            }
        }
        found.sort_by(|a, b| (a.start, a.end, &a.kind).cmp(&(b.start, b.end, &b.kind)));
        n_spelling += found.iter().filter(|f| f.kind == KIND_SPELLING).count() as u32;
        n_grammar += found.iter().filter(|f| f.kind == KIND_GRAMMAR).count() as u32;
        outcome.findings.extend(found);
    }
    outcome.spelling = dict.is_some().then_some(n_spelling);
    outcome.grammar = grammar.enabled.then_some(n_grammar);
    outcome
}

/// Rechtschreibbefunde eines Bereichs (ohne `section`).
fn spell(
    text: &str,
    dict: &dictionary::Dictionary,
    personal: &HashSet<String>,
    ignored: &HashSet<String>,
    cache: &mut HashMap<String, Vec<String>>,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for token in tokens::tokenize(text) {
        for word in tokens::words(&token) {
            let normalized = word.text.replace('’', "'");
            let lower = normalized.to_lowercase();
            if personal.contains(&lower) || ignored.contains(&lower) || dict.check(&normalized) {
                continue;
            }
            let suggestions = cache
                .entry(normalized.clone())
                .or_insert_with(|| {
                    let mut all = Vec::new();
                    dict.suggest(&normalized, &mut all);
                    all.truncate(3);
                    all
                })
                .clone();
            out.push(Finding {
                kind: KIND_SPELLING.to_string(),
                section: String::new(),
                text: word.text.to_string(),
                detail: SPELLING_DETAIL.to_string(),
                suggestions,
                rule_id: None,
                start: word.start,
                end: word.start + word.text.chars().count(),
            });
        }
    }
    out
}

/// Prüft eine HTML-Seite (Aufruf aus `parse_html`, nur bei eingeschalteter Rechtschreib- oder Grammatikprüfung).
pub fn run(ctx: &PageCtx<'_>, page: &mut ParsedPage) {
    let declared = lang::declared_html(ctx.doc);
    let body = crate::content::area_text(ctx.doc, &ctx.cfg.content_area);
    let input = Input {
        declared: declared.as_deref(),
        title: page.titles.first().map(|t| t.text.as_str()),
        description: page.meta_descriptions.first().map(|d| d.text.as_str()),
        body: &body,
    };
    let outcome = check(
        &input,
        &ctx.cfg.spelling,
        &ctx.cfg.grammar,
        &HashSet::new(),
        &dictionary::resources(),
    );
    page.ext.extend(outcome.facts(declared.as_deref()));
}

/// Sprachangabe des PDF-Dokumentkatalogs (`/Lang`).
pub fn pdf_language(doc: &lopdf::Document) -> Option<String> {
    let raw = doc.catalog().ok()?.get(b"Lang").ok()?.as_str().ok()?;
    let text = match raw {
        [0xFE, 0xFF, rest @ ..] => String::from_utf16_lossy(
            &rest
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect::<Vec<_>>(),
        ),
        _ => String::from_utf8_lossy(raw).into_owned(),
    };
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// Prüft eine PDF mit gespeichertem Textlayer (ONPAGE-166): Bereiche `Page Title` (Dokumenttitel) und `Body`.
pub fn check_pdf(
    declared: Option<&str>,
    title: Option<&str>,
    text: &str,
    cfg: &crate::ParseConfig,
) -> Vec<(ExtKey, Value)> {
    let input = Input {
        declared,
        title,
        description: None,
        body: text,
    };
    check(
        &input,
        &cfg.spelling,
        &cfg.grammar,
        &HashSet::new(),
        &dictionary::resources(),
    )
    .facts(declared)
}

/// Sprachcode eines Werts der Spalte `Language` (ohne Zusatz wie „(unsupported)“).
pub fn language_code(label: &str) -> &str {
    label.split(" (").next().unwrap_or(label)
}

/// Liest die Befunde aus einem Wert von `spelling.findings`.
pub fn decode_findings(value: &Value) -> Vec<Finding> {
    match value {
        Value::Text(json) => serde_json::from_str(json).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Befunde als Schreibzeilen für `spelling_findings`.
pub fn to_writes(url: UrlId, language: &str, findings: &[Finding]) -> Vec<SpellingWrite> {
    findings
        .iter()
        .map(|f| SpellingWrite {
            url_id: url,
            kind: f.kind.clone(),
            language: language.to_string(),
            section: f.section.clone(),
            text: f.text.clone(),
            detail: Some(f.detail.clone()),
            suggestion: f.suggestion(),
            rule_id: f.rule_id.clone(),
        })
        .collect()
}

/// Nimmt `spelling.findings` aus den Erweiterungswerten einer Seite und liefert die Schreibzeilen für
/// `spelling_findings` (Transport in der Engine, `WriteBatch.spelling`). Ohne Prüfung bleibt alles unverändert.
pub fn take_findings(url: UrlId, facts: &mut Vec<(ExtKey, Value)>) -> Vec<SpellingWrite> {
    let Some(at) = facts.iter().position(|(k, _)| k.as_str() == KEY_FINDINGS) else {
        return Vec::new();
    };
    let (_, value) = facts.remove(at);
    let language = facts
        .iter()
        .find(|(k, _)| k.as_str() == KEY_LANGUAGE)
        .and_then(|(_, v)| match v {
            Value::Text(t) => Some(language_code(t).to_string()),
            _ => None,
        })
        .unwrap_or_default();
    to_writes(url, &language, &decode_findings(&value))
}
