//! Validierungsfilter, Zählspalten und Befunddetails des Tabs Structured Data (M2-D20) gegen einen echten Crawl der
//! Fixture `crates/os-parse/src/sd/validate/fixtures/tech-sd` (Parser → Engine → Store → Ansicht).

use std::collections::HashMap;
use std::path::Path;

use os_analyze::issues::Lang;
use os_analyze::tabs::structured_data::{
    detail_validation_label, finding_link, finding_text, finding_type, severity_label,
    validation_label,
};
use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::{crawl_dir_with, test_deps, FixtureCrawl};
use os_model::{CrawlConfig, UrlId, Value};
use os_store::CrawlFile;

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../os-parse/src/sd/validate/fixtures/tech-sd")
}

fn config() -> CrawlConfig {
    let mut cfg = CrawlConfig::default();
    let sd = &mut cfg.extraction.structured_data;
    sd.json_ld = true;
    sd.microdata = true;
    sd.rdfa = true;
    cfg
}

async fn crawl(cfg: &CrawlConfig) -> FixtureCrawl {
    crawl_dir_with(&fixture_dir(), "/index.html", cfg.clone(), test_deps()).await
}

/// Zeilen eines Filters: Pfad → Spalte → Wert.
fn table(
    file: &CrawlFile,
    cfg: &CrawlConfig,
    filter: &str,
) -> HashMap<String, HashMap<String, Value>> {
    let mut q = ViewQuery::new("Structured Data", filter);
    q.columns = ColumnSelection::All;
    q.limit = 500;
    let page = query_view(file, cfg, &q).unwrap();
    let address = page.columns.iter().position(|c| c == "Address").unwrap();
    page.rows
        .iter()
        .map(|row| {
            let Value::Text(url) = &row[address] else {
                panic!("Address")
            };
            let path = format!("/{}", url.splitn(4, '/').nth(3).unwrap_or(""));
            let cells = page
                .columns
                .iter()
                .cloned()
                .zip(row.iter().cloned())
                .collect();
            (path, cells)
        })
        .collect()
}

fn paths(file: &CrawlFile, cfg: &CrawlConfig, filter: &str) -> Vec<String> {
    let mut p: Vec<_> = table(file, cfg, filter).into_keys().collect();
    p.sort();
    p
}

fn int(t: &HashMap<String, HashMap<String, Value>>, path: &str, column: &str) -> i64 {
    match &t[path][column] {
        Value::Int(n) => *n,
        other => panic!("{path} {column}: {other:?}"),
    }
}

fn text(t: &HashMap<String, HashMap<String, Value>>, path: &str, column: &str) -> String {
    match &t[path][column] {
        Value::Text(s) => s.clone(),
        Value::Null => String::new(),
        other => panic!("{path} {column}: {other:?}"),
    }
}

fn url_id(file: &CrawlFile, path: &str) -> UrlId {
    file.conn()
        .query_row(
            "SELECT id FROM urls WHERE url LIKE '%' || ?1",
            [path],
            |r| r.get(0),
        )
        .map(UrlId)
        .unwrap()
}

// spec: TECH-187
// spec: TECH-183
#[tokio::test]
async fn schema_org_counts_and_filters() {
    let cfg = config();
    let fx = crawl(&cfg).await;
    let all = table(&fx.file, &cfg, "All");
    assert_eq!(int(&all, "/schema-errors.html", "Errors"), 5);
    assert_eq!(int(&all, "/schema-errors.html", "Warnings"), 0);
    assert_eq!(int(&all, "/data-vocabulary.html", "Warnings"), 1);
    assert_eq!(int(&all, "/data-vocabulary.html", "Errors"), 0);
    assert_eq!(int(&all, "/case.html", "Errors"), 0);
    assert_eq!(int(&all, "/rich-product-complete.html", "Errors"), 0);
    assert_eq!(
        paths(&fx.file, &cfg, "Validation Errors"),
        ["/rich-event-bad-date.html", "/schema-errors.html"]
    );
    assert_eq!(
        paths(&fx.file, &cfg, "Validation Warnings"),
        ["/data-vocabulary.html"]
    );
    // Findings of a format that is switched off afterwards are neither counted nor filtered.
    let mut no_micro = cfg.clone();
    no_micro.extraction.structured_data.microdata = false;
    assert_eq!(
        int(
            &table(&fx.file, &no_micro, "All"),
            "/data-vocabulary.html",
            "Warnings"
        ),
        0
    );
    assert!(paths(&fx.file, &no_micro, "Validation Warnings").is_empty());
}

// spec: TECH-187
#[tokio::test]
async fn case_sensitive_crawl_reports_type_and_property() {
    let mut cfg = config();
    cfg.extraction.structured_data.case_sensitive = true;
    let fx = crawl(&cfg).await;
    assert_eq!(
        int(&table(&fx.file, &cfg, "All"), "/case.html", "Errors"),
        2
    );
}

// spec: TECH-188
// spec: TECH-183
#[tokio::test]
async fn rich_result_counts_features_and_filters() {
    let cfg = config();
    let fx = crawl(&cfg).await;
    let all = table(&fx.file, &cfg, "All");
    for (path, errors, warnings, features) in [
        (
            "/rich-product-complete.html",
            0,
            0,
            "Product snippet, Merchant listing",
        ),
        (
            "/rich-product-missing-currency.html",
            1,
            0,
            "Product snippet, Merchant listing",
        ),
        (
            "/rich-product-no-brand.html",
            0,
            1,
            "Product snippet, Merchant listing",
        ),
        (
            "/rich-product-no-name.html",
            2,
            0,
            "Product snippet, Merchant listing",
        ),
        ("/rich-product-no-offers.html", 1, 0, "Product snippet"),
        (
            "/rich-product-review.html",
            0,
            0,
            "Product snippet, Review snippet",
        ),
        ("/rich-recipe-no-image.html", 1, 0, "Recipe"),
        ("/rich-event-bad-date.html", 1, 0, "Event"),
        ("/rich-breadcrumb-no-position.html", 3, 0, "Breadcrumb"),
        ("/rich-faq.html", 0, 0, ""),
        ("/rich-website-search.html", 0, 0, ""),
        ("/article.html", 0, 3, "Article"),
        ("/article-amp.html", 3, 0, "Article"),
    ] {
        assert_eq!(int(&all, path, "Rich Result Errors"), errors, "{path}");
        assert_eq!(int(&all, path, "Rich Result Warnings"), warnings, "{path}");
        assert_eq!(text(&all, path, "Rich Result Features"), features, "{path}");
    }
    let errors = paths(&fx.file, &cfg, "Rich Result Validation Errors");
    assert!(errors.contains(&"/rich-breadcrumb-no-position.html".to_owned()));
    assert!(!errors.contains(&"/rich-product-no-brand.html".to_owned()));
    assert!(!errors.contains(&"/rich-faq.html".to_owned()));
    let warnings = paths(&fx.file, &cfg, "Rich Result Validation Warnings");
    // The Product in schema-errors.html also lacks recommended Merchant listing properties.
    assert_eq!(
        warnings,
        [
            "/article.html",
            "/rich-product-no-brand.html",
            "/schema-errors.html"
        ]
    );
}

// spec: CRAWL-089
// spec: TECH-187
// spec: TECH-188
#[tokio::test]
async fn ignore_non_indexable_restricts_only_the_validation_filters() {
    let mut cfg = config();
    let fx = crawl(&cfg).await;
    let all = table(&fx.file, &cfg, "All");
    // Spalten bleiben gefüllt.
    assert_eq!(int(&all, "/noindex-errors.html", "Errors"), 1);
    assert!(int(&all, "/noindex-errors.html", "Rich Result Errors") > 0);
    assert!(int(&all, "/noindex-errors.html", "Rich Result Warnings") > 0);
    for filter in [
        "Validation Errors",
        "Rich Result Validation Errors",
        "Rich Result Validation Warnings",
    ] {
        assert!(
            !paths(&fx.file, &cfg, filter).contains(&"/noindex-errors.html".to_owned()),
            "{filter}"
        );
    }
    cfg.issues.ignore_non_indexable = false;
    for filter in [
        "Validation Errors",
        "Rich Result Validation Errors",
        "Rich Result Validation Warnings",
    ] {
        assert!(
            paths(&fx.file, &cfg, filter).contains(&"/noindex-errors.html".to_owned()),
            "{filter}"
        );
    }
}

// spec: TECH-189
#[tokio::test]
async fn detail_rows_list_each_counted_finding_once_with_property_paths_and_texts() {
    let cfg = config();
    let fx = crawl(&cfg).await;
    let all = table(&fx.file, &cfg, "All");
    let detail = |path: &str| {
        let d = os_store::queries::structured_data::detail_rows(
            &fx.file,
            &[url_id(&fx.file, path)],
            0,
            1000,
        )
        .unwrap();
        let col = |name: &str| d.columns.iter().position(|c| c == name).unwrap();
        let (v, s, issue, prop, feature, json) = (
            col("Validation Type"),
            col("Severity"),
            col("Issue"),
            col("Property"),
            col("Feature"),
            col("JSON"),
        );
        d.rows
            .iter()
            .filter(|r| r[v] != Value::Null)
            .map(|r| {
                let t = |i: usize| match &r[i] {
                    Value::Text(s) => s.clone(),
                    _ => String::new(),
                };
                (t(v), t(s), t(issue), t(prop), t(feature), t(json))
            })
            .collect::<Vec<_>>()
    };
    let rows = detail("/rich-product-missing-currency.html");
    assert_eq!(rows.len(), 1);
    let (validation, severity, code, property, feature, json) = &rows[0];
    assert_eq!(
        (
            validation.as_str(),
            severity.as_str(),
            code.as_str(),
            property.as_str(),
            feature.as_str()
        ),
        (
            "rich_results",
            "error",
            "sd.google.missing_required",
            "offers[0].priceCurrency",
            "Merchant listing"
        )
    );
    let item: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(
        detail_validation_label(validation, Some(feature)),
        "Google: Merchant listing"
    );
    assert_eq!(severity_label(severity), "Error");
    assert_eq!(
        finding_text(code, Some(property), Some(feature), Some(&item), Lang::De),
        "Eigenschaft `priceCurrency` fehlt (Pflicht für Merchant listing)."
    );
    assert_eq!(
        finding_text(code, Some(property), Some(feature), Some(&item), Lang::En),
        "Property `priceCurrency` is missing (required for Merchant listing)."
    );
    assert_eq!(finding_type(&item, property).as_deref(), Some("Offer"));

    // Jede gezählte Befundzeile genau einmal: Zeilen je Art und Schwere = Zählspalten der Seite.
    for path in [
        "/schema-errors.html",
        "/rich-breadcrumb-no-position.html",
        "/noindex-errors.html",
        "/article.html",
    ] {
        let rows = detail(path);
        for (validation, severity, column) in [
            ("schema_org", "error", "Errors"),
            ("schema_org", "warning", "Warnings"),
            ("rich_results", "error", "Rich Result Errors"),
            ("rich_results", "warning", "Rich Result Warnings"),
        ] {
            let n = rows
                .iter()
                .filter(|r| r.0 == validation && r.1 == severity)
                .count();
            assert_eq!(n as i64, int(&all, path, column), "{path} {column}");
        }
        assert!(
            rows.iter().all(|r| r.2 != "sd.google.feature"),
            "Feature-Fakten sind keine Befunde"
        );
    }
    let schema = detail("/schema-errors.html");
    let texts: Vec<_> = schema
        .iter()
        .filter(|r| r.0 == "schema_org")
        .map(|r| {
            let item: serde_json::Value = serde_json::from_str(&r.5).unwrap();
            (
                validation_label(&r.0),
                finding_text(&r.2, Some(&r.3), None, Some(&item), Lang::En),
                finding_link(&r.2, Some(&r.3)),
            )
        })
        .collect();
    assert_eq!(
        texts,
        [
            (
                "Schema.org",
                "Type `Prodcut` is not defined in the schema.org vocabulary.".to_owned(),
                None
            ),
            (
                "Schema.org",
                "Property `pricee` is not defined in the schema.org vocabulary.".to_owned(),
                None
            ),
            (
                "Schema.org",
                "Property `recipeIngredient` is not expected on type `Product`.".to_owned(),
                Some("https://schema.org/recipeIngredient".to_owned())
            ),
            (
                "Schema.org",
                "The value of `author` has type `Product`, which this property does not expect."
                    .to_owned(),
                Some("https://schema.org/author".to_owned())
            ),
            (
                "Schema.org",
                "The value of `dateCreated` does not match the expected data type.".to_owned(),
                Some("https://schema.org/dateCreated".to_owned())
            ),
        ]
    );
    let offers = detail("/rich-product-no-offers.html");
    assert_eq!(
        finding_text(
            &offers[0].2,
            Some(&offers[0].3),
            Some(&offers[0].4),
            None,
            Lang::En
        ),
        "At least one of `review`, `aggregateRating`, `offers` is required for Product snippet."
    );
}

// spec: TECH-188
#[tokio::test]
async fn rules_and_vocabulary_versions_are_stored_with_the_crawl() {
    let cfg = config();
    let fx = crawl(&cfg).await;
    let versions: Vec<(String, String)> = fx
        .file
        .conn()
        .prepare("SELECT DISTINCT key, v_text FROM url_ext WHERE key LIKE 'sdval.%' ORDER BY key")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        versions,
        [
            ("sdval.rules_version".to_owned(), os_parse_rules_version()),
            ("sdval.vocab_version".to_owned(), "30.1".to_owned()),
        ]
    );
}

fn os_parse_rules_version() -> String {
    let rules: serde_json::Value = serde_json::from_str(include_str!(
        "../../os-parse/resources/rich-results/rules.json"
    ))
    .unwrap();
    rules["version"].as_str().unwrap().to_owned()
}
