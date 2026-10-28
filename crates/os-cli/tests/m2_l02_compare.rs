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
