//! CRAWL-060 / TECH-210: Sitemap-Parser für `urlset` und `sitemapindex`.

use os_model::{SitemapDoc, SitemapKind};
use os_parse::parse_sitemap;

fn url() -> url::Url {
    url::Url::parse("https://site.test/sitemap.xml").unwrap()
}

fn parse(xml: &str) -> SitemapDoc {
    parse_sitemap(&url(), xml.as_bytes(), Some("application/xml"))
}

fn urls(doc: &SitemapDoc) -> Vec<Option<&str>> {
    doc.locs.iter().map(|l| l.url.as_deref()).collect()
}

// spec: CRAWL-060
#[test]
fn urlset_with_loc_and_lastmod() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://site.test/a</loc><lastmod>2026-01-02</lastmod><changefreq>daily</changefreq><priority>0.8</priority></url>
  <url><loc>https://site.test/b?x=1</loc></url>
</urlset>"#;
    let doc = parse(xml);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.invalid_entries, 0);
    assert_eq!(doc.size_bytes, xml.len() as u64);
    assert_eq!(doc.locs.len(), 2);
    assert_eq!(doc.locs[0].loc_raw, "https://site.test/a");
    assert_eq!(doc.locs[0].url.as_deref(), Some("https://site.test/a"));
    assert_eq!(doc.locs[0].lastmod.as_deref(), Some("2026-01-02"));
    assert_eq!(doc.locs[1].lastmod, None);
    assert!(doc.locs[1].hreflang.is_empty());
}

// spec: CRAWL-060
#[test]
fn sitemapindex_is_recognized() {
    let doc = parse(
        r#"<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
        <sitemap><loc>https://site.test/s1.xml</loc><lastmod>2026-02-01T10:00:00+00:00</lastmod></sitemap>
        <sitemap><loc>https://site.test/s2.xml.gz</loc></sitemap></sitemapindex>"#,
    );
    assert_eq!(doc.kind, SitemapKind::Index);
    assert_eq!(
        urls(&doc),
        [
            Some("https://site.test/s1.xml"),
            Some("https://site.test/s2.xml.gz")
        ]
    );
    assert_eq!(
        doc.locs[0].lastmod.as_deref(),
        Some("2026-02-01T10:00:00+00:00")
    );
}

// spec: CRAWL-060, TECH-210
#[test]
fn loc_is_trimmed_and_entities_are_decoded() {
    let doc = parse(
        "<urlset><url><loc>\n  https://site.test/a?x=1&amp;y=2&#38;z=&lt;3&gt;  \n</loc></url>\
         <url><loc><![CDATA[https://site.test/c?a=1&b=2]]></loc></url>\
         <url><loc>https://site.test/q?name=&quot;x&quot;&apos;</loc></url></urlset>",
    );
    assert_eq!(doc.locs[0].loc_raw, "https://site.test/a?x=1&y=2&z=<3>");
    assert_eq!(doc.locs[1].loc_raw, "https://site.test/c?a=1&b=2");
    assert_eq!(doc.locs[2].loc_raw, "https://site.test/q?name=\"x\"'");
    assert_eq!(doc.invalid_entries, 0);
    assert_eq!(
        doc.locs[1].url.as_deref(),
        Some("https://site.test/c?a=1&b=2")
    );
}

// spec: CRAWL-060, TECH-210
#[test]
fn invalid_locs_are_counted_but_not_adopted() {
    let doc = parse(
        "<urlset>\
         <url><loc>https://site.test/ok</loc></url>\
         <url><loc>not a url</loc></url>\
         <url><loc>/relative/path</loc></url>\
         <url><loc>ftp://site.test/file</loc></url>\
         <url><loc>http://[bad</loc></url>\
         <url><loc>   </loc></url>\
         <url><lastmod>2026-01-01</lastmod></url>\
         <url/>\
         <url><loc>http://other.test/fine</loc></url></urlset>",
    );
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.invalid_entries, 7);
    assert_eq!(
        doc.locs.len(),
        9,
        "alle Einträge bleiben für die 50 000er-Grenze gezählt"
    );
    let valid: Vec<&str> = doc.locs.iter().filter_map(|l| l.url.as_deref()).collect();
    assert_eq!(valid, ["https://site.test/ok", "http://other.test/fine"]);
    assert_eq!(doc.locs[1].loc_raw, "not a url");
    assert_eq!(doc.locs[6].lastmod.as_deref(), Some("2026-01-01"));
}

// spec: CRAWL-060
#[test]
fn namespace_prefixes_and_extension_elements_are_handled() {
    let doc = parse(
        r#"<sm:urlset xmlns:sm="http://www.sitemaps.org/schemas/sitemap/0.9"
            xmlns:image="http://www.google.com/schemas/sitemap-image/1.1">
          <sm:url>
            <sm:loc>https://site.test/page</sm:loc>
            <image:image><image:loc>https://site.test/img.png</image:loc></image:image>
            <sm:lastmod>2026-03-03</sm:lastmod>
          </sm:url>
        </sm:urlset>"#,
    );
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 1, "image:loc ist kein Eintrag");
    assert_eq!(doc.locs[0].url.as_deref(), Some("https://site.test/page"));
    assert_eq!(doc.locs[0].lastmod.as_deref(), Some("2026-03-03"));
}

// spec: CRAWL-060
#[test]
fn broken_xml_keeps_the_entries_read_before_the_error() {
    let mut xml = String::from("<urlset>");
    for i in 0..10 {
        xml.push_str(&format!("<url><loc>https://site.test/{i}</loc></url>"));
    }
    xml.push_str("<url><loc>https://site.test/10</loc></urlset>"); // Endtag passt nicht
    xml.push_str("<url><loc>https://site.test/11</loc></url>");
    let doc = parse(&xml);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 10);

    // abgeschnittene Datei
    let cut = &xml[..xml.find("<url><loc>https://site.test/10").unwrap() + 20];
    let doc = parse(cut);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 10);
}

// spec: CRAWL-060
#[test]
fn not_a_sitemap_is_invalid() {
    for body in [
        "",
        "   \n",
        "<html><body>Not found</body></html>",
        "<?xml version=\"1.0\"?><foo><url><loc>https://site.test/a</loc></url></foo>",
        "Not Found",
    ] {
        let doc = parse(body);
        assert_eq!(doc.kind, SitemapKind::Invalid, "{body:?}");
        assert!(doc.locs.is_empty(), "{body:?}");
        assert_eq!(doc.size_bytes, body.len() as u64);
    }
}

// spec: CRAWL-060
#[test]
fn empty_urlset_is_a_valid_empty_sitemap() {
    let doc = parse("<urlset/>");
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert!(doc.locs.is_empty());
}

// spec: CRAWL-060
#[test]
fn more_than_50000_entries_are_read_completely() {
    let mut xml = String::from("<urlset>");
    for i in 0..50_001 {
        xml.push_str(&format!("<url><loc>https://site.test/p/{i}</loc></url>"));
    }
    xml.push_str("</urlset>");
    let doc = parse(&xml);
    assert_eq!(doc.locs.len(), 50_001);
    assert_eq!(doc.invalid_entries, 0);
    assert_eq!(doc.size_bytes, xml.len() as u64);
}

// spec: CRAWL-060
#[test]
fn text_sitemap_one_url_per_line() {
    let doc = parse_sitemap(
        &url(),
        "\u{feff}# Kommentar\nhttps://site.test/a\r\n\r\n  https://site.test/b  \nkein link\nhttp://site.test/c".as_bytes(),
        Some("text/plain"),
    );
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 4);
    assert_eq!(doc.invalid_entries, 1);
    assert_eq!(doc.locs[1].loc_raw, "https://site.test/b");
}

// spec: CRAWL-060
#[test]
fn rss_and_atom_feeds_are_accepted() {
    let rss = parse(
        r#"<?xml version="1.0"?><rss version="2.0"><channel><title>t</title><link>https://site.test/</link>
        <item><title>a</title><link>https://site.test/post-1</link><pubDate>Mon, 01 Jan 2026 00:00:00 GMT</pubDate></item>
        <item><link>https://site.test/post-2</link></item></channel></rss>"#,
    );
    assert_eq!(rss.kind, SitemapKind::Urlset);
    assert_eq!(
        urls(&rss),
        [
            Some("https://site.test/post-1"),
            Some("https://site.test/post-2")
        ]
    );
    assert_eq!(
        rss.locs[0].lastmod.as_deref(),
        Some("Mon, 01 Jan 2026 00:00:00 GMT")
    );

    let atom = parse(
        r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>t</title><link href="https://site.test/" rel="self"/>
        <entry><title>a</title><link href="https://site.test/e1"/><updated>2026-01-01T00:00:00Z</updated></entry>
        <entry><link rel="alternate" href="https://site.test/e2"/><link rel="enclosure" href="https://site.test/x.mp3"/></entry></feed>"#,
    );
    assert_eq!(atom.kind, SitemapKind::Urlset);
    assert_eq!(
        urls(&atom),
        [Some("https://site.test/e1"), Some("https://site.test/e2")]
    );
    assert_eq!(
        atom.locs[0].lastmod.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
}

// spec: CRAWL-060
#[test]
fn utf16_and_declared_legacy_encodings_are_decoded() {
    let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><urlset><url><loc>https://site.test/ü</loc></url></urlset>";
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let doc = parse_sitemap(&url(), &bytes, None);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs[0].url.as_deref(), Some("https://site.test/%C3%BC"));

    let latin1 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><urlset><url><loc>https://site.test/\xfc</loc></url></urlset>";
    let doc = parse_sitemap(&url(), latin1, None);
    assert_eq!(doc.locs[0].url.as_deref(), Some("https://site.test/%C3%BC"));
    assert_eq!(doc.size_bytes, latin1.len() as u64);
}

// spec: CRAWL-060
#[test]
fn utf8_bom_is_skipped() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(b"<urlset><url><loc>https://site.test/a</loc></url></urlset>");
    let doc = parse_sitemap(&url(), &bytes, None);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 1);
}
