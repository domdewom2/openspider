//! XML-Sitemaps: `urlset` und `sitemapindex` (TECH-210).
//!
//! STUB(M1-P06)

use os_model::{SitemapDoc, SitemapKind};

/// Wertet eine Sitemap aus (gzip wird erkannt).
// STUB(M1-P06): `Invalid`
pub fn parse(_url: &url::Url, body: &[u8], _content_type: Option<&str>) -> SitemapDoc {
    SitemapDoc {
        kind: SitemapKind::Invalid,
        locs: Vec::new(),
        invalid_entries: 0,
        size_bytes: body.len() as u64,
    }
}
