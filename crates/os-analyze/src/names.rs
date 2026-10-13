//! Namensauflösung `Tab:Filter` (case-insensitiv, Vorschläge ab Levenshtein ≤ 3).
//!
//! STUB(M1-P14)

use crate::error::NameError;

/// Löst `"Tab:Filter"` in die kanonischen Namen `(tab, filter)` auf.
// STUB(M1-P14): lehnt jede Eingabe ab.
pub fn resolve(input: &str) -> Result<(String, String), NameError> {
    Err(NameError::Unknown {
        input: input.to_string(),
        suggestions: Vec::new(),
    })
}
