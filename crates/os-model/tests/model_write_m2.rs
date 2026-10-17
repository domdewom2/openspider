//! Schreib- und Parsemodell ab M2 (M2-F01): `WriteBatch` und `ParsedPage` bleiben mit `..Default::default()` bauen.

use os_model::*;

// spec: OUT-226
#[test]
fn write_batch_default_is_empty_in_all_m2_parts() {
    let b = WriteBatch::default();
    assert!(b.ext.is_empty() && b.structured_data.is_empty() && b.sd_issues.is_empty());
    assert!(b.render.is_empty() && b.a11y.is_empty() && b.spelling.is_empty());
    assert!(b.custom_search.is_empty() && b.custom_extraction.is_empty() && b.custom_js.is_empty());
    assert!(b.forms.is_empty() && b.minhash.is_empty() && b.api_values.is_empty());
    // Bestehende Aufrufer bauen mit Struct-Update; das muss weiter kompilieren.
    let b2 = WriteBatch {
        meta: vec![("k".into(), "v".into())],
        ..WriteBatch::default()
    };
    assert_eq!(b2.meta.len(), 1);
}

// spec: OUT-226
#[test]
fn m2_write_parts_hold_the_fields_of_schema_v3() {
    let b = WriteBatch {
        ext: vec![ExtWrite {
            url: UrlId(1),
            key: ExtKey::parse("security.csp_present").unwrap(),
            value: Value::Bool(true),
        }],
        structured_data: vec![SdItemWrite {
            url_id: UrlId(1),
            format: "json-ld".into(),
            type_path: "Product".into(),
            json: "{}".into(),
        }],
        sd_issues: vec![SdIssueWrite {
            url_id: UrlId(1),
            item: Some(0),
            validation: "schema_org".into(),
            feature: None,
            severity: "error".into(),
            property: Some("name".into()),
            code: "missing".into(),
        }],
        render: vec![RenderWrite {
            url_id: UrlId(1),
            status: "ok".into(),
            html: Some(b"<html></html>".to_vec()),
            word_count: Some(3),
            timings: RenderTimings::default(),
            console: vec![ConsoleEntry {
                level: "error".into(),
                text: "x".into(),
                source: None,
                line: None,
                ..Default::default()
            }],
            blocked: vec![BlockedResource {
                url: "https://a.test/x.js".into(),
                reason: "robots".into(),
                status: None,
            }],
            network: vec![NetworkEntry {
                url: "https://a.test/".into(),
                status: 200,
                resource_type: "document".into(),
                bytes: 10,
                duration_ms: Some(5),
            }],
            screenshots: vec![ScreenshotTile {
                tile: 0,
                data: vec![1, 2],
                width: 10,
                height: 20,
            }],
            console_dropped: 0,
        }],
        a11y: vec![A11yWrite {
            url_id: UrlId(1),
            rules: vec![A11yRuleResult {
                rule_id: "image-alt".into(),
                status: "violation".into(),
                impact: Some("critical".into()),
            }],
            nodes: vec![A11yNode {
                rule_id: "image-alt".into(),
                target: "img".into(),
                snippet: "<img>".into(),
                summary: "alt fehlt".into(),
            }],
        }],
        spelling: vec![SpellingWrite {
            url_id: UrlId(1),
            kind: "spelling".into(),
            language: "de-DE".into(),
            section: "body".into(),
            text: "Fehlr".into(),
            detail: None,
            suggestion: Some("Fehler".into()),
            rule_id: None,
        }],
        custom_search: vec![CustomSearchWrite {
            url_id: UrlId(1),
            def_index: 0,
            count: 2,
            error: None,
        }],
        custom_extraction: vec![CustomExtractionWrite {
            url_id: UrlId(1),
            def_index: 0,
            number: 0,
            value: "9,99".into(),
            error: None,
        }],
        custom_js: vec![CustomJsWrite {
            url_id: UrlId(1),
            def_index: 0,
            number: 0,
            value: Some("\"x\"".into()),
            error: None,
        }],
        forms: vec![FormWrite {
            url_id: UrlId(1),
            idx: 0,
            action: Some("/login".into()),
            method: "post".into(),
        }],
        minhash: vec![(UrlId(1), vec![1, 2, 3])],
        api_values: vec![ApiValueWrite {
            url_id: Some(UrlId(1)),
            url: "https://a.test/".into(),
            source: "gsc".into(),
            metric: "clicks".into(),
            level: "url".into(),
            v_real: Some(12.0),
            v_text: None,
            fetched_at: Timestamp::default(),
        }],
        ..WriteBatch::default()
    };
    assert_eq!(b.clone(), b);
    assert_eq!(b.render[0].screenshots[0].data.len(), 2);
}

// spec: CRAWL-129
#[test]
fn memory_stage_has_stable_strings() {
    assert_eq!(MemoryStage::default(), MemoryStage::Normal);
    let all: Vec<_> = MemoryStage::ALL.iter().map(|s| s.as_str()).collect();
    assert_eq!(all, ["normal", "warning", "paused"]);
    assert_eq!(
        "paused".parse::<MemoryStage>().unwrap(),
        MemoryStage::Paused
    );
}

// spec: OUT-226
#[test]
fn parsed_page_m2_fields_default_to_empty_and_compare() {
    let p = ParsedPage::default();
    assert_eq!(
        p,
        ParsedPage {
            ..Default::default()
        }
    );
    let q = ParsedPage {
        forms: vec![FormRef {
            idx: 0,
            action: None,
            action_resolved: None,
            method: "get".into(),
        }],
        validation: Some(ValidationFacts::default()),
        custom_search: vec![SearchHit {
            def_index: 0,
            count: 1,
            error: None,
        }],
        custom_extraction: vec![ExtractedValue {
            def_index: 0,
            number: 0,
            value: "x".into(),
            error: None,
        }],
        minhash: Some(vec![1]),
        structured_data: vec![SdItem {
            format: "json-ld".into(),
            type_path: "Thing".into(),
            json: "{}".into(),
            parse_error: None,
        }],
        ..ParsedPage::default()
    };
    assert_ne!(p, q);
}
