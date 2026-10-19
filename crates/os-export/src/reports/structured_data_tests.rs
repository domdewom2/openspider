//! Tests der Reports OUT-100 bis OUT-103 und der Validierungszeilen der Tripel-Exporte (OUT-056) gegen einen echten
//! Crawl der Fixture `crates/os-parse/src/sd/validate/fixtures/out-structured-data`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use os_engine::testing::{crawl_dir_with, test_deps, FixtureCrawl};
use os_model::CrawlConfig;

use super::ENTRIES;
use crate::catalog::{export_table, TableEntry, TableJob};
use crate::types::Format;

fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    let sd = &mut cfg.extraction.structured_data;
    sd.json_ld = true;
    sd.microdata = true;
    sd.rdfa = true;
    cfg
}

async fn crawl() -> FixtureCrawl {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../os-parse/src/sd/validate/fixtures/out-structured-data");
    crawl_dir_with(&dir, "/index.html", config(), test_deps()).await
}

/// Exportiert einen Eintrag als CSV und liefert Kopf und Zeilen; Adressen ohne Schema und Host.
fn export(fx: &FixtureCrawl, entry: &TableEntry) -> (Vec<String>, Vec<Vec<String>>) {
    let cfg = config();
    let tabs = os_analyze::tabs::configured(&cfg);
    let job = TableJob {
        entry_name: entry.name,
        tabs: &tabs,
        file: &fx.file,
        cfg: &cfg,
    };
    let path = fx.dir.path().join("report.csv");
    let a = export_table(entry, &job, &path, Format::Csv).unwrap();
    let first = std::fs::read(&path).unwrap();
    // Deterministisch: ein zweiter Lauf schreibt dieselben Bytes.
    let b = export_table(entry, &job, &path, Format::Csv).unwrap();
    assert_eq!(first, std::fs::read(&path).unwrap());
    assert_eq!(a.rows, b.rows);
    let text = String::from_utf8(first).unwrap();
    let mut reader =
        csv::ReaderBuilder::new().from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    let header = reader
        .headers()
        .unwrap()
        .iter()
        .map(str::to_owned)
        .collect();
    let site = fx.site.url("/");
    let rows = reader
        .records()
        .map(|r| {
            r.unwrap()
                .iter()
                .map(|c| c.replace(site.trim_end_matches('/'), ""))
                .collect()
        })
        .collect::<Vec<Vec<String>>>();
    assert_eq!(a.rows, rows.len() as u64);
    (header, rows)
}

fn entry(name: &str) -> &'static TableEntry {
    ENTRIES.iter().find(|e| e.name.ends_with(name)).unwrap()
}

fn by_header(header: &[String], rows: &[Vec<String>]) -> Vec<HashMap<String, String>> {
    rows.iter()
        .map(|r| header.iter().cloned().zip(r.iter().cloned()).collect())
        .collect()
}

const NO_OFFERS: &str =
    "At least one of `review`, `aggregateRating`, `offers` is required for Product snippet.";

// spec: OUT-100
// spec: TECH-190
#[tokio::test]
async fn summary_groups_findings_with_sample_url_and_order() {
    let fx = crawl().await;
    let (header, rows) = export(&fx, entry("Validation Errors & Warnings Summary"));
    assert_eq!(
        header,
        [
            "Validation Type",
            "Severity",
            "Type",
            "Feature",
            "Issue",
            "URLs",
            "% of Total",
            "Sample URL"
        ]
    );
    let rows = by_header(&header, &rows);
    let no_offers: Vec<_> = rows.iter().filter(|r| r["Issue"] == NO_OFFERS).collect();
    assert_eq!(no_offers.len(), 1);
    let r = no_offers[0];
    assert_eq!(
        [
            &r["Validation Type"],
            &r["Severity"],
            &r["Type"],
            &r["Feature"],
            &r["URLs"],
            &r["Sample URL"]
        ],
        [
            "Google Rich Results",
            "Error",
            "Product",
            "Product snippet",
            "3",
            "/product-no-offers-1.html"
        ]
    );
    // 11 Seiten im Tab (Startseite und zehn Fixture-Seiten): 3 / 11.
    assert_eq!(r["% of Total"], "27.27");
    // Fehler vor Warnungen, dann URLs absteigend.
    let order: Vec<(bool, i64)> = rows
        .iter()
        .map(|r| {
            (
                r["Severity"] == "Warning",
                -r["URLs"].parse::<i64>().unwrap(),
            )
        })
        .collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    assert_eq!(rows[0]["Issue"], NO_OFFERS);
    // Die schema.org-Prüfung des FAQ-Markups bleibt bestehen.
    assert!(rows.iter().any(|r| r["Validation Type"] == "Schema.org"
        && r["Type"] == "Question"
        && r["Issue"] == "Property `answerr` is not defined in the schema.org vocabulary."));
}

// spec: OUT-101
// spec: OUT-100
// spec: TECH-190
#[tokio::test]
async fn detail_rows_add_up_to_the_summary() {
    let fx = crawl().await;
    let (sh, summary) = export(&fx, entry("Validation Errors & Warnings Summary"));
    let (dh, detail) = export(&fx, entry(":Validation Errors & Warnings"));
    assert_eq!(
        dh,
        [
            "Address",
            "Validation Type",
            "Severity",
            "Type",
            "Feature",
            "Issue",
            "Property Path"
        ]
    );
    let summary = by_header(&sh, &summary);
    let detail = by_header(&dh, &detail);
    let key = |r: &HashMap<String, String>| {
        ["Validation Type", "Severity", "Type", "Feature", "Issue"].map(|k| r[k].clone())
    };
    let mut url_sum = 0;
    for s in &summary {
        let rows: Vec<_> = detail.iter().filter(|d| key(d) == key(s)).collect();
        let urls: BTreeSet<_> = rows.iter().map(|d| d["Address"].clone()).collect();
        let n: usize = s["URLs"].parse().unwrap();
        assert_eq!(urls.len(), n, "{s:?}");
        // Je Seite kommt jede Meldung hier genau einmal vor: Zeilen = URLs × 1.
        assert_eq!(rows.len(), n, "{s:?}");
        url_sum += n;
    }
    let pairs: BTreeSet<_> = detail
        .iter()
        .map(|d| (d["Address"].clone(), d["Issue"].clone()))
        .collect();
    assert!(url_sum >= pairs.len());
    let currency: Vec<_> = detail
        .iter()
        .filter(|d| d["Address"] == "/breadcrumb-product-error.html")
        .map(|d| {
            (
                d["Feature"].as_str(),
                d["Property Path"].as_str(),
                d["Type"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        currency,
        [
            ("Product snippet", "name", "Product"),
            ("Merchant listing", "name", "Product")
        ]
    );
    assert!(detail.iter().all(|d| d["Address"] != "/broken-jsonld.html"));
}

// spec: OUT-102
#[tokio::test]
async fn features_summary_counts_valid_warning_and_error_pages() {
    let fx = crawl().await;
    let (header, rows) = export(&fx, entry("Google Rich Results Features Summary"));
    assert_eq!(
        header,
        [
            "Feature",
            "URLs",
            "% of Total",
            "Valid",
            "With Warnings",
            "With Errors",
            "Sample URL"
        ]
    );
    let rows: Vec<Vec<String>> = rows;
    let pick = |r: &Vec<String>| [0, 1, 3, 4, 5, 6].map(|i| r[i].clone());
    assert_eq!(
        rows.iter().map(pick).collect::<Vec<_>>(),
        [
            [
                "Product snippet",
                "6",
                "2",
                "0",
                "4",
                "/breadcrumb-product-error.html"
            ],
            [
                "Breadcrumb",
                "3",
                "3",
                "0",
                "0",
                "/breadcrumb-microdata.html"
            ],
            [
                "Merchant listing",
                "3",
                "1",
                "1",
                "1",
                "/breadcrumb-product-error.html"
            ],
        ]
        .map(|r| r.map(str::to_owned))
    );
    for r in &rows {
        let n = |i: usize| r[i].parse::<u64>().unwrap();
        assert_eq!(n(3) + n(4) + n(5), n(1), "{r:?}");
    }
    // Abgekündigtes Feature (FAQ): keine Zeile.
    assert!(rows.iter().all(|r| r[0] != "FAQ"));
}

// spec: OUT-103
#[tokio::test]
async fn features_matrix_marks_each_page_and_feature() {
    let fx = crawl().await;
    let (header, rows) = export(&fx, entry(":Google Rich Results Features"));
    assert_eq!(
        header,
        [
            "Address",
            "Breadcrumb",
            "Merchant listing",
            "Product snippet"
        ]
    );
    let rows = by_header(&header, &rows);
    let row = |path: &str| {
        let r = rows.iter().find(|r| r["Address"] == path).unwrap();
        ["Breadcrumb", "Merchant listing", "Product snippet"].map(|k| r[k].clone())
    };
    assert_eq!(
        row("/breadcrumb-product-error.html"),
        ["Valid", "Errors", "Errors"]
    );
    assert_eq!(
        row("/breadcrumb-product-warning.html"),
        ["Valid", "Warnings", "Valid"]
    );
    assert_eq!(row("/breadcrumb-microdata.html"), ["Valid", "", ""]);
    assert_eq!(row("/product-no-offers-2.html"), ["", "", "Errors"]);
    assert!(rows.iter().all(|r| r["Address"] != "/faq-deprecated.html"));
    assert_eq!(rows.len(), 7);
}

// spec: OUT-056
#[tokio::test]
async fn triple_exports_carry_validation_findings() {
    let fx = crawl().await;
    let cfg = config();
    let tabs = os_analyze::tabs::configured(&cfg);
    let run = |name: &str| {
        let entry = crate::catalog::bulk_tables()
            .into_iter()
            .find(|e| e.name == name)
            .unwrap();
        let job = TableJob {
            entry_name: entry.name,
            tabs: &tabs,
            file: &fx.file,
            cfg: &cfg,
        };
        let path = fx.dir.path().join("triples.csv");
        export_table(entry, &job, &path, Format::Csv).unwrap();
        std::fs::read_to_string(path).unwrap()
    };
    let errors = run("Structured Data:Validation Errors Triples");
    assert!(errors.contains("faq-deprecated.html"));
    assert!(errors.contains(
        "mainEntity[0].answerr,,Schema.org,Error,Property `answerr` is not defined in the schema.org vocabulary."
    ));
    assert!(!errors.contains("Google: "));
    let rich = run("Structured Data:Rich Result Feature Detected Triples");
    // Meldungen mit Kommas stehen in Anführungszeichen (CSV).
    assert!(
        rich.contains(&format!(",Google: Product snippet,Error,\"{NO_OFFERS}\"")),
        "{rich}"
    );
    assert!(!rich.contains("faq-deprecated.html"));
    let all = run("Structured Data:Contains Structured Data Triples");
    assert!(all.contains("_:b0,rdf:type,schema:Product"));
    assert!(all.contains("Google: Merchant listing,Warning,Property `brand` is missing (recommended for Merchant listing)."));
}
