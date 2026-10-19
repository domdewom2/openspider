use os_model::{CrawlConfig, Value};
use os_parse::{pdf, ParseConfig};
const PDF: &[u8] = include_bytes!("../../../tests/fixtures/onpage-m2-content/text.pdf");
// spec: CRAWL-057
#[test]
fn annotations_keep_page_numbers_and_skip_relative_links() {
    let result = pdf::parse(
        PDF,
        &ParseConfig::from_crawl_config(&CrawlConfig::default()),
    );
    assert_eq!(result.links.len(), 1);
    assert_eq!(result.links[0].pdf_page, Some(1));
    assert_eq!(
        result.links[0].resolved.as_deref(),
        Some("https://site.test/next")
    );
    assert_eq!(result.meta.title.as_deref(), Some("Test document"));
    assert_eq!(result.word_count, None);
}
// spec: ONPAGE-166
#[test]
fn stored_text_has_word_count_and_flesch() {
    let mut cfg = CrawlConfig::default();
    cfg.pdf.store = true;
    let result = pdf::parse(PDF, &ParseConfig::from_crawl_config(&cfg));
    assert_eq!(result.word_count, Some(6));
    assert_eq!(
        result
            .ext
            .iter()
            .find(|(k, _)| k.as_str() == "readability.flesch")
            .unwrap()
            .1,
        Value::Float(100.0)
    );
    assert!(result.text.unwrap().contains("The cat"));
    assert!(pdf::parse(b"broken", &ParseConfig::from_crawl_config(&cfg))
        .error
        .is_some());
}

// spec: ONPAGE-166
#[test]
fn image_only_pdf_has_zero_words_and_no_readability() {
    let mut doc = lopdf::Document::load_mem(PDF).unwrap();
    for id in doc.get_pages().values() {
        doc.get_dictionary_mut(*id).unwrap().remove(b"Contents");
    }
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.pdf.store = true;
    let result = pdf::parse(&bytes, &ParseConfig::from_crawl_config(&cfg));
    assert_eq!(result.word_count, Some(0));
    assert_eq!(result.meta.title.as_deref(), Some("Test document"));
    assert_eq!(
        result.error.as_deref(),
        Some("PDF enthält keinen Textlayer")
    );
    assert!(result
        .ext
        .iter()
        .any(|(key, value)| key.as_str() == "content.pdf_error"
            && value == &Value::Text("PDF enthält keinen Textlayer".into())));
    assert!(!result
        .ext
        .iter()
        .any(|(key, _)| key.as_str().starts_with("readability.")));
    cfg.pdf.store = false;
    let result = pdf::parse(&bytes, &ParseConfig::from_crawl_config(&cfg));
    assert!(result.error.is_none());
    assert!(result.ext.is_empty());
    assert!(result.text.is_none());
}
