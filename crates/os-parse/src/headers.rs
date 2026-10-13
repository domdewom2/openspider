//! Hinweise aus HTTP-Headern (`Link`, `X-Robots-Tag`, `Refresh`) und ihre Zusammenführung mit der Seite.
//!
//! STUB(M1-P06)

use os_model::{HeaderHints, ParsedPage};

use crate::ParseConfig;

/// Wertet die Header einer Antwort aus (auch Nicht-HTML).
// STUB(M1-P06)
pub fn hints(_url: &url::Url, _headers: &[(String, String)], _cfg: &ParseConfig) -> HeaderHints {
    HeaderHints::default()
}

/// Fügt die Header-Hinweise den Angaben der Seite hinzu (Header nach HTML).
// STUB(M1-P06)
pub fn merge(page: &mut ParsedPage, hints: HeaderHints) {
    page.canonicals.extend(hints.canonicals);
    page.directives.extend(hints.directives);
    page.refresh.extend(hints.refresh);
    page.hreflang.extend(hints.hreflang);
    page.pagination.extend(hints.pagination);
}
