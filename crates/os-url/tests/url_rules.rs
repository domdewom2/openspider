//! Tests zu CRAWL-109 (Include) und CRAWL-110 (Exclude), Fixture `crawl-include-exclude`.
//! Das Pause-Kriterium von CRAWL-110 (Änderung während einer Pause) liegt in der Engine
//! `[M1-P19]`; hier wird nur das Entscheidungsverhalten geprüft.

use os_url::{RuleDecision, RuleError, RuleKind, RuleSet};

fn lines(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn rules(include: &[&str], exclude: &[&str], partial: bool) -> RuleSet {
    RuleSet::compile(&lines(include), &lines(exclude), partial).unwrap()
}

fn allowed(rules: &RuleSet, url: &str) -> bool {
    rules.decide(url) == RuleDecision::Allowed
}

const BASE: &str = "https://site.test";

fn u(path: &str) -> String {
    format!("{BASE}{path}")
}

// spec: CRAWL-109
#[test]
fn include_requires_full_match() {
    let r = rules(&[".*/blog/.*"], &[], false);
    assert!(allowed(&r, &u("/blog/a")));
    assert!(allowed(&r, &u("/blog/a?x=1")));
    assert!(allowed(&r, &u("/blog/b")));
    assert_eq!(r.decide(&u("/shop/x")), RuleDecision::NotIncluded);
    assert_eq!(r.decide(&u("/")), RuleDecision::NotIncluded);
    // Ressourcen sind ebenfalls betroffen
    assert_eq!(r.decide(&u("/img/a.png")), RuleDecision::NotIncluded);

    // Teilstück ohne `.*` passt auf keine URL
    let r = rules(&["/blog/"], &[], false);
    assert_eq!(r.decide(&u("/blog/a")), RuleDecision::NotIncluded);

    // Vollständiges Muster passt nur auf genau diese URL
    let r = rules(&[r"https://site\.test/blog/a"], &[], false);
    assert!(allowed(&r, &u("/blog/a")));
    assert_eq!(r.decide(&u("/blog/a?x=1")), RuleDecision::NotIncluded);
}

// spec: CRAWL-109
#[test]
fn include_partial_match_switch() {
    let r = rules(&["/blog/"], &[], true);
    assert!(allowed(&r, &u("/blog/a")));
    assert!(allowed(&r, &u("/blog/a?x=1")));
    assert_eq!(r.decide(&u("/shop/x")), RuleDecision::NotIncluded);
}

// spec: CRAWL-109
#[test]
fn include_is_case_sensitive_unless_flagged_and_ignores_empty_lines() {
    let r = rules(&["", ".*/blog/.*", "  "], &[], false);
    assert!(allowed(&r, &u("/blog/a")));
    assert_eq!(r.decide(&u("/Blog/c")), RuleDecision::NotIncluded);
    let r = rules(&["(?i).*/blog/.*"], &[], false);
    assert!(allowed(&r, &u("/Blog/c")));
    // nur leere Zeilen: keine Einschränkung
    let r = rules(&["", "\r"], &[], false);
    assert!(allowed(&r, &u("/irgendwas")));
    assert!(r.hits().is_empty());
    // Windows-Zeilenende am Zeilenende stört nicht
    let r = rules(&[".*/blog/.*\r"], &[], false);
    assert!(allowed(&r, &u("/blog/a")));
}

// spec: CRAWL-109
#[test]
fn include_matches_the_encoded_url() {
    let r = rules(&[".*caf%C3%A9.*"], &[], false);
    assert!(allowed(&r, &u("/caf%C3%A9")));
    let r = rules(&[".*café.*"], &[], false);
    assert_eq!(r.decide(&u("/caf%C3%A9")), RuleDecision::NotIncluded);
}

// spec: CRAWL-109
#[test]
fn include_hit_counter_per_line_reports_lines_without_hits() {
    let r = rules(&[".*/blog/.*", "", "/blog/", ".*/shop/.*"], &[], false);
    for p in [
        "/blog/a",
        "/blog/a?x=1",
        "/blog/b",
        "/shop/x",
        "/Blog/c",
        "/",
    ] {
        let _ = r.decide(&u(p));
    }
    assert_eq!(
        r.hits(),
        vec![
            (".*/blog/.*".to_string(), 3),
            ("/blog/".to_string(), 0),
            (".*/shop/.*".to_string(), 1),
        ]
    );
    let detailed = r.include_hits();
    // Zeilennummer zählt leere Zeilen mit: Zeile 2 (0-basiert) ist `/blog/` ohne Treffer
    assert_eq!(detailed[1].line, 2);
    assert_eq!(detailed[1].hits, 0);
    assert_eq!(detailed[2].line, 3);
}

// spec: CRAWL-109
#[test]
fn include_counts_every_matching_line() {
    let r = rules(&[".*/blog/.*", ".*a.*"], &[], false);
    let _ = r.decide(&u("/blog/a"));
    assert_eq!(r.hits().iter().map(|h| h.1).collect::<Vec<_>>(), vec![1, 1]);
}

// spec: CRAWL-109
#[test]
fn invalid_regex_is_reported_with_line_and_list() {
    let err = RuleSet::compile(&lines(&["ok.*", "(unclosed"]), &[], false).unwrap_err();
    let RuleError::Invalid { line, message } = err;
    assert_eq!(line, 1);
    assert!(message.starts_with("Include: "), "{message}");
    let err = RuleSet::compile(&[], &lines(&["", "[a-"]), false).unwrap_err();
    let RuleError::Invalid { line, message } = err;
    assert_eq!(line, 1);
    assert!(message.starts_with("Exclude: "), "{message}");
    // Klammern dürfen nicht durch die Umhüllung ausgeglichen werden
    assert!(RuleSet::compile(&lines(&["a)(b"]), &[], false).is_err());
    assert!(RuleSet::compile(&lines(&["a)(b"]), &[], true).is_err());
}

// spec: CRAWL-109
#[test]
fn regex_dialect_supports_lookaround_and_backreferences() {
    let r = rules(
        &[r"(?=.*blog)(?!.*draft).*", r"https://site\.test/(a)/\1"],
        &[],
        false,
    );
    assert!(allowed(&r, &u("/blog/a")));
    assert!(allowed(&r, &u("/a/a")));
    assert_eq!(r.decide(&u("/blog/draft")), RuleDecision::NotIncluded);
}

// spec: CRAWL-110
#[test]
fn exclude_examples_with_full_match() {
    let r = rules(&[], &[r".*\.pdf"], false);
    assert_eq!(r.decide(&u("/handbuch.pdf")), RuleDecision::Excluded(0));
    assert!(allowed(&r, &u("/handbuch.pdf?v=2")));

    let r = rules(&[], &[r".*[?&]sortierung=.*"], false);
    assert_eq!(
        r.decide(&u("/liste?sortierung=preis")),
        RuleDecision::Excluded(0)
    );
    assert_eq!(
        r.decide(&u("/liste?seite=2&sortierung=preis")),
        RuleDecision::Excluded(0)
    );
    assert!(allowed(&r, &u("/liste?seite=2")));

    let r = rules(&[], &[r".*/artikel/[0-9]{4}/.*"], false);
    assert_eq!(
        r.decide(&u("/artikel/2024/titel")),
        RuleDecision::Excluded(0)
    );
    assert!(allowed(&r, &u("/artikel/24/titel")));

    let r = rules(&[], &[r"https://site\.test/kalender/.*"], false);
    assert_eq!(r.decide(&u("/kalender/2025-01")), RuleDecision::Excluded(0));
    assert!(allowed(&r, "https://blog.site.test/kalender/x"));
}

// spec: CRAWL-110
#[test]
fn exclude_matches_the_encoded_url() {
    let r = rules(&[], &[".*%20.*", ".*%C3%BC.*"], false);
    assert_eq!(r.decide(&u("/a%20b")), RuleDecision::Excluded(0));
    assert_eq!(r.decide(&u("/%C3%BCber")), RuleDecision::Excluded(1));
    let r = rules(&[], &[".* .*", ".*ü.*"], false);
    assert!(allowed(&r, &u("/a%20b")));
    assert!(allowed(&r, &u("/%C3%BCber")));
}

// spec: CRAWL-110
#[test]
fn exclude_without_dot_star_needs_partial_switch() {
    let r = rules(&[], &[r"\.pdf"], false);
    assert!(allowed(&r, &u("/handbuch.pdf")));
    let r = rules(&[], &[r"\.pdf"], true);
    assert_eq!(r.decide(&u("/handbuch.pdf")), RuleDecision::Excluded(0));
    assert_eq!(r.decide(&u("/handbuch.pdf?v=2")), RuleDecision::Excluded(0));
}

// spec: CRAWL-110
#[test]
fn exclude_wins_over_include_and_reports_the_first_matching_line() {
    let r = rules(
        &[".*/blog/.*"],
        &[".*nothing.*", "", ".*draft.*", ".*/blog/x.*"],
        false,
    );
    assert_eq!(r.decide(&u("/blog/draft")), RuleDecision::Excluded(2));
    assert_eq!(r.decide(&u("/blog/x")), RuleDecision::Excluded(3));
    assert!(allowed(&r, &u("/blog/ok")));
    // Excluded zählt nicht als Include-Treffer
    assert_eq!(r.hits()[0].1, 1);
    // Exclude gilt auch ohne Include-Liste
    let r = rules(&[], &[".*draft.*"], false);
    assert_eq!(r.decide(&u("/draft")), RuleDecision::Excluded(0));
    assert_eq!(r.exclude_hits(), vec![(0, 1)]);
}

// spec: CRAWL-110
#[test]
fn exclude_is_case_sensitive_and_empty_rules_allow_everything() {
    let r = rules(&[], &[".*/privat/.*"], false);
    assert!(allowed(&r, &u("/Privat/a")));
    let r = rules(&[], &[], false);
    assert!(allowed(&r, &u("/x")));
    assert!(r.take_warnings().is_empty());
}

// spec: CRAWL-109, CRAWL-110
#[test]
fn step_limit_gives_no_match_and_one_warning() {
    // Katastrophales Backtracking: Alternative unter Quantor plus Rückverweis zwingt die
    // Backtracking-Engine (kein Delegieren an die lineare Engine).
    let evil = r"^(a|a)*\1$";
    let subject = format!("{}b", "a".repeat(40));
    let started = std::time::Instant::now();

    // Include: Abgleich bricht ab → kein Treffer → URL nicht eingeschlossen
    let r = rules(&[evil, "zzz"], &[], true);
    assert_eq!(r.decide(&subject), RuleDecision::NotIncluded);
    assert_eq!(r.decide(&subject), RuleDecision::NotIncluded);
    let warnings = r.take_warnings();
    assert_eq!(
        warnings.len(),
        1,
        "einmalige Warnung je Regel: {warnings:?}"
    );
    assert_eq!(warnings[0].kind, RuleKind::Include);
    assert_eq!(warnings[0].line, 0);
    assert_eq!(warnings[0].pattern, evil);
    assert!(warnings[0].message.contains("aufwendig"));
    assert!(r.take_warnings().is_empty(), "Warnungen werden geleert");
    // später treffende URLs funktionieren weiter
    assert!(allowed(&r, "https://site.test/zzz"));

    // Exclude: Abbruch → kein Treffer → URL nicht ausgeschlossen
    let r = rules(&[], &[evil], true);
    assert_eq!(r.decide(&subject), RuleDecision::Allowed);
    let warnings = r.take_warnings();
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].kind, RuleKind::Exclude);
    assert!(
        started.elapsed().as_secs() < 30,
        "Schrittlimit muss Hänger verhindern"
    );
}

#[test]
fn rule_set_is_shareable_and_cloneable() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RuleSet>();
    let r = rules(&[".*a.*"], &[], false);
    let _ = r.decide("a");
    let copy = r.clone();
    assert_eq!(copy.hits(), r.hits());
    let _ = copy.decide("a");
    assert_eq!(copy.hits()[0].1, 2);
    assert_eq!(r.hits()[0].1, 1);
}
