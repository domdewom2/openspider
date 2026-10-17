//! Mock: Google Search Console (Search Analytics, URL-Inspektion; INTEG-115 ff.).
//!
//! STUB(M2-I03): leer; `mock()` meldet bis zur Umsetzung keinen Mock.

use std::sync::Arc;

use super::MockApi;

// STUB(M2-I03)
/// Der Mock für die Registrierung in `mocks::registry`; `None`, solange M2-I03 ihn nicht liefert.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    None
}
