//! Tests zu CRAWL-006 bis CRAWL-009 (Crawl-Bereich), Fixtures `crawl-scope-*` und die
//! eTLD+1-Unit-Tests aus CRAWL-008.

use os_model::ScopeConfig;
use os_url::{parse_start_url, registrable_domain, Scope, ScopeClass};

fn scope(start: &str, cfg: ScopeConfig) -> Scope {
    Scope::new(&parse_start_url(start).unwrap(), &cfg)
}

fn class(scope: &Scope, url: &str) -> ScopeClass {
    scope.classify(&parse_start_url(url).unwrap())
}

fn all_subdomains() -> ScopeConfig {
    ScopeConfig {
        all_subdomains: true,
        ..ScopeConfig::default()
    }
}

fn subfolder() -> ScopeConfig {
    ScopeConfig {
        subfolder_only: true,
        crawl_outside_start_folder: false,
        ..ScopeConfig::default()
    }
}

use ScopeClass::{External, Internal};

// spec: CRAWL-006
#[test]
fn scheme_change_and_other_port_stay_internal() {
    let s = scope("https://www.site.test/", ScopeConfig::default());
    assert_eq!(class(&s, "http://www.site.test/"), Internal);
    assert_eq!(class(&s, "https://www.site.test/a"), Internal);
    assert_eq!(class(&s, "https://www.site.test:8081/"), Internal);
    assert_eq!(class(&s, "http://www.site.test:8081/x"), Internal);
}

// spec: CRAWL-006
#[test]
fn other_hosts_are_external() {
    let s = scope("https://www.site.test/", ScopeConfig::default());
    assert_eq!(class(&s, "https://other.test/"), External);
    assert_eq!(class(&s, "https://site.test/"), External);
    assert_eq!(class(&s, "https://blog.site.test/"), External);
    assert_eq!(class(&s, "https://www.site.test.evil.test/"), External);
    // Nicht-Web-Schemata haben keinen Host
    assert_eq!(
        s.classify(&os_url::normalize(
            &url::Url::parse("mailto:a@site.test").unwrap()
        )),
        External
    );
}

// spec: CRAWL-006
#[test]
fn www_and_bare_domain_are_different_subdomains() {
    // Start auf `www`: die Stammdomain ist eine andere Subdomain und damit extern, und umgekehrt
    // (Start auf der Stammdomain greift die Ausnahme aus CRAWL-007, siehe unten).
    let s = scope("https://www.site.test/", ScopeConfig::default());
    assert_eq!(class(&s, "https://site.test/"), External);
    assert_eq!(class(&s, "https://www.site.test/"), Internal);
    let blog = scope("https://blog.site.test/", ScopeConfig::default());
    assert_eq!(class(&blog, "https://www.site.test/"), External);
    assert_eq!(class(&blog, "https://site.test/"), External);
}

// spec: CRAWL-007
#[test]
fn default_scope_is_exact_host_of_the_start_url() {
    let s = scope("https://www.site.test/", ScopeConfig::default());
    assert_eq!(class(&s, "https://www.site.test/x"), Internal);
    assert_eq!(class(&s, "https://WWW.SITE.TEST./x"), Internal);
    assert_eq!(class(&s, "https://blog.site.test/"), External);
    assert_eq!(class(&s, "https://shop.site.test/"), External);
}

// spec: CRAWL-007
#[test]
fn start_on_root_domain_treats_all_subdomains_as_internal() {
    let s = scope("https://site.test/", ScopeConfig::default());
    for u in [
        "https://site.test/",
        "https://www.site.test/",
        "https://blog.site.test/",
        "https://shop.site.test/x",
        "https://a.b.site.test/",
    ] {
        assert_eq!(class(&s, u), Internal, "{u}");
    }
    assert_eq!(class(&s, "https://other.test/"), External);
    assert_eq!(class(&s, "https://sitexyz.test/"), External);
    assert_eq!(class(&s, "https://notsite.test/"), External);
}

// spec: CRAWL-007
#[test]
fn start_on_www_does_not_get_the_root_exception() {
    let s = scope("https://www.site.test/", ScopeConfig::default());
    assert_eq!(class(&s, "https://blog.site.test/"), External);
}

// spec: CRAWL-007
#[test]
fn ips_and_single_label_hosts_match_exactly() {
    let ip = scope("http://127.0.0.1:8080/", ScopeConfig::default());
    assert_eq!(class(&ip, "http://127.0.0.1:9090/x"), Internal);
    assert_eq!(class(&ip, "http://127.0.0.2/"), External);
    let local = scope("http://localhost:3000/", all_subdomains());
    assert_eq!(class(&local, "http://localhost:3000/a"), Internal);
    assert_eq!(class(&local, "http://app.localhost:3000/a"), External);
    let v6 = scope("http://[::1]:3000/", ScopeConfig::default());
    assert_eq!(class(&v6, "http://[::1]:4000/"), Internal);
    assert_eq!(class(&v6, "http://[::2]/"), External);
}

// spec: CRAWL-008
#[test]
fn all_subdomains_share_the_registrable_domain() {
    let s = scope("https://www.site.test/", all_subdomains());
    for u in [
        "https://www.site.test/",
        "https://blog.site.test/",
        "https://shop.site.test/",
        "https://a.b.site.test/",
        "https://site.test/",
    ] {
        assert_eq!(class(&s, u), Internal, "{u}");
    }
    assert_eq!(class(&s, "https://other.test/"), External);
    assert_eq!(class(&s, "https://site.test.other.test/"), External);
}

// spec: CRAWL-008
#[test]
fn all_subdomains_uses_the_public_suffix_list() {
    let co_uk = scope("https://www.example.co.uk/", all_subdomains());
    assert_eq!(class(&co_uk, "https://foo.example.co.uk/"), Internal);
    assert_eq!(class(&co_uk, "https://other.co.uk/"), External);
    let gh = scope("https://user.github.io/", all_subdomains());
    assert_eq!(class(&gh, "https://other.github.io/"), External);
    assert_eq!(class(&gh, "https://user.github.io/x"), Internal);
    assert_eq!(class(&gh, "https://sub.user.github.io/x"), Internal);
}

// spec: CRAWL-008
#[test]
fn registrable_domain_unit_vectors() {
    assert_eq!(
        registrable_domain("www.example.co.uk").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(
        registrable_domain("example.co.uk").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(registrable_domain("co.uk"), None);
    assert_eq!(
        registrable_domain("user.github.io").as_deref(),
        Some("user.github.io")
    );
    assert_eq!(
        registrable_domain("a.b.user.github.io").as_deref(),
        Some("user.github.io")
    );
    assert_eq!(
        registrable_domain("www.example.com").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        registrable_domain("WWW.Example.COM.").as_deref(),
        Some("example.com")
    );
    // `.test` zählt als Suffix
    assert_eq!(
        registrable_domain("site.test").as_deref(),
        Some("site.test")
    );
    assert_eq!(
        registrable_domain("a.b.site.test").as_deref(),
        Some("site.test")
    );
    assert_eq!(registrable_domain("test"), None);
    // IDN
    assert_eq!(
        registrable_domain("www.müller.de").as_deref(),
        Some("xn--mller-kva.de")
    );
    // keine Domain
    assert_eq!(registrable_domain("localhost"), None);
    assert_eq!(registrable_domain("127.0.0.1"), None);
    assert_eq!(registrable_domain("[::1]"), None);
    assert_eq!(registrable_domain("1.2.3"), None);
    assert_eq!(registrable_domain(""), None);
    assert_eq!(registrable_domain("a..b.com"), None);
}

// spec: CRAWL-009
#[test]
fn subfolder_only_crawls_below_the_start_folder() {
    let s = scope("https://site.test/blog/", subfolder());
    assert_eq!(s.start_folder(), Some("/blog/"));
    assert_eq!(class(&s, "https://site.test/blog/a/"), Internal);
    assert_eq!(class(&s, "https://site.test/blog/b.html"), Internal);
    assert_eq!(class(&s, "https://site.test/blog/a/tief/"), Internal);
    assert_eq!(class(&s, "https://site.test/blog/"), Internal);
    assert_eq!(class(&s, "https://site.test/shop/"), External);
    assert_eq!(class(&s, "https://site.test/"), External);
    // Präfix-Ähnlichkeit zählt nicht
    assert_eq!(class(&s, "https://site.test/blogger/"), External);
    assert_eq!(class(&s, "https://site.test/blog"), External);
    // Groß-/Kleinschreibung des Pfads wird beachtet
    assert_eq!(class(&s, "https://site.test/Blog/a"), External);
    // Schema und Query ändern nichts
    assert_eq!(class(&s, "http://site.test/blog/a?x=/shop/"), Internal);
}

// spec: CRAWL-009
#[test]
fn start_folder_is_the_path_up_to_the_last_slash() {
    let s = scope("https://site.test/blog", subfolder());
    assert_eq!(s.start_folder(), Some("/"));
    assert_eq!(class(&s, "https://site.test/shop/"), Internal);
    let s = scope("https://site.test/blog/seite.html", subfolder());
    assert_eq!(s.start_folder(), Some("/blog/"));
    assert_eq!(class(&s, "https://site.test/blog/x"), Internal);
    assert_eq!(class(&s, "https://site.test/other/x"), External);
    let s = scope("https://site.test/blog/?x=1#y", subfolder());
    assert_eq!(s.start_folder(), Some("/blog/"));
    let s = scope("https://site.test", subfolder());
    assert_eq!(s.start_folder(), Some("/"));
    // ohne Ordnerbeschränkung gibt es keinen Startordner
    assert_eq!(
        scope("https://site.test/blog/", ScopeConfig::default()).start_folder(),
        None
    );
}

// spec: CRAWL-009
#[test]
fn subfolder_matches_on_the_encoded_path() {
    let s = scope("https://site.test/café/", subfolder());
    assert_eq!(s.start_folder(), Some("/caf%C3%A9/"));
    assert_eq!(class(&s, "https://site.test/caf%C3%A9/x"), Internal);
    assert_eq!(class(&s, "https://site.test/café/x"), Internal);
    assert_eq!(class(&s, "https://site.test/cafe/x"), External);
}

// spec: CRAWL-009
#[test]
fn crawl_outside_start_folder_lifts_the_folder_limit_but_not_the_host_limit() {
    let cfg = ScopeConfig {
        subfolder_only: true,
        crawl_outside_start_folder: true,
        ..ScopeConfig::default()
    };
    let s = scope("https://www.site.test/blog/", cfg);
    assert_eq!(s.start_folder(), None);
    assert_eq!(class(&s, "https://www.site.test/shop/"), Internal);
    assert_eq!(class(&s, "https://blog.site.test/shop/"), External);
}

// spec: CRAWL-009
#[test]
fn subfolder_combined_with_all_subdomains_applies_on_every_subdomain() {
    let cfg = ScopeConfig {
        all_subdomains: true,
        subfolder_only: true,
        crawl_outside_start_folder: false,
        ..ScopeConfig::default()
    };
    let s = scope("https://www.site.test/blog/", cfg);
    assert_eq!(class(&s, "https://www.site.test/blog/a"), Internal);
    assert_eq!(class(&s, "https://shop.site.test/blog/a"), Internal);
    assert_eq!(class(&s, "https://shop.site.test/shop/a"), External);
    assert_eq!(class(&s, "https://www.site.test/shop/a"), External);
    assert_eq!(class(&s, "https://other.test/blog/a"), External);
}
