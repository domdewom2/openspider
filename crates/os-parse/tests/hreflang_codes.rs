//! TECH-072: Validierung der hreflang-Codes gegen die eingecheckten ISO-Tabellen.

use os_model::CrawlConfig;
use os_parse::dom::Document;
use os_parse::hreflang::{self, validate_code};
use os_parse::ParseConfig;

fn valid(code: &str) -> bool {
    validate_code(code, true).is_ok()
}

// spec: TECH-072
#[test]
fn valid_forms() {
    for code in [
        "EN-gb",
        "de",
        "pt-BR",
        "zh-Hant",
        "zh-Hant-TW",
        "X-Default",
        "x-default",
        "en",
        "fr-CA",
    ] {
        assert_eq!(validate_code(code, true), Ok(()), "{code}");
    }
}

// spec: TECH-072
#[test]
fn invalid_codes_from_the_spec_with_their_reason() {
    let cases = [
        ("en-UK", "invalid_region"),
        ("en_GB", "invalid_format"),
        ("eng", "invalid_language"),
        ("jp", "invalid_language"),
        ("es-419", "invalid_region"),
        ("EU", "invalid_language"),
        ("de-", "invalid_format"),
        ("", "invalid_format"),
        ("x-default-de", "invalid_format"),
    ];
    for (code, reason) in cases {
        assert_eq!(validate_code(code, true), Err(reason), "{code:?}");
    }
}

// spec: TECH-072
#[test]
fn a_country_code_that_is_also_a_language_is_a_valid_language() {
    assert!(valid("be"));
    assert!(valid("BE-fr"), "Sprache be (Belarussisch) mit Region FR");
    // klein geschriebenes `eu` ist Baskisch
    assert!(valid("eu"));
}

// spec: TECH-072
#[test]
fn withdrawn_iso_639_1_codes_are_invalid() {
    for code in ["iw", "in", "ji", "IW", "jw", "mo", "sh"] {
        assert_eq!(validate_code(code, true), Err("invalid_language"), "{code}");
    }
    // Ersatzcodes bleiben gültig
    for code in ["he", "id", "yi", "tl", "tw"] {
        assert!(valid(code), "{code}");
    }
}

// spec: TECH-072
#[test]
fn script_subtag_follows_the_option() {
    assert_eq!(validate_code("zh-Hant", false), Err("invalid_format"));
    assert_eq!(validate_code("zh-Hant-TW", false), Err("invalid_format"));
    assert_eq!(validate_code("zh-TW", false), Ok(()));
    // unbekannter Skriptcode
    assert_eq!(validate_code("zh-Xxxx", true), Err("invalid_format"));
    assert_eq!(validate_code("zh-Hant-XX", true), Err("invalid_region"));
}

// spec: TECH-072
#[test]
fn further_format_errors() {
    for code in [
        "en-",
        "-en",
        "en--US",
        "en-US-x",
        "en US",
        "e",
        "en-U",
        "en-1234",
        "de-DE-1996-x",
    ] {
        assert!(!valid(code), "{code:?}");
    }
    // Region ohne gültigen ISO-3166-Code
    assert_eq!(validate_code("en-ZZ", true), Err("invalid_region"));
    assert_eq!(validate_code("de-DE", true), Ok(()));
}

// spec: TECH-072
#[test]
fn annotations_carry_validity_and_reason() {
    let html = r#"<head>
        <link rel="alternate" hreflang="EN-gb" href="/a">
        <link rel="alternate" hreflang="en-UK" href="/b">
        <link rel="alternate" hreflang="en_GB" href="/c">
        <link rel="alternate" hreflang="zh-Hant" href="/d">
        </head>"#;
    let base = url::Url::parse("https://site.test/").unwrap();
    let mut cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let (refs, _) = hreflang::extract(&Document::parse(html), &base, &cfg);
    let view: Vec<(bool, Option<&str>)> = refs
        .iter()
        .map(|r| (r.code_valid, r.invalid_reason.as_deref()))
        .collect();
    assert_eq!(
        view,
        [
            (true, None),
            (false, Some("invalid_region")),
            (false, Some("invalid_format")),
            (true, None)
        ]
    );
    assert_eq!(refs[0].code_raw, "EN-gb");
    assert_eq!(refs[0].code_norm, "en-gb");

    cfg.allow_script_subtag = false;
    let (refs, _) = hreflang::extract(&Document::parse(html), &base, &cfg);
    assert!(!refs[3].code_valid);
}

// spec: TECH-072
#[test]
fn tables_contain_the_documented_codes() {
    // Stichproben der eingecheckten Tabellen (Quelle: CLDR-Validitätsdaten, siehe resources/iso/gen_iso.py)
    for lang in ["en", "de", "fr", "zh", "pt", "ja", "uk", "ar"] {
        assert!(valid(lang), "{lang}");
    }
    for bad in ["uk-UK", "ja-JA", "en-EN", "de-AA"] {
        assert!(!valid(bad), "{bad}");
    }
    assert!(valid("en-US"));
    assert!(valid("en-GB"));
    assert!(!valid("en-EU"));
}
