//! URL-Mapping zwischen den Crawls (OUT-146, führend; CRAWL-034, TECH-306). Fixture: `out-m2-compare/mapping` mit
//! `staging.site.test` (Trailing Slashes, Previous) und `www.site.test` (ohne, Current).

#[path = "compare_common.rs"]
mod common;

use common::*;
use os_analyze::compare::{
    mapping::validate_pattern, CompareClass, CompareError, CompareNote, Comparison, UrlMapping,
};
use os_model::CancelToken;

fn stg(path: &str) -> String {
    format!("http://staging.site.test{path}")
}

/// Host-Regel und Trailing-Slash-Regel (die Startseite behält ihren Schrägstrich).
fn staging_to_live() -> Vec<os_model::MappingRule> {
    vec![
        rule(r"^http://staging\.site\.test", "http://www.site.test"),
        rule(r"(\.test/.+)/$", "$1"),
    ]
}

// spec: OUT-146
// spec: CRAWL-034
// spec: TECH-306
#[tokio::test(flavor = "multi_thread")]
async fn staging_urls_are_treated_as_their_live_counterparts() {
    let (old, new) = (staging().await, live().await);
    let mut cmp = compare(&old, &new, &staging_to_live());
    let html = cmp.filter("Internal", "HTML").unwrap();
    // Alle Seiten haben einen Partner; nur die unterlegene Kollision /dup/ bleibt ohne.
    assert_eq!(html.class(CompareClass::New), Some(0));
    let missing = cmp
        .class_ids("Internal", "HTML", CompareClass::Missing)
        .unwrap();
    assert_eq!(addresses(&old.file, &missing), [stg("/dup/")]);
    let a = id_of(&new.file, &www("/a"));
    let partner = cmp.partner(os_analyze::compare::Side::Current, a).unwrap();
    assert_eq!(
        addresses(&old.file, &[partner as u32].into_iter().collect()),
        [stg("/a/")]
    );
}

// spec: OUT-146
// spec: TECH-306
#[tokio::test(flavor = "multi_thread")]
async fn back_references_carry_the_path_over() {
    let (old, new) = (staging().await, live().await);
    let rules = vec![
        rule(r"^http://staging\.site\.test/(.*)$", "http://www.site.test/$1"),
        rule(r"(\.test/.+)/$", "$1"),
    ];
    let mapping = UrlMapping::compile(&rules).unwrap();
    assert_eq!(mapping.apply(&stg("/a")).url, www("/a"));
    let mut cmp = compare(&old, &new, &rules);
    assert_eq!(
        cmp.filter("Internal", "HTML").unwrap().class(CompareClass::New),
        Some(0)
    );
}

// spec: OUT-146
// spec: CRAWL-034
#[test]
fn trailing_slash_rule_and_rule_order() {
    let slash = UrlMapping::compile(&[rule("/$", "")]).unwrap();
    assert_eq!(slash.apply(&www("/a/")).url, www("/a"));
    // Mehrere Regeln: jede auf das Ergebnis der vorigen.
    let chain = UrlMapping::compile(&[rule("/old/", "/mid/"), rule("/mid/", "/new/")]).unwrap();
    let trace = chain.trace(&www("/old/x"));
    assert_eq!(trace.output, www("/new/x"));
    assert_eq!(trace.steps[0].after, www("/mid/x"));
    assert_eq!(trace.steps[1].before, www("/mid/x"));
    assert!(trace.steps.iter().all(|s| s.matched));
    let reversed = UrlMapping::compile(&[rule("/mid/", "/new/"), rule("/old/", "/mid/")]).unwrap();
    assert_eq!(reversed.apply(&www("/old/x")).url, www("/mid/x"));
}

// spec: OUT-146
// spec: CRAWL-034
#[test]
fn search_semantics_and_lookaround() {
    let amp = UrlMapping::compile(&[rule("/amp/", "/")]).unwrap();
    assert_eq!(
        amp.apply("https://site.test/de/amp/a").url,
        "https://site.test/de/a"
    );
    let shop = UrlMapping::compile(&[rule(
        r"^https://staging\.site\.test(?=/shop/)",
        "https://www.site.test",
    )])
    .unwrap();
    assert_eq!(
        shop.apply("https://staging.site.test/shop/a").url,
        "https://www.site.test/shop/a"
    );
    let blog = shop.apply("https://staging.site.test/blog/b");
    assert_eq!(blog.url, "https://staging.site.test/blog/b");
    assert_eq!(blog.first_rule, None);
}

// spec: OUT-146
#[tokio::test(flavor = "multi_thread")]
async fn lookahead_maps_only_the_shop_in_the_fixture() {
    let (old, new) = (staging().await, live().await);
    let rules = vec![
        rule(r"^http://staging\.site\.test(?=/shop/)", "http://www.site.test"),
        rule(r"(\.test/.+)/$", "$1"),
    ];
    let mut cmp = compare(&old, &new, &rules);
    let new_ids = cmp.class_ids("Internal", "HTML", CompareClass::New).unwrap();
    let new_pages = addresses(&new.file, &new_ids);
    assert!(!new_pages.contains(&www("/shop/a")));
    assert!(new_pages.contains(&www("/blog/b")));
}

// spec: OUT-146
// Kollision: unverändert identische Previous-URL vor jeder gemappten; bei gleicher erster Regel die lexikographisch
// kleinste; Unterlegene bleiben ohne Partner, Warnung im Ergebnis.
#[tokio::test(flavor = "multi_thread")]
async fn collisions_follow_the_rule_order_and_are_reported() {
    let old = staging().await;
    // Previous = Current = Staging; `/dup/` → `/dup` kollidiert mit der unveränderten `/dup`.
    let cmp = compare(&old, &old, &[rule("/$", "")]);
    let collision = cmp
        .notes()
        .into_iter()
        .find_map(|n| match n {
            CompareNote::Collision(c) => Some(c),
            _ => None,
        })
        .expect("Kollision gemeldet");
    assert_eq!(collision.current_url, stg("/dup"));
    assert_eq!(collision.winner, stg("/dup"));
    assert_eq!(collision.losers, [stg("/dup/")]);
    let dup_slash = id_of(&old.file, &stg("/dup/"));
    assert_eq!(cmp.partner(os_analyze::compare::Side::Previous, dup_slash), None);

    // Gleiche erste Regel (Host-Regel trifft beide): die lexikographisch kleinste Previous-URL gewinnt.
    let new = live().await;
    let cmp = compare(&old, &new, &staging_to_live());
    let collision = cmp
        .pairing()
        .collisions
        .iter()
        .find(|c| c.current_url == www("/dup"))
        .expect("Kollision auf /dup");
    assert_eq!(collision.winner, stg("/dup"));
    assert_eq!(collision.losers, [stg("/dup/")]);
}

// spec: OUT-146
#[tokio::test(flavor = "multi_thread")]
async fn invalid_patterns_report_their_position_and_block_the_comparison() {
    let err = validate_pattern("ab(c").unwrap_err();
    assert_eq!(err.rule, 0);
    assert!(err.position.is_some());
    let err = UrlMapping::compile(&[rule("ok", ""), rule("x[", "")]).unwrap_err();
    assert_eq!(err.rule, 1);
    assert!(err.position.is_some_and(|p| p <= 2), "{err:?}");

    let (old, new) = (staging().await, live().await);
    let result = Comparison::open(
        side(&old),
        side(&new),
        &[rule("(", "")],
        &CancelToken::new(),
    );
    assert!(matches!(result, Err(CompareError::Mapping(e)) if e.rule == 0));
}

// spec: OUT-146
// Schrittlimit: die URL bleibt für die Regel unverändert, die Regel wird einmal als Warnung gemeldet.
#[test]
fn step_limit_leaves_the_url_unchanged_and_warns_once() {
    // Alternative unter Quantor plus Rückverweis zwingt die Backtracking-Engine (sonst linear delegiert).
    let mapping = UrlMapping::compile(&[rule(r"(a|a)*\1$", "x"), rule("^", "pre:")]).unwrap();
    let input = format!("https://site.test/{}b", "a".repeat(40));
    let trace = mapping.trace(&input);
    assert_eq!(trace.steps[0].after, input);
    assert!(trace.steps[0].error.is_some());
    assert_eq!(trace.output, format!("pre:{input}"));
    mapping.apply(&input);
    let warnings = mapping.take_warnings();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].rule, 0);
}

// spec: OUT-146
// spec: CRAWL-034
// Test-Reiter: URL nach jeder einzelnen Regel und das Endergebnis.
#[test]
fn the_test_tab_shows_each_step() {
    let mapping = UrlMapping::compile(&staging_to_live()).unwrap();
    let trace = mapping.trace(&stg("/shop/a/"));
    assert_eq!(trace.steps.len(), 2);
    assert_eq!(trace.steps[0].after, www("/shop/a/"));
    assert_eq!(trace.steps[1].after, www("/shop/a"));
    assert_eq!(trace.output, www("/shop/a"));
    let root = mapping.trace(&stg("/"));
    assert!(!root.steps[1].matched);
    assert_eq!(root.output, www("/"));
}
