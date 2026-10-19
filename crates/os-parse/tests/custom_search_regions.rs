use os_model::{
    ContentAreaConfig, CrawlConfig, CustomSearchConfig, SearchFilter, SearchHit, SearchKind,
    SearchScope,
};
use os_parse::{custom_search, ParseConfig, ParseInput};
use url::Url;

const HTML: &str = include_str!("../../../tests/fixtures/integ-custom-search/regions.html");
const XPATH: &str = include_str!("../../../tests/fixtures/integ-custom-search/xpath.html");
const REGEX: &str = include_str!("../../../tests/fixtures/integ-custom-search/regex.txt");

fn filter(scope: SearchScope, expression: &str) -> SearchFilter {
    SearchFilter {
        scope,
        expression: expression.into(),
        ..Default::default()
    }
}
fn hits(
    body: &[u8],
    content_type: &str,
    filters: Vec<SearchFilter>,
    area: ContentAreaConfig,
) -> Vec<SearchHit> {
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.custom_search.filters = filters;
    cfg.content_area = area;
    custom_search::run_on_body(&ParseInput {
        status: 404,
        url: &Url::parse("https://site.test/page").unwrap(),
        body,
        content_type: Some(content_type),
        headers: &[],
        cfg: &cfg,
    })
}
fn counts(scope: SearchScope, expressions: &[&str]) -> Vec<u32> {
    let hits = hits(
        HTML.as_bytes(),
        "text/html",
        expressions.iter().map(|s| filter(scope, s)).collect(),
        ContentAreaConfig::default(),
    );
    assert!(hits.iter().all(|h| h.error.is_none()), "{hits:?}");
    hits.into_iter().map(|h| h.count).collect()
}

// spec: INTEG-021
#[test]
fn html_search_preserves_comments_attributes_entities_and_decodes_charset() {
    assert_eq!(
        counts(
            SearchScope::Html,
            &["GTM-ABC123", "data-sku=\"X-99\"", "&amp;", "&"]
        ),
        [1, 1, 1, 4]
    );
    let found = hits(
        b"<p>f\xfcr</p>",
        "text/html; charset=iso-8859-1",
        vec![filter(SearchScope::Html, "für")],
        ContentAreaConfig::default(),
    );
    assert_eq!(found[0].count, 1);
}

// spec: INTEG-022
#[test]
fn page_text_decodes_entities_and_joins_inline_text_excluding_hidden_regions() {
    assert_eq!(
        counts(
            SearchScope::PageText,
            &[
                "Out of stock",
                "script-only",
                "style-only",
                "noscript-only",
                "template-only",
                "comment-only",
                "title-only",
                "alt-only",
                "&amp;",
                "&"
            ]
        ),
        [2, 0, 0, 0, 0, 0, 0, 0, 0, 2]
    );
}

// spec: INTEG-023
#[test]
fn no_anchors_excludes_nested_link_text_but_keeps_unlinked_text() {
    assert_eq!(counts(SearchScope::PageText, &["Laufschuhe"]), [1]);
    assert_eq!(counts(SearchScope::PageTextNoAnchors, &["Laufschuhe"]), [0]);
    let body = format!("{HTML}<p>Laufschuhe</p>");
    assert_eq!(
        hits(
            body.as_bytes(),
            "text/html",
            vec![filter(SearchScope::PageTextNoAnchors, "Laufschuhe")],
            ContentAreaConfig::default()
        )[0]
        .count,
        1
    );
}

// spec: INTEG-024
#[test]
fn implicit_head_is_used_and_body_content_is_excluded() {
    assert_eq!(
        counts(
            SearchScope::HtmlHead,
            &["head-snippet", "body-snippet", "GTM-ABC123"]
        ),
        [1, 0, 1]
    );
    for i in 0..10 {
        let html = format!(
            "<title>Title</title>{}<p>body</p>",
            if i < 3 { "<script>GTM-ID</script>" } else { "" }
        );
        assert_eq!(
            hits(
                html.as_bytes(),
                "text/html",
                vec![filter(SearchScope::HtmlHead, "GTM-ID")],
                ContentAreaConfig::default()
            )[0]
            .count,
            u32::from(i < 3)
        );
    }
}

// spec: INTEG-025
#[test]
fn body_includes_markup_and_noscript_but_excludes_head() {
    assert_eq!(
        counts(
            SearchScope::HtmlBody,
            &[
                "head-snippet",
                "body-snippet",
                "noscript-only",
                "class=\"price\""
            ]
        ),
        [0, 1, 1, 1]
    );
}

// spec: INTEG-026
#[test]
fn xpath_uses_repaired_html_and_sums_elements_and_attributes_separately() {
    let filters = [
        "//h3",
        "//missing",
        "//img/@alt",
        "//h3[2]",
        "//h3 ! upper-case(string(.))",
    ]
    .into_iter()
    .map(|xpath| SearchFilter {
        xpath: Some(xpath.into()),
        ..filter(SearchScope::XPath, "Preis")
    })
    .collect();
    let found = hits(
        XPATH.as_bytes(),
        "text/html",
        filters,
        ContentAreaConfig::default(),
    );
    assert!(found.iter().all(|h| h.error.is_none()), "{found:?}");
    assert_eq!(
        found.iter().map(|h| h.count).collect::<Vec<_>>(),
        [2, 0, 1, 1, 2]
    );
    let cfg = CustomSearchConfig {
        filters: vec![SearchFilter {
            xpath: Some("//*[".into()),
            ..filter(SearchScope::XPath, "Preis")
        }],
        ..Default::default()
    };
    assert_eq!(
        custom_search::validate(&cfg)[0].path,
        "custom_search.filters[0].xpath"
    );
}

// spec: INTEG-027
#[test]
fn content_area_obeys_default_and_custom_include_exclude_rules() {
    assert_eq!(
        counts(
            SearchScope::ContentArea,
            &["footer-only", "Laufschuhe", "comment-marker", "Preis"]
        ),
        [0, 0, 1, 3]
    );
    let mut area = ContentAreaConfig::default();
    area.exclude_ids.push("#comments".into());
    area.include_elements.push("MAIN".into());
    let found = hits(
        HTML.as_bytes(),
        "text/html",
        vec![
            filter(SearchScope::ContentArea, "comment-marker"),
            filter(SearchScope::ContentArea, "Preis"),
            filter(SearchScope::ContentArea, "Out of stock"),
        ],
        area,
    );
    assert_eq!(found.iter().map(|h| h.count).collect::<Vec<_>>(), [0, 3, 0]);
}

// spec: INTEG-028
#[test]
fn regex_counts_nonoverlapping_matches_including_empty_lookarounds_and_flags() {
    let expressions = [
        "pr",
        r"\bpr\b",
        r"(?i)\bpr\b",
        r"\bnatural\b|\borganic\b|\bfree\b",
        "(?s)(?=.*rot)(?=.*blau).+",
        "rot.blau",
        "(?s)rot.blau",
        "x*",
        "(?<=pre-)rendering",
        "(?s)(?=.*rot)(?=.*blau)",
    ];
    let found = hits(
        REGEX.as_bytes(),
        "text/html",
        expressions
            .iter()
            .map(|s| SearchFilter {
                kind: SearchKind::Regex,
                ..filter(SearchScope::Html, s)
            })
            .collect(),
        ContentAreaConfig::default(),
    );
    assert!(found.iter().all(|h| h.error.is_none()), "{found:?}");
    assert_eq!(
        found[..7].iter().map(|h| h.count).collect::<Vec<_>>(),
        [3, 1, 2, 4, 1, 0, 1]
    );
    assert!(
        found[7].count > 0,
        "x* terminates while counting empty matches"
    );
    assert_eq!(found[8].count, 1);
    assert!(found[9].count >= 1, "the spec's pure lookahead must match");
    let missing = hits(
        b"<p>rot</p>",
        "text/html",
        vec![SearchFilter {
            kind: SearchKind::Regex,
            ..filter(SearchScope::Html, expressions[9])
        }],
        ContentAreaConfig::default(),
    );
    assert_eq!(missing[0].count, 0);
    assert_eq!(missing[0].error, None);
}

// spec: INTEG-028
#[test]
fn pathological_regex_is_bounded_and_does_not_stop_other_filters() {
    let started = std::time::Instant::now();
    let found = hits(
        REGEX.as_bytes(),
        "text/html",
        vec![
            SearchFilter {
                kind: SearchKind::Regex,
                ..filter(SearchScope::Html, r"(?:(a+)\1)+$")
            },
            filter(SearchScope::Html, "natural"),
        ],
        ContentAreaConfig::default(),
    );
    assert_eq!(found[0].error.as_deref(), Some("Regex timeout"));
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(found[1].count, 2);
}

// spec: INTEG-029
#[test]
fn text_is_literal_and_unicode_aware_without_sharp_s_expansion() {
    assert_eq!(
        counts(SearchScope::PageText, &["openspider", "straße", "a.b"]),
        [1, 2, 1]
    );
    let found = hits(
        HTML.as_bytes(),
        "text/html",
        vec![
            filter(SearchScope::PageText, "openspider"),
            SearchFilter {
                case_sensitive: true,
                ..filter(SearchScope::PageText, "openspider")
            },
        ],
        ContentAreaConfig::default(),
    );
    assert!(found.iter().all(|hit| hit.error.is_none()));
    assert_eq!(
        found.iter().map(|hit| hit.count).collect::<Vec<_>>(),
        [1, 0]
    );
}

// spec: INTEG-020
#[test]
fn validation_enforces_limits_names_expressions_and_roundtrips_profiles() {
    let cfg = CustomSearchConfig {
        filters: (0..100)
            .map(|_| filter(SearchScope::Html, "GTM-ABC123\nnext line"))
            .collect(),
        ..Default::default()
    };
    assert!(custom_search::validate(&cfg).is_empty());
    assert_eq!(
        custom_search::filter_name(&cfg.filters[99], 99),
        "Filter 100"
    );
    let crawl = CrawlConfig {
        custom_search: cfg.clone(),
        ..Default::default()
    };
    let loaded: CrawlConfig =
        serde_json::from_str(&serde_json::to_string(&crawl).unwrap()).unwrap();
    assert_eq!(loaded.custom_search, cfg);
    let mut cfg = cfg;
    cfg.filters.push(filter(SearchScope::Html, "extra"));
    assert!(custom_search::validate(&cfg)
        .iter()
        .any(|i| i.message == "Maximal 100 Suchfilter"));
    cfg.filters[0].name = "Filter 2".into();
    cfg.filters[2].expression.clear();
    cfg.filters[3].kind = SearchKind::Regex;
    cfg.filters[3].expression = "(abc".into();
    let issues = custom_search::validate(&cfg);
    for (path, code) in [
        ("custom_search.filters", "limit"),
        ("custom_search.filters[1].name", "duplicate_name"),
        ("custom_search.filters[2].expression", "empty_expression"),
        ("custom_search.filters[3].expression", "invalid_regex"),
    ] {
        assert!(issues
            .iter()
            .any(|i| i.path == path && i.code == format!("invalid_custom_search.{code}")));
    }
    cfg.filters[0].scope = SearchScope::XPath;
    for xpath in ["", "//*["] {
        cfg.filters[0].xpath = Some(xpath.into());
        let issues = custom_search::validate(&cfg);
        assert!(issues
            .iter()
            .any(|i| i.path == "custom_search.filters[0].xpath"
                && i.code == "invalid_custom_search.invalid_xpath"));
    }
}

// spec: INTEG-035
#[test]
fn bodyless_responses_produce_no_search_rows_and_parse_html_calls_the_module() {
    assert!(hits(
        b"",
        "text/html",
        vec![filter(SearchScope::Html, "GTM")],
        ContentAreaConfig::default()
    )
    .is_empty());
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.custom_search.filters = vec![filter(SearchScope::Html, "GTM-ABC123")];
    let url = Url::parse("https://site.test/missing").unwrap();
    let page = os_parse::parse_html(&ParseInput {
        status: 404,
        url: &url,
        body: HTML.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    assert_eq!(page.custom_search[0].count, 1);
}

// spec: INTEG-032
#[test]
fn bulk_add_skips_empty_duplicate_records_and_reports_capacity() {
    let records = include_str!("../../../tests/fixtures/integ-custom-search/bulk-keywords.txt");
    let template = SearchFilter {
        contains: false,
        case_sensitive: true,
        ..filter(SearchScope::PageTextNoAnchors, "unused")
    };
    let mut cfg = CustomSearchConfig::default();
    let report = custom_search::bulk_add(&mut cfg, records.lines().map(str::to_owned), &template);
    assert_eq!(
        report,
        custom_search::BulkAddReport {
            added: 100,
            empty: 3,
            duplicates: 2,
            over_limit: 0
        }
    );
    assert!(cfg.filters.iter().all(|f| !f.contains
        && f.case_sensitive
        && f.scope == SearchScope::PageTextNoAnchors
        && f.name == f.expression));
    let mut cfg = CustomSearchConfig::default();
    custom_search::bulk_add(
        &mut cfg,
        (0..95).map(|i| format!("Existing {i}")),
        &template,
    );
    let report = custom_search::bulk_add(&mut cfg, records.lines().map(str::to_owned), &template);
    assert_eq!(report.added, 5);
    assert_eq!(report.over_limit, 95);
    assert_eq!(cfg.filters.len(), 100);
}

// spec: INTEG-028
#[test]
fn regular_nested_repetition_uses_the_linear_engine_without_a_false_timeout() {
    // The spec's (a+)+$ example is delegated to a linear matcher by fancy-regex;
    // it cannot trigger catastrophic backtracking in this implementation.
    let found = hits(
        REGEX.as_bytes(),
        "text/html",
        vec![SearchFilter {
            kind: SearchKind::Regex,
            ..filter(SearchScope::Html, "(a+)+$")
        }],
        ContentAreaConfig::default(),
    );
    assert_eq!(found[0].count, 0);
    assert_eq!(found[0].error, None);
}

// spec: INTEG-028
#[test]
fn lookbehind_on_large_html_finds_late_matches_and_handles_no_match() {
    // Fixture-derived page of about 200 KB: successful and unsuccessful scans
    // both exceed the old fixed 50,000-backtrack cap.
    let html = format!(
        "<html><body>{}<p class=\"price\">42</p></body></html>",
        HTML.repeat(200_000 / HTML.len() + 1)
            .replace("class=\"price\"", "class=\"other\"")
    );
    assert!(html.len() > 200_000);
    let found = hits(
        html.as_bytes(),
        "text/html",
        [r#"(?<=class=")price"#, r#"(?<=class=")missing"#]
            .into_iter()
            .map(|expression| SearchFilter {
                kind: SearchKind::Regex,
                ..filter(SearchScope::Html, expression)
            })
            .collect(),
        ContentAreaConfig::default(),
    );
    assert!(found.iter().all(|hit| hit.error.is_none()), "{found:?}");
    assert_eq!(found[0].count, 1);
    assert_eq!(found[1].count, 0);
}

// spec: INTEG-028
#[test]
fn exhausted_page_budget_marks_regex_filters_but_keeps_text_search_working() {
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.custom_search.regex_timeout_ms = 0;
    cfg.custom_search.filters = ["natural", "missing"]
        .into_iter()
        .map(|expression| SearchFilter {
            kind: SearchKind::Regex,
            ..filter(SearchScope::Html, expression)
        })
        .chain([filter(SearchScope::Html, "natural")])
        .collect();
    let found = custom_search::run_on_body(&ParseInput {
        status: 200,
        url: &Url::parse("https://site.test/budget").unwrap(),
        body: REGEX.as_bytes(),
        content_type: Some("text/html"),
        headers: &[],
        cfg: &cfg,
    });
    assert!(found[..2]
        .iter()
        .all(|hit| hit.count == 0 && hit.error.as_deref() == Some("Regex timeout")));
    assert_eq!(found[2].count, 2);
    assert_eq!(found[2].error, None);
}

// spec: INTEG-027
#[test]
fn content_area_text_shares_word_count_rules_including_blank_and_nested_rules() {
    let doc = os_parse::dom::Document::parse(HTML);
    for area in [
        ContentAreaConfig::default(),
        ContentAreaConfig {
            include_elements: vec!["  ".into(), " MAIN ".into()],
            exclude_classes: vec![".price".into()],
            exclude_ids: vec!["#comments".into()],
            ..Default::default()
        },
    ] {
        let text = os_parse::content::area_text(&doc, &area);
        // This fixture has no list items (whose word-count bonus is intentional).
        assert_eq!(
            text.split_whitespace().count() as u32,
            os_parse::content::word_count(&doc, &area)
        );
        let found = hits(
            HTML.as_bytes(),
            "text/html",
            vec![filter(SearchScope::ContentArea, &text)],
            area,
        );
        assert_eq!(found[0].count, 1);
        assert_eq!(found[0].error, None);
    }
}
