//! `os-parse` – HTML-Extraktion: Links, Meta, Headings, Canonicals, hreflang, Bilder, Directives,
//! Sitemaps, PDF-Metadaten (HTML-Baum je Seite, Architekturgrundsatz 3).
//!
//! Die Modulsignaturen sind festgeschrieben (M1-Plan 3.6); `parse_html` ruft sie in der verbindlichen
//! Reihenfolge auf. Implementierungen: P04 (`charset`, `base`, `links`, `images`, `css`), P05 (`elements`,
//! `content`, `pixel`, `pdf`), P06 (`directives`, `canonical`, `hreflang`, `refresh`, `pagination`,
//! `headers`, `effective`, `sitemap`).

pub mod alternates;
pub mod base;
pub mod bookmarks;
pub mod canonical;
pub mod charset;
pub mod content;
pub mod crawlability;
pub mod css;
pub mod custom_extract;
pub mod custom_search;
pub mod directives;
pub mod dom;
pub mod effective;
pub mod elements;
pub mod headers;
pub mod hreflang;
pub mod images;
pub mod link_position;
pub mod links;
pub mod pagination;
pub mod pdf;
pub mod pixel;
pub mod readability;
pub mod refresh;
pub mod sd;
pub mod security;
pub mod sitemap;
pub mod spelling;
pub mod switches;
pub mod text;
pub mod validation;

use os_model::{
    ContentAreaConfig, CrawlConfig, CustomExtractionConfig, CustomSearchConfig, Directive,
    DirectiveSource, ExtractedLink, ExtractionConfig, GrammarConfig, HeaderHints,
    HreflangOverLimit, LinkPositionsConfig, ParsedPage, PdfMeta, SitemapDoc, SpellingConfig,
    StructuredDataConfig,
};

/// Einstellungen der Auswertung einer Seite.
#[derive(Debug, Clone)]
pub struct ParseConfig {
    pub crawl: CrawlConfig,
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
    /// HTML-Validierung auf Token-Ebene (`advanced.html_validation`, TECH-008; ab M2).
    pub html_validation: bool,
    /// Lesbarkeit berechnen (`extraction.readability`, ONPAGE-149; ab M2).
    pub readability: bool,
    /// Lesbarkeitsformeln für weitere Sprachen als Englisch (`content.readability.language_aware`; ab M2).
    pub readability_language_aware: bool,
    /// MinHash-Signatur für Nahezu-Duplikate berechnen (`duplicates.near_enabled`, ONPAGE-144; ab M2).
    pub minhash: bool,
    /// Strukturierte Daten: Formate und Groß-/Kleinschreibung (`extraction.structured_data`, ab M2).
    pub structured_data: StructuredDataConfig,
    /// Rechtschreibprüfung (`spelling`, ONPAGE-153; ab M2).
    pub spelling: SpellingConfig,
    /// Grammatikprüfung (`grammar`, ONPAGE-154; ab M2).
    pub grammar: GrammarConfig,
    /// Custom-Search-Filter (`custom_search.filters`, INTEG-020 ff.; ab M2).
    pub custom_search: CustomSearchConfig,
    /// Custom-Extraction-Extraktoren (`custom_extraction.extractors`, INTEG-040 ff.; ab M2).
    pub custom_extraction: CustomExtractionConfig,
    /// Regeln der Linkpositionen (`link_positions.rules`, INTEG-080 ff.; ab M2).
    pub link_positions: LinkPositionsConfig,
}

impl ParseConfig {
    /// Leitet die Parse-Konfiguration aus der Crawl-Konfiguration ab (`pixel` aus).
    pub fn from_crawl_config(cfg: &CrawlConfig) -> Self {
        ParseConfig {
            crawl: cfg.clone(),
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
            html_validation: cfg.advanced.html_validation,
            readability: cfg.extraction.readability,
            readability_language_aware: cfg.content.readability.language_aware,
            minhash: cfg.duplicates.near_enabled,
            structured_data: cfg.extraction.structured_data,
            spelling: cfg.spelling.clone(),
            grammar: cfg.grammar.clone(),
            custom_search: cfg.custom_search.clone(),
            custom_extraction: cfg.custom_extraction.clone(),
            link_positions: cfg.link_positions.clone(),
        }
    }

    /// Ist mindestens ein Format der strukturierten Daten eingeschaltet?
    pub fn structured_data_enabled(&self) -> bool {
        let sd = &self.structured_data;
        sd.json_ld || sd.microdata || sd.rdfa
    }
}

/// Eingabe der Auswertungsmodule ab M2 (`security`, `validation`, `readability`, `custom_search`, `custom_extract`,
/// `sd`, `spelling`): alles, was `parse_html` nach dem Aufbau des Baums kennt. Die Module lesen daraus und tragen ihre
/// Ergebnisse in die `ParsedPage` ein (`ext`, `forms`, `validation`, `structured_data`, `custom_search`,
/// `custom_extraction`, `minhash`).
pub struct PageCtx<'a> {
    /// URL der Antwort.
    pub url: &'a url::Url,
    /// Basis-URL nach `<base href>`.
    pub base: &'a url::Url,
    /// Der HTML-Baum dieser Seite (Architekturgrundsatz 3).
    pub doc: &'a dom::Document,
    /// Der dekodierte Quelltext.
    pub html: &'a str,
    /// Antwort-Header.
    pub headers: &'a [(String, String)],
    /// Einstellungen.
    pub cfg: &'a ParseConfig,
}

/// Eingabe der Auswertung einer HTML-Antwort.
pub struct ParseInput<'a> {
    pub status: u16,
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
/// 5 `elements::extract`, `content::analyze_with_source` (Quelltextlänge = `html.chars().count()`, A-7), `pixel::annotate` (Titel 20 px, Meta-Descriptions 14 px, Regular;
/// nur bei `cfg.pixel`) → 6 `directives::extract` (Token `[robots_token, "googlebot"]`, K-12),
/// `canonical::extract`, `hreflang::extract`, `refresh::extract`, `pagination::extract` →
/// 7 `headers::merge(headers::hints(…))` → 8 `effective::compute` → 9 (ab M2) die Auswertungsmodule über
/// [`PageCtx`], jeweils nur bei eingeschaltetem Schalter: `security` (immer), `validation`, `readability`
/// (Lesbarkeit und MinHash), `custom_search`, `custom_extract`, `sd`, `spelling`.
pub fn parse_html(input: &ParseInput) -> ParsedPage {
    let cfg = input.cfg;

    let (html, charset) = charset::decode(input.body, input.content_type, input.headers);
    let doc = dom::Document::parse(&html);
    let base = base::resolve_base(&doc, input.url);

    let mut links = links::extract(&doc, &base, cfg);
    let (images, image_links) = images::extract_with(&doc, &base, cfg);
    links.extend(image_links);
    if cfg.crawl.spider.store_uncrawlable_links {
        links.extend(crawlability::inspect(&doc, &base, cfg));
    }
    links.extend(alternates::extract(&doc, &base, cfg));
    links.extend(css::extract_inline(&doc, &base));

    let elements::ElementSets {
        mut titles,
        mut meta_descriptions,
        meta_keywords,
        h1,
        h2,
    } = elements::extract(&doc);
    let stats = content::analyze_with_source(&doc, &cfg.content_area, html.chars().count());
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
        ..Default::default()
    };

    headers::merge(&mut page, headers::hints(input.url, input.headers, cfg));

    switches::filter_directives(&mut page.directives, cfg);
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

    // Ab M2: weitere Auswertungen über Konfigurationsschalter (M2-F02 legt die Aufrufe an, die Fachpakete füllen sie).
    let ctx = PageCtx {
        url: input.url,
        base: &base,
        doc: &doc,
        html: &html,
        headers: input.headers,
        cfg,
    };
    bookmarks::run(&ctx, &mut page);
    elements::annotate_facts(&doc, cfg, &mut page);
    content::run(&ctx, &mut page);
    security::run(&ctx, &mut page);
    if cfg.html_validation {
        validation::run(&ctx, &mut page);
    }
    if cfg.readability || cfg.minhash {
        readability::run(&ctx, &mut page);
    }
    if !cfg.custom_search.filters.is_empty() {
        custom_search::run(&ctx, &mut page);
    }
    if (200..300).contains(&input.status) && !cfg.custom_extraction.extractors.is_empty() {
        custom_extract::run(&ctx, &mut page);
    }
    if cfg.structured_data_enabled() {
        sd::run(&ctx, &mut page);
    }
    if cfg.spelling.enabled || cfg.grammar.enabled {
        spelling::run(&ctx, &mut page);
    }
    switches::apply(&mut page, cfg);
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

/// Antwortfakten unabhängig vom HTML-Parser.
pub struct ResponseInput<'a> {
    pub url: &'a url::Url,
    pub internal: bool,
    pub purpose: os_model::FetchPurpose,
    pub status: u16,
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    pub content_length: Option<u64>,
    pub untruncated_bytes: Option<u64>,
    pub truncated: bool,
    pub cfg: &'a ParseConfig,
}
/// Feste Reihenfolge der Antwortmodule; auch für 3xx und externe Ressourcen.
pub fn response_ext(input: &ResponseInput<'_>) -> Vec<(os_model::ExtKey, os_model::Value)> {
    response_ext_with(
        input,
        &[
            security::response_facts,
            validation::response_facts,
            images::response_facts,
            canonical::response_facts,
            pagination::response_facts,
        ],
    )
}

type ResponseHook = fn(&ResponseInput<'_>) -> Vec<(os_model::ExtKey, os_model::Value)>;
fn response_ext_with(
    input: &ResponseInput<'_>,
    hooks: &[ResponseHook],
) -> Vec<(os_model::ExtKey, os_model::Value)> {
    hooks.iter().flat_map(|hook| hook(input)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // spec: TECH-008, TECH-001, ONPAGE-059
    // Infrastruktur (M2-F05a); Fachprüfung M2-D04/D05/D10/D11.
    #[test]
    fn response_hooks_keep_order_and_receive_non_html_redirect_and_external_inputs() {
        fn first(input: &ResponseInput<'_>) -> Vec<(os_model::ExtKey, os_model::Value)> {
            assert_eq!(input.status, 302);
            assert!(!input.internal);
            assert_eq!(input.body, b"fixture");
            vec![(
                os_model::ExtKey::from_static("security.fixture"),
                os_model::Value::Int(1),
            )]
        }
        fn second(_: &ResponseInput<'_>) -> Vec<(os_model::ExtKey, os_model::Value)> {
            vec![(
                os_model::ExtKey::from_static("validation.fixture"),
                os_model::Value::Int(2),
            )]
        }
        let url = url::Url::parse("https://site.test/fixture.bin").unwrap();
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        let input = ResponseInput {
            url: &url,
            internal: false,
            purpose: os_model::FetchPurpose::External,
            status: 302,
            headers: &[],
            body: b"fixture",
            content_length: Some(7),
            untruncated_bytes: Some(7),
            truncated: false,
            cfg: &cfg,
        };
        let facts = response_ext_with(&input, &[first, second]);
        assert_eq!(
            facts.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            vec![os_model::Value::Int(1), os_model::Value::Int(2)]
        );
    }

    #[test]
    fn parse_entry_points_smoke() {
        let url = url::Url::parse("https://site.test/a").unwrap();
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        let input = ParseInput {
            status: 200,
            url: &url,
            body: b"<html><head><title>T</title></head><body><a href=\"/b\">b</a></body></html>",
            content_type: Some("text/html"),
            headers: &[],
            cfg: &cfg,
        };
        let page = parse_html(&input);
        assert_eq!(page.base_url, "https://site.test/a");
        assert!(page.effective_index && page.effective_follow);
        // „b“; ein Link im Absatz zählt nicht extra (A-35b: nur `<li>`)
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

    fn page_with(cfg: &CrawlConfig, body: &str) -> ParsedPage {
        let url = url::Url::parse("https://site.test/a").unwrap();
        let cfg = ParseConfig::from_crawl_config(cfg);
        parse_html(&ParseInput {
            status: 200,
            url: &url,
            body: body.as_bytes(),
            content_type: Some("text/html"),
            headers: &[],
            cfg: &cfg,
        })
    }

    const M2_HTML: &str = "<html><head><title>T</title></head><body><nav><a href=\"/n\" target=\"_blank\">n</a></nav>\
        <main><a href=\"/c\">c</a></main><footer><a href=\"/f\" target=\"_self\">f</a></footer></body></html>";

    // Infrastruktur (M2-F02): `LinkRef.target`; die Fachprüfung (ONPAGE-036) gehört M2-D11.
    #[test]
    fn link_target_attribute_is_part_of_every_extracted_link() {
        let page = page_with(&CrawlConfig::default(), M2_HTML);
        let targets: Vec<(&str, Option<&str>)> = page
            .links
            .iter()
            .map(|l| (l.href_raw.as_str(), l.target.as_deref()))
            .collect();
        assert_eq!(
            targets,
            [("/n", Some("_blank")), ("/c", None), ("/f", Some("_self"))]
        );
    }

    // Infrastruktur (M2-F02): Aufrufe der Stub-Module; die Fachprüfungen gehören den Paketen D02, D09, D10, D14 – D16, D19.
    #[test]
    fn the_m2_modules_are_called_through_switches_and_keep_the_m1_output_with_default_rules() {
        let plain = page_with(&CrawlConfig::default(), M2_HTML);
        // Alle Schalter an: die Stub-Module (M2-F02) verändern die Ausgabe von M1 nicht.
        let mut all_on = CrawlConfig::default();
        all_on.advanced.html_validation = true;
        all_on.extraction.readability = true;
        all_on.duplicates.near_enabled = true;
        all_on.extraction.structured_data.json_ld = true;
        all_on.extraction.structured_data.microdata = true;
        all_on.extraction.structured_data.rdfa = true;
        all_on.spelling.enabled = true;
        all_on.grammar.enabled = true;
        all_on
            .custom_search
            .filters
            .push(os_model::SearchFilter::default());
        all_on
            .custom_extraction
            .extractors
            .push(os_model::Extractor::default());
        let switched = page_with(&all_on, M2_HTML);
        let m1_only = |mut page: ParsedPage| {
            page.ext.clear();
            page.structured_data.clear();
            page.forms.clear();
            page.validation = None;
            page.custom_search.clear();
            page.custom_extraction.clear();
            page.minhash = None;
            page
        };
        assert_eq!(m1_only(plain.clone()), m1_only(switched));
        // Standardregeln = Positionen von M1.
        let positions: Vec<os_model::LinkPosition> =
            plain.links.iter().map(|l| l.position.clone()).collect();
        assert_eq!(
            positions,
            [
                os_model::LinkPosition::Navigation,
                os_model::LinkPosition::Content,
                os_model::LinkPosition::Footer
            ]
        );
        let cfg = ParseConfig::from_crawl_config(&all_on);
        assert!(
            cfg.html_validation && cfg.readability && cfg.minhash && cfg.structured_data_enabled()
        );
        assert!(!ParseConfig::from_crawl_config(&CrawlConfig::default()).structured_data_enabled());
    }

    #[test]
    fn robots_token_defaults_to_preset() {
        let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
        assert_eq!(cfg.robots_token, "openspider");
        assert!(!cfg.pixel);
    }
}
