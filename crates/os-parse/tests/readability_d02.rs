use os_model::Value;
use os_parse::readability::{facts, label, syllables};

// spec: ONPAGE-149
#[test]
fn syllables_sentences_and_scores_are_deterministic() {
    for (word, n) in [
        ("the", 1),
        ("cake", 1),
        ("table", 2),
        ("beautiful", 3),
        ("readability", 5),
        ("rhythm", 1),
        ("a", 1),
    ] {
        assert_eq!(syllables(word), n, "{word}");
    }
    let f = facts("The cat sat on the mat.");
    assert_eq!(f[0].1, Value::Int(1));
    assert_eq!(f[1].1, Value::Float(6.0));
    assert_eq!(f[2].1, Value::Float(100.0));
    assert_eq!(facts("Ohne Punkt\nNoch ein Satz")[0].1, Value::Int(2));
    assert_eq!(facts("Es kostet 3.5 Euro.")[0].1, Value::Int(1));
    assert!(facts("123 –").is_empty());
    assert_eq!(facts("Internationalization considerations necessitate comprehensive organizational restructuring.")[2].1,Value::Float(0.0));
}
// spec: ONPAGE-150
#[test]
fn difficult_band_includes_both_boundaries() {
    assert_eq!(label(30.0), "Difficult");
    assert_eq!(label(49.9), "Difficult");
    assert_ne!(label(50.0), "Difficult");
}
// spec: ONPAGE-151
#[test]
fn very_difficult_band_stops_at_thirty() {
    assert_eq!(label(0.0), "Very Difficult");
    assert_eq!(label(29.9), "Very Difficult");
    assert_ne!(label(30.0), "Very Difficult");
}
// spec: ONPAGE-149
#[test]
fn fixture_preserves_block_boundaries_and_excludes_footer() {
    let doc = os_parse::dom::Document::parse(include_str!(
        "../../../tests/fixtures/onpage-m2-content/area.html"
    ));
    let text = os_parse::content::area_text(&doc, &Default::default());
    assert!(!text.contains("navigation"));
    assert!(!text.contains("IPSUM"));
    assert_eq!(facts(&text)[0].1, Value::Int(4));
}
