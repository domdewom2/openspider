//! Gemeinsamer Testhelfer: setzt jedes Feld, das die Konfiguration v2 ergänzt, auf einen Nicht-Standardwert.
#![allow(dead_code)]

use os_model::*;

/// Verändert alle in v2 neuen Felder (und nur diese) gegenüber den Standardwerten.
pub fn change_all_v2_fields(c: &mut CrawlConfig) {
    c.limits.max_folder_depth = Some(6);
    c.limits.max_query_params = Some(3);
    c.limits.max_urls_per_subdomain = Some(1000);
    c.limits.max_url_length = 2000;
    c.limits.max_links_per_url = 500;

    c.advanced.crawl_fragments = true;
    c.advanced.respect_noindex = true;
    c.advanced.respect_canonical = true;
    c.advanced.html_validation = false;
    c.advanced.green_hosting = true;

    c.auth.form = vec![FormLogin {
        login_url: "https://a.test/login".into(),
        profile: Some("shop".into()),
        captured_at: Some("2026-10-16T08:00:00Z".into()),
        cookie_count: 4,
    }];
    c.auth.session_guard = vec!["/logout".into()];
    c.auth.profile_refs = vec!["shop".into()];

    c.rendering.ajax_timeout_s = 12;
    c.rendering.js_error_reporting = false;
    c.rendering.viewport = ViewportConfig {
        preset: "custom".into(),
        width: 1280,
        height: 800,
        scale_factor: 2.0,
        mobile: false,
        touch: false,
    };
    c.rendering.screenshots = ScreenshotConfig {
        enabled: false,
        width: Some(1000),
        height: Some(2000),
        fit_to_content: true,
        wait_ms: 900,
        format: ScreenshotFormat::Jpeg,
        jpeg_quality: 70,
    };
    c.rendering.tabs = Some(3);
    c.rendering.restart_after_pages = 100;
    c.rendering.restart_memory_mb = 900;
    c.rendering.page_timeout_s = 30;
    c.rendering.chromium_path = Some("/opt/chromium/chrome".into());

    c.extraction.store_rendered_html = true;
    c.extraction.indexability = false;
    c.extraction.word_count = false;
    c.extraction.readability = false;
    c.extraction.text_ratio = false;
    c.extraction.page_size = false;
    c.extraction.forms = false;
    c.extraction.accessibility = true;
    c.extraction.cookies = true;
    c.extraction.meta_robots = false;
    c.extraction.x_robots_tag = false;
    c.extraction.srcset = true;
    c.extraction.structured_data = StructuredDataConfig {
        json_ld: true,
        microdata: true,
        rdfa: true,
        schema_org_validation: true,
        rich_results_validation: true,
        case_sensitive: true,
    };

    c.content.readability.language_aware = true;
    c.pdf.extract_properties = true;
    c.pdf.extract_link_text = true;
    c.analysis.near_duplicates = false;

    c.spelling = SpellingConfig {
        enabled: true,
        mode: SpellingMode::Manual,
        manual_language: "de-DE".into(),
        default_variants: [("de".to_string(), "de-AT".to_string())]
            .into_iter()
            .collect(),
        default_language: "de".into(),
    };
    c.grammar = GrammarConfig {
        enabled: true,
        disabled_rules: vec!["R1".into()],
    };
    c.custom_search.filters = vec![SearchFilter {
        name: "GTM".into(),
        contains: false,
        kind: SearchKind::Regex,
        expression: "GTM-[A-Z0-9]+".into(),
        scope: SearchScope::XPath,
        xpath: Some("//script".into()),
        case_sensitive: true,
    }];
    c.custom_extraction.extractors = vec![Extractor {
        name: "Preis".into(),
        mode: ExtractorMode::Css,
        expression: "span.price".into(),
        attribute: Some("content".into()),
        output: ExtractorOutput::InnerHtml,
    }];
    c.custom_js.snippets = vec![JsSnippet {
        name: "Titel".into(),
        kind: JsSnippetKind::Action,
        content_types: vec!["text/html".into(), "application/xhtml+xml".into()],
        timeout_s: 20,
        code: "return document.title;".into(),
    }];
    c.link_positions.rules = vec![
        LinkPositionRule {
            position: "Breadcrumb".into(),
            contains: vec!["breadcrumb".into()],
        },
        LinkPositionRule {
            position: "Content".into(),
            contains: vec![String::new()],
        },
    ];
    c.rewrite = RewriteConfig {
        remove_parameters: vec!["sid".into(), "utm_source".into()],
        remove_all_parameters: true,
        regex_replace: vec![RegexReplace {
            pattern: "/index\\.html$".into(),
            replacement: "/".into(),
        }],
    };
    c.cdns = vec!["cdn.other.test".into(), "other.test/assets/".into()];
    c.http = HttpConfig {
        headers: vec![HeaderEntry {
            name: "X-Test".into(),
            value: "1".into(),
            host: Some("a.test".into()),
        }],
        accept_language: Some("de-DE,de;q=0.9".into()),
        version: HttpVersion::Http2Preferred,
        retries_5xx: 2,
        retries_network: 3,
        throttle_on_429: false,
        respect_hsts: false,
        ignore_certificate_errors: true,
    };
    c.api = ApiConfig {
        ga4: Ga4Config {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/ga4".into()),
            property: Some("properties/123".into()),
            date_range: DateRangeConfig {
                preset: DateRangePreset::Custom,
                from: Some("2026-01-01".into()),
                to: Some("2026-01-31".into()),
            },
            metrics: vec!["sessions".into()],
            page_dimension: Some("landingPage".into()),
            filter: Some("country==DE".into()),
        },
        gsc: GscConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/gsc".into()),
            site: Some("sc-domain:a.test".into()),
            date_range: DateRangeConfig {
                preset: DateRangePreset::Last90Days,
                from: Some("2026-02-01".into()),
                to: Some("2026-02-28".into()),
            },
            search_type: SearchType::Image,
            filter: Some("device==MOBILE".into()),
            url_inspection: true,
        },
        psi: PsiConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/psi".into()),
            device: PsiDevice::Desktop,
            local: true,
            metric_groups: vec!["crux".into(), "lab".into()],
        },
        majestic: LinkMetricsConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/majestic".into()),
            levels: vec![ApiLevel::Domain],
            metrics: vec!["TrustFlow".into()],
        },
        ahrefs: LinkMetricsConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/ahrefs".into()),
            levels: vec![ApiLevel::Url, ApiLevel::Subdomain],
            metrics: vec!["refdomains".into()],
        },
        moz: LinkMetricsConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/moz".into()),
            levels: vec![ApiLevel::Subdomain],
            metrics: vec!["da".into()],
        },
        drive: DriveConfig {
            enabled: true,
            base_url: Some("http://127.0.0.1:9/drive".into()),
            root_folder: Some("Crawls".into()),
        },
    };
    c.segments.list = vec![SegmentConfig {
        name: "Shop".into(),
        color: "#ff8800".into(),
        groups: vec![SegmentGroup {
            conditions: vec![SegmentCondition {
                field: "Address".into(),
                op: SegmentOp::Contains,
                value: "/produkt/".into(),
            }],
        }],
    }];
    c.compare = CompareConfig {
        mapping: vec![MappingRule {
            pattern: "^https://staging\\.".into(),
            replacement: "https://www.".into(),
        }],
        change_detection: ChangeDetectionConfig {
            elements: vec![ChangeElement::Indexability, ChangeElement::Content],
            content_threshold_pct: 25,
        },
    };
    c.sitemap_gen = SitemapGenConfig {
        include_noindex: true,
        include_canonicalised: true,
        include_paginated: true,
        include_pdfs: true,
        include_3xx: true,
        include_4xx: true,
        include_5xx: true,
        include_no_response: true,
        exclude_patterns: vec!["/intern/".into()],
        lastmod: SitemapLastmod {
            enabled: false,
            source: LastmodSource::Custom,
            custom_date: Some("2026-10-16".into()),
        },
        priority: SitemapPriority {
            enabled: false,
            by_depth: vec![0.5, 0.4],
        },
        changefreq: SitemapChangefreq {
            enabled: false,
            basis: ChangeFreqBasis::Depth,
            by_depth: vec![ChangeFreq::Hourly],
        },
        images: SitemapImages {
            enabled: true,
            external_regex: vec!["cdn\\.".into()],
            max_inlinks: Some(5),
            exclude_noindex: true,
        },
        hreflang: SitemapHreflang {
            enabled: true,
            only_valid_targets: false,
        },
    };
}
