//! Mock: Google OAuth (Autorisierung, Token, Widerruf; INTEG-003).
//!
//! STUB(M2-I01): leer; `mock()` meldet bis zur Umsetzung keinen Mock.

use std::sync::Arc;

use super::MockApi;

// STUB(M2-I01)
/// Der Mock für die Registrierung in `mocks::registry`; `None`, solange M2-I01 ihn nicht liefert.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    None
}
