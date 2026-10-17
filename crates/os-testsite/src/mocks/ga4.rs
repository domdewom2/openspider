//! Mock: Google Analytics 4 Data API (INTEG-097 ff.).
//!
//! STUB(M2-I02): leer; `mock()` meldet bis zur Umsetzung keinen Mock.

use std::sync::Arc;

use super::MockApi;

// STUB(M2-I02)
/// Der Mock für die Registrierung in `mocks::registry`; `None`, solange M2-I02 ihn nicht liefert.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    None
}
