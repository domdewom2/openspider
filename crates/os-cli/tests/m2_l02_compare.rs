mod common;
use common::Env;
use os_model::CrawlConfig;
#[path = "../../os-analyze/tests/change_detection_fixture.rs"]
mod fixture;

// spec: OUT-202
#[test]
fn explicit_pair_is_order_independent_and_self_comparison_is_rejected() {
    let env = Env::new();
    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let old = fixture::fixture(&env.crawls_dir(), 1, 1, CrawlConfig::default());
    let new = fixture::fixture(&env.crawls_dir(), 2, 2, CrawlConfig::default());
    let ids = [
        old.meta().unwrap().crawl_id.to_string(),
        new.meta().unwrap().crawl_id.to_string(),
    ];
    for pair in [[&ids[0], &ids[1]], [&ids[1], &ids[0]]] {
        let result = env.run(&[
            "--crawl-comparison",
            pair[0],
            pair[1],
            "--save-report",
            "Crawl Overview",
            "--export-tabs",
            "Change Detection:Page Titles",
            "--output-folder",
            env.out().to_str().unwrap(),
            "--overwrite",
        ]);
        assert_eq!(result.code, 0, "{}", result.dump());
        let overview = std::fs::read_to_string(env.out().join("crawl_overview.csv")).unwrap();
        assert!(overview
            .lines()
            .next()
            .unwrap()
            .contains("Current,Previous,Change"));
        let change =
            std::fs::read_to_string(env.out().join("change_detection_page_titles.csv")).unwrap();
        assert!(change.contains("shop,Shop"));
    }
    let result = env.run(&["--crawl-comparison", &ids[0], &ids[0]]);
    assert_eq!(result.code, 2, "{}", result.dump());
}

// spec: OUT-202
#[test]
fn optional_boolean_is_case_insensitive_and_does_not_consume_the_next_flag() {
    use clap::Parser;
    let home = tempfile::tempdir().unwrap();
    let paths = os_store::paths::app_paths_with_home(Some(home.path().as_os_str()));
    for (tail, enabled) in [
        (vec!["--project-crawl-comparison"], true),
        (vec!["--project-crawl-comparison", "--headless"], true),
        (vec!["--project-crawl-comparison", "TRUE"], true),
        (vec!["--project-crawl-comparison", "FaLsE"], false),
        (vec!["--compare-with-previous"], true),
    ] {
        let cli = os_cli::Cli::try_parse_from(
            [
                "openspider",
                "--crawl",
                "http://site.test/",
                "--project-name",
                "p",
            ]
            .into_iter()
            .chain(tail),
        )
        .unwrap();
        let plan = os_cli::plan::build(
            &cli.run,
            paths.clone(),
            &os_cli::reporter::Reporter::new(false),
        )
        .unwrap();
        assert_eq!(plan.compare_with_previous, enabled);
    }
    let env = Env::new();
    for tail in [
        vec!["--project-crawl-comparison"],
        vec!["--compare-with-previous"],
        vec!["--project-crawl-comparison", "yes"],
        vec!["--project-crawl-comparison", "1"],
        vec!["--project-crawl-comparison", "MeinProjekt"],
        vec![
            "--compare-with-previous",
            "--project-crawl-comparison",
            "false",
        ],
    ] {
        let result = env.run(&[vec!["--crawl", "http://site.test/"], tail].concat());
        assert_eq!(result.code, 2, "{}", result.dump());
        assert!(
            result.stderr.contains("Projektname fehlt")
                || result.stderr.contains("true oder false"),
            "{}",
            result.dump()
        );
    }
}

// spec: OUT-152
// spec: OUT-202
#[test]
fn explicit_comparison_uses_invocation_mapping_and_elements() {
    use os_model::{ChangeElement, MappingRule};
    let env = Env::new();
    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let mut stored = CrawlConfig::default();
    stored.compare.change_detection.elements = vec![ChangeElement::PageTitles];
    let old = fixture::fixture(&env.crawls_dir(), 1, 1, stored.clone());
    let new = fixture::fixture(&env.crawls_dir(), 2, 2, stored);
    old.conn().execute_batch("UPDATE urls SET url=replace(url,'site.test','old.test'), address=replace(address,'site.test','old.test')").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.compare.mapping = vec![MappingRule {
        pattern: r"old\.test".into(),
        replacement: "site.test".into(),
    }];
    cfg.compare.change_detection.elements = vec![ChangeElement::H1];
    let config = env.home.path().join("compare.osconfig");
    cfg.save_file(&config).unwrap();
    let output = env.out();
    for configured in [false, true] {
        let mut args = vec![
            "--crawl-comparison",
            old.path().to_str().unwrap(),
            new.path().to_str().unwrap(),
            "--export-tabs",
            if configured {
                "Change Detection:H1"
            } else {
                "Change Detection:Page Titles"
            },
            "--output-folder",
            output.to_str().unwrap(),
            "--overwrite",
        ];
        if configured {
            args.extend(["--config", config.to_str().unwrap()]);
        }
        let result = env.run(&args);
        assert_eq!(result.code, 0, "{}", result.dump());
        let csv = std::fs::read_to_string(env.out().join(if configured {
            "change_detection_h1.csv"
        } else {
            "change_detection_page_titles.csv"
        }))
        .unwrap();
        let mut reader = csv::Reader::from_reader(csv.as_bytes());
        let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), usize::from(configured), "{csv}");
        if configured {
            assert!(csv.contains("H1-1 (Current)"));
            assert!(!csv.contains("Title 1 (Current)"));
            assert!(rows[0].iter().any(|v| v == "https://site.test/c"), "{csv}");
            assert!(!result.stderr.contains("gespeicherte Konfiguration"));
        }
    }
}

// spec: OUT-011
// spec: OUT-113
#[test]
fn comparison_reports_keep_named_and_all_segments() {
    let env = Env::new();
    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let cfg = CrawlConfig::load_str(
        r##"
[[segments.list]]
name = "Blog"
color = "#336699"
[[segments.list.groups]]
[[segments.list.groups.conditions]]
field = "Address"
op = "contains"
value = "/a"
"##,
    )
    .unwrap()
    .config;
    let old = fixture::fixture(&env.crawls_dir(), 1, 1, cfg.clone());
    let new = fixture::fixture(&env.crawls_dir(), 2, 2, cfg.clone());
    for file in [&old, &new] {
        file.conn()
            .execute_batch("UPDATE urls SET status_code=301,redirect_target_id=3 WHERE id IN (1,2)")
            .unwrap();
        os_analyze::segments::reassign(file, &cfg, &os_model::CancelToken::new()).unwrap();
    }
    for segment in ["Blog", "all"] {
        let result = env.run(&[
            "--crawl-comparison",
            old.path().to_str().unwrap(),
            new.path().to_str().unwrap(),
            "--segment",
            segment,
            "--save-report",
            "Redirects:All Redirects,Crawl Overview",
            "--output-folder",
            env.out().to_str().unwrap(),
            "--overwrite",
        ]);
        assert_eq!(result.code, 0, "{}", result.dump());
        let rows = common::read_csv(&env.out().join("all_redirects_segment_blog.csv"));
        assert_eq!(common::column(&rows, "Address"), ["https://site.test/a"]);
        let overview = common::read_csv(&env.out().join("crawl_overview_segment_blog.csv"));
        assert!(overview[0].iter().any(|s| s == "Previous"));
        let html = overview
            .iter()
            .find(|r| r[0] == "Internal" && r[1] == "HTML")
            .unwrap();
        assert_eq!(&html[2..4], ["1", "1"]);
        if segment == "all" {
            let all = common::read_csv(&env.out().join("all_redirects.csv"));
            assert_eq!(common::column(&all, "Address").len(), 2);
        } else {
            assert!(!env.out().join("all_redirects.csv").exists());
        }
    }
}

// spec: OUT-112
// spec: OUT-113
// spec: OUT-204
#[test]
fn comparison_report_warnings_are_counted_and_strict_fails() {
    let env = Env::new();
    std::fs::create_dir_all(env.crawls_dir()).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.extraction.cookies = false;
    let old = fixture::fixture(&env.crawls_dir(), 1, 1, cfg.clone());
    let new = fixture::fixture(&env.crawls_dir(), 2, 2, cfg);
    let output = env.out();
    for strict in [false, true] {
        let mut args = vec![
            "--crawl-comparison",
            old.path().to_str().unwrap(),
            new.path().to_str().unwrap(),
            "--save-report",
            "Cookie Summary",
            "--output-folder",
            output.to_str().unwrap(),
            "--overwrite",
        ];
        if strict {
            args.push("--strict");
        }
        let result = env.run(&args);
        assert_eq!(result.code, if strict { 4 } else { 0 }, "{}", result.dump());
        assert!(
            result
                .stderr
                .contains("report 'Cookie Summary' requires cookie extraction"),
            "{}",
            result.dump()
        );
        assert!(result.stderr.contains("1 Warnungen"), "{}", result.dump());
        assert!(output.join("cookie_summary.csv").is_file());
    }
}
