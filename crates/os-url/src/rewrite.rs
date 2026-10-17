//! URL-Umschreibung bei Entdeckung (CRAWL-103/104).
use std::borrow::Cow;
pub struct Rewritten<'a> {
    pub url: Cow<'a, str>,
    pub warning: Option<String>,
}
// STUB(M2-E04)
pub fn apply_discovered<'a>(
    _cfg: &os_model::RewriteConfig,
    raw_resolved: &'a str,
) -> Rewritten<'a> {
    Rewritten {
        url: Cow::Borrowed(raw_resolved),
        warning: None,
    }
}
