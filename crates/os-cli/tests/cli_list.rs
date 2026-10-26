//! Listenmodus: `--crawl-list`, `--crawl-sitemap` (OUT-183, CRAWL-135) mit den Fixtures `out-list-order` und
//! `out-sitemaps-in`.

mod common;

use std::path::{Path, PathBuf};

use common::{column, read_csv, resolve, site, Env};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

fn args(site: &os_testsite::TestSite, extra: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = [
        "--resolve".to_string(),
        resolve(site),
        "--set".into(),
        "threads=1".into(),
    ]
    .into_iter()
    .collect();
    v.extend(extra.iter().map(|s| s.to_string()));
    v
}

fn paths(rows: &[Vec<String>]) -> Vec<String> {
    column(rows, "Address")
        .iter()
        .map(|a| {
            a.split_once("site.test")
                .map_or(a.clone(), |(_, p)| p.to_string())
        })
        .collect()
}

// spec: OUT-183
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ten_urls_with_two_duplicates_crawl_eight_urls_and_report_the_counters() {
    let site = site("out-list-order").await;
    let env = Env::new();
    let list = fixture("out-list-order/urls10.txt");
    let mut a = vec![
        "--crawl-list".to_string(),
        list.to_string_lossy().into_owned(),
    ];
    a.extend(args(
        &site,
        &[
            "--export-tabs",
            "Internal:All",
            "--output-folder",
            &env.out().to_string_lossy(),
        ],
    ));
    let run = env.run_async(a).await;
    assert_eq!(run.code, 0, "{}", run.dump());

    // Zähler wie im Zusammenfassungsdialog des Listenmodus (CRAWL-027)
    assert!(
        run.stderr.contains("Gelesen 10")
            && run.stderr.contains("Duplikate entfernt 2")
            && run.stderr.contains("Ungültig 0")
            && run.stderr.contains("Zu crawlen 8"),
        "{}",
        run.dump()
    );
    let rows = read_csv(&env.out().join("internal_all.csv"));
    assert_eq!(
        paths(&rows),
        [
            "/p1.html", "/p2.html", "/p3.html", "/p4.html", "/p5.html", "/p6.html", "/p7.html",
            "/p8.html", "/p1.html", "/p3.html"
        ]
    );
    // Jede Seite genau einmal abgerufen; im Listenmodus weder robots.txt noch Folgelinks.
    for i in 1..=8 {
        assert_eq!(site.count("site.test", &format!("/p{i}.html")), 1, "p{i}");
    }
    assert_eq!(site.total_requests(), 8);
    // Name des Crawls = Dateiname der Liste, Modus list.
    let list_json = env.run(&["--list-crawls", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&list_json.stdout).unwrap();
    assert_eq!(v[0]["name"], "urls10.txt");
    assert_eq!(v[0]["mode"], "list");
}

// spec: OUT-183
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_duplicate_and_excluded_lines_are_counted_and_not_crawled() {
    let site = site("out-list-order").await;
    let env = Env::new();
    let list = fixture("out-list-order/list.txt");
    let config = fixture("out-list-order/exclude.osconfig");
    let mut a = vec![
        "--crawl-list".to_string(),
        list.to_string_lossy().into_owned(),
        "--config".into(),
        config.to_string_lossy().into_owned(),
    ];
    a.extend(args(
        &site,
        &[
            "--export-tabs",
            "Internal:All",
            "--output-folder",
            &env.out().to_string_lossy(),
        ],
    ));
    let run = env.run_async(a).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    assert!(
        run.stderr.contains("Gelesen 7")
            && run.stderr.contains("Duplikate entfernt 1")
            && run.stderr.contains("Durch Exclude/Include entfernt 1")
            && run.stderr.contains("Zu crawlen 4"),
        "{}",
        run.dump()
    );
    let rows = read_csv(&env.out().join("internal_all.csv"));
    // Upload order retains duplicates, corrected addresses and invalid/excluded entries (OUT-015).
    assert!(run.stderr.contains("Ignorierte Zeilen 0"), "{}", run.dump());
    assert_eq!(
        paths(&rows),
        ["/p2.html", "/p1.html", "/p2.html", "/p3.html", "", "", "/p4.html"]
    );
    assert_eq!(site.count("site.test", "/privat/geheim.html"), 0);
}

// spec: OUT-183
#[test]
fn an_empty_list_and_a_missing_file_are_usage_errors() {
    let env = Env::new();
    let empty = env.work.path().join("leer.txt");
    std::fs::write(&empty, "# nur ein Kommentar\n\n").unwrap();
    let run = env.run(&["--crawl-list", &empty.to_string_lossy()]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(run.stderr.contains("Leere Liste"), "{}", run.dump());

    let run = env.run(&[
        "--crawl-list",
        &env.work.path().join("fehlt.txt").to_string_lossy(),
    ]);
    assert_eq!(run.code, 2, "{}", run.dump());
    assert!(run.stderr.contains("nicht gefunden"), "{}", run.dump());
    assert!(env.crawl_files().is_empty());
}

// spec: OUT-183
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crawl_list_and_crawl_sitemap_make_identical_crawls_through_an_index() {
    let site = site("out-sitemaps-in").await;
    let mut sequences: Vec<Vec<String>> = Vec::new();
    let mut urls: Vec<Vec<String>> = Vec::new();
    for flag in ["--crawl-list", "--crawl-sitemap"] {
        site.reset_request_log();
        let env = Env::new();
        let mut a = vec![
            flag.to_string(),
            "http://site.test/sitemap_index.xml".to_string(),
        ];
        a.extend(args(
            &site,
            &[
                "--export-tabs",
                "Internal:All",
                "--output-folder",
                &env.out().to_string_lossy(),
            ],
        ));
        let run = env.run_async(a).await;
        assert_eq!(run.code, 0, "{flag}: {}", run.dump());
        sequences.push(
            site.requests()
                .iter()
                .map(|r| r.path_and_query.clone())
                .collect(),
        );
        let rows = read_csv(&env.out().join("internal_all.csv"));
        urls.push(paths(&rows));
    }
    // alle URLs aller untergeordneten Sitemaps
    for i in 1..=5 {
        assert!(urls[0].contains(&format!("/s{i}.html")), "{:?}", urls[0]);
    }
    assert_eq!(
        urls[0], urls[1],
        "gleiche URL-Menge in gleicher Reihenfolge"
    );
    assert_eq!(
        sequences[0], sequences[1],
        "gleiche Reihenfolge im Request-Log"
    );
    assert!(sequences[0].iter().any(|p| p == "/sitemap-a.xml"));
}

// spec: OUT-183
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn several_crawl_list_sources_are_merged() {
    let site = site("out-list-order").await;
    let env = Env::new();
    let first = env.work.path().join("a.txt");
    let second = env.work.path().join("b.txt");
    std::fs::write(
        &first,
        "http://site.test/p1.html\nhttp://site.test/p2.html\n",
    )
    .unwrap();
    std::fs::write(
        &second,
        "http://site.test/p2.html\nhttp://site.test/p3.html\n",
    )
    .unwrap();
    let mut a = vec![
        "--crawl-list".to_string(),
        first.to_string_lossy().into_owned(),
        "--crawl-list".into(),
        second.to_string_lossy().into_owned(),
    ];
    a.extend(args(
        &site,
        &[
            "--export-tabs",
            "Internal:All",
            "--output-folder",
            &env.out().to_string_lossy(),
        ],
    ));
    let run = env.run_async(a).await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let rows = read_csv(&env.out().join("internal_all.csv"));
    assert_eq!(
        paths(&rows),
        ["/p1.html", "/p2.html", "/p2.html", "/p3.html"]
    );
}

// spec: OUT-183 [M2]
// spec: OUT-015 [M2]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_list_export_keeps_the_upload_order_including_duplicates() {
    let site = site("out-list-order").await;
    let env = Env::new();
    let list = fixture("out-m2-cli/upload.txt");
    let run = env
        .run_async(vec![
            "--crawl-list".into(),
            list.to_string_lossy().into_owned(),
            "--resolve".into(),
            resolve(&site),
            "--set".into(),
            "threads=1".into(),
            "--export-tabs".into(),
            "Internal:All".into(),
            "--output-folder".into(),
            env.out().to_string_lossy().into_owned(),
        ])
        .await;
    assert_eq!(run.code, 0, "{}", run.dump());
    let rows = read_csv(&env.out().join("internal_all.csv"));
    assert_eq!(rows[0][0], "Original Url");
    assert_eq!(
        column(&rows, "Original Url"),
        [
            "http://site.test/p2.html",
            "http://site.test/p1.html",
            "http://site.test/p2.html",
            "site.test/p3.html"
        ]
    );
    assert_eq!(site.count("site.test", "/p2.html"), 1);
}
