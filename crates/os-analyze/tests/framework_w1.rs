//! Infrastruktur (M2-F05a); Fachprüfung TECH-008 durch M2-D10.
mod framework_common;
use framework_common::*;
use os_analyze::{
    views::{self, ViewQuery},
    FilterDef, FilterKind, Requirement,
};
use os_model::{CrawlConfig, FilterState};

// spec: TECH-008
#[test]
fn coverage_states_count_and_return_only_checked_rows_inside_existing_transactions() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "coverage.ospider", 42);
    insert(&file, &[row(1, "/a"), row(2, "/b")]);
    let mut tab = os_analyze::defs::placeholder_tab("Fixture", Vec::new());
    tab.population = "1";
    let mut filter = FilterDef::placeholder("Checked", FilterKind::Live, &[]);
    filter.predicate = "1";
    filter.requires = vec![Requirement::Coverage {
        config: "advanced.html_validation",
        marker: "validation.checked",
    }];
    tab.filters = vec![filter];
    let tabs = vec![tab];
    let mut cfg = CrawlConfig::default();
    cfg.advanced.html_validation = false;
    assert!(matches!(
        views::state_of_in(&tabs, &file, &cfg, "Fixture", "Checked").unwrap(),
        FilterState::NeedsConfig(_)
    ));
    cfg.advanced.html_validation = true;
    assert_eq!(
        views::state_of_in(&tabs, &file, &cfg, "Fixture", "Checked").unwrap(),
        FilterState::NeedsRecrawl
    );
    file.conn()
        .execute(
            "INSERT INTO url_ext(url_id,key,v_int,change_seq) VALUES (1,'validation.checked',1,1)",
            [],
        )
        .unwrap();
    assert_eq!(
        views::state_of_in(&tabs, &file, &cfg, "Fixture", "Checked").unwrap(),
        FilterState::Incomplete { unchecked: 1 }
    );
    let tx = file.conn().unchecked_transaction().unwrap();
    let page =
        views::query_view_in(&tabs, &file, &cfg, &ViewQuery::new("Fixture", "Checked")).unwrap();
    assert_eq!(page.total, 1);
    let counts = views::filter_counts_in(&tabs, &file, &cfg).unwrap();
    assert_eq!(counts[0].count, Some(1));
    tx.rollback().unwrap();
    file.conn()
        .execute(
            "INSERT INTO url_ext(url_id,key,v_int,change_seq) VALUES (2,'validation.checked',1,2)",
            [],
        )
        .unwrap();
    assert_eq!(
        views::state_of_in(&tabs, &file, &cfg, "Fixture", "Checked").unwrap(),
        FilterState::Ready
    );
}

// spec: TECH-008
// Infrastruktur (M2-F05a) für TECH-008; Fachprüfung M2-D10.
#[test]
fn overview_batches_shared_and_distinct_coverage_markers_per_tab() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "markers.ospider", 42);
    insert(&file, &[row(1, "/a"), row(2, "/b")]);
    let mut tab = os_analyze::defs::placeholder_tab("Fixture", Vec::new());
    tab.population = "1";
    tab.filters = [
        ("Checked", "validation.checked"),
        ("Same Marker", "validation.checked"),
        ("Other Marker", "security.checked"),
    ]
    .into_iter()
    .map(|(name, marker)| {
        let mut f = FilterDef::placeholder(name, FilterKind::Live, &[]);
        f.predicate = "1";
        f.requires = vec![Requirement::Coverage {
            config: "advanced.html_validation",
            marker,
        }];
        f
    })
    .collect();
    file.conn().execute_batch("INSERT INTO url_ext(url_id,key,v_int,change_seq) VALUES(1,'validation.checked',1,1), (1,'security.checked',1,1), (2,'security.checked',1,1)").unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.advanced.html_validation = true;
    let counts = views::filter_counts_in(&[tab], &file, &cfg).unwrap();
    assert_eq!(
        counts.iter().map(|c| c.count).collect::<Vec<_>>(),
        vec![Some(1), Some(1), Some(2)]
    );
    assert_eq!(
        counts.iter().map(|c| c.state.clone()).collect::<Vec<_>>(),
        vec![
            FilterState::Incomplete { unchecked: 1 },
            FilterState::Incomplete { unchecked: 1 },
            FilterState::Ready
        ]
    );
}
