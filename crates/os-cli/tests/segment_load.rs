//! Additive CRAWL-134 regression for the load/export path without a crawl observer.
mod common;
#[path = "../../os-export/tests/links_support.rs"]
mod fixture;

use os_analyze::{
    search::FilterSource,
    segments::{self, SegmentFilter},
};
use os_model::{SegmentCondition, SegmentConfig, SegmentGroup, SegmentOp};
use os_store::{CrawlFile, OpenMode};

// spec: CRAWL-134
// spec: OUT-011
#[test]
fn load_without_resume_refreshes_saved_segments_after_finishing_analysis() {
    let env = common::Env::new();
    let path = env.work.path().join("pending.ospider");
    let file = fixture::new_file(env.work.path(), "pending.ospider");
    fixture::exec(
        &file,
        include_str!("../../../tests/fixtures/out-m2-export/crawl.sql"),
    );
    file.set_meta("state", "Completed").unwrap();
    let mut cfg = file.config().unwrap();
    cfg.analysis.auto_at_end = false;
    cfg.analysis.near_duplicates = true;
    cfg.duplicates.near_enabled = true;
    cfg.segments.list = vec![SegmentConfig {
        name: "Analysed".into(),
        color: "#336699".into(),
        groups: vec![SegmentGroup {
            conditions: vec![SegmentCondition {
                field: "No. Near Duplicates".into(),
                op: SegmentOp::IsEmpty,
                value: String::new(),
            }],
        }],
    }];
    file.set_config(&cfg).unwrap();
    let pending = segments::reassign(&file, &cfg, &Default::default()).unwrap();
    assert_eq!(pending.pending.len(), 1);
    assert!(pending.segments[0].is_empty());
    drop(file);

    let run = env.run(&[
        "--load-crawl",
        path.to_str().unwrap(),
        "--export-tabs",
        "Internal:All",
        "--segment",
        "Analysed",
        "--output-folder",
        "out",
        "--overwrite",
    ]);
    assert_eq!(run.code, 0, "{}", run.dump());
    let rows = common::read_csv(&env.out().join("internal_all_segment_analysed.csv"));
    let mut addresses = common::column(&rows, "Address");
    addresses.sort();
    assert_eq!(
        addresses,
        [
            "https://site.test/a",
            "https://site.test/b",
            "https://site.test/c"
        ]
    );
    // The export and subsequent views read the same persisted assignment.
    let file = CrawlFile::open(&path, OpenMode::ReadOnly).unwrap();
    assert_eq!(
        SegmentFilter::new("Analysed").bitmap(&file).unwrap().len(),
        3
    );
    assert!(segments::preview(&file, &cfg, &Default::default())
        .unwrap()
        .pending
        .is_empty());
}
