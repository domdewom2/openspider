//! Exit-Codes (OUT-204) und Fehlertyp der Kommandozeile.

use std::fmt;

/// Rückgabewerte für Automatisierung (OUT-204).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Erfolg, auch mit Warnungen (außer mit `--strict`).
    Ok,
    /// Unerwarteter interner Fehler.
    Internal,
    /// Aufruf- oder Konfigurationsfehler, vor dem Crawl erkannt.
    Usage,
    /// Der Crawl konnte nicht durchgeführt werden (Crawl-Datei nicht schreibbar, Datenträger voll,
    /// mit `--fail-on-start-error` auch Start-URL nicht erreichbar).
    CrawlFailed,
    /// Der Crawl war erfolgreich, mindestens ein Export schlug fehl (mit `--strict` auch Warnungen).
    ExportFailed,
    /// Durch den Benutzer abgebrochen (SIGINT / Strg+C).
    Interrupted,
}

impl Exit {
    /// Numerischer Wert.
    pub fn code(self) -> i32 {
        match self {
            Exit::Ok => 0,
            Exit::Internal => 1,
            Exit::Usage => 2,
            Exit::CrawlFailed => 3,
            Exit::ExportFailed => 4,
            Exit::Interrupted => 130,
        }
    }
}

/// Fehler mit zugeordnetem Exit-Code. Die Nachricht darf mehrzeilig sein; die erste Zeile erscheint
/// in der Schlusszeile auf stderr.
#[derive(Debug, Clone)]
pub struct CliError {
    /// Exit-Code.
    pub exit: Exit,
    /// Meldung für den Benutzer.
    pub message: String,
}

impl CliError {
    /// Aufruf- oder Konfigurationsfehler (Exit 2).
    pub fn usage(message: impl Into<String>) -> Self {
        CliError {
            exit: Exit::Usage,
            message: message.into(),
        }
    }

    /// Interner Fehler (Exit 1).
    pub fn internal(message: impl Into<String>) -> Self {
        CliError {
            exit: Exit::Internal,
            message: message.into(),
        }
    }

    /// Crawl nicht durchführbar (Exit 3).
    pub fn crawl(message: impl Into<String>) -> Self {
        CliError {
            exit: Exit::CrawlFailed,
            message: message.into(),
        }
    }

    /// Erste Zeile der Meldung.
    pub fn headline(&self) -> &str {
        self.message.lines().next().unwrap_or("")
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: OUT-204
    #[test]
    fn exit_codes_have_the_documented_values() {
        let all = [
            (Exit::Ok, 0),
            (Exit::Internal, 1),
            (Exit::Usage, 2),
            (Exit::CrawlFailed, 3),
            (Exit::ExportFailed, 4),
            (Exit::Interrupted, 130),
        ];
        for (exit, code) in all {
            assert_eq!(exit.code(), code);
        }
    }
}
