//! Validierung (CRAWL-140): Wertebereiche, min > max, Regex-Syntax, Warnung statt Ablehnung.

use std::path::Path;

use os_model::*;

fn paths(issues: &[ConfigIssue]) -> Vec<&str> {
    issues.iter().map(|i| i.path.as_str()).collect()
}

fn fixture(rel: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}

// spec: CRAWL-140
#[test]
fn default_configuration_is_valid_without_warnings() {
    let rep = CrawlConfig::default().validate();
    assert!(rep.is_ok(), "{:?}", rep.errors);
    assert!(rep.warnings.is_empty(), "{:?}", rep.warnings);
    assert_eq!(rep.clone().into_result(), Ok(Vec::new()));
}

// spec: CRAWL-140
#[test]
fn thread_count_must_be_between_1_and_200() {
    for (threads, ok) in [(0, false), (1, true), (5, true), (200, true), (201, false)] {
        let mut c = CrawlConfig::default();
        c.speed.threads = threads;
        let rep = c.validate();
        assert_eq!(rep.is_ok(), ok, "threads = {threads}");
        if !ok {
            assert_eq!(paths(&rep.errors), ["speed.threads"]);
            assert!(
                rep.errors[0].message.contains("zwischen 1 und 200"),
                "{}",
                rep.errors[0]
            );
            assert!(rep.errors[0].message.contains(&threads.to_string()));
            assert_eq!(rep.errors[0].kind, ConfigIssueKind::InvalidValue);
        }
    }
    let mut c = CrawlConfig::default();
    c.speed.max_per_host = Some(0);
    assert_eq!(paths(&c.validate().errors), ["speed.max_per_host"]);
    c.speed.max_per_host = Some(4);
    assert!(c.validate().is_ok());
}

// spec: CRAWL-140
#[test]
fn rate_must_be_a_positive_finite_number() {
    for (rate, ok) in [
        (Some(0.5), true),
        (Some(1000.0), true),
        (None, true),
        (Some(0.0), false),
        (Some(-1.5), false),
        (Some(f64::NAN), false),
        (Some(f64::INFINITY), false),
    ] {
        let mut c = CrawlConfig::default();
        c.speed.max_urls_per_sec = rate;
        let rep = c.validate();
        assert_eq!(rep.is_ok(), ok, "{rate:?}");
        if !ok {
            assert_eq!(paths(&rep.errors), ["speed.max_urls_per_sec"]);
        }
    }
}

// spec: CRAWL-140
#[test]
fn counts_below_their_minimum_are_rejected() {
    let mut c = CrawlConfig::default();
    c.limits.max_urls = 0;
    c.limits.max_page_kb = 0;
    c.advanced.response_timeout_s = 0;
    c.url.max_chars = 0;
    c.image.max_kb = 0;
    c.validation.html_doc_max_bytes = 0;
    c.schema_version = 0;
    let rep = c.validate();
    let mut got = paths(&rep.errors);
    got.sort();
    assert_eq!(
        got,
        [
            "advanced.response_timeout_s",
            "image.max_kb",
            "limits.max_urls",
            "schema_version",
            "url.max_chars",
            "validation.html_doc_max_bytes",
        ]
    );
    assert!(rep.errors.iter().all(|e| e.message.contains("(ist 0)")));
    assert!(rep
        .errors
        .iter()
        .any(|e| e.message == "muss mindestens 1 sein (ist 0)"));

    // u64-Werte oberhalb der TOML-Ganzzahlgrenze sind nicht speicherbar.
    let mut c = CrawlConfig::default();
    c.limits.max_urls = u64::MAX;
    assert_eq!(paths(&c.validate().errors), ["limits.max_urls"]);
}

// spec: CRAWL-140
#[test]
fn near_duplicate_threshold_is_50_to_100() {
    for (pct, ok) in [
        (49, false),
        (50, true),
        (90, true),
        (100, true),
        (101, false),
    ] {
        let mut c = CrawlConfig::default();
        c.duplicates.near_threshold_pct = pct;
        assert_eq!(c.validate().is_ok(), ok, "{pct}");
    }
}

// spec: CRAWL-140
#[test]
fn min_greater_than_max_is_rejected() {
    let mut c = CrawlConfig::default();
    c.title.min_chars = 100;
    c.title.max_chars = 60;
    c.meta_description.min_px = 2000;
    let rep = c.validate();
    assert_eq!(
        paths(&rep.errors),
        ["title.min_chars", "meta_description.min_px"]
    );
    assert!(rep.errors[0].message.contains("title.max_chars"));
    assert!(rep.errors[0].message.contains("100 > 60"));

    // min == max ist erlaubt
    let mut c = CrawlConfig::default();
    c.title.min_chars = c.title.max_chars;
    c.title.min_px = c.title.max_px;
    assert!(c.validate().is_ok());
}

// spec: CRAWL-140
#[test]
fn regex_syntax_is_checked_with_fancy_regex() {
    let mut c = CrawlConfig::default();
    c.scope.include = vec![
        ".*/blog/.*".into(),
        "(offen".into(),
        r"^https://www\.example\.com/(?!privat/).*".into(), // Lookahead: nur mit fancy-regex gültig
    ];
    c.scope.exclude = vec!["[z-a]".into(), r"(\w+)\1".into()]; // Rückreferenz ist gültig
    let rep = c.validate();
    assert_eq!(paths(&rep.errors), ["scope.include[1]", "scope.exclude[0]"]);
    assert!(rep.errors[0].message.contains("regulärer Ausdruck"));

    // Gültig als Roh-Muster, aber nicht in der Vollübereinstimmungs-Form `^(?:…)$`.
    let mut c = CrawlConfig::default();
    c.scope.exclude = vec!["(?x) a #".into()];
    assert_eq!(paths(&c.validate().errors), ["scope.exclude[0]"]);
}

// spec: CRAWL-140
#[test]
fn resolve_entries_need_host_and_ip() {
    let mut c = CrawlConfig::default();
    c.network.resolve = vec![
        "*.site.test=127.0.0.1".into(),
        "a.test=127.0.0.1:8080".into(),
        "b.test=::1".into(),
        "c.test=[::1]:8080".into(),
    ];
    assert!(c.validate().is_ok(), "{:?}", c.validate().errors);
    c.network.resolve = vec![
        "kein-gleichheitszeichen".into(),
        "x.test=kein-ip".into(),
        "=127.0.0.1".into(),
        "bad host=127.0.0.1".into(),
        "a.test=127.0.0.1:99999".into(),
    ];
    assert_eq!(
        paths(&c.validate().errors),
        [
            "network.resolve[0]",
            "network.resolve[1]",
            "network.resolve[2]",
            "network.resolve[3]",
            "network.resolve[4]"
        ]
    );
}

// spec: CRAWL-132
#[test]
fn auth_entries_and_user_agent_are_checked() {
    let mut c = CrawlConfig::default();
    c.auth.basic = vec![BasicAuth {
        prefix: "https://a.test/".into(),
        username: " ".into(),
        secret_ref: "".into(),
    }];
    c.user_agent.http = Some("Bot\r\nX-Evil: 1".into());
    c.user_agent.robots_token = Some("zwei worte".into());
    c.user_agent.preset = "".into();
    let rep = c.validate();
    assert_eq!(
        paths(&rep.errors),
        [
            "auth.basic[0].username",
            "auth.basic[0].secret_ref",
            "user_agent.preset",
            "user_agent.http",
            "user_agent.robots_token"
        ]
    );
}

// spec: CRAWL-140
#[test]
fn oversized_validation_limit_is_accepted_with_warning() {
    // Standard: Grenzwert 2 MiB, Seitengröße 50 MB → keine Warnung.
    assert!(CrawlConfig::default().validate().warnings.is_empty());

    // html_doc_max_bytes 3 MiB bei Seitengrenze 1 MiB: angenommen, aber gewarnt.
    let mut c = CrawlConfig::default();
    c.limits.max_page_kb = 1024;
    c.validation.html_doc_max_bytes = 3 * 1024 * 1024;
    c.validation.resource_max_bytes = 1024 * 1024; // genau gleich: keine Warnung
    let rep = c.validate();
    assert!(rep.is_ok());
    assert_eq!(paths(&rep.warnings), ["validation.html_doc_max_bytes"]);
    assert_eq!(rep.warnings[0].kind, ConfigIssueKind::Warning);
    assert!(rep.warnings[0].message.starts_with(
        "Grenzwert kann nicht erkannt werden, wenn Seiten vorher abgeschnitten werden"
    ));
    assert_eq!(rep.clone().into_result().unwrap().len(), 1);
}

// spec: CRAWL-140
#[test]
fn empty_soft404_list_is_valid() {
    // Eine leere Phrasenliste schaltet den Filter ab (Hinweis in der Oberfläche), ist aber keine Fehlkonfiguration.
    let mut c = CrawlConfig::default();
    c.content.soft404_phrases.clear();
    let rep = c.validate();
    assert!(rep.is_ok() && rep.warnings.is_empty());
}

// spec: CRAWL-140, OUT-221
#[test]
fn invalid_fixture_reports_errors_with_line_numbers() {
    let loaded =
        CrawlConfig::load_file(&fixture("config/invalid/invalid-values.osconfig")).unwrap();
    let rep = loaded.validate();
    let got: Vec<_> = rep
        .errors
        .iter()
        .map(|e| (e.path.as_str(), e.line))
        .collect();
    assert_eq!(
        got,
        [
            ("speed.threads", Some(5)),
            ("speed.max_urls_per_sec", Some(6)),
            ("title.min_chars", Some(9)),
            ("scope.exclude[0]", Some(13)),
            ("network.resolve[0]", Some(16)),
        ]
    );
    let shown = rep.errors[0].to_string();
    assert!(
        shown.starts_with("speed.threads (Zeile 5, Spalte 1): "),
        "{shown}"
    );
    // ohne Datei keine Positionen
    assert!(loaded
        .config
        .validate()
        .errors
        .iter()
        .all(|e| e.line.is_none()));
}

// spec: CRAWL-140
#[test]
fn valid_fixtures_pass_validation() {
    for rel in [
        "config/default.osconfig",
        "config/commented.osconfig",
        "config/partial.osconfig",
        "config/unknown-fields.osconfig",
        "config/future-version.osconfig",
        "crawl-config/minimal.osconfig",
        "crawl-config/profile.osconfig",
    ] {
        let l = CrawlConfig::load_file(&fixture(rel)).unwrap();
        let rep = l.validate();
        assert!(rep.is_ok(), "{rel}: {:?}", rep.errors);
    }
}
