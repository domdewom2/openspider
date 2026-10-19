use os_parse::pixel::{serp_profile, truncate_at, width_px};
// spec: ONPAGE-194
#[test]
fn profiles_and_word_truncation() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ui/fx-ui-serp/profiles.json"
    ))
    .unwrap();
    assert_eq!(
        width_px(fixture["wideTitle"].as_str().unwrap(), 20.0, false),
        fixture["titlePx"].as_u64().unwrap() as u32
    );
    let description = fixture["description"].as_str().unwrap();
    assert_eq!(
        width_px(description, 14.0, false),
        fixture["descriptionPx"].as_u64().unwrap() as u32
    );
    assert_eq!(truncate_at(description, 985, 14.0, false), None);
    assert!(truncate_at(description, 680, 14.0, false).is_some());
    assert!(width_px(&"W".repeat(60), 20.0, false) > 561);
    assert!(width_px(&"i".repeat(70), 20.0, false) < 561);
    let mut cfg = os_model::CrawlConfig::default();
    assert_eq!(width_px(&"W".repeat(70), 20.0, false), 1260);
    assert_eq!(truncate_at("Start", 561, 20.0, false), None);
    assert_eq!(truncate_at(&"W".repeat(70), 561, 20.0, false), Some(0));
    let text = "Über schöne Häuser und grüne Wälder";
    let cut = truncate_at(text, 200, 20.0, false).unwrap();
    let prefix: String = text.chars().take(cut as usize).collect();
    assert!(!prefix.is_empty());
    assert!(width_px(&format!("{prefix} …"), 20.0, false) <= 200);
    assert!(text[prefix.len()..].starts_with(' '));
    assert_eq!(serp_profile(&cfg, "mobile").1, 680);
    assert_eq!(serp_profile(&cfg, "tablet").1, 680);
    cfg.meta_description.max_px = 900;
    assert_eq!(serp_profile(&cfg, "desktop").1, 900);
    cfg.serp.mobile.description_max_px = Some(950);
    assert_eq!(serp_profile(&cfg, "mobile").1, 950);
    assert_eq!(serp_profile(&cfg, "desktop").1, 900);
}

// spec: ONPAGE-194
#[test]
fn truncation_keeps_words_and_unicode_scalars_at_exact_limit() {
    let limit = width_px("Über …", 20.0, false);
    let text = "Über schöne Häuser";
    assert_eq!(truncate_at(text, limit, 20.0, false), Some(4));
    assert_eq!(
        truncate_at("Über", width_px("Über", 20.0, false), 20.0, false),
        None
    );
    assert_eq!(truncate_at("", 0, 20.0, false), None);
    assert_eq!(truncate_at("W", 0, 20.0, false), Some(0));
}

// spec: ONPAGE-194
#[test]
fn measurement_updates_metrics_and_bounds_the_final_preview() {
    use os_parse::pixel::measure_serp;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ui/fx-ui-serp/profiles.json"
    ))
    .unwrap();
    let description = fixture["description"].as_str().unwrap();
    let mut cfg = os_model::CrawlConfig::default();
    for device in ["desktop", "mobile", "tablet"] {
        let (_, max, _, font) = serp_profile(&cfg, device);
        let m = measure_serp(description, max, font, false);
        assert_eq!(m.px, 900);
        assert_eq!(m.remaining, i64::from(max) - 900);
        assert_eq!(m.cut_at.is_some(), device != "desktop");
        assert!(width_px(&m.preview, font, false) <= max);
    }
    cfg.serp.mobile.description_max_px = Some(900);
    let (_, max, _, font) = serp_profile(&cfg, "mobile");
    assert_eq!(
        measure_serp(description, max, font, false).preview,
        description
    );
    let edited = measure_serp("Über 👨‍👩‍👧‍👦 Häuser und Bäume", 180, 20.0, false);
    assert_eq!(edited.chars, 29);
    assert!(edited.remaining < 0);
    assert!(width_px(&edited.preview, 20.0, false) <= 180);
    assert_eq!(measure_serp("", 680, 14.0, false).preview, "");
    assert_eq!(measure_serp("W", 1, 20.0, false).preview, "");
}
