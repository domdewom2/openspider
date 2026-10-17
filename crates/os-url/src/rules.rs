//! Include/Exclude-Regeln (CRAWL-109/110): Vollübereinstimmung gegen die kodierte URL.
//!
//! Dialekt: `fancy-regex` mit Schrittlimit (Backtrack-Limit 1 000 000 je Abgleich). Wird das Limit
//! überschritten, gilt der Abgleich als „kein Treffer“, und die Regel meldet einmalig eine Warnung
//! („Ausdruck zu aufwendig“, siehe [`RuleSet::take_warnings`]). Standardmäßig wird die gesamte URL
//! verlangt (`^(?:…)$`); mit `partial` genügt ein Treffer irgendwo in der URL.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use fancy_regex::{Regex, RegexBuilder};

/// Schrittlimit (Backtrack-Limit) je Abgleich.
const BACKTRACK_LIMIT: usize = 1_000_000;

/// Fehler beim Übersetzen der Regeln.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// Ungültige Regex-Syntax.
    #[error("ungültiger regulärer Ausdruck in Zeile {line}: {message}")]
    Invalid {
        /// Zeile in der jeweiligen Liste (0-basiert).
        line: usize,
        /// Meldung des Regex-Übersetzers, mit `Include: ` bzw. `Exclude: ` als Präfix.
        message: String,
    },
}

/// Ergebnis der Prüfung einer URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleDecision {
    /// Erlaubt.
    Allowed,
    /// Durch Exclude ausgeschlossen; Index (0-basiert) der treffenden Exclude-Zeile.
    Excluded(usize),
    /// Von keiner Include-Regel erfasst.
    NotIncluded,
}

/// Liste, zu der eine Regel gehört.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleKind {
    /// Include-Liste (CRAWL-109).
    Include,
    /// Exclude-Liste (CRAWL-110).
    Exclude,
}

/// Warnung zu einer Regel, deren Abgleich das Schrittlimit überschritten hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleWarning {
    /// Liste der Regel.
    pub kind: RuleKind,
    /// Zeile (0-basiert) in der Liste.
    pub line: usize,
    /// Text der Regel.
    pub pattern: String,
    /// Meldung („Ausdruck zu aufwendig“).
    pub message: String,
}

/// Trefferzähler einer Include-Zeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeHit {
    /// Zeile (0-basiert) in der Include-Liste.
    pub line: usize,
    /// Text der Zeile.
    pub pattern: String,
    /// Anzahl der URLs, auf die die Zeile gepasst hat.
    pub hits: u64,
}

#[derive(Debug)]
struct Rule {
    line: usize,
    text: String,
    regex: Regex,
    hits: AtomicU64,
    warned: AtomicBool,
}

impl Clone for Rule {
    fn clone(&self) -> Self {
        Rule {
            line: self.line,
            text: self.text.clone(),
            regex: self.regex.clone(),
            hits: AtomicU64::new(self.hits.load(Ordering::Relaxed)),
            warned: AtomicBool::new(self.warned.load(Ordering::Relaxed)),
        }
    }
}

/// Übersetzte Include/Exclude-Regeln. Von mehreren Threads gemeinsam nutzbar.
#[derive(Debug)]
pub struct RuleSet {
    include: Vec<Rule>,
    exclude: Vec<Rule>,
    warnings: Mutex<Vec<RuleWarning>>,
}

impl Clone for RuleSet {
    fn clone(&self) -> Self {
        RuleSet {
            include: self.include.clone(),
            exclude: self.exclude.clone(),
            warnings: Mutex::new(self.lock_warnings().clone()),
        }
    }
}

fn compile_list(lines: &[String], partial: bool, label: &str) -> Result<Vec<Rule>, RuleError> {
    let mut rules = Vec::new();
    for (line, raw) in lines.iter().enumerate() {
        let text = raw.trim_end_matches(['\r', '\n']);
        if text.trim().is_empty() {
            continue;
        }
        let fail = |e: fancy_regex::Error| RuleError::Invalid {
            line,
            message: format!("{label}: {e}"),
        };
        let build = |pattern: &str| {
            RegexBuilder::new(pattern)
                .backtrack_limit(BACKTRACK_LIMIT)
                .build()
        };
        // Das Muster zuerst allein prüfen, damit unausgewogene Klammern nicht durch die
        // Umhüllung `^(?:…)$` „repariert“ werden.
        let bare = build(text).map_err(fail)?;
        let regex = if partial {
            bare
        } else {
            build(&format!("^(?:{text})$")).map_err(fail)?
        };
        rules.push(Rule {
            line,
            text: text.to_string(),
            regex,
            hits: AtomicU64::new(0),
            warned: AtomicBool::new(false),
        });
    }
    Ok(rules)
}

impl RuleSet {
    /// Übersetzt die Regeln (`partial` = Teilübereinstimmung statt Vollübereinstimmung).
    ///
    /// Leere Zeilen werden ignoriert; Zeilennummern in Ergebnissen und Fehlern zählen in den
    /// übergebenen Listen (leere Zeilen eingeschlossen). Groß-/Kleinschreibung wird beachtet
    /// (`(?i)` schaltet sie ab).
    pub fn compile(
        include: &[String],
        exclude: &[String],
        partial: bool,
    ) -> Result<RuleSet, RuleError> {
        Ok(RuleSet {
            include: compile_list(include, partial, "Include")?,
            exclude: compile_list(exclude, partial, "Exclude")?,
            warnings: Mutex::new(Vec::new()),
        })
    }

    /// Prüft eine kodierte URL (Exclude vor Include).
    ///
    /// Passt ein Exclude, ist die URL ausgeschlossen (Index der ersten treffenden Zeile) und die
    /// Include-Liste wird nicht ausgewertet. Sonst gilt: ohne Include-Regeln erlaubt; mit
    /// Include-Regeln nur bei mindestens einem Treffer. Jede passende Include-Zeile zählt einen
    /// Treffer. Die Start-URL ist von beiden Listen ausgenommen; das entscheidet die Engine.
    pub fn decide(&self, url: &str) -> RuleDecision {
        for rule in &self.exclude {
            if self.matches(rule, RuleKind::Exclude, url) {
                rule.hits.fetch_add(1, Ordering::Relaxed);
                return RuleDecision::Excluded(rule.line);
            }
        }
        if self.include.is_empty() {
            return RuleDecision::Allowed;
        }
        let mut included = false;
        for rule in &self.include {
            if self.matches(rule, RuleKind::Include, url) {
                rule.hits.fetch_add(1, Ordering::Relaxed);
                included = true;
            }
        }
        if included {
            RuleDecision::Allowed
        } else {
            RuleDecision::NotIncluded
        }
    }

    /// Trefferzähler je Include-Zeile (Zeilentext, Treffer), in Listenreihenfolge ohne leere Zeilen.
    pub fn hits(&self) -> Vec<(String, u64)> {
        self.include
            .iter()
            .map(|r| (r.text.clone(), r.hits.load(Ordering::Relaxed)))
            .collect()
    }

    /// Wie [`RuleSet::hits`], aber mit Zeilennummer (für „Include-Zeile n ohne Treffer“).
    pub fn include_hits(&self) -> Vec<IncludeHit> {
        self.include
            .iter()
            .map(|r| IncludeHit {
                line: r.line,
                pattern: r.text.clone(),
                hits: r.hits.load(Ordering::Relaxed),
            })
            .collect()
    }

    /// Anzahl der Treffer je Exclude-Zeile (Zeilennummer, Treffer).
    pub fn exclude_hits(&self) -> Vec<(usize, u64)> {
        self.exclude
            .iter()
            .map(|r| (r.line, r.hits.load(Ordering::Relaxed)))
            .collect()
    }

    /// Gibt die bisher aufgelaufenen Warnungen (Schrittlimit überschritten) zurück und leert sie.
    /// Jede Regel warnt höchstens einmal.
    pub fn take_warnings(&self) -> Vec<RuleWarning> {
        std::mem::take(&mut *self.lock_warnings())
    }

    fn lock_warnings(&self) -> std::sync::MutexGuard<'_, Vec<RuleWarning>> {
        self.warnings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn matches(&self, rule: &Rule, kind: RuleKind, url: &str) -> bool {
        match rule.regex.is_match(url) {
            Ok(hit) => hit,
            Err(_) => {
                if !rule.warned.swap(true, Ordering::Relaxed) {
                    self.lock_warnings().push(RuleWarning {
                        kind,
                        line: rule.line,
                        pattern: rule.text.clone(),
                        message: "Ausdruck zu aufwendig".to_string(),
                    });
                }
                false
            }
        }
    }
}

/// Regeln für den Konfigurations-Test (CRAWL-109/110).
#[derive(Debug, Clone, Default)]
pub struct RuleTraceInput {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub partial_match: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleTrace {
    pub matched: Vec<String>,
    pub warnings: Vec<String>,
}
// STUB(M2-E04)
pub fn trace(_rules: &RuleTraceInput, _url: &str) -> RuleTrace {
    RuleTrace::default()
}
// STUB(M2-E04)
pub fn validate(_cfg: &os_model::CrawlConfig) -> Vec<os_model::ConfigIssue> {
    Vec::new()
}
