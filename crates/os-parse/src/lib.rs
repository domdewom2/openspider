//! `os-parse` – HTML-Extraktion: Links, Meta, Headings, Canonicals, hreflang, Bilder, Directives,
//! Sitemaps, PDF-Metadaten (HTML-Baum je Seite, Architekturgrundsatz 3).
//!
//! Die Modulsignaturen sind festgeschrieben (M1-Plan 3.6); `parse_html` ruft sie in der verbindlichen
//! Reihenfolge auf. Implementierungen: P04 (`charset`, `base`, `links`, `images`, `css`), P05 (`elements`,
//! `content`, `pixel`, `pdf`), P06 (`directives`, `canonical`, `hreflang`, `refresh`, `pagination`,
//! `headers`, `effective`, `sitemap`).

pub mod base;
pub mod canonical;
pub mod charset;
pub mod content;
pub mod css;
pub mod directives;
pub mod dom;
pub mod effective;
pub mod elements;
pub mod headers;
pub mod hreflang;
pub mod images;
pub mod links;
pub mod pagination;
pub mod pdf;
pub mod pixel;
pub mod refresh;
pub mod sitemap;
pub mod text;

use os_model::{
    ContentAreaConfig, CrawlConfig, Directive, DirectiveSource, ExtractedLink, ExtractionConfig,
    HeaderHints, HreflangOverLimit, ParsedPage, PdfMeta, SitemapDoc,
};

/// Einstellungen der Auswertung einer Seite.
#[derive(Debug, Clone)]
pub struct ParseConfig {
    /// robots.txt-Token des Crawlers (für `<meta name=…>` und `X-Robots-Tag`).
    pub robots_token: String,
    /// Inhaltsbereich für Wortzahl und Textanteil (ONPAGE-140).
    pub content_area: ContentAreaConfig,
    /// Welche Elemente erfasst werden.
    pub extraction: ExtractionConfig,
    /// Umgang mit mehr als 500 hreflang-Annotationen.
    pub hreflang_over_limit: HreflangOverLimit,
    /// Skript-Subtag im hreflang-Code zulassen.
    pub allow_script_subtag: bool,
    /// Pixelbreiten berechnen (UI-038).
    pub pixel: bool,
}

impl ParseConfig {
    /// Leitet die Parse-Konfiguration aus der Crawl-Konfiguration ab (`pixel` aus).
    pub fn from_crawl_config(cfg: &CrawlConfig) -> Self {
        ParseConfig {
            robots_token: cfg
                .user_agent
                .robots_token
                .clone()
                .unwrap_or_else(|| cfg.user_agent.preset.clone()),
            content_area: cfg.content.area.clone(),
            extraction: cfg.extraction.clone(),
            hreflang_over_limit: cfg.hreflang.over_limit,
            allow_script_subtag: cfg.hreflang.allow_script_subtag,
            pixel: false,
        }
    }
}

/// Eingabe der Auswertung einer HTML-Antwort.
pub struct ParseInput<'a> {
    /// URL der Antwort.
    pub url: &'a url::Url,
    /// Body (dekomprimiert).
    pub body: &'a [u8],
    /// `Content-Type` der Antwort.
    pub content_type: Option<&'a str>,
    /// Antwort-Header.
    pub headers: &'a [(String, String)],
    /// Einstellungen.
    pub cfg: &'a ParseConfig,
}

/// Wertet eine HTML-Antwort aus. Reihenfolge verbindlich (M1-Plan 3.6):
/// 1 `charset::decode` → 2 `dom::Document::parse` → 3 `base::resolve_base` →
/// 4 `links::extract`, `images::extract`, `css::extract_inline` →
/// 5 `elements::extract`, `content::analyze`, `pixel::annotate` (Titel 20 px, Meta-Descriptions 14 px, Regular;
/// nur bei `cfg.pixel`) → 6 `directives::extract` (Token `[robots_token, "googlebot"]`, K-12),
/// `canonical::extract`, `hreflang::extract`, `refresh::extract`, `pagination::extract` →
/// 7 `headers::merge(headers::hints(…))` → 8 `effective::compute`.
pub fn parse_html(input: &ParseInput) -> ParsedPage {
    let cfg = input.cfg;

    let (html, charset) = charset::decode(input.body, input.content_type, input.headers);
    let doc = dom::Document::parse(&html);
    let base = base::resolve_base(&doc, input.url);

    let mut links = links::extract(&doc, &base, cfg);
    let images = images::extract(&doc, &base);
    links.extend(css::extract_inline(&doc, &base));

    let elements::ElementSets {
        mut titles,
        mut meta_descriptions,
        meta_keywords,
        h1,
        h2,
    } = elements::extract(&doc);
    let stats = content::analyze(&doc, &cfg.content_area);
    if cfg.pixel {
        pixel::annotate(&mut titles, 20.0, false);
        pixel::annotate(&mut meta_descriptions, 14.0, false);
    }

    let tokens = [cfg.robots_token.clone(), "googlebot".to_string()];
    let directives = directives::extract(&doc, &tokens);
    let canonicals = canonical::extract(&doc, &base);
    let (hreflang, hreflang_truncated) = hreflang::extract(&doc, &base, cfg);
    let refresh = refresh::extract(&doc, &base);
    let pagination = pagination::extract(&doc, &base);

    let mut page = ParsedPage {
        base_url: base.to_string(),
        charset,
        titles,
        meta_descriptions,
        meta_keywords,
        h1,
        h2,
        links,
        images,
        canonicals,
        directives,
        refresh,
        hreflang,
        hreflang_truncated,
        pagination,
        word_count: Some(stats.word_count),
        text_ratio: Some(stats.text_ratio),
        effective_index: true,
        effective_follow: true,
    };

    headers::merge(&mut page, headers::hints(input.url, input.headers, cfg));

    let by_source = |source: DirectiveSource| -> Vec<Directive> {
        page.directives
            .iter()
            .filter(|d| d.source == source)
            .cloned()
            .collect()
    };
    let html_directives = by_source(DirectiveSource::MetaRobots);
    let header_directives = by_source(DirectiveSource::XRobotsTag);
    (page.effective_index, page.effective_follow) =
        effective::compute(&html_directives, &header_directives);
    page
}

/// Wertet nur die Header einer Antwort aus (auch für Nicht-HTML).
pub fn parse_headers(
    url: &url::Url,
    headers: &[(String, String)],
    cfg: &ParseConfig,
) -> HeaderHints {
    headers::hints(url, headers, cfg)
}

/// Extrahiert Verweise aus einer CSS-Datei.
pub fn parse_css(url: &url::Url, body: &[u8]) -> Vec<ExtractedLink> {
    css::extract_sheet(url, body)
}

/// Wertet eine Sitemap-Datei aus (gzip wird erkannt).
pub fn parse_sitemap(url: &url::Url, body: &[u8], content_type: Option<&str>) -> SitemapDoc {
    sitemap::parse(url, body, content_type)
}

/// Liest Titel und Schlüsselwörter aus den Dokumenteigenschaften einer PDF-Datei.
pub fn parse_pdf_meta(body: &[u8]) -> PdfMeta {
    pdf::meta(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entry_points_smoke() {
        let url = url::Url::parse("https://site.test/a").unwrap();
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        let input = ParseInput {
            url: &url,
            body: b"<html><head><title>T</title></head><body><a href=\"/b\">b</a></body></html>",
            content_type: Some("text/html"),
            headers: &[],
            cfg: &cfg,
        };
        let page = parse_html(&input);
        assert_eq!(page.base_url, "https://site.test/a");
        assert!(page.effective_index && page.effective_follow);
        assert_eq!(page.word_count, Some(1));
        assert!(parse_headers(&url, &[], &cfg).canonicals.is_empty());
        assert!(parse_css(&url, b"a{}").is_empty());
        assert_eq!(parse_pdf_meta(b"%PDF"), PdfMeta::default());
        assert_eq!(
            parse_sitemap(&url, b"<urlset/>", None).kind,
            os_model::SitemapKind::Urlset
        );
        assert_eq!(
            parse_sitemap(&url, b"<html/>", None).kind,
            os_model::SitemapKind::Invalid
        );
    }

    #[test]
    fn robots_token_defaults_to_preset() {
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        assert_eq!(cfg.robots_token, "openspider");
        assert!(!cfg.pixel);
    }
}
