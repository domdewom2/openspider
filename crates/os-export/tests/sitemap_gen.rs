//! Tests des XML-Sitemap-Generators (OUT-120, OUT-129, OUT-131) gegen die Fixture `out-sitemap-gen`, gegen
//! synthetisch befüllte Crawl-Dateien (Grenzen von 49 999 und 50 000 URLs) und gegen die eingecheckten
//! Schemas (`tests/data/xsd`, Strukturprüfer `sitemap_xsd.rs`).

#[path = "reports_common.rs"]
mod common;
#[path = "sitemap_xsd.rs"]
mod xsd;

use std::path::Path;
use std::time::Duration;

use common::base_config;
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_export::sitemap_gen::{escape_xml, generate_sitemap_with, sitemap_loc, SELECTION_SQL};
use os_export::{generate_sitemap, ExportError, SitemapOptions};
use os_model::time::{IdSource, SeededIds};
use os_model::{CrawlConfig, CrawlMode, FixedClock, Timestamp};
use os_store::{CrawlFile, CrawlInit, StorageProfile};
use os_testsite::golden::{assert_golden, normalize};

fn ts() -> Timestamp {
    Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap()
}

fn clock() -> FixedClock {
    FixedClock::new(ts(), Duration::ZERO)
}

// ---------------------------------------------------------------------------------------------
// Hilfen

async fn fixture() -> FixtureCrawl {
    crawl_fixture("out-sitemap-gen", base_config()).await
}

/// Inhalt der `<loc>`-Elemente einer Sitemap-Datei (entity-dekodiert, geprüft gegen das Schema).
fn locs(path: &Path) -> Vec<String> {
    let doc = xsd::validate_file(&xsd::sitemap_schema(), path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    doc.children
        .iter()
        .map(|u| u.children[0].text.clone())
        .collect()
}

/// Legt eine leere Crawl-Datei an (Start-URL `https://site.test/`).
fn empty_crawl(dir: &Path, name: &str) -> CrawlFile {
    let created = ts();
    CrawlFile::create(
        &dir.join(name),
        &CrawlInit {
            crawl_id: SeededIds::new(7).new_crawl_id(created),
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".to_string()),
            list_source: None,
            project: "Projekt".to_string(),
            name: "Sitemap".to_string(),
            config: CrawlConfig::default(),
            created_at: created,
        },
        StorageProfile::Balanced,
    )
    .unwrap()
}

/// Fügt `n` indexierbare interne HTML-Seiten `https://site.test/p<i>` ein (Entdeckungsreihenfolge = `id`).
fn bulk_pages(file: &CrawlFile, n: u32, extra_path: &str) {
    file.conn()
        .execute_batch(&format!(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < {n}) \
             INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                crawl_depth, folder_depth, url_length, status_code, content_class, indexability, \
                effective_index, has_rel_prev, robots_blocked) \
             SELECT i, 'https://site.test/{extra_path}' || i, 'https://site.test/{extra_path}' || i, \
                'https://site.test', 1, 1, 'done', 'link', 1, 0, 30, 200, 'HTML', 'Indexable', 1, 0, 0 FROM n;"
        ))
        .unwrap();
}

/// Fügt eine einzelne Seite mit den Standardwerten einer auswählbaren Seite ein; `set` überschreibt Spalten.
fn page(file: &CrawlFile, id: i64, url: &str, set: &str) {
    let url = &url.replace('\'', "''");
    file.conn()
        .execute_batch(&format!(
            "INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                crawl_depth, folder_depth, url_length, status_code, content_class, indexability, \
                effective_index, has_rel_prev, robots_blocked) \
             VALUES ({id}, '{url}', '{url}', 'https://site.test', 1, 1, 'done', 'link', 1, 0, 30, 200, 'HTML', \
                'Indexable', 1, 0, 0);"
        ))
        .unwrap();
    if !set.is_empty() {
        file.conn()
            .execute_batch(&format!("UPDATE urls SET {set} WHERE id = {id};"))
            .unwrap();
    }
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------------------------
// OUT-120: Auswahl

// spec: OUT-120
#[tokio::test]
async fn default_selection_is_internal_indexable_html_200_in_crawl_order() {
    let fx = fixture().await;
    let base = fx.site.base_url();
    let tmp = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&fx.file, &SitemapOptions::default(), tmp.path()).unwrap();

    // Eine einzige Datei ohne Index; Reihenfolge = Entdeckungsreihenfolge (Start-URL zuerst).
    assert_eq!(files_in(tmp.path()), vec!["sitemap.xml"]);
    assert_eq!(report.files.len(), 1);
    let expected: Vec<String> = [
        "/",
        "/p1.html",
        "/%C3%BCber-uns",
        "/p2.html",
        "/search?a=1&b=2",
        "/p3.html",
        "/p4.html",
        "/p5.html",
    ]
    .iter()
    .map(|p| format!("{base}{p}"))
    .collect();
    let got = locs(&tmp.path().join("sitemap.xml"));
    assert_eq!(got, expected);
    assert_eq!(report.urls, 8);
    assert_eq!(report.files[0].rows, 8);

    // Nie enthalten: noindex, kanonisiert, rel=prev, PDF, 301, 404, 500, robots-gesperrt, nicht gecrawlt
    // (Pagination-Ziele), extern, robots.txt.
    for never in [
        "noindex",
        "canon",
        "list",
        "doc.pdf",
        "redirect",
        "missing",
        "error",
        "privat",
        "long",
        "ext.html",
        "robots.txt",
    ] {
        assert!(
            !got.iter().any(|l| l.contains(never)),
            "{never} darf nicht in der Sitemap stehen"
        );
    }
    // Jede URL höchstens einmal.
    let mut sorted = got.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), got.len());
}

// spec: OUT-120
#[tokio::test]
async fn generated_sitemap_matches_the_golden_file() {
    let fx = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    generate_sitemap(&fx.file, &SitemapOptions::default(), tmp.path()).unwrap();
    let text = std::fs::read_to_string(tmp.path().join("sitemap.xml")).unwrap();
    let bases = fx.site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    assert_golden(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/golden/out-sitemap-gen/sitemap.xml"
        ),
        &normalize(&text, &bases),
    );
}

// spec: OUT-120
#[test]
fn selection_rules_one_by_one() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "rules.ospider");
    let mut id = 0;
    let mut add = |name: &str, set: &str| {
        id += 1;
        page(&file, id, &format!("https://site.test/{name}"), set);
    };
    add("ok", "");
    add(
        "self-canonical",
        "canonical_url = 'https://site.test/self-canonical', canonical_self = 1",
    );
    add("rel-next", "has_rel_prev = 0");
    // ausgeschlossen:
    add(
        "canonicalised",
        "canonical_url = 'https://site.test/ok', canonical_self = 0",
    );
    add("noindex", "effective_index = 0");
    add("non-indexable", "indexability = 'Non-Indexable'");
    add("rel-prev", "has_rel_prev = 1");
    add("robots", "robots_blocked = 1");
    add("external", "is_internal = 0");
    add("hidden-row", "in_tables = 0");
    add("pdf", "content_class = 'PDF'");
    add("redirect", "status_code = 301");
    add("no-content", "status_code = 204");
    add("not-found", "status_code = 404");
    add("no-response", "status_code = 0");
    add("not-crawled", "status_code = NULL, state = 'discovered'");
    let out = tempfile::tempdir().unwrap();
    generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert_eq!(
        locs(&out.path().join("sitemap.xml")),
        vec![
            "https://site.test/ok",
            "https://site.test/self-canonical",
            "https://site.test/rel-next",
        ]
    );
}

// spec: OUT-120
#[test]
fn no_matching_url_writes_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "empty.ospider");
    page(&file, 1, "https://site.test/x", "status_code = 404");
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert!(report.files.is_empty());
    assert_eq!(report.urls, 0);
    assert!(files_in(out.path()).is_empty());
}

// spec: OUT-120
#[test]
fn selection_sql_is_the_documented_predicate() {
    // Die Auswahl steht in einer Konstante; sie nennt alle Bedingungen aus OUT-120.
    for part in [
        "is_internal = 1",
        "content_class = 'HTML'",
        "status_code = 200",
        "indexability = 'Indexable'",
        "effective_index",
        "canonical_self",
        "has_rel_prev",
        "robots_blocked = 0",
        "ORDER BY id",
    ] {
        assert!(SELECTION_SQL.contains(part), "{part}");
    }
}

// ---------------------------------------------------------------------------------------------
// OUT-131: Kodierung

// spec: OUT-131
#[tokio::test]
async fn encoding_is_utf8_without_bom_with_escaped_and_percent_encoded_urls() {
    let fx = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    generate_sitemap(&fx.file, &SitemapOptions::default(), tmp.path()).unwrap();
    let bytes = std::fs::read(tmp.path().join("sitemap.xml")).unwrap();
    // Deklaration, keine BOM.
    assert!(bytes.starts_with(b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"));
    assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
    // Reines ASCII: Umlaute sind prozentkodiert.
    assert!(bytes.is_ascii());
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("/%C3%BCber-uns"));
    // `&` in der Query ist maskiert, nirgends steht ein nacktes `&`.
    assert!(text.contains("/search?a=1&amp;b=2"));
    assert!(!text.replace("&amp;", "").contains('&'));
}

// spec: OUT-131
#[test]
fn special_characters_idn_hosts_and_fragments() {
    assert_eq!(
        escape_xml("https://x.test/a?b=1&c=2&d='e'<f>\"g\""),
        "https://x.test/a?b=1&amp;c=2&amp;d=&apos;e&apos;&lt;f&gt;&quot;g&quot;"
    );
    assert_eq!(
        sitemap_loc("https://x.test/über-uns").as_deref(),
        Some("https://x.test/%C3%BCber-uns")
    );
    assert_eq!(
        sitemap_loc("https://bücher.example/").as_deref(),
        Some("https://xn--bcher-kva.example/")
    );
    // Eine bereits kodierte URL bleibt unverändert, ein Fragment entfällt.
    assert_eq!(
        sitemap_loc("https://x.test/a%20b#frag").as_deref(),
        Some("https://x.test/a%20b")
    );

    // Durch die Datei hindurch: Apostroph und IDN-Host.
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "enc.ospider");
    page(&file, 1, "https://bücher.example/it's", "");
    page(&file, 2, "https://site.test/a?x=1&y=2", "");
    let out = tempfile::tempdir().unwrap();
    generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    let text = std::fs::read_to_string(out.path().join("sitemap.xml")).unwrap();
    assert!(text.contains("<loc>https://xn--bcher-kva.example/it&apos;s</loc>"));
    assert!(text.contains("<loc>https://site.test/a?x=1&amp;y=2</loc>"));
    // Parser sehen die Originalwerte.
    assert_eq!(
        locs(&out.path().join("sitemap.xml")),
        vec![
            "https://xn--bcher-kva.example/it's",
            "https://site.test/a?x=1&y=2"
        ]
    );
}

// spec: OUT-131
#[tokio::test]
async fn urls_over_2048_chars_are_left_out_and_listed_in_the_report() {
    let fx = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&fx.file, &SitemapOptions::default(), tmp.path()).unwrap();
    assert_eq!(report.skipped_too_long.len(), 1);
    let long = &report.skipped_too_long[0];
    assert!(long.chars().count() > 2048);
    assert!(long.contains("/long?q=aaa"));
    // Die ausgelassene URL steht weder in der Datei noch in der Zählung.
    let text = std::fs::read_to_string(tmp.path().join("sitemap.xml")).unwrap();
    assert!(!text.contains("/long"));
    assert_eq!(report.urls, 8);
    // Genau 2 048 Zeichen sind noch erlaubt, 2 049 nicht.
    let tmp2 = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp2.path(), "len.ospider");
    let prefix = "https://site.test/";
    let url_with_len = |n: usize| format!("{prefix}{}", "a".repeat(n - prefix.len()));
    page(&file, 1, &url_with_len(2048), "");
    page(&file, 2, &url_with_len(2049), "");
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert_eq!(report.urls, 1);
    assert_eq!(report.skipped_too_long, vec![url_with_len(2049)]);
}

// ---------------------------------------------------------------------------------------------
// OUT-120/129: Schema

// spec: OUT-120
#[tokio::test]
async fn generated_files_validate_against_the_schemas() {
    let fx = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    generate_sitemap(&fx.file, &SitemapOptions::default(), tmp.path()).unwrap();
    xsd::validate_file(&xsd::sitemap_schema(), &tmp.path().join("sitemap.xml")).unwrap();
}

// spec: OUT-120, OUT-129
#[test]
fn schema_checker_rejects_what_the_protocol_forbids() {
    let schema = xsd::sitemap_schema();
    let check = |xml: &str| -> Result<(), String> {
        let doc = xsd::parse_xml(xml.as_bytes())?;
        schema.validate(&doc)
    };
    const NS: &str = "xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\"";
    let ok = format!(
        "<urlset {NS}><url><loc>https://site.test/a</loc><lastmod>2026-10-13</lastmod>\
         <changefreq>daily</changefreq><priority>0.5</priority></url></urlset>"
    );
    check(&ok).unwrap();
    let bad = |xml: String, why: &str| {
        assert!(check(&xml).is_err(), "{why}: {xml}");
    };
    bad(format!("<urlset {NS}/>"), "urlset ohne url");
    bad(format!("<urlset {NS}><url></url></urlset>"), "url ohne loc");
    bad(
        format!("<urlset {NS}><url><loc>https://site.test/a</loc><foo/></url></urlset>"),
        "unbekanntes Element",
    );
    bad(
        format!(
            "<urlset {NS}><url><lastmod>2026-10-13</lastmod><loc>https://site.test/a</loc></url></urlset>"
        ),
        "falsche Reihenfolge",
    );
    bad(
        format!("<urlset {NS}><url><loc>https://site.test/a b</loc></url></urlset>"),
        "Leerzeichen in der URL",
    );
    bad(
        format!(
            "<urlset {NS}><url><loc>https://site.test/{}</loc></url></urlset>",
            "a".repeat(2049)
        ),
        "loc über 2048 Zeichen",
    );
    bad(
        format!(
            "<urlset {NS}><url><loc>https://site.test/a</loc><changefreq>sometimes</changefreq></url></urlset>"
        ),
        "changefreq außerhalb der Aufzählung",
    );
    bad(
        format!(
            "<urlset {NS}><url><loc>https://site.test/a</loc><priority>1.5</priority></url></urlset>"
        ),
        "priority über 1.0",
    );
    bad(
        "<urlset><url><loc>https://site.test/a</loc></url></urlset>".to_string(),
        "ohne Namensraum",
    );
    bad(
        format!("<urlset {NS}><url><loc>https://site.test/a?x=1&y=2</loc></url></urlset>"),
        "nacktes & ist kein wohlgeformtes XML",
    );
    // Mehr als 50 000 Einträge.
    let many = format!(
        "<urlset {NS}>{}</urlset>",
        "<url><loc>https://site.test/a</loc></url>".repeat(50_001)
    );
    bad(many, "50 001 Einträge");
}

// ---------------------------------------------------------------------------------------------
// OUT-129: Aufteilung und Index

// spec: OUT-129
#[test]
fn exactly_49999_urls_make_one_file_without_index() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "a.ospider");
    bulk_pages(&file, 49_999, "p");
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert_eq!(files_in(out.path()), vec!["sitemap.xml"]);
    assert_eq!(report.urls, 49_999);
    assert_eq!(report.files.len(), 1);
    assert_eq!(report.files[0].rows, 49_999);
    let got = locs(&out.path().join("sitemap.xml"));
    assert_eq!(got.len(), 49_999);
    assert_eq!(got[0], "https://site.test/p1");
    assert_eq!(got[49_998], "https://site.test/p49999");
}

// spec: OUT-129
#[test]
fn fifty_thousand_urls_make_two_files_and_an_index() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "b.ospider");
    bulk_pages(&file, 50_000, "p");
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap_with(
        &file,
        &SitemapOptions::default(),
        None,
        &clock(),
        out.path(),
    )
    .unwrap();
    assert_eq!(
        files_in(out.path()),
        vec!["sitemap_1.xml", "sitemap_2.xml", "sitemap_index.xml"]
    );
    assert_eq!(report.urls, 50_000);
    let first = locs(&out.path().join("sitemap_1.xml"));
    let second = locs(&out.path().join("sitemap_2.xml"));
    assert_eq!(first.len(), 49_999);
    assert_eq!(second, vec!["https://site.test/p50000"]);
    assert_eq!(first[0], "https://site.test/p1");

    // Der Index validiert gegen siteindex.xsd, die <loc> sind absolute URLs unter der Start-URL, lastmod ist
    // das Erzeugungsdatum.
    let index =
        xsd::validate_file(&xsd::index_schema(), &out.path().join("sitemap_index.xml")).unwrap();
    let entries: Vec<(String, String)> = index
        .children
        .iter()
        .map(|s| (s.children[0].text.clone(), s.children[1].text.clone()))
        .collect();
    assert_eq!(
        entries,
        vec![
            (
                "https://site.test/sitemap_1.xml".to_string(),
                "2026-10-13".to_string()
            ),
            (
                "https://site.test/sitemap_2.xml".to_string(),
                "2026-10-13".to_string()
            ),
        ]
    );
    // Berichte: zwei Sitemaps (49 999 + 1) und der Index.
    let rows: Vec<u64> = report.files.iter().map(|f| f.rows).collect();
    assert_eq!(rows, vec![49_999, 1, 2]);
    let names: Vec<String> = report
        .files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["sitemap_1.xml", "sitemap_2.xml", "sitemap_index.xml"]
    );
}

// spec: OUT-129
#[test]
fn index_uses_the_given_base_url() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "c.ospider");
    bulk_pages(&file, 5, "p");
    let out = tempfile::tempdir().unwrap();
    let opts = SitemapOptions {
        max_urls_per_file: 2,
        ..SitemapOptions::default()
    };
    generate_sitemap_with(
        &file,
        &opts,
        Some("https://cdn.example/maps"),
        &clock(),
        out.path(),
    )
    .unwrap();
    assert_eq!(
        files_in(out.path()),
        vec![
            "sitemap_1.xml",
            "sitemap_2.xml",
            "sitemap_3.xml",
            "sitemap_index.xml"
        ]
    );
    let index =
        xsd::validate_file(&xsd::index_schema(), &out.path().join("sitemap_index.xml")).unwrap();
    let index_locs: Vec<&str> = index
        .children
        .iter()
        .map(|s| s.children[0].text.as_str())
        .collect();
    assert_eq!(
        index_locs,
        vec![
            "https://cdn.example/maps/sitemap_1.xml",
            "https://cdn.example/maps/sitemap_2.xml",
            "https://cdn.example/maps/sitemap_3.xml"
        ]
    );
    // 2 + 2 + 1 URLs.
    let sizes: Vec<usize> = (1..=3)
        .map(|i| locs(&out.path().join(format!("sitemap_{i}.xml"))).len())
        .collect();
    assert_eq!(sizes, vec![2, 2, 1]);
}

// spec: OUT-129
#[test]
fn index_uses_base_url_from_the_options() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "o.ospider");
    bulk_pages(&file, 3, "q");
    let out = tempfile::tempdir().unwrap();
    let opts = SitemapOptions {
        max_urls_per_file: 2,
        base_url: Some("https://static.example/maps/".to_string()),
        ..SitemapOptions::default()
    };
    generate_sitemap(&file, &opts, out.path()).unwrap();
    let index =
        xsd::validate_file(&xsd::index_schema(), &out.path().join("sitemap_index.xml")).unwrap();
    let index_locs: Vec<&str> = index
        .children
        .iter()
        .map(|s| s.children[0].text.as_str())
        .collect();
    assert_eq!(
        index_locs,
        vec![
            "https://static.example/maps/sitemap_1.xml",
            "https://static.example/maps/sitemap_2.xml"
        ]
    );
    // Ohne Angabe: Ursprung der Start-URL.
    let out2 = tempfile::tempdir().unwrap();
    let opts = SitemapOptions {
        max_urls_per_file: 2,
        ..SitemapOptions::default()
    };
    assert_eq!(opts.base_url, None);
    generate_sitemap(&file, &opts, out2.path()).unwrap();
    let text = std::fs::read_to_string(out2.path().join("sitemap_index.xml")).unwrap();
    assert!(
        text.contains("<loc>https://site.test/sitemap_1.xml</loc>"),
        "{text}"
    );
}

// spec: OUT-129
#[test]
fn byte_limit_splits_files_before_they_exceed_it() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "d.ospider");
    bulk_pages(&file, 100, "seite-");
    let out = tempfile::tempdir().unwrap();
    // Jeder Eintrag ist rund 70 Byte groß; mit 1 000 Byte passen nur wenige in eine Datei.
    let opts = SitemapOptions {
        max_bytes_per_file: 1_000,
        ..SitemapOptions::default()
    };
    let report = generate_sitemap_with(&file, &opts, None, &clock(), out.path()).unwrap();
    assert!(report.files.len() > 3, "es muss mehrere Dateien geben");
    let mut all = Vec::new();
    for info in report
        .files
        .iter()
        .filter(|f| f.path.file_name().unwrap().to_string_lossy() != "sitemap_index.xml")
    {
        let size = std::fs::metadata(&info.path).unwrap().len();
        assert!(size <= 1_000, "{} hat {size} Byte", info.path.display());
        assert_eq!(size, info.bytes);
        all.extend(locs(&info.path));
    }
    // Alle URLs genau einmal, in der Reihenfolge der Entdeckung.
    let expected: Vec<String> = (1..=100)
        .map(|i| format!("https://site.test/seite-{i}"))
        .collect();
    assert_eq!(all, expected);
    xsd::validate_file(&xsd::index_schema(), &out.path().join("sitemap_index.xml")).unwrap();
}

// spec: OUT-129
#[test]
#[ignore = "Benchmark (P28): 50-MiB-Grenze mit dem Originalwert (rund 60 MB Testdaten in der Crawl-Datei und der Ausgabe)"]
fn default_byte_limit_of_50_mib_splits_files_with_very_long_urls() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "big.ospider");
    // 26 000 URLs mit je rund 2 000 Zeichen = rund 52 MB Nutzdaten: über der Grenze von 50 MiB (52 428 800 Byte).
    file.conn()
        .execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 26000) \
             INSERT INTO urls (id, url, address, origin, is_internal, in_tables, state, discovered_via, \
                crawl_depth, folder_depth, url_length, status_code, content_class, indexability, \
                effective_index, has_rel_prev, robots_blocked) \
             SELECT i, 'https://site.test/' || substr(printf('%06d', i) || hex(zeroblob(1000)), 1, 2000), \
                'x', 'https://site.test', 1, 1, 'done', 'link', 1, 0, 30, 200, 'HTML', 'Indexable', 1, 0, 0 FROM n;",
        )
        .unwrap();
    let out = tempfile::tempdir().unwrap();
    let report = generate_sitemap(&file, &SitemapOptions::default(), out.path()).unwrap();
    assert_eq!(report.urls, 26_000);
    let parts: Vec<&os_export::ExportFileInfo> = report
        .files
        .iter()
        .filter(|f| f.path.file_name().unwrap() != "sitemap_index.xml")
        .collect();
    assert!(parts.len() >= 2, "die 50-MiB-Grenze muss teilen");
    for p in parts {
        assert!(p.bytes <= 52_428_800);
    }
}

// spec: OUT-129
#[test]
fn urls_per_file_must_stay_within_the_protocol_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let file = empty_crawl(tmp.path(), "e.ospider");
    page(&file, 1, "https://site.test/a", "");
    let out = tempfile::tempdir().unwrap();
    for bad in [0, 50_001] {
        let opts = SitemapOptions {
            max_urls_per_file: bad,
            ..SitemapOptions::default()
        };
        assert!(matches!(
            generate_sitemap(&file, &opts, out.path()),
            Err(ExportError::Write(_))
        ));
    }
    // 50 000 sind erlaubt.
    let opts = SitemapOptions {
        max_urls_per_file: 50_000,
        ..SitemapOptions::default()
    };
    generate_sitemap(&file, &opts, out.path()).unwrap();
}

// spec: OUT-129
#[test]
fn default_options_follow_the_spec() {
    let d = SitemapOptions::default();
    assert_eq!(d.max_urls_per_file, 49_999);
    assert_eq!(d.max_bytes_per_file, 52_428_800);
}

// spec: OUT-129
#[tokio::test]
async fn index_base_defaults_to_the_start_url_origin_of_a_crawl() {
    let fx = fixture().await;
    let tmp = tempfile::tempdir().unwrap();
    let opts = SitemapOptions {
        max_urls_per_file: 3,
        ..SitemapOptions::default()
    };
    generate_sitemap(&fx.file, &opts, tmp.path()).unwrap();
    let index =
        xsd::validate_file(&xsd::index_schema(), &tmp.path().join("sitemap_index.xml")).unwrap();
    let base = fx.site.base_url();
    let got: Vec<&str> = index
        .children
        .iter()
        .map(|s| s.children[0].text.as_str())
        .collect();
    assert_eq!(
        got,
        vec![
            format!("{base}/sitemap_1.xml"),
            format!("{base}/sitemap_2.xml"),
            format!("{base}/sitemap_3.xml")
        ]
    );
    // 8 URLs in Dreiergruppen: 3 + 3 + 2.
    let n: Vec<usize> = (1..=3)
        .map(|i| locs(&tmp.path().join(format!("sitemap_{i}.xml"))).len())
        .collect();
    assert_eq!(n, vec![3, 3, 2]);
}
