//! CRAWL-060 / TECH-210: gzip-Sitemaps (Magic-Bytes) und entpackte Größe.

use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;
use os_model::SitemapKind;
use os_parse::parse_sitemap;

fn url(name: &str) -> url::Url {
    url::Url::parse(&format!("https://site.test/{name}")).unwrap()
}

fn gzip(data: &[u8]) -> Vec<u8> {
    let mut enc = GzEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

const XML: &str = "<urlset><url><loc>https://site.test/a</loc></url><url><loc>https://site.test/b</loc></url></urlset>";

// spec: CRAWL-060, TECH-210
#[test]
fn gzip_is_detected_by_magic_bytes_regardless_of_name_and_content_type() {
    let gz = gzip(XML.as_bytes());
    assert_eq!(&gz[..2], &[0x1f, 0x8b]);
    for (name, ct) in [
        ("sitemap.xml.gz", Some("application/gzip")),
        ("sitemap.xml", Some("application/xml")),
        ("sitemap.txt", Some("text/plain")),
        ("sitemap", None),
    ] {
        let doc = parse_sitemap(&url(name), &gz, ct);
        assert_eq!(doc.kind, SitemapKind::Urlset, "{name}");
        assert_eq!(doc.locs.len(), 2, "{name}");
        assert_eq!(doc.size_bytes, XML.len() as u64, "{name}: entpackte Größe");
    }
}

// spec: CRAWL-060
#[test]
fn gzip_content_type_without_magic_is_parsed_as_plain_xml() {
    let doc = parse_sitemap(&url("s.xml.gz"), XML.as_bytes(), Some("application/x-gzip"));
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 2);
}

// spec: CRAWL-060, TECH-210
#[test]
fn size_is_the_unpacked_size_and_counts_to_the_end_of_the_file() {
    let mut xml = String::from("<urlset><url><loc>https://site.test/a</loc></url>");
    xml.push_str(&" ".repeat(1_000_000));
    xml.push_str("</urlset>");
    let gz = gzip(xml.as_bytes());
    assert!(gz.len() < 10_000, "komprimiert klein");
    let doc = parse_sitemap(&url("big.xml.gz"), &gz, None);
    assert_eq!(doc.size_bytes, xml.len() as u64);
    assert_eq!(doc.locs.len(), 1);

    // Abbruch durch einen Fehler beendet das Zählen nicht
    let broken = format!("<urlset></wrong>{}", " ".repeat(500_000));
    let doc = parse_sitemap(&url("b.xml.gz"), &gzip(broken.as_bytes()), None);
    assert_eq!(doc.size_bytes, broken.len() as u64);
}

// spec: CRAWL-060
#[test]
fn concatenated_gzip_members_and_damaged_streams() {
    let mut two = gzip(b"<urlset><url><loc>https://site.test/a</loc></url>");
    two.extend(gzip(b"<url><loc>https://site.test/b</loc></url></urlset>"));
    let doc = parse_sitemap(&url("m.xml.gz"), &two, None);
    assert_eq!(doc.locs.len(), 2);

    let gz = gzip(XML.as_bytes());
    let damaged = &gz[..gz.len() - 12];
    let doc = parse_sitemap(&url("d.xml.gz"), damaged, None);
    assert!(doc.locs.len() <= 2);
    assert_eq!(doc.kind, SitemapKind::Urlset);
}

// spec: CRAWL-060
#[test]
#[ignore = "Benchmark: erzeugt eine > 50 MiB große Datei im Speicher (E2); P28 führt ihn aus"]
fn over_50_mib_unpacked_is_read_completely_by_streaming() {
    let limit = 52_428_800u64;
    let mut enc = GzEncoder::new(Vec::new(), Compression::fast());
    enc.write_all(b"<urlset><url><loc>https://site.test/a</loc></url>")
        .unwrap();
    let chunk = vec![b' '; 1 << 20];
    for _ in 0..51 {
        enc.write_all(&chunk).unwrap();
    }
    enc.write_all(b"<url><loc>https://site.test/z</loc></url></urlset>")
        .unwrap();
    let gz = enc.finish().unwrap();
    let doc = parse_sitemap(&url("huge.xml.gz"), &gz, None);
    assert!(doc.size_bytes > limit, "{}", doc.size_bytes);
    assert_eq!(doc.locs.len(), 2);
}
