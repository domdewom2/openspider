use os_model::{SitemapExtKind, SitemapKind};
use os_parse::sitemap::parse_reader;
use std::io::{Read, Write};
fn url() -> url::Url {
    url::Url::parse("https://site.test/s.xml").unwrap()
}
const XML: &[u8] = include_bytes!("../../../tests/fixtures/crawl-m2-sitemaps/extensions.xml");

// spec: CRAWL-060 [M2]
#[test]
fn metadata_and_namespaced_extensions_survive_without_becoming_page_urls() {
    let (doc, error) = parse_reader(&url(), XML);
    assert_eq!(error, None);
    assert_eq!(doc.kind, SitemapKind::Urlset);
    assert_eq!(doc.locs.len(), 2);
    let first = &doc.locs[0];
    assert_eq!(first.url.as_deref(), Some("https://site.test/page?a=1&b=2"));
    assert_eq!(first.changefreq.as_deref(), Some("daily"));
    assert_eq!(first.priority, Some(0.7));
    assert_eq!(
        first.extensions.iter().map(|e| e.kind).collect::<Vec<_>>(),
        [
            SitemapExtKind::Image,
            SitemapExtKind::Video,
            SitemapExtKind::News
        ]
    );
    for (extension, needle) in first
        .extensions
        .iter()
        .zip(["image.jpg", "allow_embed", "Journal"])
    {
        let value: serde_json::Value = serde_json::from_str(&extension.json).unwrap();
        assert!(value["xml"].as_str().unwrap().contains(needle));
        assert!(value["namespace"]
            .as_str()
            .unwrap()
            .starts_with("http://www.google.com/"));
    }
}

// spec: CRAWL-060 [M2]
#[test]
fn broken_xml_keeps_complete_entries_and_reports_error() {
    let broken = [
        XML.strip_suffix(b"</urlset>\n").unwrap(),
        b"<url><loc>unfinished",
    ]
    .concat();
    let (doc, error) = parse_reader(&url(), broken.as_slice());
    assert_eq!(doc.locs.len(), 2);
    assert!(error.is_some());
    assert_eq!(doc.size_bytes, broken.len() as u64);
    let malformed = b"<urlset><url><loc>https://site.test/</loc></url><bad></urlset>";
    let (doc, error) = parse_reader(&url(), malformed.as_slice());
    assert_eq!(doc.locs.len(), 1);
    assert!(error.is_some());
}

// spec: TECH-218, CRAWL-060
#[test]
fn gzip_over_fifty_mib_is_streamed_and_counts_uncompressed_bytes() {
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let start = b"<urlset><url><loc>https://site.test/a</loc></url>";
    gzip.write_all(start).unwrap();
    let chunk = b"<!-- padding for streaming -->\n";
    let count = 52_428_800 / chunk.len() + 1;
    for _ in 0..count {
        gzip.write_all(chunk).unwrap();
    }
    gzip.write_all(b"</urlset>").unwrap();
    let compressed = gzip.finish().unwrap();
    // Reader deliberately supplies small chunks; the API never requires a full expanded body.
    struct Chunks<'a>(&'a [u8]);
    impl Read for Chunks<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = out.len().min(4096).min(self.0.len());
            out[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    let (doc, error) = parse_reader(&url(), Chunks(&compressed));
    assert_eq!(error, None);
    assert_eq!(doc.locs.len(), 1);
    assert_eq!(
        doc.size_bytes,
        (start.len() + count * chunk.len() + b"</urlset>".len()) as u64
    );
    assert!(doc.size_bytes > 52_428_800);
}

// spec: CRAWL-060, TECH-218
#[test]
fn encoding_and_gzip_detection_work_even_when_reads_return_one_byte() {
    struct Bytes<'a>(&'a [u8]);
    impl Read for Bytes<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if out.is_empty() || self.0.is_empty() {
                return Ok(0);
            }
            out[0] = self.0[0];
            self.0 = &self.0[1..];
            Ok(1)
        }
    }
    let mut utf16 = vec![0xff, 0xfe];
    for ch in String::from_utf8_lossy(XML).encode_utf16() {
        utf16.extend(ch.to_le_bytes());
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&utf16).unwrap();
    let gzip = gzip.finish().unwrap();
    for data in [&utf16, &gzip] {
        let (doc, error) = parse_reader(&url(), Bytes(data));
        assert_eq!(error, None);
        assert_eq!(doc.locs.len(), 2);
        assert_eq!(doc.size_bytes, utf16.len() as u64);
        assert_eq!(doc.locs[0].extensions.len(), 3);
    }
}
