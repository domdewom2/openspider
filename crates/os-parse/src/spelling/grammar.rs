//! Grammatikprüfung (ONPAGE-154; Klärung K2-03).
//!
//! **Option a) eigene Regelmenge:** Jede Regel hat eine Kennung, eine Kategorie und eine Beschreibung (eigene Texte)
//! und ist über `grammar.disabled_rules` einzeln abschaltbar. Die Regeln laufen auf dem normalisierten Text eines
//! Bereichs; Zeilenumbrüche sind Blockgrenzen, Satzgrenzen wie ONPAGE-149 (`.`, `!`, `?`, `…` am Tokenende, jeder
//! Block endet einen Satz). Für die Regel `SENTENCE_START_LOWER` beendet eine Abkürzung (`z.B.`, `bzw.`, `e.g.`,
//! Initiale wie `J.`) keinen Satz. Sprachunabhängig gelten acht Regeln, dazu 16 englische und 18 deutsche (je Sprache
//! mindestens 20).
//!
//! **Option b) externer Server:** [`set_external_backend`] hängt einen vom Benutzer betriebenen Grammatik-Server ein
//! ([`GrammarBackend`], z. B. ein HTTP-Adapter der App). Ist einer gesetzt, liefert er die Fundstellen anstelle der
//! eigenen Regeln; schlägt die Anfrage fehl, gelten die eigenen Regeln. Gebündelt wird kein Server.

use std::sync::{Arc, OnceLock, RwLock};

use super::tokens::{tokenize, Token};

/// Eine Grammatikregel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    /// Kennung (für `grammar.disabled_rules` und die Spalte `Rule ID`).
    pub id: &'static str,
    /// Kategorie.
    pub category: &'static str,
    /// Beschreibung (Spalte `Detail`).
    pub description: &'static str,
    /// Basissprache (`None` = alle Sprachen).
    pub lang: Option<&'static str>,
}

const fn rule(
    id: &'static str,
    category: &'static str,
    description: &'static str,
    lang: Option<&'static str>,
) -> Rule {
    Rule {
        id,
        category,
        description,
        lang,
    }
}

const REPETITION: &str = "Repetition";
const CAPITALIZATION: &str = "Capitalization";
const PUNCTUATION: &str = "Punctuation";
const CONFUSION: &str = "Confused Words";
const STYLE: &str = "Word Form";

/// Alle Regeln der eigenen Regelmenge.
pub const RULES: &[Rule] = &[
    // Sprachunabhängig
    rule(
        "WORD_REPEAT",
        REPETITION,
        "The same word appears twice in a row.",
        None,
    ),
    rule(
        "SENTENCE_START_LOWER",
        CAPITALIZATION,
        "A sentence should start with a capital letter.",
        None,
    ),
    rule(
        "DOUBLE_PUNCT",
        PUNCTUATION,
        "Two identical punctuation marks in a row.",
        None,
    ),
    rule(
        "SPACE_BEFORE_PUNCT",
        PUNCTUATION,
        "There should be no space before this punctuation mark.",
        None,
    ),
    rule(
        "SPACE_AFTER_COMMA",
        PUNCTUATION,
        "A space is missing after the comma or semicolon.",
        None,
    ),
    rule(
        "PUNCT_SEQUENCE",
        PUNCTUATION,
        "Two different punctuation marks follow each other.",
        None,
    ),
    rule(
        "UNPAIRED_BRACKET",
        PUNCTUATION,
        "This bracket has no matching counterpart.",
        None,
    ),
    rule(
        "SPACE_INSIDE_BRACKETS",
        PUNCTUATION,
        "There should be no space directly inside brackets.",
        None,
    ),
    // Englisch
    rule(
        "EN_COULD_OF",
        CONFUSION,
        "Use 'have' after could, should, would or must.",
        Some("en"),
    ),
    rule(
        "EN_MIGHT_OF",
        CONFUSION,
        "Use 'have' after might.",
        Some("en"),
    ),
    rule(
        "EN_A_AN",
        CONFUSION,
        "Use 'an' before a vowel sound and 'a' before a consonant sound.",
        Some("en"),
    ),
    rule(
        "EN_ITS_IT_IS",
        CONFUSION,
        "'its' is possessive; 'it's' means 'it is'.",
        Some("en"),
    ),
    rule(
        "EN_YOUR_YOURE",
        CONFUSION,
        "'your' is possessive; 'you're' means 'you are'.",
        Some("en"),
    ),
    rule(
        "EN_THEN_THAN",
        CONFUSION,
        "Use 'than' in comparisons.",
        Some("en"),
    ),
    rule(
        "EN_ALOT",
        STYLE,
        "'a lot' is written as two words.",
        Some("en"),
    ),
    rule("EN_IRREGARDLESS", STYLE, "Use 'regardless'.", Some("en")),
    rule(
        "EN_INFACT",
        STYLE,
        "'in fact' is written as two words.",
        Some("en"),
    ),
    rule(
        "EN_THEIR_THERE",
        CONFUSION,
        "'their' is possessive; 'there' refers to a place or existence.",
        Some("en"),
    ),
    rule(
        "EN_I_LOWER",
        CAPITALIZATION,
        "The pronoun 'I' is always capitalized.",
        Some("en"),
    ),
    rule(
        "EN_WHOSE_WHOS",
        CONFUSION,
        "'whose' is possessive; 'who's' means 'who is'.",
        Some("en"),
    ),
    rule(
        "EN_SUPPOSED_TO",
        STYLE,
        "Use the past participle: 'supposed to', 'used to'.",
        Some("en"),
    ),
    rule(
        "EN_TOO_TO",
        CONFUSION,
        "Use 'too' in the sense of 'excessively'.",
        Some("en"),
    ),
    rule(
        "EN_AFFECT_EFFECT",
        CONFUSION,
        "The noun is usually 'effect'.",
        Some("en"),
    ),
    rule(
        "EN_LOOSE_LOSE",
        CONFUSION,
        "The verb meaning 'to misplace' is 'lose'.",
        Some("en"),
    ),
    // Deutsch
    rule(
        "DE_SEIT_SEID",
        CONFUSION,
        "Time expressions use 'seit', not 'seid'.",
        Some("de"),
    ),
    rule(
        "DE_SO_DASS",
        CONFUSION,
        "The conjunction is written 'dass'.",
        Some("de"),
    ),
    rule(
        "DE_KOMMA_DASS",
        PUNCTUATION,
        "A comma is required before 'dass'.",
        Some("de"),
    ),
    rule(
        "DE_WIDER",
        CONFUSION,
        "In the sense of 'against', the prefix is 'wider'.",
        Some("de"),
    ),
    rule(
        "DE_STANDARD",
        CONFUSION,
        "The word is spelled 'Standard'.",
        Some("de"),
    ),
    rule(
        "DE_ZUMINDEST",
        STYLE,
        "Use 'zumindest' or 'mindestens'.",
        Some("de"),
    ),
    rule(
        "DE_EINZIGSTE",
        STYLE,
        "'einzig' has no superlative.",
        Some("de"),
    ),
    rule(
        "DE_ALS_WIE",
        CONFUSION,
        "Comparatives are followed by 'als'.",
        Some("de"),
    ),
    rule(
        "DE_KEINSTER",
        STYLE,
        "'kein' has no superlative.",
        Some("de"),
    ),
    rule(
        "DE_DES_WEITEREN",
        STYLE,
        "'des Weiteren' is written as two words.",
        Some("de"),
    ),
    rule(
        "DE_VORAUS",
        CONFUSION,
        "The word is spelled with one 'r': 'voraus'.",
        Some("de"),
    ),
    rule(
        "DE_AN_SICH",
        STYLE,
        "'an sich' is written as two words.",
        Some("de"),
    ),
    rule(
        "DE_GAR_NICHT",
        STYLE,
        "'gar nicht' and 'gar kein' are written as two words.",
        Some("de"),
    ),
    rule(
        "DE_IMMER_NOCH",
        STYLE,
        "'immer noch' is written as two words.",
        Some("de"),
    ),
    rule(
        "DE_BISSCHEN",
        CONFUSION,
        "The word is spelled 'bisschen'.",
        Some("de"),
    ),
    rule(
        "DE_WEISST",
        CONFUSION,
        "The second person of 'wissen' is 'weißt'.",
        Some("de"),
    ),
    rule(
        "DE_ENDGUELTIG",
        CONFUSION,
        "The word is spelled 'endgültig'.",
        Some("de"),
    ),
    rule(
        "DE_WIEDERUM",
        CONFUSION,
        "The word is spelled 'wiederum'.",
        Some("de"),
    ),
];

/// Regel zu einer Kennung.
pub fn rule_by_id(id: &str) -> Option<&'static Rule> {
    RULES.iter().find(|r| r.id == id)
}

/// Regeln, die für die Basissprache `lang` gelten.
pub fn rules_for(lang: &str) -> Vec<&'static Rule> {
    RULES
        .iter()
        .filter(|r| r.lang.is_none_or(|l| l == lang))
        .collect()
}

/// Eine Fundstelle der Grammatikprüfung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Regelkennung.
    pub rule_id: String,
    /// Kategorie.
    pub category: String,
    /// Beschreibung.
    pub description: String,
    /// Betroffener Text.
    pub text: String,
    /// Erste Zeichenposition im Bereich.
    pub start: usize,
    /// Zeichenposition hinter der Fundstelle.
    pub end: usize,
    /// Vorschläge (höchstens 3).
    pub suggestions: Vec<String>,
}

// --- Option b: externer Server --------------------------------------------------------------------------------

/// Ein externer, vom Benutzer betriebener Grammatik-Server (Option b, nicht gebündelt).
pub trait GrammarBackend: Send + Sync {
    /// Prüft `text` (normalisierter Bereich, Zeilenumbrüche = Blockgrenzen) in der Sprache `language` (`de-DE`).
    /// Positionen der Antwort zählen Unicode-Zeichen.
    fn check(&self, text: &str, language: &str) -> Result<Vec<Match>, String>;
}

fn backend() -> &'static RwLock<Option<Arc<dyn GrammarBackend>>> {
    static BACKEND: OnceLock<RwLock<Option<Arc<dyn GrammarBackend>>>> = OnceLock::new();
    BACKEND.get_or_init(|| RwLock::new(None))
}

/// Setzt oder entfernt (`None`) den externen Grammatik-Server für alle folgenden Prüfungen des Prozesses.
pub fn set_external_backend(server: Option<Arc<dyn GrammarBackend>>) {
    *backend().write().unwrap_or_else(|e| e.into_inner()) = server;
}

fn external() -> Option<Arc<dyn GrammarBackend>> {
    backend().read().unwrap_or_else(|e| e.into_inner()).clone()
}

// --- Prüfung ------------------------------------------------------------------------------------------------

/// Prüft einen Bereich in der Sprache `code` (`de-DE`) mit dem gesetzten externen Server bzw. der eigenen
/// Regelmenge; Regeln aus `disabled` entfallen. Sortiert nach Position.
pub fn check(text: &str, code: &str, disabled: &[String]) -> Vec<Match> {
    check_with(text, code, disabled, external().as_deref())
}

/// Wie [`check`] mit ausdrücklich gegebenem Server (`None` = eigene Regelmenge).
pub fn check_with(
    text: &str,
    code: &str,
    disabled: &[String],
    server: Option<&dyn GrammarBackend>,
) -> Vec<Match> {
    let off = |id: &str| disabled.iter().any(|d| d.trim() == id);
    if let Some(server) = server {
        if let Ok(found) = server.check(text, code) {
            let mut found: Vec<Match> = found.into_iter().filter(|m| !off(&m.rule_id)).collect();
            found.sort_by_key(|m| (m.start, m.end));
            return found;
        }
    }
    builtin(text, code, disabled)
}

/// Nur die eigene Regelmenge (Option a).
pub fn builtin(text: &str, code: &str, disabled: &[String]) -> Vec<Match> {
    let off = |id: &str| disabled.iter().any(|d| d.trim() == id);
    let lang = super::lang::base_of(code);
    let mut out = Vec::new();
    let tokens = tokenize(text);
    let chars: Vec<char> = text.chars().collect();
    let mut ctx = Ctx {
        lang,
        chars: &chars,
        tokens: &tokens,
        out: &mut out,
    };
    ctx.punctuation();
    ctx.brackets();
    ctx.words();
    out.retain(|m| {
        !off(&m.rule_id) && rule_by_id(&m.rule_id).is_some_and(|r| r.lang.is_none_or(|l| l == lang))
    });
    out.sort_by_key(|m| (m.start, m.end));
    out
}

struct Ctx<'a, 'b> {
    lang: &'a str,
    chars: &'a [char],
    tokens: &'a [Token<'b>],
    out: &'a mut Vec<Match>,
}

/// Erster Buchstabe groß, Rest unverändert.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// Übernimmt die Großschreibung des ersten Buchstabens von `original` in `replacement`.
fn keep_case(original: &str, replacement: &str) -> String {
    if original.chars().next().is_some_and(char::is_uppercase) {
        capitalize(replacement)
    } else {
        replacement.to_string()
    }
}

/// Abkürzungen, die keinen Satz beenden (klein geschrieben, ohne abschließenden Punkt).
const ABBREVIATIONS: &[&str] = &[
    "bzw", "ca", "usw", "etc", "vs", "dr", "nr", "mr", "mrs", "ms", "prof", "st", "evtl", "ggf",
    "inkl", "max", "min", "vgl", "z", "ff", "abs", "bsp", "jh", "jr", "sr", "approx", "dept",
    "fig", "mio", "mrd", "str", "tel", "u", "zzgl", "sog", "d", "h", "o", "s",
];

/// Endet mit `token` ein Satz im Sinne der Großschreibungsregel?
fn ends_sentence(token: &Token<'_>) -> bool {
    if !token.ends_sentence() {
        return false;
    }
    if !token
        .raw
        .trim_end_matches(['"', '\'', ')', ']', '»', '“', '”'])
        .ends_with('.')
    {
        return true;
    }
    let core = token.core;
    let lower = core.to_lowercase();
    let initial = core.chars().count() == 1 && core.chars().all(char::is_uppercase);
    let ordinal = !core.is_empty() && core.chars().all(|c| c.is_ascii_digit());
    let ellipsis = token.raw.ends_with("..") || token.raw.ends_with('…');
    !(core.contains('.')
        || initial
        || (ordinal && !ellipsis)
        || ABBREVIATIONS.contains(&lower.as_str()))
}

impl<'b> Ctx<'_, 'b> {
    fn push(&mut self, id: &str, start: usize, end: usize, suggestions: Vec<String>) {
        let Some(rule) = rule_by_id(id) else {
            return;
        };
        let text: String = self.chars[start..end].iter().collect();
        self.out.push(Match {
            rule_id: rule.id.to_string(),
            category: rule.category.to_string(),
            description: rule.description.to_string(),
            text,
            start,
            end,
            suggestions: suggestions
                .into_iter()
                .filter(|s| !s.is_empty())
                .take(3)
                .collect(),
        });
    }

    /// Vorheriges Token im selben Block.
    fn previous_in_block(&self, i: usize) -> Option<Token<'b>> {
        (i > 0 && !self.tokens[i].block_start).then(|| self.tokens[i - 1].clone())
    }

    /// `DOUBLE_PUNCT`, `SPACE_BEFORE_PUNCT`, `SPACE_AFTER_COMMA`, `PUNCT_SEQUENCE`.
    fn punctuation(&mut self) {
        const MARKS: &[char] = &['.', ',', '!', '?', ';', ':'];
        for i in 0..self.tokens.len() {
            let t = self.tokens[i].clone();
            if t.is_url_or_email() {
                continue;
            }
            let raw: Vec<char> = t.raw.chars().collect();
            // Genau zwei gleiche Zeichen (`...` ist eine Auslassung, `?!` gemischt).
            let mut k = 0;
            while k < raw.len() {
                let c = raw[k];
                let run = raw[k..].iter().take_while(|&&x| x == c).count();
                if MARKS.contains(&c) && run == 2 {
                    self.push(
                        "DOUBLE_PUNCT",
                        t.start + k,
                        t.start + k + 2,
                        vec![c.to_string()],
                    );
                }
                k += run;
            }
            // Zwei verschiedene Satzzeichen aus `.,;:` direkt hintereinander.
            for k in 1..raw.len() {
                let (a, b) = (raw[k - 1], raw[k]);
                let set = ['.', ',', ';', ':'];
                let in_ellipsis = a == '.' && k >= 2 && raw[k - 2] == '.';
                if set.contains(&a) && set.contains(&b) && a != b && !in_ellipsis {
                    let strongest = ['.', ';', ':', ',']
                        .into_iter()
                        .find(|c| *c == a || *c == b);
                    self.push(
                        "PUNCT_SEQUENCE",
                        t.start + k - 1,
                        t.start + k + 1,
                        strongest.map(|c| c.to_string()).into_iter().collect(),
                    );
                }
            }
            // Fehlendes Leerzeichen nach Komma oder Semikolon zwischen zwei Buchstaben.
            for k in 1..raw.len().saturating_sub(1) {
                if matches!(raw[k], ',' | ';')
                    && raw[k - 1].is_alphabetic()
                    && raw[k + 1].is_alphabetic()
                {
                    let (from, to) = word_around(&raw, k);
                    let left: String = raw[from..k + 1].iter().collect();
                    let right: String = raw[k + 1..to].iter().collect();
                    self.push(
                        "SPACE_AFTER_COMMA",
                        t.start + from,
                        t.start + to,
                        vec![format!("{left} {right}")],
                    );
                }
            }
            // Leerzeichen vor einem Satzzeichen (`Hallo , Welt`); Französisch setzt es vor `; : ! ?`.
            let only_marks = raw.iter().all(|c| MARKS.contains(c));
            let ellipsis = raw.len() >= 3 && raw.iter().all(|c| *c == '.');
            if only_marks && raw.len() <= 2 && !ellipsis {
                let french_ok = self.lang == "fr" && matches!(raw[0], ';' | ':' | '!' | '?');
                if let Some(prev) = self.previous_in_block(i) {
                    if !french_ok && !prev.raw.ends_with(['(', '[', '-']) {
                        let suggestion = format!("{}{}", prev.raw, t.raw);
                        let start = prev.start;
                        self.push("SPACE_BEFORE_PUNCT", start, t.end(), vec![suggestion]);
                    }
                }
            }
        }
    }

    /// `UNPAIRED_BRACKET`, `SPACE_INSIDE_BRACKETS` (je Block).
    fn brackets(&mut self) {
        let mut open: Vec<(char, usize)> = Vec::new();
        for i in 0..self.tokens.len() {
            let t = self.tokens[i].clone();
            if t.block_start {
                self.flush_unpaired(&mut open);
            }
            if t.is_url_or_email() {
                continue;
            }
            let raw: Vec<char> = t.raw.chars().collect();
            // Aufzählungszeichen `a)`, `1)`, `(a)` am Blockanfang und Emoticons zählen nicht.
            if t.block_start && raw.len() <= 3 && raw.last() == Some(&')') {
                continue;
            }
            for (k, c) in raw.iter().enumerate() {
                let emoticon = k > 0 && matches!(raw[k - 1], ':' | ';' | '-' | '=');
                match c {
                    '(' | '[' | '{' => open.push((*c, t.start + k)),
                    ')' | ']' | '}' if !emoticon => {
                        let wanted = match c {
                            ')' => '(',
                            ']' => '[',
                            _ => '{',
                        };
                        if open.last().is_some_and(|(o, _)| *o == wanted) {
                            open.pop();
                        } else {
                            self.push("UNPAIRED_BRACKET", t.start + k, t.start + k + 1, Vec::new());
                        }
                    }
                    _ => {}
                }
            }
            if matches!(t.raw, "(" | "[") {
                if let Some(next) = self.tokens.get(i + 1).filter(|n| !n.block_start) {
                    let start = t.start;
                    let end = next.end();
                    self.push(
                        "SPACE_INSIDE_BRACKETS",
                        start,
                        end,
                        vec![format!("{}{}", t.raw, next.raw)],
                    );
                }
            }
            if matches!(t.raw, ")" | "]" | ")." | ")," | "].") {
                if let Some(prev) = self.previous_in_block(i) {
                    let start = prev.start;
                    let end = t.end();
                    let suggestion = format!("{}{}", prev.raw, t.raw);
                    self.push("SPACE_INSIDE_BRACKETS", start, end, vec![suggestion]);
                }
            }
        }
        self.flush_unpaired(&mut open);
    }

    fn flush_unpaired(&mut self, open: &mut Vec<(char, usize)>) {
        for (_, at) in std::mem::take(open) {
            self.push("UNPAIRED_BRACKET", at, at + 1, Vec::new());
        }
    }

    /// Wort- und Phrasenregeln innerhalb von Sätzen.
    fn words(&mut self) {
        let mut sentence_start = true;
        let mut after_ellipsis = false;
        for i in 0..self.tokens.len() {
            let t = self.tokens[i].clone();
            if t.block_start {
                sentence_start = true;
                after_ellipsis = false;
            }
            if !t.core.is_empty() {
                if sentence_start && !after_ellipsis {
                    self.sentence_start(&t);
                }
                if sentence_start {
                    sentence_start = false;
                }
                self.repeat(i);
                self.phrases(i);
                if self.lang == "en" {
                    self.en_special(i);
                }
            }
            if ends_sentence(&t) {
                sentence_start = true;
                after_ellipsis = t.raw.ends_with("...") || t.raw.ends_with('…');
            }
        }
    }

    fn sentence_start(&mut self, t: &Token<'_>) {
        let core = t.core;
        let Some(first) = core.chars().next() else {
            return;
        };
        let has_upper = core.chars().any(char::is_uppercase);
        if first.is_lowercase() && !has_upper && !t.is_url_or_email() {
            let end = t.core_end();
            self.push(
                "SENTENCE_START_LOWER",
                t.core_start,
                end,
                vec![capitalize(core)],
            );
        }
    }

    fn repeat(&mut self, i: usize) {
        let Some(next) = self.tokens.get(i + 1) else {
            return;
        };
        let t = &self.tokens[i];
        if next.block_start || ends_sentence(t) || t.is_url_or_email() {
            return;
        }
        let (a, b) = (t.core.to_lowercase(), next.core.to_lowercase());
        if a.is_empty() || a != b || !a.chars().any(char::is_alphabetic) {
            return;
        }
        let allowed: &[&str] = match self.lang {
            "de" => &["der", "die", "das", "den", "dem"],
            "en" => &["had", "that"],
            _ => &[],
        };
        if allowed.contains(&a.as_str()) {
            return;
        }
        let (start, end, suggestion) = (t.core_start, next.core_end(), t.core.to_string());
        self.push("WORD_REPEAT", start, end, vec![suggestion]);
    }

    /// Kerne (klein) der Token ab `i` im selben Satz, höchstens `n`; vor dem letzten ohne Satzzeichen.
    fn window(&self, i: usize, n: usize) -> Option<Vec<String>> {
        let mut out = Vec::with_capacity(n);
        for k in 0..n {
            let t = self.tokens.get(i + k)?;
            if (k > 0 && t.block_start) || t.core.is_empty() || t.core_start != t.start {
                return None;
            }
            if k + 1 < n && t.has_trailing() {
                return None;
            }
            out.push(t.core.to_lowercase());
        }
        Some(out)
    }

    fn span(&self, i: usize, from: usize, to: usize) -> (usize, usize) {
        (
            self.tokens[i + from].core_start,
            self.tokens[i + to - 1].core_end(),
        )
    }

    fn phrases(&mut self, i: usize) {
        let lang = self.lang;
        for p in PHRASES.iter().filter(|p| p.lang == lang) {
            let Some(words) = self.window(i, p.seq.len()) else {
                continue;
            };
            if !p
                .seq
                .iter()
                .zip(&words)
                .all(|(alts, w)| alts.contains(&w.as_str()))
            {
                continue;
            }
            let originals: Vec<&str> = (0..p.seq.len()).map(|k| self.tokens[i + k].core).collect();
            let suggestion = (p.fix)(&originals[p.mark.0..p.mark.1]);
            let suggestion = keep_case(originals[p.mark.0], &suggestion);
            let (start, end) = self.span(i, p.mark.0, p.mark.1);
            self.push(p.rule, start, end, vec![suggestion]);
        }
    }

    /// `EN_A_AN`, `EN_I_LOWER`.
    fn en_special(&mut self, i: usize) {
        let t = self.tokens[i].clone();
        if t.core == "i" && t.raw.trim_end_matches([',', '.', '!', '?', ';', ':']) == "i" {
            self.push("EN_I_LOWER", t.core_start, t.core_end(), vec!["I".into()]);
        }
        let Some(words) = self.window(i, 2) else {
            return;
        };
        let article = words[0].as_str();
        if article != "a" && article != "an" {
            return;
        }
        let next = self.tokens[i + 1].core;
        let lower = next.to_lowercase();
        let acronym = next
            .chars()
            .filter(|c| c.is_alphabetic())
            .all(char::is_uppercase);
        if acronym || !lower.chars().next().is_some_and(char::is_alphabetic) {
            return;
        }
        const CONSONANT_SOUND: &[&str] = &[
            "uni", "use", "usu", "uti", "ura", "ure", "uro", "eu", "one", "once", "ubi", "uga",
            "uku", "ufo", "ewe",
        ];
        const VOWEL_SOUND: &[&str] = &["hour", "honest", "honor", "honour", "heir", "herb"];
        let vowel_letter = lower.starts_with(['a', 'e', 'i', 'o', 'u']);
        let wants_an = (vowel_letter && !CONSONANT_SOUND.iter().any(|p| lower.starts_with(p)))
            || VOWEL_SOUND.iter().any(|p| lower.starts_with(p));
        let wrong = (article == "a" && wants_an) || (article == "an" && !wants_an);
        if wrong {
            let fixed = keep_case(self.tokens[i].core, if wants_an { "an" } else { "a" });
            let (start, end) = self.span(i, 0, 2);
            self.push("EN_A_AN", start, end, vec![format!("{fixed} {next}")]);
        }
    }
}

/// Grenzen des Wortes um `k` (bis Leerraum bzw. Tokenrand), ohne Satzzeichen am Rand.
fn word_around(raw: &[char], k: usize) -> (usize, usize) {
    let mut from = k;
    while from > 0 && !super::tokens::is_punct(raw[from - 1]) {
        from -= 1;
    }
    let mut to = k + 1;
    while to < raw.len() && !super::tokens::is_punct(raw[to]) {
        to += 1;
    }
    (from, to)
}

/// Eine Phrasenregel: Folge von Alternativen (klein), markierter Bereich, Vorschlag aus den markierten Wörtern.
struct Phrase {
    rule: &'static str,
    lang: &'static str,
    seq: &'static [&'static [&'static str]],
    mark: (usize, usize),
    fix: fn(&[&str]) -> String,
}

fn replace_wieder(w: &[&str]) -> String {
    w[0].to_lowercase().replacen("wieder", "wider", 1)
}

fn drop_r(w: &[&str]) -> String {
    w[0].to_lowercase().replacen("vorr", "vor", 1)
}

const PHRASES: &[Phrase] = &[
    // Englisch
    Phrase {
        rule: "EN_COULD_OF",
        lang: "en",
        seq: &[&["could", "should", "would", "must"], &["of"]],
        mark: (0, 2),
        fix: |w| format!("{} have", w[0].to_lowercase()),
    },
    Phrase {
        rule: "EN_MIGHT_OF",
        lang: "en",
        seq: &[&["might"], &["of"]],
        mark: (0, 2),
        fix: |_| "might have".into(),
    },
    Phrase {
        rule: "EN_ITS_IT_IS",
        lang: "en",
        seq: &[
            &["its"],
            &[
                "a",
                "an",
                "the",
                "not",
                "been",
                "going",
                "very",
                "so",
                "too",
                "important",
                "possible",
                "likely",
                "true",
            ],
        ],
        mark: (0, 1),
        fix: |_| "it's".into(),
    },
    Phrase {
        rule: "EN_YOUR_YOURE",
        lang: "en",
        seq: &[&["your"], &["going", "not", "being", "coming", "trying"]],
        mark: (0, 1),
        fix: |_| "you're".into(),
    },
    Phrase {
        rule: "EN_THEN_THAN",
        lang: "en",
        seq: &[
            &[
                "more", "less", "better", "worse", "rather", "greater", "bigger", "smaller",
                "larger", "higher", "lower", "faster", "slower", "fewer", "older", "younger",
            ],
            &["then"],
        ],
        mark: (1, 2),
        fix: |_| "than".into(),
    },
    Phrase {
        rule: "EN_ALOT",
        lang: "en",
        seq: &[&["alot"]],
        mark: (0, 1),
        fix: |_| "a lot".into(),
    },
    Phrase {
        rule: "EN_IRREGARDLESS",
        lang: "en",
        seq: &[&["irregardless"]],
        mark: (0, 1),
        fix: |_| "regardless".into(),
    },
    Phrase {
        rule: "EN_INFACT",
        lang: "en",
        seq: &[&["infact"]],
        mark: (0, 1),
        fix: |_| "in fact".into(),
    },
    Phrase {
        rule: "EN_THEIR_THERE",
        lang: "en",
        seq: &[&["their"], &["is", "are", "was", "were"]],
        mark: (0, 1),
        fix: |_| "there".into(),
    },
    Phrase {
        rule: "EN_THEIR_THERE",
        lang: "en",
        seq: &[&["there"], &["own"]],
        mark: (0, 1),
        fix: |_| "their".into(),
    },
    Phrase {
        rule: "EN_WHOSE_WHOS",
        lang: "en",
        seq: &[&["whose"], &["going", "coming", "been"]],
        mark: (0, 1),
        fix: |_| "who's".into(),
    },
    Phrase {
        rule: "EN_SUPPOSED_TO",
        lang: "en",
        seq: &[
            &["am", "is", "are", "was", "were", "be", "been", "being"],
            &["suppose", "use"],
            &["to"],
        ],
        mark: (1, 2),
        fix: |w| format!("{}d", w[0].to_lowercase()),
    },
    Phrase {
        rule: "EN_TOO_TO",
        lang: "en",
        seq: &[
            &["is", "was", "are", "were", "be", "been", "far", "way"],
            &["to"],
            &[
                "late",
                "early",
                "much",
                "many",
                "big",
                "small",
                "long",
                "short",
                "expensive",
                "cheap",
                "hot",
                "cold",
                "difficult",
                "hard",
                "easy",
                "fast",
                "slow",
            ],
        ],
        mark: (1, 2),
        fix: |_| "too".into(),
    },
    Phrase {
        rule: "EN_AFFECT_EFFECT",
        lang: "en",
        seq: &[&["an", "positive", "negative", "side"], &["affect"]],
        mark: (1, 2),
        fix: |_| "effect".into(),
    },
    Phrase {
        rule: "EN_LOOSE_LOSE",
        lang: "en",
        seq: &[
            &[
                "to", "will", "would", "could", "can", "might", "may", "not", "never",
            ],
            &["loose"],
        ],
        mark: (1, 2),
        fix: |_| "lose".into(),
    },
    // Deutsch
    Phrase {
        rule: "DE_SEIT_SEID",
        lang: "de",
        seq: &[&["seid"], &["langem", "kurzem", "gestern", "heute"]],
        mark: (0, 2),
        fix: |w| format!("seit {}", w[1]),
    },
    Phrase {
        rule: "DE_SO_DASS",
        lang: "de",
        seq: &[
            &["so"],
            &["das"],
            &["ich", "du", "er", "sie", "es", "wir", "ihr", "man"],
        ],
        mark: (0, 2),
        fix: |_| "so dass".into(),
    },
    Phrase {
        rule: "DE_KOMMA_DASS",
        lang: "de",
        seq: &[
            &[
                "glaube", "denke", "hoffe", "weiß", "weiss", "finde", "meine", "glauben", "denken",
                "hoffen", "wissen", "finden", "meinen", "sicher", "klar", "wichtig", "schade",
                "sagt", "sagte", "zeigt", "bedeutet", "scheint",
            ],
            &["dass"],
        ],
        mark: (0, 2),
        fix: |w| format!("{}, dass", w[0]),
    },
    Phrase {
        rule: "DE_WIDER",
        lang: "de",
        seq: &[&[
            "wiederspiegeln",
            "wiederspiegelt",
            "wiederspiegelte",
            "wiederspruch",
            "wiedersprechen",
            "wiederspricht",
            "wiedersprach",
            "wiederstand",
            "wiederstehen",
            "wiedersteht",
            "erwiedern",
            "erwiedert",
            "erwiederte",
            "wiedersacher",
        ]],
        mark: (0, 1),
        fix: replace_wieder,
    },
    Phrase {
        rule: "DE_STANDARD",
        lang: "de",
        seq: &[&["standart", "standarts"]],
        mark: (0, 1),
        fix: |w| w[0].to_lowercase().replace("standart", "standard"),
    },
    Phrase {
        rule: "DE_ZUMINDEST",
        lang: "de",
        seq: &[&["zumindestens"]],
        mark: (0, 1),
        fix: |_| "zumindest".into(),
    },
    Phrase {
        rule: "DE_EINZIGSTE",
        lang: "de",
        seq: &[&[
            "einzigste",
            "einzigster",
            "einzigstes",
            "einzigsten",
            "einzigstem",
        ]],
        mark: (0, 1),
        fix: |w| w[0].to_lowercase().replacen("einzigst", "einzig", 1),
    },
    Phrase {
        rule: "DE_ALS_WIE",
        lang: "de",
        seq: &[
            &[
                "besser",
                "größer",
                "kleiner",
                "mehr",
                "weniger",
                "schneller",
                "langsamer",
                "länger",
                "kürzer",
                "höher",
                "niedriger",
                "älter",
                "jünger",
                "teurer",
                "billiger",
                "günstiger",
                "schöner",
                "stärker",
                "schwächer",
                "leichter",
                "schwerer",
            ],
            &["wie"],
        ],
        mark: (1, 2),
        fix: |_| "als".into(),
    },
    Phrase {
        rule: "DE_KEINSTER",
        lang: "de",
        seq: &[&["in"], &["keinster"], &["weise"]],
        mark: (1, 2),
        fix: |_| "keiner".into(),
    },
    Phrase {
        rule: "DE_DES_WEITEREN",
        lang: "de",
        seq: &[&["desweiteren"]],
        mark: (0, 1),
        fix: |_| "des Weiteren".into(),
    },
    Phrase {
        rule: "DE_VORAUS",
        lang: "de",
        seq: &[&[
            "vorraus",
            "vorraussetzung",
            "vorraussetzungen",
            "vorrausgesetzt",
            "vorraussichtlich",
        ]],
        mark: (0, 1),
        fix: drop_r,
    },
    Phrase {
        rule: "DE_AN_SICH",
        lang: "de",
        seq: &[&["ansich"]],
        mark: (0, 1),
        fix: |_| "an sich".into(),
    },
    Phrase {
        rule: "DE_GAR_NICHT",
        lang: "de",
        seq: &[&[
            "garnicht",
            "garnichts",
            "garkein",
            "garkeine",
            "garkeinen",
            "garkeiner",
        ]],
        mark: (0, 1),
        fix: |w| w[0].to_lowercase().replacen("gar", "gar ", 1),
    },
    Phrase {
        rule: "DE_IMMER_NOCH",
        lang: "de",
        seq: &[&["immernoch"]],
        mark: (0, 1),
        fix: |_| "immer noch".into(),
    },
    Phrase {
        rule: "DE_BISSCHEN",
        lang: "de",
        seq: &[&["bischen"]],
        mark: (0, 1),
        fix: |_| "bisschen".into(),
    },
    Phrase {
        rule: "DE_WEISST",
        lang: "de",
        seq: &[&["weist"], &["du"]],
        mark: (0, 1),
        fix: |_| "weißt".into(),
    },
    Phrase {
        rule: "DE_ENDGUELTIG",
        lang: "de",
        seq: &[&[
            "entgültig",
            "entgültige",
            "entgültigen",
            "entgültiger",
            "entgültiges",
        ]],
        mark: (0, 1),
        fix: |w| w[0].to_lowercase().replacen("entg", "endg", 1),
    },
    Phrase {
        rule: "DE_WIEDERUM",
        lang: "de",
        seq: &[&["wiederrum"]],
        mark: (0, 1),
        fix: |_| "wiederum".into(),
    },
];
