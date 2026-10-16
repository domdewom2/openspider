//! Beenden und Schließen mit laufendem Crawl (F-11).
//!
//! STUB(M2-K04): `QuitDecision` ist nur ein Platzhalter, M2-K04 setzt die Rückfrage-Logik um.

/// Entscheidung beim Beenden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitDecision {
    /// Sofort beenden.
    Quit,
}
