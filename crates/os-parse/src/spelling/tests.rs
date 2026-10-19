//! Tests der Rechtschreib- und Grammatikprüfung gegen die Fixture `tests/fixtures/ui/fx-ui-spell` (M2-D19).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use os_model::{CrawlConfig, ParsedPage, SpellingMode, UrlId, Value};

use super::dictionary::{self, Resources};
use super::grammar::{self, GrammarBackend, Match};
use super::*;
use crate::{parse_html, ParseConfig, ParseInput};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-spell")
}

fn fixture_resources() -> Resources {
    Resources {
        dictionary_dirs: vec![fixture().join("dictionaries")],
        personal_dir: None,
    }
}

/// Wörterbuchverzeichnis der Fixture setzen (alle Tests setzen denselben Wert).
fn use_fixture_dictionaries() {
    dictionary::configure(fixture_resources());
}

/// Rechtschreibprüfung an, Sprachwahl mit den Defaults der Spec (`en` → `en-GB`, globale Sprache `en-GB`).
fn spec_cfg() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    cfg.spelling.enabled = true;
    cfg.spelling.default_language = "en-GB".into();
    cfg.spelling.default_variants = [
        ("en", "en-GB"),
        ("de", "de-DE"),
        ("pt", "pt-PT"),
        ("fa", "fa-IR"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    cfg
}

fn parse_body(body: &[u8], cfg: &CrawlConfig) -> ParsedPage {
    use_fixture_dictionaries();
    let url = url::Url::parse("https://site.test/page.html").unwrap();
    let cfg = ParseConfig::from_crawl_config(cfg);
    parse_html(&ParseInput {
        status: 200,
        url: &url,
        body,
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    })
}

fn parse(name: &str, cfg: &CrawlConfig) -> ParsedPage {
    let body = std::fs::read(fixture().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
    parse_body(&body, cfg)
}

fn ext(page: &ParsedPage, key: &str) -> Option<Value> {
    page.ext
        .iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| v.clone())
}

fn text(page: &ParsedPage, key: &str) -> String {
    match ext(page, key) {
        Some(Value::Text(t)) => t,
        other => panic!("{key}: {other:?}"),
    }
}

fn count(page: &ParsedPage, key: &str) -> Option<i64> {
    match ext(page, key) {
        Some(Value::Int(n)) => Some(n),
        Some(Value::Null) | None => None,
        other => panic!("{key}: {other:?}"),
    }
}

fn findings(page: &ParsedPage) -> Vec<Finding> {
    ext(page, KEY_FINDINGS)
        .map(|v| decode_findings(&v))
        .unwrap_or_default()
}

fn spelling_texts(page: &ParsedPage) -> Vec<String> {
    findings(page)
        .into_iter()
        .filter(|f| f.kind == KIND_SPELLING)
        .map(|f| f.text)
        .collect()
}

// --- ONPAGE-153: Sprache und Rechtschreibung ---------------------------------------------------------------

// spec: ONPAGE-153
#[test]
fn de_errors_reports_three_spelling_errors_in_title_description_and_body() {
    let page = parse("de-errors.html", &spec_cfg());
    assert_eq!(text(&page, KEY_LANGUAGE), "de-DE");
    assert_eq!(count(&page, KEY_SPELLING_ERRORS), Some(3));
    assert_eq!(count(&page, KEY_GRAMMAR_ERRORS), None);
    let found = findings(&page);
    let rows: Vec<(&str, &str)> = found
        .iter()
        .map(|f| (f.section.as_str(), f.text.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Page Title", "Kontackt"),
            ("Meta Description", "Beschreibng"),
            ("Body", "Rechtschreibfeler"),
        ]
    );
    assert!(found
        .iter()
        .all(|f| f.kind == "Spelling" && f.rule_id.is_none()));
    assert_eq!(
        found[0].suggestions.first().map(String::as_str),
        Some("Kontakt")
    );
}

// spec: ONPAGE-153
#[test]
fn the_same_wrong_word_three_times_counts_three() {
    let page = parse("de-repeat.html", &spec_cfg());
    assert_eq!(count(&page, KEY_SPELLING_ERRORS), Some(3));
    assert_eq!(spelling_texts(&page), ["Testfeler"; 3]);
}

// spec: ONPAGE-153
#[test]
fn english_without_region_uses_the_default_variant() {
    let gb = parse("en-colour.html", &spec_cfg());
    assert_eq!(text(&gb, KEY_LANGUAGE), "en-GB");
    assert_eq!(spelling_texts(&gb), ["color"]);
    let mut cfg = spec_cfg();
    cfg.spelling
        .default_variants
        .insert("en".into(), "en-US".into());
    let us = parse("en-colour.html", &cfg);
    assert_eq!(text(&us, KEY_LANGUAGE), "en-US");
    assert_eq!(spelling_texts(&us), ["colour"]);
}

// spec: ONPAGE-153
#[test]
fn language_codes_ignore_case_and_keep_the_region() {
    let cfg = spec_cfg();
    for (page, language) in [
        ("de-at.html", "de-AT"),
        ("pt.html", "pt-PT"),
        ("pt-br.html", "pt-BR"),
    ] {
        let parsed = parse(page, &cfg);
        assert_eq!(text(&parsed, KEY_LANGUAGE), language, "{page}");
        assert_eq!(count(&parsed, KEY_SPELLING_ERRORS), Some(0), "{page}");
    }
}

// spec: ONPAGE-153
#[test]
fn an_unsupported_language_is_marked_and_not_checked() {
    let mut cfg = spec_cfg();
    cfg.grammar.enabled = true;
    let page = parse("xx.html", &cfg);
    assert_eq!(text(&page, KEY_LANGUAGE), "xx (unsupported)");
    assert_eq!(count(&page, KEY_SPELLING_ERRORS), None);
    assert_eq!(count(&page, KEY_GRAMMAR_ERRORS), None);
    assert!(findings(&page).is_empty());
    for code in super::lang::WITHOUT_WORD_SPACES {
        let lang = super::lang::resolve(Some(code), &cfg.spelling);
        assert_eq!(lang.label(), format!("{code} (unsupported)"));
    }
}

// spec: ONPAGE-153
#[test]
fn without_lang_the_global_default_applies_and_manual_mode_ignores_lang() {
    let page = parse("no-lang.html", &spec_cfg());
    assert_eq!(text(&page, KEY_LANGUAGE), "en-GB");
    assert_eq!(count(&page, KEY_SPELLING_ERRORS), Some(0));
    assert_eq!(text(&page, KEY_LANG_SOURCE), "");
    let mut manual = spec_cfg();
    manual.spelling.mode = SpellingMode::Manual;
    manual.spelling.manual_language = "de-DE".into();
    let german = parse("en-colour.html", &manual);
    assert_eq!(text(&german, KEY_LANGUAGE), "de-DE");
    assert_eq!(text(&german, KEY_LANG_SOURCE), "en");
    assert!(spelling_texts(&german).contains(&"colour".to_string()));
}

// spec: ONPAGE-153
#[test]
fn xml_lang_counts_only_without_lang() {
    let cfg = spec_cfg();
    let xml = parse_body(
        b"<html xml:lang=\"de\"><body><main><p>Ein Haus</p></main></body></html>",
        &cfg,
    );
    assert_eq!(text(&xml, KEY_LANGUAGE), "de-DE");
    let both = parse_body(
        b"<html lang=\"en\" xml:lang=\"de\"><body><main><p>A house</p></main></body></html>",
        &cfg,
    );
    assert_eq!(text(&both, KEY_LANGUAGE), "en-GB");
    let empty = parse_body(
        b"<html lang=\"\"><body><main><p>A house</p></main></body></html>",
        &cfg,
    );
    assert_eq!(text(&empty, KEY_LANGUAGE), "en-GB");
}

// spec: ONPAGE-153
#[test]
fn digits_urls_emails_and_short_acronyms_are_not_checked() {
    let page = parse("tokens.html", &spec_cfg());
    assert_eq!(spelling_texts(&page), ["NASAKURS"]);
}

// spec: ONPAGE-153
#[test]
fn a_wrong_word_only_in_the_footer_is_not_reported() {
    let page = parse("footer-only.html", &spec_cfg());
    assert_eq!(count(&page, KEY_SPELLING_ERRORS), Some(0));
}

// spec: ONPAGE-153
#[test]
fn every_language_of_stages_1_and_2_has_a_smoke_test() {
    const STAGES: &[&str] = &[
        "de-DE", "de-AT", "de-CH", "en-GB", "en-US", "en-AU", "en-CA", "en-NZ", "en-ZA", "fr",
        "es", "it", "nl", "pt-PT", "pt-BR", "pl", "sv", "da", "ru", "ga", "ar", "ast", "be", "br",
        "ca", "gl", "el", "fa-AF", "fa-IR", "pt-AO", "pt-MZ", "ro", "sk", "sl", "tl", "ta", "uk",
    ];
    let cfg = spec_cfg();
    for code in STAGES {
        let page = parse(&format!("langs/{code}.html"), &cfg);
        assert_eq!(text(&page, KEY_LANGUAGE), *code);
        assert_eq!(count(&page, KEY_SPELLING_ERRORS), Some(1), "{code}");
        let found = findings(&page);
        let body = std::fs::read_to_string(fixture().join(format!("langs/{code}.html"))).unwrap();
        assert!(
            body.contains(&format!(" {}</p>", found[0].text)),
            "{code}: {found:?}"
        );
    }
    let files = std::fs::read_dir(fixture().join("langs")).unwrap().count();
    assert_eq!(files, STAGES.len());
}

// spec: ONPAGE-153
#[test]
fn bundled_english_dictionaries_cover_all_english_variants() {
    let none = Resources::default();
    for code in ["en-GB", "en-US", "en-AU", "en-CA", "en-NZ", "en-ZA"] {
        let dict = dictionary::find(code, &none).unwrap_or_else(|| panic!("{code}"));
        assert!(dict.check("house"), "{code}");
        assert!(!dict.check("hosue"), "{code}");
    }
    assert!(dictionary::find("de-DE", &none).is_none());
    assert!(dictionary::find("en-GB", &none).unwrap().check("colour"));
    assert!(!dictionary::find("en-GB", &none).unwrap().check("color"));
    assert!(dictionary::find("en-US", &none).unwrap().check("color"));
}

// spec: ONPAGE-153
#[test]
fn a_supported_language_without_dictionary_is_labelled_and_left_empty() {
    let mut cfg = spec_cfg();
    cfg.grammar.enabled = true;
    let input = Input {
        declared: Some("nb"),
        body: "Hei hei",
        ..Default::default()
    };
    let out = check(
        &input,
        &cfg.spelling,
        &cfg.grammar,
        &HashSet::new(),
        &Resources::default(),
    );
    assert_eq!(out.label, "nb (no dictionary)");
    assert_eq!(out.spelling, None);
    assert_eq!(out.grammar, Some(1));
}

// spec: ONPAGE-153
#[test]
fn switched_off_checks_leave_the_page_unchanged() {
    let page = parse("de-errors.html", &CrawlConfig::default());
    assert!(page.ext.iter().all(|(k, _)| k.namespace() != "spelling"));
}

// spec: ONPAGE-153
#[test]
fn tokens_and_words_carry_character_positions() {
    let tokens = tokens::tokenize("„Grüße“, Welt!\nzweiter Block");
    let cores: Vec<(&str, usize, bool)> = tokens
        .iter()
        .map(|t| (t.core, t.core_start, t.block_start))
        .collect();
    assert_eq!(
        cores,
        [
            ("Grüße", 1, true),
            ("Welt", 9, false),
            ("zweiter", 15, true),
            ("Block", 23, false)
        ]
    );
    assert!(tokens[1].ends_sentence());
    let t = &tokens::tokenize("and/or (z.B.) ISO9001")[..];
    assert_eq!(
        tokens::words(&t[0])
            .iter()
            .map(|w| (w.text, w.start))
            .collect::<Vec<_>>(),
        [("and", 0), ("or", 4)]
    );
    assert!(tokens::words(&t[1]).is_empty());
    assert!(tokens::words(&t[2]).is_empty());
}

// --- ONPAGE-154: Grammatik ------------------------------------------------------------------------------------

fn rules(text: &str, code: &str) -> Vec<(String, String, Vec<String>)> {
    grammar::builtin(text, code, &[])
        .into_iter()
        .map(|m| (m.rule_id, m.text, m.suggestions))
        .collect()
}

fn ids(text: &str, code: &str) -> Vec<String> {
    rules(text, code).into_iter().map(|(id, _, _)| id).collect()
}

// spec: ONPAGE-154
#[test]
fn word_repeat_reports_one_error_and_suggests_the_word() {
    assert_eq!(
        rules("Das ist ist ein Test.", "de-DE"),
        [(
            "WORD_REPEAT".into(),
            "ist ist".into(),
            vec!["ist".to_string()]
        )]
    );
    assert!(ids("Die Frau, die die Zeitung liest, lacht.", "de-DE").is_empty());
    assert!(ids("I knew that that was true and he had had enough.", "en-GB").is_empty());
}

// spec: ONPAGE-154
#[test]
fn lower_case_sentence_starts_are_reported() {
    assert_eq!(
        ids("das ist gut. heute regnet es.", "de-DE"),
        ["SENTENCE_START_LOWER", "SENTENCE_START_LOWER"]
    );
    assert!(ids("2026 war gut.", "de-DE").is_empty());
    assert!(ids("Das ist z.B. gut. Siehe bzw. lies das.", "de-DE").is_empty());
    assert!(ids("https://example.org ist gut.", "de-DE").is_empty());
}

// spec: ONPAGE-154
#[test]
fn exactly_two_identical_punctuation_marks_are_reported() {
    let found = rules("Wirklich?? Ja.. Na gut...", "de-DE");
    assert_eq!(
        found
            .iter()
            .map(|(id, text, _)| (id.as_str(), text.as_str()))
            .collect::<Vec<_>>(),
        [("DOUBLE_PUNCT", "??"), ("DOUBLE_PUNCT", "..")]
    );
    assert!(ids("Was?! Echt!?", "de-DE").is_empty());
}

// spec: ONPAGE-154
#[test]
fn space_before_punctuation_except_french() {
    assert_eq!(
        rules("Hallo , Welt", "de-DE"),
        [(
            "SPACE_BEFORE_PUNCT".into(),
            "Hallo ,".into(),
            vec!["Hallo,".to_string()]
        )]
    );
    assert!(ids("Bonjour !", "fr").is_empty());
    assert_eq!(ids("Bonjour , monde", "fr"), ["SPACE_BEFORE_PUNCT"]);
}

// spec: ONPAGE-154
#[test]
fn could_of_and_seid_langem_are_reported_with_suggestions() {
    assert_eq!(
        rules("You could of asked.", "en-GB"),
        [(
            "EN_COULD_OF".into(),
            "could of".into(),
            vec!["could have".to_string()]
        )]
    );
    assert_eq!(
        rules("Ich warte seid langem.", "de-DE"),
        [(
            "DE_SEIT_SEID".into(),
            "seid langem".into(),
            vec!["seit langem".to_string()]
        )]
    );
    assert!(ids("Ihr seid toll.", "de-DE").is_empty());
}

// spec: ONPAGE-154
#[test]
fn every_further_rule_finds_its_example_and_spares_the_correct_form() {
    // (Sprache, fehlerhafter Satz, Regel, Vorschlag, korrekter Satz)
    let cases = [
        (
            "de-DE",
            "Er sagte,dass es geht.",
            "SPACE_AFTER_COMMA",
            "sagte, dass",
            "Er sagte, dass es geht.",
        ),
        (
            "de-DE",
            "Das ist gut,.",
            "PUNCT_SEQUENCE",
            ".",
            "Das ist gut.",
        ),
        (
            "de-DE",
            "Das ist (gut.",
            "UNPAIRED_BRACKET",
            "",
            "Das ist (gut).",
        ),
        (
            "de-DE",
            "Das ist ( gut).",
            "SPACE_INSIDE_BRACKETS",
            "(gut).",
            "Das ist (gut).",
        ),
        (
            "en-GB",
            "It might of rained.",
            "EN_MIGHT_OF",
            "might have",
            "It might have rained.",
        ),
        (
            "en-GB",
            "This is a apple.",
            "EN_A_AN",
            "an apple",
            "This is an apple.",
        ),
        (
            "en-GB",
            "This is an car.",
            "EN_A_AN",
            "a car",
            "This is a university and an hour.",
        ),
        (
            "en-GB",
            "I think its a cat.",
            "EN_ITS_IT_IS",
            "it's",
            "The dog wagged its tail.",
        ),
        (
            "en-GB",
            "I think your going home.",
            "EN_YOUR_YOURE",
            "you're",
            "Your home is nice.",
        ),
        (
            "en-GB",
            "It is better then that.",
            "EN_THEN_THAN",
            "than",
            "It is better than that.",
        ),
        (
            "en-GB",
            "I like it alot.",
            "EN_ALOT",
            "a lot",
            "I like it a lot.",
        ),
        (
            "en-GB",
            "We go irregardless.",
            "EN_IRREGARDLESS",
            "regardless",
            "We go regardless.",
        ),
        (
            "en-GB",
            "It is infact true.",
            "EN_INFACT",
            "in fact",
            "It is in fact true.",
        ),
        (
            "en-GB",
            "I know their is a cat.",
            "EN_THEIR_THERE",
            "there",
            "There is their cat.",
        ),
        (
            "en-GB",
            "Tomorrow i will go.",
            "EN_I_LOWER",
            "I",
            "Tomorrow I will go.",
        ),
        (
            "en-GB",
            "I wonder whose going.",
            "EN_WHOSE_WHOS",
            "who's",
            "I wonder whose car it is.",
        ),
        (
            "en-GB",
            "You are suppose to go.",
            "EN_SUPPOSED_TO",
            "supposed",
            "You are supposed to go.",
        ),
        (
            "en-GB",
            "It is to late now.",
            "EN_TOO_TO",
            "too",
            "It is too late now.",
        ),
        (
            "en-GB",
            "It had a side affect.",
            "EN_AFFECT_EFFECT",
            "effect",
            "It had a side effect.",
        ),
        (
            "en-GB",
            "Do not loose it.",
            "EN_LOOSE_LOSE",
            "lose",
            "Do not lose it.",
        ),
        (
            "de-DE",
            "Es regnet, so das ich bleibe.",
            "DE_SO_DASS",
            "so dass",
            "Es regnet, so dass ich bleibe.",
        ),
        (
            "de-DE",
            "Ich glaube dass es geht.",
            "DE_KOMMA_DASS",
            "glaube, dass",
            "Ich glaube, dass es geht.",
        ),
        (
            "de-DE",
            "Das ist ein Wiederspruch.",
            "DE_WIDER",
            "Widerspruch",
            "Das ist ein Widerspruch.",
        ),
        (
            "de-DE",
            "Das ist Standart.",
            "DE_STANDARD",
            "Standard",
            "Das ist Standard.",
        ),
        (
            "de-DE",
            "Er kommt zumindestens.",
            "DE_ZUMINDEST",
            "zumindest",
            "Er kommt zumindest.",
        ),
        (
            "de-DE",
            "Die einzigste Lösung.",
            "DE_EINZIGSTE",
            "einzige",
            "Die einzige Lösung.",
        ),
        (
            "de-DE",
            "Er ist größer wie ich.",
            "DE_ALS_WIE",
            "als",
            "Er ist so groß wie ich.",
        ),
        (
            "de-DE",
            "Das passt in keinster Weise.",
            "DE_KEINSTER",
            "keiner",
            "Das passt in keiner Weise.",
        ),
        (
            "de-DE",
            "Desweiteren gilt das.",
            "DE_DES_WEITEREN",
            "Des Weiteren",
            "Des Weiteren gilt das.",
        ),
        (
            "de-DE",
            "Die Vorraussetzung fehlt.",
            "DE_VORAUS",
            "Voraussetzung",
            "Die Voraussetzung fehlt.",
        ),
        (
            "de-DE",
            "Das ist ansich gut.",
            "DE_AN_SICH",
            "an sich",
            "Das ist an sich gut.",
        ),
        (
            "de-DE",
            "Das geht garnicht.",
            "DE_GAR_NICHT",
            "gar nicht",
            "Das geht gar nicht.",
        ),
        (
            "de-DE",
            "Es regnet immernoch.",
            "DE_IMMER_NOCH",
            "immer noch",
            "Es regnet immer noch.",
        ),
        (
            "de-DE",
            "Ein bischen mehr.",
            "DE_BISSCHEN",
            "bisschen",
            "Ein bisschen mehr.",
        ),
        (
            "de-DE",
            "Weist du das?",
            "DE_WEISST",
            "Weißt",
            "Er weist darauf hin.",
        ),
        (
            "de-DE",
            "Das ist entgültig.",
            "DE_ENDGUELTIG",
            "endgültig",
            "Das ist endgültig.",
        ),
        (
            "de-DE",
            "Das ist wiederrum gut.",
            "DE_WIEDERUM",
            "wiederum",
            "Das ist wiederum gut.",
        ),
    ];
    let mut covered: HashSet<&str> = HashSet::new();
    for (code, wrong, rule, suggestion, right) in cases {
        let found = rules(wrong, code);
        assert_eq!(found.len(), 1, "{wrong}: {found:?}");
        assert_eq!(found[0].0, rule, "{wrong}");
        let expected: Vec<String> = Some(suggestion)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(found[0].2, expected, "{wrong}");
        assert!(
            ids(right, code).is_empty(),
            "{right}: {:?}",
            rules(right, code)
        );
        covered.insert(rule);
    }
    // Zusammen mit den Tests oben ist jede Regel der Regelmenge mit einem Beispiel belegt.
    for spec_rule in [
        "WORD_REPEAT",
        "SENTENCE_START_LOWER",
        "DOUBLE_PUNCT",
        "SPACE_BEFORE_PUNCT",
        "EN_COULD_OF",
        "DE_SEIT_SEID",
    ] {
        covered.insert(spec_rule);
    }
    let all: HashSet<&str> = grammar::RULES.iter().map(|r| r.id).collect();
    assert_eq!(covered, all);
}

// spec: ONPAGE-154
#[test]
fn the_rule_set_has_at_least_twenty_rules_per_language_with_id_category_and_description() {
    for lang in ["en", "de"] {
        assert!(grammar::rules_for(lang).len() >= 20, "{lang}");
    }
    let mut seen = HashSet::new();
    for r in grammar::RULES {
        assert!(seen.insert(r.id), "doppelte Kennung {}", r.id);
        assert!(
            !r.category.is_empty() && !r.description.is_empty(),
            "{}",
            r.id
        );
    }
}

// spec: ONPAGE-154
#[test]
fn disabled_rules_are_skipped() {
    let off = vec!["WORD_REPEAT".to_string()];
    assert!(grammar::builtin("Das ist ist ein Test.", "de-DE", &off).is_empty());
    assert!(grammar::check_with("Das ist ist ein Test.", "de-DE", &off, None).is_empty());
}

// spec: ONPAGE-154
#[test]
fn grammar_fixtures_count_their_errors_and_spelling_stays_separate() {
    let mut cfg = spec_cfg();
    cfg.spelling.enabled = false;
    cfg.grammar.enabled = true;
    for (page, n) in [
        ("grammar-de.html", 7),
        ("grammar-en.html", 1),
        ("grammar-fr.html", 0),
    ] {
        let parsed = parse(page, &cfg);
        assert_eq!(
            count(&parsed, KEY_GRAMMAR_ERRORS),
            Some(n),
            "{page}: {:?}",
            findings(&parsed)
        );
        assert_eq!(count(&parsed, KEY_SPELLING_ERRORS), None, "{page}");
    }
    let mut both = spec_cfg();
    both.grammar.enabled = true;
    let tesst = parse("tesst.html", &both);
    assert_eq!(count(&tesst, KEY_SPELLING_ERRORS), Some(2));
    assert_eq!(count(&tesst, KEY_GRAMMAR_ERRORS), Some(1));
    let grammar_rows: Vec<Finding> = findings(&tesst)
        .into_iter()
        .filter(|f| f.kind == "Grammar")
        .collect();
    assert_eq!(grammar_rows[0].rule_id.as_deref(), Some("WORD_REPEAT"));
    assert_eq!(grammar_rows[0].section, "Body");
}

struct Server(Result<Vec<Match>, String>);

impl GrammarBackend for Server {
    fn check(&self, _: &str, language: &str) -> Result<Vec<Match>, String> {
        assert_eq!(language, "de-DE");
        self.0.clone()
    }
}

// spec: ONPAGE-154
#[test]
fn an_external_server_replaces_the_rule_set_and_falls_back_on_errors() {
    let remote = Match {
        rule_id: "REMOTE_RULE".into(),
        category: "Remote".into(),
        description: "From the server.".into(),
        text: "Test".into(),
        start: 16,
        end: 20,
        suggestions: vec!["Tests".into()],
    };
    let ok = Server(Ok(vec![remote.clone()]));
    let text = "Das ist ist ein Test.";
    assert_eq!(grammar::check_with(text, "de-DE", &[], Some(&ok)), [remote]);
    assert!(grammar::check_with(text, "de-DE", &["REMOTE_RULE".into()], Some(&ok)).is_empty());
    let failing = Server(Err("connection refused".into()));
    let fallback = grammar::check_with(text, "de-DE", &[], Some(&failing));
    assert_eq!(fallback[0].rule_id, "WORD_REPEAT");
}

// --- ONPAGE-157: Ignorierliste, persönliches Wörterbuch, Befundzeilen -----------------------------------------

// spec: ONPAGE-157
#[test]
fn ignore_list_and_personal_dictionary_remove_the_words() {
    let cfg = spec_cfg();
    let dir = tempfile_dir();
    let resources = Resources {
        dictionary_dirs: vec![fixture().join("dictionaries")],
        personal_dir: Some(dir.clone()),
    };
    let input = Input {
        declared: Some("de"),
        title: Some("Kontackt"),
        description: Some("Beschreibng"),
        body: "Ein Rechtschreibfeler",
    };
    let run =
        |ignored: &HashSet<String>| check(&input, &cfg.spelling, &cfg.grammar, ignored, &resources);
    assert_eq!(run(&HashSet::new()).spelling, Some(3));
    let ignored: HashSet<String> = ["rechtschreibfeler".to_string()].into();
    assert_eq!(run(&ignored).spelling, Some(2));
    dictionary::add_personal_word("de-DE", "Kontackt", &resources).unwrap();
    dictionary::add_personal_word("de", "Beschreibng", &resources).unwrap();
    dictionary::add_personal_word("de-DE", "kontackt", &resources).unwrap();
    assert_eq!(run(&ignored).spelling, Some(0));
    let stored = std::fs::read_to_string(dir.join("de-DE.txt")).unwrap();
    assert_eq!(stored, "Kontackt\n");
    assert!(dictionary::add_personal_word("de-DE", "a\nb", &resources).is_err());
    assert!(dictionary::add_personal_word("de-DE", "x", &Resources::default()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

fn tempfile_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "os-parse-spelling-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

// spec: ONPAGE-157
#[test]
fn findings_travel_as_spelling_writes_with_type_section_detail_and_suggestion() {
    let mut cfg = spec_cfg();
    cfg.grammar.enabled = true;
    let mut page = parse("out-052.html", &cfg);
    let writes = take_findings(UrlId(7), &mut page.ext);
    assert!(ext(&page, KEY_FINDINGS).is_none());
    let rows: Vec<(&str, &str, &str, Option<&str>)> = writes
        .iter()
        .map(|w| {
            (
                w.kind.as_str(),
                w.section.as_str(),
                w.text.as_str(),
                w.rule_id.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Spelling", "Page Title", "Beispeil", None),
            ("Spelling", "Body", "Fehlerr", None),
            ("Grammar", "Body", "ist ist", Some("WORD_REPEAT")),
        ]
    );
    assert!(writes
        .iter()
        .all(|w| w.url_id == UrlId(7) && w.language == "de-DE"));
    assert_eq!(writes[0].suggestion.as_deref(), Some("Beispiel"));
    assert_eq!(writes[0].detail.as_deref(), Some(SPELLING_DETAIL));
    assert_eq!(writes[2].suggestion.as_deref(), Some("ist"));
    assert!(take_findings(UrlId(7), &mut page.ext).is_empty());
}

// --- ONPAGE-166: PDFs -----------------------------------------------------------------------------------------

// spec: ONPAGE-166
#[test]
fn a_stored_pdf_is_checked_in_its_catalog_language() {
    use_fixture_dictionaries();
    let body = std::fs::read(fixture().join("fehler.pdf")).unwrap();
    let mut crawl = spec_cfg();
    crawl.pdf.store = true;
    let cfg = ParseConfig::from_crawl_config(&crawl);
    let result = crate::pdf::parse(&body, &cfg);
    let value = |key: &str| {
        result
            .ext
            .iter()
            .find(|(k, _)| k.as_str() == key)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(value(KEY_LANGUAGE), Some(Value::Text("de-DE".into())));
    assert_eq!(value(KEY_SPELLING_ERRORS), Some(Value::Int(1)));
    let found = decode_findings(&value(KEY_FINDINGS).unwrap());
    assert_eq!(found[0].text, "Rechtschreibfeler");
    assert_eq!(found[0].section, "Body");
    crawl.pdf.store = false;
    let unstored = crate::pdf::parse(&body, &ParseConfig::from_crawl_config(&crawl));
    assert!(unstored
        .ext
        .iter()
        .all(|(k, _)| k.namespace() != "spelling"));
}
