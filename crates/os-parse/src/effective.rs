//! Wirksame Indexierung/Folgen aus HTML- und Header-Direktiven (TECH-066).
//!
//! STUB(M1-P06)

use os_model::Directive;

/// Berechnet `(effective_index, effective_follow)`; die restriktivste Angabe gewinnt.
// STUB(M1-P06): (true, true)
pub fn compute(_html: &[Directive], _header: &[Directive]) -> (bool, bool) {
    (true, true)
}
