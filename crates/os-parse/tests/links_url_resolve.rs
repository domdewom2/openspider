//! CRAWL-002: Auflösung relativer Links und `<base href>` gegen die Fixture `crawl-url-resolve`.

use os_model::{CrawlConfig, ExtractedLink, LinkType, PathType};
use os_parse::dom::Document;
use os_parse::{base, links, ParseConfig};

fn read(path: &str) -> String {
    let full = os_testsite::fixtures_root()
        .join("crawl-url-resolve")
        .join(path);
    String::from_utf8(std::fs::read(&full).unwrap_or_else(|e| panic!("{}: {e}", full.display())))
        .expect("Fixture ist UTF-8")
}

/// Basis-URL und Hyperlinks einer Fixture-Seite.
fn run(path: &str, url: &str) -> (url::Url, Vec<ExtractedLink>) {
    let doc = Document::parse(&read(path));
    let url = url::Url::parse(url).unwrap();
    let base_url = base::resolve_base(&doc, &url);
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let links = links::extract(&doc, &base_url, &cfg)
        .into_iter()
        .filter(|l| l.link_type == LinkType::Hyperlink)
        .collect();
    (base_url, links)
}

fn row(l: &ExtractedLink) -> (&str, Option<&str>) {
    (l.href_raw.as_str(), l.resolved.as_deref())
}

// spec: CRAWL-002
#[test]
fn relative_forms_resolve_against_document_url() {
    let (base_url, links) = run("dir/sub/page.html", "https://site.test/dir/sub/page.html");
    assert_eq!(base_url.as_str(), "https://site.test/dir/sub/page.html");

    use PathType::*;
    // (href_raw, resolved, Pfadtyp), in Dokumentreihenfolge
    let expected: Vec<(&str, Option<&str>, PathType)> = vec![
        // relative Pfade
        (
            "a.html",
            Some("https://site.test/dir/sub/a.html"),
            PathRelative,
        ),
        ("../b/", Some("https://site.test/dir/b/"), PathRelative),
        ("./c", Some("https://site.test/dir/sub/c"), PathRelative),
        ("/d", Some("https://site.test/d"), RootRelative),
        // protokollrelativ
        (
            "//cdn.test/x.js",
            Some("https://cdn.test/x.js"),
            ProtocolRelative,
        ),
        // reine Query
        (
            "?p=2",
            Some("https://site.test/dir/sub/page.html?p=2"),
            PathRelative,
        ),
        // `..` über die Wurzel hinaus bleibt bei `/`
        (
            "../../../../beyond",
            Some("https://site.test/beyond"),
            PathRelative,
        ),
        // Punktsegmente
        (
            "/dir/./x/../y",
            Some("https://site.test/dir/y"),
            RootRelative,
        ),
        // Entities werden vor dem Auflösen dekodiert
        ("a&b", Some("https://site.test/dir/sub/a&b"), PathRelative),
        (
            "x?a=1&b=2",
            Some("https://site.test/dir/sub/x?a=1&b=2"),
            PathRelative,
        ),
        // Leerraum um den href, Tab und Zeilenumbruch im href
        ("/spaces", Some("https://site.test/spaces"), RootRelative),
        (
            "/tab\ted/pa\nth",
            Some("https://site.test/tabed/path"),
            RootRelative,
        ),
        (
            "https://other.test/abs",
            Some("https://other.test/abs"),
            Absolute,
        ),
        // leerer href = Selbstlink, reines Fragment = Selbstlink mit Fragment
        (
            "",
            Some("https://site.test/dir/sub/page.html"),
            PathRelative,
        ),
        (
            "#abschnitt",
            Some("https://site.test/dir/sub/page.html#abschnitt"),
            PathRelative,
        ),
        // nicht crawlbare Schemata: kein aufgelöstes Ziel
        ("mailto:info@site.test", None, Absolute),
        ("tel:+4930123456", None, Absolute),
        ("javascript:void(0)", None, Absolute),
        ("data:text/html,x", None, Absolute),
        ("about:blank", None, Absolute),
        ("file:///etc/hosts", None, Absolute),
        // ungültige Ziele werden nicht aufgelöst
        ("hppts://site.test/x", None, Absolute),
        ("http://site test/", None, Absolute),
    ];
    let actual: Vec<(&str, Option<&str>, PathType)> = links
        .iter()
        .map(|l| (l.href_raw.as_str(), l.resolved.as_deref(), l.path_type))
        .collect();
    assert_eq!(actual, expected);
}

// spec: CRAWL-002
#[test]
fn unusable_schemes_stay_listed_but_unresolved() {
    // javascript: bleibt als Link erhalten (für die Zählung nicht crawlbarer Links, CRAWL-053).
    let (_, links) = run("dir/sub/page.html", "https://site.test/dir/sub/page.html");
    let js = links
        .iter()
        .find(|l| l.href_raw.starts_with("javascript:"))
        .expect("javascript:-Link bleibt erhalten");
    assert_eq!(js.resolved, None);
    assert_eq!(js.anchor.as_deref(), Some("javascript"));
}

// spec: CRAWL-002
#[test]
fn base_href_is_used_and_second_base_is_ignored() {
    let (base_url, links) = run("based.html", "https://site.test/based.html");
    assert_eq!(base_url.as_str(), "https://site.test/other/");
    let rows: Vec<_> = links.iter().map(row).collect();
    assert_eq!(
        rows,
        vec![
            ("rel.html", Some("https://site.test/other/rel.html")),
            ("/abs", Some("https://site.test/abs")),
            ("?q=1", Some("https://site.test/other/?q=1")),
            // leerer href zeigt auf die Basis
            ("", Some("https://site.test/other/")),
            ("https://other.test/x", Some("https://other.test/x")),
        ]
    );
}

// spec: CRAWL-002
#[test]
fn relative_base_href_resolves_against_document_url() {
    let (base_url, links) = run(
        "dir/base-relative.html",
        "https://site.test/dir/base-relative.html",
    );
    assert_eq!(base_url.as_str(), "https://site.test/dir/sub/");
    let rows: Vec<_> = links.iter().map(row).collect();
    assert_eq!(
        rows,
        vec![
            ("x.html", Some("https://site.test/dir/sub/x.html")),
            ("../up.html", Some("https://site.test/dir/up.html")),
        ]
    );
}

// spec: CRAWL-002
#[test]
fn base_outside_head_is_honoured_for_the_whole_document() {
    let (base_url, links) = run("base-body.html", "https://site.test/base-body.html");
    assert_eq!(base_url.as_str(), "https://site.test/body-base/");
    let rows: Vec<_> = links.iter().map(row).collect();
    // auch der Link vor dem <base>-Element nutzt die Basis (Browser-Verhalten); das zweite <base> zählt nicht
    assert_eq!(
        rows,
        vec![
            (
                "before.html",
                Some("https://site.test/body-base/before.html")
            ),
            ("after.html", Some("https://site.test/body-base/after.html")),
        ]
    );
}

// spec: CRAWL-002
#[test]
fn first_valid_base_wins() {
    let (base_url, links) = run("base-invalid.html", "https://site.test/base-invalid.html");
    assert_eq!(base_url.as_str(), "https://site.test/valid-second/");
    assert_eq!(
        row(&links[0]),
        ("x.html", Some("https://site.test/valid-second/x.html"))
    );
}

// spec: CRAWL-002
#[test]
fn base_in_template_is_ignored() {
    let (base_url, links) = run("base-template.html", "https://site.test/base-template.html");
    assert_eq!(base_url.as_str(), "https://site.test/base-template.html");
    assert_eq!(row(&links[0]), ("x.html", Some("https://site.test/x.html")));
}

// spec: CRAWL-002
#[test]
fn parse_html_reports_the_base_and_resolves_through_it() {
    let body = read("based.html");
    let url = url::Url::parse("https://site.test/based.html").unwrap();
    let cfg = ParseConfig::from_crawl_config(&CrawlConfig::default());
    let page = os_parse::parse_html(&os_parse::ParseInput {
        url: &url,
        body: body.as_bytes(),
        content_type: Some("text/html; charset=utf-8"),
        headers: &[],
        cfg: &cfg,
    });
    assert_eq!(page.base_url, "https://site.test/other/");
    assert_eq!(
        page.links[0].resolved.as_deref(),
        Some("https://site.test/other/rel.html")
    );
}
