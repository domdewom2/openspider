//! Tests der Validierung gegen die Fixtures in `fixtures/tech-sd/` (M2-D20) und `tests/fixtures/ui/fx-ui-sd/`.

use os_model::{CrawlConfig, ParsedPage, SdFinding, UrlId};
use serde_json::json;

use super::{rich, values, vocab};
use crate::{parse_html, ParseConfig, ParseInput};

fn parse_with(html: &str, adjust: impl FnOnce(&mut ParseConfig)) -> ParsedPage {
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    cfg.structured_data.json_ld = true;
    cfg.structured_data.microdata = true;
    cfg.structured_data.rdfa = true;
    adjust(&mut cfg);
    let url = "https://site.test/page".parse().unwrap();
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body: html.as_bytes(),
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    })
}

fn findings(html: &str) -> Vec<SdFinding> {
    parse_with(html, |_| {}).sd_findings
}

fn of<'a>(f: &'a [SdFinding], validation: &str, severity: &str) -> Vec<&'a SdFinding> {
    f.iter()
        .filter(|f| f.validation == validation && f.severity == severity)
        .collect()
}

fn summary(f: &[&SdFinding]) -> Vec<(String, String, String)> {
    f.iter()
        .map(|f| {
            (
                f.code.clone(),
                f.property.clone().unwrap_or_default(),
                f.feature.clone().unwrap_or_default(),
            )
        })
        .collect()
}

fn features(f: &[SdFinding]) -> Vec<String> {
    f.iter()
        .filter(|f| f.code == super::FEATURE_DETECTED)
        .map(|f| f.feature.clone().unwrap())
        .collect()
}

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!("fixtures/tech-sd/", $name))
    };
}

// spec: TECH-187
#[test]
fn schema_errors_fixture_yields_exactly_the_five_specified_findings() {
    let f = findings(fixture!("schema-errors.html"));
    let errors = of(&f, "schema_org", "error");
    assert_eq!(
        summary(&errors),
        [
            ("sd.schema.unknown_type", "@type", ""),
            ("sd.schema.unknown_property", "pricee", ""),
            ("sd.schema.property_not_allowed", "recipeIngredient", ""),
            ("sd.schema.value_type", "author", ""),
            ("sd.schema.value_format", "dateCreated", ""),
        ]
        .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()))
    );
    assert_eq!(
        errors.iter().map(|e| e.item).collect::<Vec<_>>(),
        [Some(0), Some(1), Some(1), Some(2), Some(2)]
    );
    assert!(of(&f, "schema_org", "warning").is_empty());
}

// spec: TECH-187
#[test]
fn case_sensitivity_switch_controls_type_and_property_names() {
    let html = fixture!("case.html");
    let off = parse_with(html, |_| {}).sd_findings;
    assert!(of(&off, "schema_org", "error").is_empty());
    let on = parse_with(html, |c| c.structured_data.case_sensitive = true).sd_findings;
    assert_eq!(
        summary(&of(&on, "schema_org", "error"))
            .into_iter()
            .map(|s| s.0)
            .collect::<Vec<_>>(),
        ["sd.schema.unknown_type", "sd.schema.unknown_property"]
    );
}

// spec: TECH-187
#[test]
fn data_vocabulary_items_raise_a_single_warning() {
    let f = findings(fixture!("data-vocabulary.html"));
    assert!(of(&f, "schema_org", "error").is_empty());
    assert_eq!(
        summary(&of(&f, "schema_org", "warning")),
        [(
            "sd.schema.deprecated_vocabulary".into(),
            "@type".into(),
            String::new()
        )]
    );
}

// spec: TECH-187
#[test]
fn arrays_nesting_roles_foreign_vocabularies_and_superseded_terms() {
    let html = r#"<script type="application/ld+json">{"@context":{"@vocab":"https://schema.org/","ex":"https://vocab.test/"},
      "@type":"Event","name":"x","ex:custom":"no finding","startDate":["2026-01-01","morgen","übermorgen"],
      "performer":{"@type":"OrganizationRole","performer":{"@type":"Person","name":"Ada"},"roleName":"Lead"},
      "workFeatured":{"@type":"CreativeWork","isFamilyFriendly":"yes"},"offers":{"@type":"Offer","eligibleQuantity":{"@type":"QuantitativeValue","value":"2"}},
      "eventStatus":"https://schema.org/EventScheduled","about":{"@type":"Thing","additionalType":"https://vocab.test/Kind"}}</script>"#;
    let f = findings(html);
    assert_eq!(
        summary(&of(&f, "schema_org", "error")),
        [
            ("sd.schema.value_format", "startDate[1]", ""),
            ("sd.schema.value_format", "startDate[2]", ""),
            (
                "sd.schema.value_format",
                "workFeatured[0].isFamilyFriendly",
                ""
            ),
        ]
        .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()))
    );
    // `pending`- und abgelöste Begriffe sind gültig.
    assert!(vocab::get().property("eligibleQuantity", true).is_some());
    assert!(vocab::get().type_name("Certification", true).is_some());
}

// spec: TECH-187
#[test]
fn id_references_are_validated_after_resolution() {
    let html = r##"<script type="application/ld+json">[{"@context":"https://schema.org","@type":"Organization","@id":"#org","name":"Org","colour":"red"},
      {"@context":"https://schema.org","@type":"Article","headline":"x","publisher":{"@id":"#org"}}]</script>"##;
    let f = findings(html);
    assert_eq!(
        summary(&of(&f, "schema_org", "error")),
        [
            ("sd.schema.unknown_property", "colour", ""),
            ("sd.schema.unknown_property", "publisher[0].colour", ""),
        ]
        .map(|(a, b, c)| (a.to_owned(), b.to_owned(), c.to_owned()))
    );
}

// spec: TECH-187
#[test]
fn literal_syntax_of_datatypes() {
    for ok in [
        "2026-01-31",
        "2026",
        "2026-02",
        "2026-02-28T10:00:00Z",
        "2026-02-29T10:00",
    ] {
        assert_eq!(
            values::is_iso_date(ok),
            !ok.starts_with("2026-02-29"),
            "{ok}"
        );
    }
    assert!(values::is_iso_date("2024-02-29T10:00:00.5+01:00"));
    for bad in ["gestern", "2026-13-40", "31.01.2026", "2026-1-5"] {
        assert!(!values::is_iso_date(bad), "{bad}");
    }
    assert!(
        values::is_number(&json!("5"))
            && values::is_number(&json!("-1.5e3"))
            && values::is_number(&json!(2))
    );
    assert!(!values::is_number(&json!("abc")) && !values::is_number(&json!("5,00")));
    assert!(
        values::is_integer(&json!(3))
            && !values::is_integer(&json!(3.5))
            && !values::is_integer(&json!("3.0"))
    );
    assert!(
        values::is_boolean(&json!(true)) && values::is_boolean(&json!("https://schema.org/False"))
    );
    assert!(!values::is_boolean(&json!("yes")));
    for ok in ["PT1H30M", "P2D", "PT45.5S", "P1Y2M3W4DT5H6M7S"] {
        assert!(values::is_duration(ok), "{ok}");
    }
    for bad in ["PT", "P", "1H", "PT1.5H", "P1H"] {
        assert!(!values::is_duration(bad), "{bad}");
    }
    assert!(
        values::is_currency("EUR") && !values::is_currency("eur") && !values::is_currency("EURO")
    );
    assert!(
        values::is_url("https://example.test/a")
            && values::is_url("/relative")
            && !values::is_url("not a url")
    );
    assert!(!values::is_url("javascript:alert(1)"));
}

/// Fixture, erwartete Fehler und Warnungen (Code, Pfad, Feature), erkannte Features.
type Case = (
    &'static str,
    &'static [Row],
    &'static [Row],
    &'static [&'static str],
);
type Row = (&'static str, &'static str, &'static str);

// spec: TECH-188
#[test]
fn rich_result_fixtures_match_the_specified_counts() {
    let cases: [Case; 9] = [
        (
            fixture!("rich-product-complete.html"),
            &[],
            &[],
            &["Product snippet", "Merchant listing"],
        ),
        (
            fixture!("rich-product-missing-currency.html"),
            &[(
                "sd.google.missing_required",
                "offers[0].priceCurrency",
                "Merchant listing",
            )],
            &[],
            &["Product snippet", "Merchant listing"],
        ),
        (
            fixture!("rich-recipe-no-image.html"),
            &[("sd.google.missing_required", "image", "Recipe")],
            &[],
            &["Recipe"],
        ),
        (
            fixture!("rich-event-bad-date.html"),
            &[("sd.google.invalid_value", "startDate", "Event")],
            &[],
            &["Event"],
        ),
        (
            fixture!("rich-breadcrumb-no-position.html"),
            &[
                (
                    "sd.google.missing_required",
                    "itemListElement[0].position",
                    "Breadcrumb",
                ),
                (
                    "sd.google.missing_required",
                    "itemListElement[1].position",
                    "Breadcrumb",
                ),
                (
                    "sd.google.missing_required",
                    "itemListElement[2].position",
                    "Breadcrumb",
                ),
            ],
            &[],
            &["Breadcrumb"],
        ),
        (
            fixture!("rich-product-no-brand.html"),
            &[],
            &[("sd.google.missing_recommended", "brand", "Merchant listing")],
            &["Product snippet", "Merchant listing"],
        ),
        (
            fixture!("rich-product-no-name.html"),
            &[
                ("sd.google.missing_required", "name", "Product snippet"),
                ("sd.google.missing_required", "name", "Merchant listing"),
            ],
            &[],
            &["Product snippet", "Merchant listing"],
        ),
        (
            fixture!("rich-product-no-offers.html"),
            &[(
                "sd.google.missing_required",
                "review|aggregateRating|offers",
                "Product snippet",
            )],
            &[],
            &["Product snippet"],
        ),
        (
            fixture!("rich-product-review.html"),
            &[],
            &[],
            &["Product snippet", "Review snippet"],
        ),
    ];
    for (html, errors, warnings, detected) in cases {
        let f = findings(html);
        let own = |v: &[(&str, &str, &str)]| {
            v.iter()
                .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            summary(&of(&f, "rich_results", "error")),
            own(errors),
            "{html}"
        );
        assert_eq!(
            summary(&of(&f, "rich_results", "warning")),
            own(warnings),
            "{html}"
        );
        assert_eq!(features(&f), detected, "{html}");
    }
}

// spec: TECH-188
#[test]
fn faq_and_sitelinks_search_box_are_neither_detected_nor_validated() {
    for html in [
        fixture!("rich-faq.html"),
        fixture!("rich-website-search.html"),
    ] {
        let f = findings(html);
        assert!(features(&f).is_empty());
        assert!(f.iter().all(|f| f.validation != "rich_results"), "{f:?}");
        // Die schema.org-Prüfung läuft weiter (beide Fixtures sind schema.org-gültig).
        assert!(of(&f, "schema_org", "error").is_empty(), "{f:?}");
    }
    let rules = rich::rules();
    for name in ["FAQ", "Sitelinks search box", "How-to"] {
        assert!(rules
            .features
            .iter()
            .any(|f| f.name == name && f.deprecated_since.is_some()));
    }
}

// spec: TECH-188
#[test]
fn article_amp_only_requirements_apply_to_amp_pages_only() {
    let plain = findings(fixture!("article.html"));
    assert!(of(&plain, "rich_results", "error").is_empty());
    assert_eq!(
        summary(&of(&plain, "rich_results", "warning"))
            .into_iter()
            .map(|s| s.1)
            .collect::<Vec<_>>(),
        ["datePublished", "headline", "image"]
    );
    let amp = findings(fixture!("article-amp.html"));
    assert!(of(&amp, "rich_results", "warning").is_empty());
    assert_eq!(
        summary(&of(&amp, "rich_results", "error"))
            .into_iter()
            .map(|s| s.1)
            .collect::<Vec<_>>(),
        ["headline", "image", "datePublished"]
    );
}

// spec: TECH-188
#[test]
fn rules_version_and_switches() {
    let page = parse_with(fixture!("rich-product-no-offers.html"), |_| {});
    let ext = |p: &ParsedPage, k: &str| {
        p.ext
            .iter()
            .find(|(key, _)| key.as_str() == k)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(
        ext(&page, "sdval.rules_version"),
        Some(os_model::Value::Text(rich::rules().version.clone()))
    );
    assert_eq!(
        ext(&page, "sdval.vocab_version"),
        Some(os_model::Value::Text("30.1".into()))
    );
    let off = parse_with(fixture!("schema-errors.html"), |c| {
        c.structured_data.schema_org_validation = false;
        c.structured_data.rich_results_validation = false;
    });
    assert!(off.sd_findings.is_empty() && ext(&off, "sdval.vocab_version").is_none());
    let schema_only = parse_with(fixture!("rich-product-no-offers.html"), |c| {
        c.structured_data.rich_results_validation = false;
    });
    assert!(schema_only
        .sd_findings
        .iter()
        .all(|f| f.validation == "schema_org"));
    // Ein abgeschaltetes Format wird weder extrahiert noch geprüft.
    let json_only = parse_with(fixture!("data-vocabulary.html"), |c| {
        c.structured_data.microdata = false;
    });
    assert!(json_only.sd_findings.is_empty());
}

// spec: TECH-189
#[test]
fn findings_transport_to_sd_issues_with_stable_ids() {
    let page = parse_with(fixture!("rich-product-missing-currency.html"), |_| {});
    let writes = super::to_writes(UrlId(7), &page.sd_findings);
    let rows: Vec<_> = writes
        .iter()
        .map(|w| {
            (
                w.url_id.0,
                w.item,
                w.validation.as_str(),
                w.severity.as_str(),
                w.code.as_str(),
                w.property.as_deref(),
                w.feature.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (
                7,
                Some(0),
                "rich_results",
                "error",
                "sd.google.missing_required",
                Some("offers[0].priceCurrency"),
                Some("Merchant listing")
            ),
            (
                7,
                None,
                "rich_results",
                "info",
                "sd.google.feature",
                None,
                Some("Product snippet")
            ),
            (
                7,
                None,
                "rich_results",
                "info",
                "sd.google.feature",
                None,
                Some("Merchant listing")
            ),
        ]
    );
}

// spec: TECH-187
// spec: TECH-188
#[test]
fn ui_fixture_formats_agree_on_findings() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-sd");
    for format in ["jsonld", "microdata", "rdfa"] {
        let read = |case: &str| {
            std::fs::read_to_string(dir.join(format!("{format}-{case}.html"))).unwrap()
        };
        let valid = findings(&read("valid"));
        assert!(
            valid.iter().all(|f| f.severity == "info"),
            "{format}: {valid:?}"
        );
        assert_eq!(
            features(&valid),
            ["Product snippet", "Merchant listing"],
            "{format}"
        );
        let warning = findings(&read("warning"));
        assert_eq!(
            summary(
                &warning
                    .iter()
                    .filter(|f| f.severity != "info")
                    .collect::<Vec<_>>()
            ),
            [(
                "sd.google.missing_recommended".into(),
                "brand".into(),
                "Merchant listing".into()
            )],
            "{format}"
        );
        let required = findings(&read("required-error"));
        assert_eq!(
            summary(
                &required
                    .iter()
                    .filter(|f| f.severity != "info")
                    .collect::<Vec<_>>()
            ),
            [
                (
                    "sd.google.missing_required".into(),
                    "name".into(),
                    "Product snippet".into()
                ),
                (
                    "sd.google.missing_required".into(),
                    "name".into(),
                    "Merchant listing".into()
                ),
            ],
            "{format}"
        );
        assert!(findings(&read("parse-error")).is_empty(), "{format}");
    }
}
