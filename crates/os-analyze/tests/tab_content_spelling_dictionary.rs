//! Persönliches Wörterbuch über Crawls hinweg (ONPAGE-157, M2-D19). Eigene Testdatei, weil der Test
//! `OPENSPIDER_HOME` für den ganzen Prozess setzt: ohne App gilt `<OPENSPIDER_HOME>/dictionaries/personal/<Code>.txt`.

use std::path::PathBuf;

use os_analyze::views::{query_view, ColumnSelection, ViewQuery};
use os_engine::testing::crawl_fixture;
use os_model::{CrawlConfig, Value};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/ui/fx-ui-spell")
}

fn spelling_errors(
    cfg: &CrawlConfig,
    file: &os_store::CrawlFile,
    base: &str,
    filter: &str,
) -> Vec<(String, Value)> {
    let mut q = ViewQuery::new("Content", filter);
    q.columns = ColumnSelection::Named(vec!["Address".into(), "Spelling Errors".into()]);
    q.limit = 500;
    let page = query_view(file, cfg, &q).unwrap();
    page.rows
        .into_iter()
        .map(|r| match &r[0] {
            Value::Text(a) => (a.strip_prefix(base).unwrap_or(a).to_string(), r[1].clone()),
            other => panic!("{other:?}"),
        })
        .collect()
}

// spec: ONPAGE-157
// spec: ONPAGE-155
#[tokio::test]
async fn words_of_the_personal_dictionary_count_in_every_following_crawl() {
    let home = tempfile::tempdir().unwrap();
    std::env::set_var("OPENSPIDER_HOME", home.path());
    std::env::set_var("OPENSPIDER_DICTIONARIES", fixture().join("dictionaries"));
    let mut cfg = CrawlConfig::default();
    cfg.spelling.enabled = true;
    cfg.spelling
        .default_variants
        .insert("de".into(), "de-DE".into());

    let first = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
    let base = first.site.base_url();
    let rows = spelling_errors(&cfg, &first.file, &base, "All");
    assert!(
        rows.contains(&("/de-errors.html".into(), Value::Int(3))),
        "{rows:?}"
    );
    assert!(spelling_errors(&cfg, &first.file, &base, "Spelling Errors")
        .iter()
        .any(|(a, _)| a == "/de-errors.html"));

    // „Zum Wörterbuch hinzufügen“ für die drei Fehlerwörter (Datei je Sprache, ein Wort je Zeile).
    let personal = home.path().join("dictionaries").join("personal");
    std::fs::create_dir_all(&personal).unwrap();
    std::fs::write(
        personal.join("de-DE.txt"),
        "Kontackt\nBeschreibng\nRechtschreibfeler\n",
    )
    .unwrap();

    let second = crawl_fixture("ui/fx-ui-spell", cfg.clone()).await;
    let base = second.site.base_url();
    let rows = spelling_errors(&cfg, &second.file, &base, "All");
    assert!(
        rows.contains(&("/de-errors.html".into(), Value::Int(0))),
        "{rows:?}"
    );
    assert!(
        !spelling_errors(&cfg, &second.file, &base, "Spelling Errors")
            .iter()
            .any(|(a, _)| a == "/de-errors.html")
    );
}
