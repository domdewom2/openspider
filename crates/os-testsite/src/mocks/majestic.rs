//! Mock: Majestic (Linkmetriken, INTEG-150 ff.).
//!
//! STUB(M2-I05): leer; `mock()` meldet bis zur Umsetzung keinen Mock.

use std::sync::Arc;

use super::MockApi;

// STUB(M2-I05)
/// Der Mock für die Registrierung in `mocks::registry`; `None`, solange M2-I05 ihn nicht liefert.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    None
}
