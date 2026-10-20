//! M2-I04: Quelle und Metrikgruppen (INTEG-130, INTEG-133).
use os_integrations::psi::{
    columns::{Group, COLUMNS, FILTERS},
    settings::{dialog_groups, Selection, Source},
};
use os_model::CrawlConfig;

fn cfg(groups: &[&str], local: bool) -> os_model::config::PsiConfig {
    let mut psi = CrawlConfig::default().api.psi;
    psi.metric_groups = groups.iter().map(|g| g.to_string()).collect();
    psi.local = local;
    psi
}

const DIAGNOSTICS: [&str; 16] = [
    "Minify CSS Savings (ms)",
    "Minify CSS Savings (Bytes)",
    "Minify JavaScript Savings (ms)",
    "Minify JavaScript Savings (Bytes)",
    "Reduce Unused CSS Savings (ms)",
    "Reduce Unused CSS Savings (Bytes)",
    "Reduce Unused JavaScript Savings (ms)",
    "Reduce Unused JavaScript Savings (Bytes)",
    "JavaScript Execution Time (sec)",
    "JavaScript Execution Time Category",
    "Efficient Cache Policy Savings (Bytes)",
    "Minimize Main-Thread Work (sec)",
    "Minimize Main-Thread Work Category",
    "DOM Size",
    "Font Display Savings (ms)",
    "User Timing Marks and Measures",
];

// spec: INTEG-133
#[test]
fn deselecting_diagnostics_removes_exactly_its_columns_and_filters() {
    let all = Selection::of(&cfg(&[], false));
    let without = Selection::of(&cfg(
        &["overview", "crux", "lighthouse", "insights", "mobile"],
        false,
    ));
    let removed: Vec<&str> = all
        .columns()
        .into_iter()
        .filter(|c| !without.column(c))
        .collect();
    assert_eq!(removed, DIAGNOSTICS);
    let hidden: Vec<&str> = FILTERS
        .iter()
        .map(|(f, _)| *f)
        .filter(|f| !without.filter(f))
        .collect();
    assert_eq!(
        hidden,
        [
            "Minify CSS",
            "Minify JavaScript",
            "Reduce Unused CSS",
            "Reduce Unused JavaScript",
            "Reduce JavaScript Execution Time",
            "Minimize Main-Thread Work",
            "Optimize DOM Size",
            "Font Display"
        ]
    );
    assert!(without.filter("Avoid Enormous Network Payloads"));
    assert!(without.column("PSI Status") && without.column("PSI Error"));
}

// spec: INTEG-133
#[test]
fn deselecting_insights_removes_its_columns_and_filters_including_payloads() {
    let sel = Selection::of(&cfg(&["overview", "lighthouse", "diagnostics"], false));
    assert!(!sel.column("Network Payload Size (Bytes)"));
    assert!(!sel.column("Document Request Latency"));
    assert!(sel.column("DOM Size") && sel.column("Font Display Savings (ms)"));
    for (filter, group) in FILTERS {
        assert_eq!(sel.filter(filter), group == Group::Diagnostics, "{filter}");
    }
    // Spaltenreihenfolge: Overview, CrUX, Lighthouse, Insights, Diagnostics.
    let order: Vec<Group> = COLUMNS.iter().map(|(g, _, _)| *g).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    // Einzelne Metriken lassen sich ohne ganze Gruppe wählen.
    let single = Selection::of(&cfg(&["Performance Score"], false));
    assert_eq!(single.columns(), ["Performance Score"]);
    assert!(!single.filter("Minify CSS"));
    assert!(!single.group(Group::Mobile));
}

// spec: INTEG-130
#[test]
fn local_source_has_no_crux_columns_and_disables_the_crux_group() {
    let local = Selection::of(&cfg(&[], true));
    assert!(!local.group(Group::Crux));
    assert!(local
        .columns()
        .iter()
        .all(|c| !c.contains("CrUX") && *c != "Core Web Vitals Assessment"));
    let explicit = Selection::of(&cfg(&["crux", "CrUX Origin Time to First Byte (ms)"], true));
    assert!(explicit.columns().is_empty());
    assert_eq!(Source::of(&cfg(&[], true)), Source::Local);
    let dialog = dialog_groups(Source::Local);
    assert!(dialog.contains(&(Group::Crux, false)));
    assert!(dialog_groups(Source::Remote).iter().all(|(_, on)| *on));
}
