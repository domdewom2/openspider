//! Sitemap-Ingest (TECH-210): Index-Rekursion, Zyklen, gzip.
//!
//! STUB(M1-P15b): IDs (`SitemapId`/`UrlId`), `SitemapWrite` und Queue-Einträge bildet P15 in `crawl.rs`.

use os_fetch::{Fetch, FetchResult};
use os_model::SitemapDoc;

/// Ausgangspunkt für den Abruf einer Sitemap.
#[derive(Debug, Clone)]
pub struct SitemapSeed {
    /// URL der Sitemap.
    pub url: url::Url,
    /// Herkunft: `"start"`, `"list"`, `"config"` oder `"robots"`.
    pub found_via: String,
}

/// Eine abgerufene Sitemap.
#[derive(Debug, Clone)]
pub struct FetchedSitemap {
    /// URL der Sitemap.
    pub url: String,
    /// Index der übergeordneten Sitemap in der Ergebnisliste.
    pub parent: Option<usize>,
    /// Tiefe in der Index-Hierarchie.
    pub depth: u32,
    /// Herkunft wie bei `SitemapSeed`.
    pub found_via: String,
    /// Abrufergebnis.
    pub fetch: FetchResult,
    /// Ausgewertete Datei, falls die Antwort auswertbar war.
    pub doc: Option<SitemapDoc>,
}

/// Ruft die Sitemaps samt Unter-Sitemaps ab (Index-Tiefe höchstens `max_index_depth`, M1: 3).
// STUB(M1-P15b): leere Liste
pub async fn ingest<F: Fetch + ?Sized>(
    _fetcher: &F,
    _seeds: Vec<SitemapSeed>,
    _max_index_depth: u32,
) -> Vec<FetchedSitemap> {
    Vec::new()
}
