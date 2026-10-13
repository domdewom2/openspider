//! Basis-URL des Dokuments (`<base href>`).
//!
//! STUB(M1-P04)

use crate::dom::Document;

/// Löst die Basis-URL auf.
// STUB(M1-P04): Stub liefert die Dokument-URL.
pub fn resolve_base(_doc: &Document, url: &url::Url) -> url::Url {
    url.clone()
}
