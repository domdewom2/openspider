//! Tab „Security“ (M1-P20, ONPAGE-031, ONPAGE-032): echte Crawls der Fixture `onpage-security` über HTTP und
//! HTTPS (F-TLS, Test-CA der Testsite). Die Hosts `site.test` und `other.test` löst `network.resolve` auf die
//! Testsite auf; HTTP und HTTPS laufen am selben Port.

use std::path::PathBuf;

use os_analyze::tabs;
use os_analyze::views::{self, ColumnSelection, ViewQuery};
use os_analyze::SortKey;
use os_engine::testing::{crawl_fixture_from, FixtureCrawl};
use os_model::{CrawlConfig, Value};
use os_testsite::golden;

// ---------------------------------------------------------------------------------------------------
// Hilfen

fn view(fx: &FixtureCrawl, filter: &str, cols: ColumnSelection) -> views::ViewPage {
    let mut q = ViewQuery::new("Security", filter);
    q.columns = cols;
    q.sort = vec![SortKey {
        column: "Address".to_string(),
        descending: false,
    }];
    q.limit = 500;
    views::query_view(&fx.file, &CrawlConfig::default(), &q).expect("Ansicht")
}

/// Adressen eines Filters, nach Address sortiert.
fn addresses(fx: &FixtureCrawl, filter: &str) -> Vec<String> {
    view(fx, filter, ColumnSelection::Named(vec!["Address".into()]))
        .rows
        .iter()
        .map(|r| match &r[0] {
            Value::Text(s) => s.clone(),
            other => panic!("Address ist kein Text: {other:?}"),
        })
        .collect()
}

fn golden_path(dir: &str, file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/onpage-security")
        .join(dir)
        .join(file)
}

/// Adressliste gegen die Golden-Datei (die Adressen enthalten keinen Port: Hostnamen unter `.test`).
fn assert_address_golden(fx: &FixtureCrawl, filter: &str, dir: &str, file: &str) {
    let mut text = String::from("Address\n");
    for a in addresses(fx, filter) {
        text.push_str(&a);
        text.push('\n');
    }
    golden::assert_golden(golden_path(dir, file), &text);
}

/// Crawl mit Start auf `site.test` über das angegebene Schema.
async fn crawl(scheme: &str) -> FixtureCrawl {
    crawl_fixture_from(
        "onpage-security",
        &format!("{scheme}://site.test/"),
        CrawlConfig::default(),
    )
    .await
}

// ---------------------------------------------------------------------------------------------------
// Aufbau des Tabs

#[test]
fn security_tab_lists_the_base_columns_and_numbered_directive_columns() {
    let tab = tabs::security::tab();
    let keys: Vec<&str> = tab.columns.iter().map(|c| c.key).collect();
    assert_eq!(
        keys,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Indexability",
            "Indexability Status",
            "Canonical Link Element",
            "Meta Robots",
            "X-Robots-Tag"
        ]
    );
    assert!(tab.default_sort.is_empty());
    let filters: Vec<&str> = tab.filters.iter().map(|f| f.id).collect();
    assert_eq!(filters, ["All", "HTTP URLs", "HTTPS URLs"]);
}

#[tokio::test]
async fn numbered_columns_grow_with_the_maximum_number_of_occurrences() {
    let fx = crawl("http").await;
    let page = view(&fx, "All", ColumnSelection::All);
    assert_eq!(
        page.columns,
        [
            "Address",
            "Content",
            "Status Code",
            "Status",
            "Indexability",
            "Indexability Status",
            "Canonical Link Element 1",
            "Canonical Link Element 2",
            "Meta Robots 1",
            "X-Robots-Tag 1"
        ]
    );
    let a = page.columns.iter().position(|c| c == "Address").unwrap();
    let row = page
        .rows
        .iter()
        .find(|r| r[a] == Value::Text("http://site.test/two-canonicals.html".into()))
        .expect("Seite mit zwei Canonicals");
    let c1 = page
        .columns
        .iter()
        .position(|c| c == "Canonical Link Element 1")
        .unwrap();
    let c2 = page
        .columns
        .iter()
        .position(|c| c == "Canonical Link Element 2")
        .unwrap();
    assert_eq!(
        row[c1],
        Value::Text("http://site.test/two-canonicals.html".into())
    );
    assert_eq!(row[c2], Value::Text("http://site.test/rel.html".into()));
}

// ---------------------------------------------------------------------------------------------------
// ONPAGE-031 / ONPAGE-032

// spec: ONPAGE-031
#[tokio::test]
async fn http_urls_filter_lists_internal_urls_with_the_http_scheme() {
    let fx = crawl("http").await;
    let http = addresses(&fx, "HTTP URLs");
    // Crawl auf http://…: alle internen URLs, die über http erreicht wurden
    for want in [
        "http://site.test/",
        "http://site.test/rel.html",
        "http://site.test/alt.html",
        "http://site.test/two-canonicals.html",
        "http://site.test/pic.png",
    ] {
        assert!(http.contains(&want.to_string()), "{want} fehlt: {http:?}");
    }
    // externe HTTP-URLs erscheinen nicht
    assert!(!http.iter().any(|a| a.contains("other.test")), "{http:?}");
    // https-URLs auch nicht
    assert!(!http.iter().any(|a| a.starts_with("https://")), "{http:?}");
    assert_address_golden(&fx, "HTTP URLs", "", "security__http_urls.csv");
}

// spec: ONPAGE-031
#[tokio::test]
async fn an_https_crawl_still_lists_the_internal_http_link_of_the_same_host() {
    let fx = crawl("https").await;
    let http = addresses(&fx, "HTTP URLs");
    // gleicher Host, anderes Schema = intern
    assert!(
        http.contains(&"http://site.test/alt.html".to_string()),
        "{http:?}"
    );
    assert!(!http.iter().any(|a| a.starts_with("https://")), "{http:?}");
    assert!(!http.iter().any(|a| a.contains("other.test")), "{http:?}");
    assert_address_golden(&fx, "HTTP URLs", "https-start", "security__http_urls.csv");
}

// spec: ONPAGE-032
#[tokio::test]
async fn https_urls_filter_lists_internal_urls_with_the_https_scheme() {
    let fx = crawl("https").await;
    let https = addresses(&fx, "HTTPS URLs");
    for want in [
        "https://site.test/",
        "https://site.test/rel.html",
        "https://site.test/secure.html",
        "https://site.test/two-canonicals.html",
        "https://site.test/pic.png",
    ] {
        assert!(https.contains(&want.to_string()), "{want} fehlt: {https:?}");
    }
    assert!(!https.iter().any(|a| a.starts_with("http://")), "{https:?}");
    assert!(!https.iter().any(|a| a.contains("other.test")), "{https:?}");
    assert_address_golden(&fx, "HTTPS URLs", "https-start", "security__https_urls.csv");

    // HTTP-Start: der https-Link auf denselben Host steht im HTTPS-Filter
    let fx = crawl("http").await;
    let https = addresses(&fx, "HTTPS URLs");
    assert!(
        https.contains(&"https://site.test/secure.html".to_string()),
        "{https:?}"
    );
    assert_address_golden(&fx, "HTTPS URLs", "", "security__https_urls.csv");
}

// spec: ONPAGE-031, ONPAGE-032
#[tokio::test]
async fn http_urls_and_https_urls_together_are_exactly_all() {
    for scheme in ["http", "https"] {
        let fx = crawl(scheme).await;
        let all = addresses(&fx, "All");
        let http = addresses(&fx, "HTTP URLs");
        let https = addresses(&fx, "HTTPS URLs");
        for a in &http {
            assert!(!https.contains(a), "{a} in beiden Filtern");
        }
        let mut union: Vec<String> = http.into_iter().chain(https).collect();
        union.sort();
        assert_eq!(union, all, "{scheme}-Start: HTTP URLs ∪ HTTPS URLs = All");
        assert_address_golden(
            &fx,
            "All",
            if scheme == "http" { "" } else { "https-start" },
            "security__all.csv",
        );
        // alle Inhaltstypen (Bild) gehören zur Grundmenge, externe URLs nie; die robots.txt nicht (K-54)
        assert!(all.iter().any(|a| a.ends_with("/pic.png")));
        assert!(!all.iter().any(|a| a.ends_with("/robots.txt")));
        assert!(!all.iter().any(|a| a.contains("other.test")));
    }
}
