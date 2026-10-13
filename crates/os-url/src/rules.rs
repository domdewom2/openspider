//! Include/Exclude-Regeln (CRAWL-109/110): Vollübereinstimmung gegen die kodierte URL.

/// Fehler beim Übersetzen der Regeln.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// Ungültige Regex-Syntax.
    #[error("ungültiger regulärer Ausdruck in Zeile {line}: {message}")]
    Invalid {
        /// Zeile in der jeweiligen Liste (0-basiert).
        line: usize,
        /// Meldung des Regex-Übersetzers.
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

/// Übersetzte Include/Exclude-Regeln.
#[derive(Debug, Clone)]
pub struct RuleSet {
    include: Vec<String>,
    #[allow(dead_code)]
    exclude: Vec<String>,
    #[allow(dead_code)]
    partial: bool,
}

impl RuleSet {
    /// Übersetzt die Regeln (`partial` = Teilübereinstimmung statt Vollübereinstimmung).
    // STUB(M1-P01): P01 übersetzt mit fancy-regex (Schrittlimit 1 000 000).
    pub fn compile(
        include: &[String],
        exclude: &[String],
        partial: bool,
    ) -> Result<RuleSet, RuleError> {
        Ok(RuleSet {
            include: include.to_vec(),
            exclude: exclude.to_vec(),
            partial,
        })
    }

    /// Prüft eine kodierte URL (Exclude vor Include).
    // STUB(M1-P01): erlaubt alles.
    pub fn decide(&self, _url: &str) -> RuleDecision {
        RuleDecision::Allowed
    }

    /// Trefferzähler je Include-Zeile (Zeilentext, Treffer).
    // STUB(M1-P01)
    pub fn hits(&self) -> Vec<(String, u64)> {
        self.include.iter().map(|l| (l.clone(), 0)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rules_allow_everything() {
        let rules = RuleSet::compile(&[], &[], false).unwrap();
        assert_eq!(rules.decide("https://site.test/a"), RuleDecision::Allowed);
        assert!(rules.hits().is_empty());
    }
}
