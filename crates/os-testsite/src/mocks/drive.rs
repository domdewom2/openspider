//! Mock: Google Drive (Dateien und Ordner mit Scope drive.file; OUT-210 ff.).
//!
//! STUB(M2-X03): leer; `mock()` meldet bis zur Umsetzung keinen Mock.

use std::sync::Arc;

use super::MockApi;

// STUB(M2-X03)
/// Der Mock für die Registrierung in `mocks::registry`; `None`, solange M2-X03 ihn nicht liefert.
pub fn mock() -> Option<Arc<dyn MockApi>> {
    None
}
