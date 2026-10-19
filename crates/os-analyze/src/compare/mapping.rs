//! URL-Mapping zwischen den Crawls (OUT-146, führend; CRAWL-034 und TECH-306 verweisen hierher).
//!
//! Regeln `Suchmuster` → `Ersetzung` werden in Listenreihenfolge nacheinander auf das Ergebnis der vorigen angewendet.
//! Dialekt und Semantik sind die gemeinsamen aus den Konventionen von `01-crawl.md`: `fancy-regex` (Lookaround,
//! Rückverweise, Inline-Flags, nicht-gierige Quantoren) mit Schrittlimit, **Suche** statt Vollübereinstimmung – jeder
//! Treffer irgendwo in der URL wird ersetzt, `$1` bzw. `${name}` im Ersatztext setzt Gruppen ein. Überschreitet ein
//! Abgleich das Schrittlimit, bleibt die URL für diese Regel unverändert; die Regel wird einmal je Mapping als Warnung
//! gemeldet ([`UrlMapping::take_warnings`]).

use std::collections::BTreeSet;
use std::sync::Mutex;

use fancy_regex::{Regex, RegexBuilder};
use os_model::MappingRule;

/// Schrittlimit (Backtrack-Limit) je Abgleich, wie überall in OpenSpider (Q23).
pub const BACKTRACK_LIMIT: usize = 1_000_000;

/// Ungültige Regel: Index in der Liste, Position im Muster (Unicode-Zeichen ab 0, falls bekannt) und Meldung.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("ungültiger regulärer Ausdruck in Regel {rule}: {message}")]
pub struct MappingError {
    /// Index der Regel (0-basiert).
    pub rule: usize,
    /// Fehlerposition im Muster in Zeichen ab 0; `None`, wenn der Übersetzer keine Position nennt.
    pub position: Option<usize>,
    /// Meldung des Übersetzers.
    pub message: String,
}

/// Warnung einer Regel beim Abgleich (Schrittlimit überschritten).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingWarning {
    /// Index der Regel.
    pub rule: usize,
    /// Muster der Regel.
    pub pattern: String,
    /// Meldung.
    pub message: String,
}

/// Ergebnis der Abbildung einer URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapped {
    /// URL nach allen Regeln.
    pub url: String,
    /// Index der ersten Regel, die getroffen hat; `None` = keine Regel passt (URL unverändert identisch).
    pub first_rule: Option<usize>,
}

/// Wirkung einer Regel im Test-Reiter (OUT-146: URL nach jeder einzelnen Regel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingStep {
    /// Index der Regel.
    pub index: usize,
    /// Die Regel hat getroffen.
    pub matched: bool,
    /// URL vor der Regel.
    pub before: String,
    /// URL nach der Regel.
    pub after: String,
    /// Fehler (Schrittlimit überschritten).
    pub error: Option<String>,
}

/// Ergebnis des Test-Reiters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingTrace {
    /// Endergebnis.
    pub output: String,
    /// Je Regel ein Schritt.
    pub steps: Vec<MappingStep>,
}

struct Rule {
    pattern: String,
    replacement: String,
    regex: Regex,
}

/// Übersetzte Mapping-Regeln.
pub struct UrlMapping {
    rules: Vec<Rule>,
    warned: Mutex<BTreeSet<usize>>,
    warnings: Mutex<Vec<MappingWarning>>,
}

impl std::fmt::Debug for UrlMapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.rules.iter().map(|r| (&r.pattern, &r.replacement)))
            .finish()
    }
}

/// Byte-Position → Zeichenposition.
fn char_position(text: &str, byte: usize) -> usize {
    text.char_indices().take_while(|(i, _)| *i < byte).count()
}

/// Prüft ein einzelnes Muster (Dialog: Fehlermeldung mit Position, Speichern nicht möglich).
pub fn validate_pattern(pattern: &str) -> Result<(), MappingError> {
    compile_rule(0, pattern).map(|_| ())
}

fn compile_rule(index: usize, pattern: &str) -> Result<Regex, MappingError> {
    RegexBuilder::new(pattern)
        .backtrack_limit(BACKTRACK_LIMIT)
        .build()
        .map_err(|e| MappingError {
            rule: index,
            position: match &e {
                fancy_regex::Error::ParseError(pos, _) => Some(char_position(pattern, *pos)),
                _ => None,
            },
            message: e.to_string(),
        })
}

impl UrlMapping {
    /// Übersetzt die Regeln; die erste ungültige Regel bricht mit ihrer Position ab.
    pub fn compile(rules: &[MappingRule]) -> Result<UrlMapping, MappingError> {
        let rules = rules
            .iter()
            .enumerate()
            .map(|(i, r)| {
                Ok(Rule {
                    pattern: r.pattern.clone(),
                    replacement: r.replacement.clone(),
                    regex: compile_rule(i, &r.pattern)?,
                })
            })
            .collect::<Result<Vec<_>, MappingError>>()?;
        Ok(UrlMapping {
            rules,
            warned: Mutex::new(BTreeSet::new()),
            warnings: Mutex::new(Vec::new()),
        })
    }

    /// Keine Regeln (Paare nur über identische URLs).
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Anzahl der Regeln.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    fn warn(&self, index: usize, message: String) {
        let first = self
            .warned
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(index);
        if first {
            let rule = &self.rules[index];
            tracing::warn!(rule = index, pattern = %rule.pattern, "URL-Mapping: {message}");
            self.warnings
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(MappingWarning {
                    rule: index,
                    pattern: rule.pattern.clone(),
                    message,
                });
        }
    }

    /// Wendet eine Regel an: `(getroffen, Ergebnis, Fehler)`.
    fn apply_rule(&self, index: usize, input: &str) -> (bool, String, Option<String>) {
        let rule = &self.rules[index];
        let matched = match rule.regex.is_match(input) {
            Ok(m) => m,
            Err(e) => {
                let message = format!("Ausdruck zu aufwendig: {e}");
                self.warn(index, message.clone());
                return (false, input.to_string(), Some(message));
            }
        };
        if !matched {
            return (false, input.to_string(), None);
        }
        match rule
            .regex
            .try_replacen(input, 0, rule.replacement.as_str())
        {
            Ok(out) => (true, out.into_owned(), None),
            Err(e) => {
                let message = format!("Ausdruck zu aufwendig: {e}");
                self.warn(index, message.clone());
                (false, input.to_string(), Some(message))
            }
        }
    }

    /// Bildet eine URL des Previous-Crawls ab.
    pub fn apply(&self, url: &str) -> Mapped {
        let mut current = url.to_string();
        let mut first_rule = None;
        for index in 0..self.rules.len() {
            let (matched, out, _) = self.apply_rule(index, &current);
            if matched && first_rule.is_none() {
                first_rule = Some(index);
            }
            current = out;
        }
        Mapped {
            url: current,
            first_rule,
        }
    }

    /// Test-Reiter: URL nach jeder Regel und Endergebnis.
    pub fn trace(&self, input: &str) -> MappingTrace {
        let mut output = input.to_string();
        let mut steps = Vec::with_capacity(self.rules.len());
        for index in 0..self.rules.len() {
            let (matched, after, error) = self.apply_rule(index, &output);
            steps.push(MappingStep {
                index,
                matched,
                before: output.clone(),
                after: after.clone(),
                error,
            });
            output = after;
        }
        MappingTrace { output, steps }
    }

    /// Gesammelte Warnungen (je Regel höchstens eine) und leert die Liste.
    pub fn take_warnings(&self) -> Vec<MappingWarning> {
        std::mem::take(&mut *self.warnings.lock().unwrap_or_else(|p| p.into_inner()))
    }
}
