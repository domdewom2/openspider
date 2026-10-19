#[path = "bulk_web_support.rs"]
mod support;
use os_export::{save_report, Format};
use support::*;

// spec: OUT-111
// spec: CRAWL-062
#[test]
fn cookie_summary_groups_by_the_seven_attributes_and_counts_distinct_urls() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.rendering.mode = os_model::RenderingMode::JavaScript;
    let result = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    let text = std::fs::read_to_string(&result.files[0].path).unwrap();
    golden("cookie-summary.csv", &text);
    let rows = records(&text);
    let shared = rows.iter().find(|r| r[0] == "shared").unwrap();
    assert_eq!(
        shared,
        &[
            "shared",
            ".site.test",
            "/",
            "1",
            "True",
            "True",
            "Lax",
            "HTTP",
            "True",
            "12",
            "https://www.site.test/01"
        ]
    );
    assert_eq!(rows.iter().filter(|r| r[0] == "third").count(), 2);
    assert!(rows
        .iter()
        .filter(|r| r[0] == "third")
        .all(|r| r[8] == "False"));
    assert_eq!(rows.iter().find(|r| r[0] == "expires").unwrap()[3], "1");
    assert_eq!(
        rows.iter().find(|r| r[0] == "session").unwrap()[3],
        "Session"
    );
    // Each attribute in the specified identity produces a separate group.
    for (position, assignment) in [
        (10, "name='other'"),
        (11, "domain='other.test'"),
        (12, "path='/other'"),
        (13, "secure=0"),
        (14, "http_only=0"),
        (15, "same_site='Strict'"),
        (16, "origin='javascript'"),
    ] {
        file.conn().execute(&format!("INSERT INTO cookies SELECT url_id,{position},origin,name,value,domain,path,expires,max_age,secure,http_only,same_site,raw FROM cookies WHERE url_id=1 AND position=0"),[]).unwrap();
        file.conn()
            .execute(
                &format!("UPDATE cookies SET {assignment} WHERE url_id=1 AND position={position}"),
                [],
            )
            .unwrap();
    }
    let changed = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(changed.files[0].rows, result.files[0].rows + 7);
}

// spec: OUT-112 [M2]
#[test]
fn cookie_report_availability_and_cli_warning_explain_missing_extraction() {
    let dir = tempfile::tempdir().unwrap();
    let file = file(dir.path());
    let mut cfg = config();
    cfg.extraction.cookies = false;
    let entry = &os_export::reports::cookies::entries()[0];
    let state = os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg);
    assert!(!state.enabled);
    assert_eq!(
        state.reason.as_deref(),
        Some("report 'Cookie Summary' requires cookie extraction")
    );
    let result = save_report(&file, &cfg, "Cookie Summary", dir.path(), Format::Csv).unwrap();
    assert_eq!(
        result.warnings,
        vec!["report 'Cookie Summary' requires cookie extraction"]
    );
    assert_eq!(result.files[0].rows, 0);
    cfg.extraction.cookies = true;
    assert!(
        os_export::reports::prereq::availability(entry.name, entry.requires, &file, &cfg).enabled
    );
}
