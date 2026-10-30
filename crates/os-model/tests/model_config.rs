//! Konfiguration: Defaults (M1-Plan 3.4), TOML-Rundreise, Vorwärtskompatibilität, JSON-Schema.

use os_model::*;
use proptest::prelude::*;

mod v2_support;

// spec: CRAWL-132, ONPAGE-002
#[test]
fn defaults_match_table_3_4() {
    let c = CrawlConfig::default();
    assert_eq!(c.schema_version, 2);
    assert!(c.generator.starts_with("openspider "));
    assert_eq!(c.mode, CrawlMode::Spider);

    assert!(!c.scope.all_subdomains && !c.scope.subfolder_only);
    assert!(c.scope.crawl_outside_start_folder && !c.scope.regex_partial_match);
    assert!(c.scope.include.is_empty() && c.scope.exclude.is_empty());

    for t in [
        c.spider.images,
        c.spider.css,
        c.spider.javascript,
        c.spider.media,
        c.spider.internal,
        c.spider.external,
        c.spider.canonicals,
        c.spider.meta_refresh,
        c.spider.iframes,
    ] {
        assert_eq!(
            t,
            StoreCrawl {
                store: true,
                crawl: true
            }
        );
    }
    assert_eq!(
        c.spider.hreflang,
        StoreCrawl {
            store: true,
            crawl: false
        }
    );
    assert_eq!(
        c.spider.pagination,
        StoreCrawl {
            store: true,
            crawl: false
        }
    );
    assert_eq!(
        c.spider.amp,
        StoreCrawl {
            store: false,
            crawl: false
        }
    );
    assert_eq!(
        c.spider.mobile_alternate,
        StoreCrawl {
            store: true,
            crawl: false
        }
    );
    assert!(!c.spider.follow_internal_nofollow && !c.spider.follow_external_nofollow);

    assert_eq!(c.limits.max_urls, 5_000_000);
    assert_eq!(c.limits.max_depth, None);
    assert_eq!(c.limits.max_redirects, 10);
    assert_eq!(c.limits.max_page_kb, 51_200);

    assert_eq!(c.speed.threads, 5);
    assert_eq!(
        (c.speed.max_per_host, c.speed.max_urls_per_sec),
        (None, None)
    );

    assert_eq!(c.user_agent.preset, "openspider");
    assert_eq!(
        (&c.user_agent.http, &c.user_agent.robots_token),
        (&None, &None)
    );

    assert_eq!(c.robots.mode, RobotsMode::Respect);
    assert_eq!(
        c.robots.unreachable_policy,
        RobotsUnreachablePolicy::BlockHost
    );
    assert_eq!((c.robots.retries, c.robots.retry_delay_s), (2, 5));

    assert_eq!(c.advanced.cookies, CookieMode::SessionOnly);
    assert_eq!(c.advanced.response_timeout_s, 20);
    // K-57: Standard aus wie die Referenz
    assert!(!c.advanced.assume_html && !c.advanced.respect_self_ref_meta_refresh);

    assert!(c.auth.basic.is_empty() && c.network.resolve.is_empty());
    assert_eq!(c.rendering.mode, RenderingMode::Text);

    let x = &c.extraction;
    assert!(x.page_titles && x.meta_description && x.meta_keywords && x.h1 && x.h2);
    assert!(x.hash && x.response_time && x.last_modified && !x.http_headers);
    assert!(!x.store_html);

    assert_eq!(
        c.content.area.exclude_elements,
        ["nav", "footer", "header", "aside"]
    );
    assert!(c.content.area.include_elements.is_empty());
    assert!(c.content.area.include_classes.is_empty() && c.content.area.include_ids.is_empty());
    assert!(c.content.area.exclude_classes.is_empty() && c.content.area.exclude_ids.is_empty());
    assert_eq!(c.content.low_word_count, 200);
    assert_eq!(c.content.soft404_phrases.len(), 15);
    assert!(c
        .content
        .soft404_phrases
        .contains(&"seite nicht gefunden".to_string()));

    assert_eq!(
        (
            c.title.max_chars,
            c.title.min_chars,
            c.title.max_px,
            c.title.min_px
        ),
        (60, 30, 561, 200)
    );
    let m = &c.meta_description;
    assert_eq!(
        (m.max_chars, m.min_chars, m.max_px, m.min_px),
        (155, 70, 985, 400)
    );
    assert_eq!(c.url.max_chars, 115);
    assert_eq!((c.h1.max_chars, c.h2.max_chars), (70, 70));
    assert_eq!((c.image.max_kb, c.image.alt_max_chars), (100, 100));

    let d = &c.duplicates;
    assert!(d.only_indexable && d.ignore_paginated && d.exact_enabled && !d.near_enabled);
    assert_eq!(d.near_threshold_pct, 90);
    assert!(c.issues.ignore_non_indexable);
    assert!(!c.pdf.store);

    assert_eq!(c.links.high_crawl_depth, 4);
    assert_eq!(c.links.high_internal_outlinks, 1000);
    assert_eq!(c.links.high_external_outlinks, 100);
    assert_eq!(c.links.non_descriptive_anchors.len(), 26);
    assert_eq!(c.links.non_descriptive_anchors[0], "click here");

    assert_eq!(c.hreflang.over_limit, HreflangOverLimit::Truncate);
    assert!(c.hreflang.allow_script_subtag);
    assert!(
        !c.sitemaps.crawl_linked && c.sitemaps.auto_discover_robots && c.sitemaps.urls.is_empty()
    );
    assert!(c.analysis.auto_at_end);
    assert_eq!(c.validation.html_doc_max_bytes, 2_097_152);
    assert_eq!(c.validation.resource_max_bytes, 2_097_152);
    assert!(c.export.utf8_bom);
    assert_eq!(c.export.line_ending, LineEnding::Crlf);
}

fn all_fields_changed() -> CrawlConfig {
    let mut c = CrawlConfig {
        schema_version: 2,
        generator: "openspider 9.9.9".into(),
        mode: CrawlMode::List,
        scope: ScopeConfig {
            all_subdomains: true,
            subfolder_only: true,
            crawl_outside_start_folder: true,
            regex_partial_match: true,
            include: vec!["^https://a\\.test/".into(), "x".into()],
            exclude: vec!["\\?utm_".into()],
        },
        spider: SpiderConfig {
            images: StoreCrawl::OFF,
            css: StoreCrawl::STORE_ONLY,
            javascript: StoreCrawl::OFF,
            media: StoreCrawl::STORE_ONLY,
            internal: StoreCrawl::STORE_ONLY,
            external: StoreCrawl::OFF,
            canonicals: StoreCrawl::OFF,
            hreflang: StoreCrawl::ON,
            pagination: StoreCrawl::ON,
            meta_refresh: StoreCrawl::OFF,
            iframes: StoreCrawl::STORE_ONLY,
            amp: StoreCrawl::ON,
            mobile_alternate: StoreCrawl::ON,
            follow_internal_nofollow: true,
            follow_external_nofollow: true,
            ..Default::default()
        },
        limits: LimitsConfig {
            max_urls: 1234,
            max_depth: Some(3),
            max_redirects: 4,
            max_page_kb: 77,
            ..LimitsConfig::default()
        },
        speed: SpeedConfig {
            threads: 11,
            max_per_host: Some(2),
            max_urls_per_sec: Some(7.5),
            ..Default::default()
        },
        user_agent: UserAgentConfig {
            preset: "custom".into(),
            http: Some("Mozilla/5.0 (X)".into()),
            robots_token: Some("mybot".into()),
        },
        robots: RobotsConfig {
            mode: RobotsMode::IgnoreButReport,
            unreachable_policy: RobotsUnreachablePolicy::AllowAll,
            retries: 0,
            retry_delay_s: 1,
            ..Default::default()
        },
        advanced: AdvancedConfig {
            cookies: CookieMode::Persistent,
            response_timeout_s: 3,
            assume_html: true,
            respect_self_ref_meta_refresh: false,
            ..AdvancedConfig::default()
        },
        auth: AuthConfig {
            basic: vec![
                BasicAuth {
                    prefix: "https://a.test/".into(),
                    username: "u".into(),
                    secret_ref: "auth/a".into(),
                },
                BasicAuth {
                    prefix: "https://b.test/".into(),
                    username: "v".into(),
                    secret_ref: "auth/b".into(),
                },
            ],
            ..AuthConfig::default()
        },
        network: NetworkConfig {
            resolve: vec!["*.site.test=127.0.0.1:8080".into(), "x.test=::1".into()],
        },
        rendering: RenderingConfig {
            mode: RenderingMode::JavaScript,
            ..RenderingConfig::default()
        },
        extraction: ExtractionConfig {
            page_titles: false,
            meta_description: false,
            meta_keywords: false,
            h1: false,
            h2: false,
            hash: false,
            response_time: false,
            last_modified: false,
            http_headers: false,
            store_html: true,
            ..ExtractionConfig::default()
        },
        content: ContentConfig {
            area: ContentAreaConfig {
                include_elements: vec!["main".into()],
                include_classes: vec!["c".into()],
                include_ids: vec!["i".into()],
                exclude_elements: vec!["aside".into()],
                exclude_classes: vec!["mobile-menu".into()],
                exclude_ids: vec!["comments".into()],
            },
            low_word_count: 50,
            soft404_phrases: vec!["weg".into(), "nicht da".into()],
            ..ContentConfig::default()
        },
        title: TextLimits {
            max_chars: 50,
            min_chars: 10,
            max_px: 500,
            min_px: 100,
        },
        meta_description: TextLimits {
            max_chars: 150,
            min_chars: 60,
            max_px: 900,
            min_px: 300,
        },
        url: UrlLimits {
            max_chars: 100,
            ..Default::default()
        },
        h1: HeadingLimits { max_chars: 60 },
        h2: HeadingLimits { max_chars: 65 },
        image: ImageLimits {
            max_kb: 50,
            alt_max_chars: 80,
        },
        duplicates: DuplicatesConfig {
            only_indexable: false,
            ignore_paginated: false,
            exact_enabled: false,
            near_enabled: true,
            near_threshold_pct: 80,
        },
        issues: IssuesConfig {
            ignore_non_indexable: false,
        },
        pdf: PdfConfig {
            store: true,
            ..PdfConfig::default()
        },
        links: LinksConfig {
            high_crawl_depth: 6,
            high_internal_outlinks: 10,
            high_external_outlinks: 5,
            non_descriptive_anchors: vec!["foo".into()],
        },
        hreflang: HreflangConfig {
            over_limit: HreflangOverLimit::Skip,
            allow_script_subtag: false,
        },
        sitemaps: SitemapsConfig {
            crawl_linked: true,
            auto_discover_robots: false,
            urls: vec!["https://a.test/sitemap.xml".into()],
        },
        analysis: AnalysisConfig {
            auto_at_end: false,
            link_score: false,
            canonicals: false,
            pagination: false,
            hreflang: false,
            links: false,
            sitemaps: false,
            content: false,
            images: false,
            analytics: false,
            search_console: false,
            link_metrics: false,
            amp: false,
            ..AnalysisConfig::default()
        },
        validation: ValidationConfig {
            html_doc_max_bytes: 1000,
            resource_max_bytes: 2000,
        },
        export: ExportConfig {
            utf8_bom: false,
            line_ending: LineEnding::Lf,
            ..Default::default()
        },
        ..CrawlConfig::default()
    };
    v2_support::change_all_v2_fields(&mut c);
    c
}

// spec: CRAWL-132
#[test]
fn toml_round_trip_all_fields() {
    for cfg in [CrawlConfig::default(), all_fields_changed()] {
        let text = cfg.to_toml_string().unwrap();
        let back = CrawlConfig::from_toml_str(&text).unwrap();
        assert_eq!(back, cfg, "{text}");
        // stabil: gleiche Konfiguration, gleicher Text
        assert_eq!(back.to_toml_string().unwrap(), text);
    }
}

// spec: CRAWL-132
#[test]
fn toml_uses_spec_paths() {
    let text = all_fields_changed().to_toml_string().unwrap();
    let doc: toml::Table = text.parse().unwrap();
    assert_eq!(doc["mode"].as_str(), Some("list"));
    assert_eq!(doc["speed"]["threads"].as_integer(), Some(11));
    assert_eq!(doc["spider"]["hreflang"]["crawl"].as_bool(), Some(true));
    assert_eq!(doc["robots"]["mode"].as_str(), Some("ignore_but_report"));
    assert_eq!(doc["advanced"]["cookies"].as_str(), Some("persistent"));
    assert_eq!(
        doc["content"]["area"]["exclude_classes"][0].as_str(),
        Some("mobile-menu")
    );
    assert_eq!(
        doc["auth"]["basic"][1]["secret_ref"].as_str(),
        Some("auth/b")
    );
    assert_eq!(doc["export"]["line_ending"].as_str(), Some("lf"));
    assert_eq!(doc["hreflang"]["over_limit"].as_str(), Some("skip"));
    assert_eq!(doc["analysis"]["search_console"].as_bool(), Some(false));
    // Passwörter gehören nie in die Datei (OUT-225): es gibt kein entsprechendes Feld.
    assert!(!text.contains("password"));
}

// spec: CRAWL-132
#[test]
fn partial_toml_fills_defaults_and_ignores_unknown_keys() {
    let c = CrawlConfig::from_toml_str(
        "[speed]\nthreads = 2\n[title]\nmax_chars = 50\nzukunft = 1\n[unbekannt]\nx = 1\n",
    )
    .unwrap();
    assert_eq!(c.speed.threads, 2);
    assert_eq!(c.speed.max_per_host, None);
    assert_eq!(c.title.max_chars, 50);
    assert_eq!(c.title.min_chars, 30);
    assert_eq!(c.limits, LimitsConfig::default());
    assert_eq!(
        CrawlConfig::from_toml_str("").unwrap().speed,
        SpeedConfig::default()
    );
}

// spec: CRAWL-132
#[test]
fn invalid_toml_and_values_are_errors() {
    assert!(matches!(
        CrawlConfig::from_toml_str("[speed\n"),
        Err(ModelError::Config(_))
    ));
    assert!(CrawlConfig::from_toml_str("[speed]\nthreads = \"viele\"").is_err());
    assert!(CrawlConfig::from_toml_str("mode = \"turbo\"").is_err());
    assert!(CrawlConfig::from_toml_str("[speed]\nthreads = -1").is_err());
}

#[test]
fn json_schema_lists_sections_and_enum_values() {
    let schema = schemars::schema_for!(CrawlConfig);
    let v = serde_json::to_value(&schema).unwrap();
    let props = v["properties"].as_object().unwrap();
    for key in [
        "mode", "scope", "spider", "limits", "speed", "robots", "analysis", "export",
    ] {
        assert!(props.contains_key(key), "{key}");
    }
    let text = v.to_string();
    assert!(text.contains("ignore_but_report"));
}

proptest! {
    // spec: CRAWL-132
    #[test]
    fn toml_round_trip_random_values(
        threads in 1u32..500,
        max_urls in 0u64..10_000_000,
        depth in proptest::option::of(0u32..100),
        rate in proptest::option::of(0.1f64..1000.0),
        phrase in "[a-zA-Z0-9 äöüß\\-]{0,20}",
        suffix in "[a-z]{0,8}",
    ) {
        let mut c = CrawlConfig::default();
        c.speed.threads = threads;
        c.speed.max_urls_per_sec = rate.map(|r| (r * 100.0).round() / 100.0);
        c.limits.max_urls = max_urls;
        c.limits.max_depth = depth;
        c.content.soft404_phrases.push(phrase);
        c.user_agent.preset = format!("bot{suffix}");
        let back = CrawlConfig::from_toml_str(&c.to_toml_string().unwrap()).unwrap();
        prop_assert_eq!(back, c);
    }
}
