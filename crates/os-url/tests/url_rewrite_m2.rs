use os_model::{CrawlConfig, RegexReplace, RewriteConfig};
use os_url::rewrite::apply_discovered;

// spec: CRAWL-103
#[test]
fn parameters_preserve_survivor_encoding_order_and_fragment() {
    // Fixture: crawl-rewrite.
    let cfg = RewriteConfig {
        remove_parameters: vec!["sid".into(), "utm_source".into(), "a b".into()],
        ..Default::default()
    };
    for line in
        include_str!("../../../tests/fixtures/onpage-m2-url/crawl-rewrite/rewrite.tsv").lines()
    {
        let (input, expected) = line.split_once('\t').unwrap();
        let actual = apply_discovered(&cfg, input);
        assert_eq!(actual.url, expected, "{input}");
        assert!(actual.warning.is_none());
    }
    let all = RewriteConfig {
        remove_all_parameters: true,
        ..Default::default()
    };
    assert_eq!(
        apply_discovered(&all, "https://site.test/a?#q=x").url,
        "https://site.test/a#q=x"
    );
}

fn rule(pattern: &str, replacement: &str) -> RegexReplace {
    RegexReplace {
        pattern: pattern.into(),
        replacement: replacement.into(),
    }
}

// spec: CRAWL-104
#[test]
fn regex_search_is_global_sequential_and_supports_captures() {
    let cfg = RewriteConfig {
        regex_replace: vec![
            rule("http://", "https://"),
            rule("(.*)/index\\.html$", "$1/"),
            rule("/x", "/y"),
        ],
        ..Default::default()
    };
    assert_eq!(
        apply_discovered(&cfg, "http://site.test/x/x/index.html").url,
        "https://site.test/y/y/"
    );
    let query = RewriteConfig {
        regex_replace: vec![rule("\\?.*", "")],
        ..Default::default()
    };
    assert_eq!(
        apply_discovered(&query, "https://site.test/y?foo=bar").url,
        "https://site.test/y"
    );
}

// spec: CRAWL-104
#[test]
fn invalid_results_patterns_and_backtracking_fall_back_with_warning() {
    for replacement in [
        rule(".*", "relative"),
        rule("https", "javascript"),
        rule("(", "x"),
    ] {
        let input = format!("https://site.test/{}!", "a".repeat(50));
        let cfg = RewriteConfig {
            regex_replace: vec![replacement],
            ..Default::default()
        };
        let result = apply_discovered(&cfg, &input);
        assert_eq!(result.url, input);
        assert!(result.warning.is_some());
    }
    let cfg = RewriteConfig {
        regex_replace: vec![rule(r"^https://site\.test/(a|a)*\1$", "x")],
        ..Default::default()
    };
    let input = format!("https://site.test/{}!", "a".repeat(50));
    assert!(apply_discovered(&cfg, &input).warning.is_some());
}

// spec: CRAWL-104
#[test]
fn validation_rejects_every_invalid_rule_with_field_path() {
    let mut cfg = CrawlConfig::default();
    cfg.scope.include = vec!["(".into(), "[".into()];
    cfg.scope.exclude = vec!["(".into()];
    cfg.rewrite.regex_replace = vec![rule("(", "")];
    let issues = os_url::rules::validate(&cfg);
    assert_eq!(issues.len(), 4);
    assert_eq!(issues[3].path, "rewrite.regex_replace[0].pattern");
    cfg.rewrite.regex_replace = vec![rule("http://", "https://")];
    cfg.scope.include.clear();
    cfg.scope.exclude.clear();
    assert!(os_url::rules::validate(&cfg).is_empty());
}

// spec: CRAWL-108
#[test]
fn cdns_extend_scope_by_exact_host_and_case_sensitive_prefix() {
    // Fixture: crawl-cdn.
    let start = os_url::parse_start_url("https://site.test/blog/").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.scope.subfolder_only = true;
    cfg.cdns = vec!["cdn.other.test".into(), "other.test/assets/".into()];
    let scope = os_url::Scope::new(&start, &cfg.scope).with_cdns(&cfg.cdns);
    for line in include_str!("../../../tests/fixtures/onpage-m2-url/crawl-cdn/cdns.tsv").lines() {
        let (url, class) = line.split_once('\t').unwrap();
        assert_eq!(
            scope.classify(&os_url::parse_start_url(url).unwrap()) == os_url::ScopeClass::Internal,
            class == "internal",
            "{url}"
        );
    }
    cfg.cdns = vec![
        "https://other.test".into(),
        "".into(),
        "other.test?a".into(),
    ];
    assert_eq!(os_url::rules::validate(&cfg).len(), 3);
}

// spec: CRAWL-108
#[test]
fn cdn_validation_explains_ports_including_explicit_default_ports() {
    let start = os_url::parse_start_url("https://site.test/").unwrap();
    for cdn in [
        "other.test:8080/assets/",
        "other.test:443/assets/",
        "other.test:80",
        "[2001:db8::1]:443/assets/",
    ] {
        let cfg = CrawlConfig {
            cdns: vec![cdn.into()],
            ..Default::default()
        };
        let issues = os_url::rules::validate(&cfg);
        assert_eq!(issues.len(), 1, "{cdn}");
        assert_eq!(issues[0].path, "cdns[0]");
        assert!(issues[0].message.contains("Port nicht erlaubt"), "{cdn}");
        let scope = os_url::Scope::new(&start, &cfg.scope).with_cdns(&cfg.cdns);
        let url = os_url::parse_start_url(&format!("https://{cdn}")).unwrap();
        assert_eq!(scope.classify(&url), os_url::ScopeClass::External, "{cdn}");
    }
    let cfg = CrawlConfig {
        cdns: vec!["other.test/assets/v1:443/".into()],
        ..Default::default()
    };
    assert!(os_url::rules::validate(&cfg).is_empty());
}

// RuleSet-Trace für M2-U13 (CRAWL-111).
#[test]
fn rule_trace_reports_all_matching_lines_and_compile_errors() {
    let mut input = os_url::rules::RuleTraceInput {
        include: vec!["".into(), ".*/a".into(), "(".into()],
        exclude: vec![".*/a".into()],
        ..Default::default()
    };
    let trace = os_url::rules::trace(&input, "https://site.test/a");
    assert_eq!(trace.matched, ["Include[1]: .*/a", "Exclude[0]: .*/a"]);
    assert_eq!(trace.warnings.len(), 1);
    input.include = vec!["/a".into()];
    input.exclude.clear();
    assert!(os_url::rules::trace(&input, "https://site.test/a")
        .matched
        .is_empty());
    input.partial_match = true;
    assert_eq!(
        os_url::rules::trace(&input, "https://site.test/a")
            .matched
            .len(),
        1
    );
}
