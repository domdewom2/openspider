//! M2-I04: Lighthouse-/PSI-Antworten → Spaltenwerte gegen die Mock-Fixtures (`os_testsite::mocks::psi`).
use os_integrations::psi::{
    columns::{fail_metric, Group, COLUMNS, FILTERS},
    lighthouse::{
        crux_category, parse_crux, parse_lighthouse, parse_psi, score_category, score_color, Cell,
    },
};
use os_testsite::mocks::psi as fx;
use serde_json::json;

const URL: &str = "https://site.test/heavy";

fn cell(c: &fx::Cell) -> Cell {
    match c {
        fx::Cell::Num(n) => Cell::Num(*n),
        fx::Cell::Text(t) => Cell::Text((*t).to_string()),
    }
}

fn get<'a>(out: &'a os_integrations::psi::lighthouse::Outcome, name: &str) -> Option<&'a Cell> {
    out.get(name)
}

fn assert_golden(out: &os_integrations::psi::lighthouse::Outcome, golden: &[(&str, fx::Cell)]) {
    for (name, want) in golden {
        assert_eq!(get(out, name), Some(&cell(want)), "Spalte {name}");
    }
}

// spec: INTEG-134
#[test]
fn overview_metrics_of_psi_heavy_match_the_golden_values() {
    let out = parse_psi(&fx::psi_heavy(URL, false));
    let overview: Vec<_> = fx::expected_heavy()
        .into_iter()
        .filter(|(n, _)| os_integrations::psi::columns::group_of(n) == Some(Group::Overview))
        .collect();
    assert_eq!(overview.len(), 20, "alle Overview-Spalten im Golden");
    assert_golden(&out, &overview);
    // Summe HTML … Other = Total Page Size; Third Party ist Querschnitt und zählt nicht mit.
    let num = |n: &str| match get(&out, n) {
        Some(Cell::Num(v)) => *v,
        other => panic!("{n}: {other:?}"),
    };
    let sum: f64 = [
        "HTML",
        "Image",
        "CSS",
        "JavaScript",
        "Font",
        "Media",
        "Other",
    ]
    .iter()
    .map(|t| num(&format!("{t} Size")))
    .sum();
    assert_eq!(sum, num("Total Page Size (Bytes)"));
    // Größen sind ganzzahlige übertragene Bytes.
    for (_, name, _) in COLUMNS.iter().filter(|(g, _, _)| *g == Group::Overview) {
        assert_eq!(num(name).fract(), 0.0, "{name}");
    }
}

// spec: INTEG-134
#[test]
fn cache_savings_are_not_counted_twice_when_both_cache_audits_are_present() {
    let mut lh = fx::lighthouse_heavy(URL, false);
    lh["audits"]["uses-long-cache-ttl"] = json!({
        "id": "uses-long-cache-ttl", "score": 0.5, "scoreDisplayMode": "metricSavings",
        "details": {"type": "table", "items": [], "overallSavingsBytes": 999_999}
    });
    let out = parse_lighthouse(&lh);
    assert_eq!(
        get(&out, "Total Size Savings (Bytes)"),
        Some(&Cell::Num(952_812.0)),
        "nur cache-insight zählt"
    );
    // Ohne cache-insight zählt das ältere Audit.
    let mut legacy = fx::lighthouse_fast(URL, false);
    legacy["audits"]
        .as_object_mut()
        .unwrap()
        .remove("cache-insight");
    legacy["audits"]["uses-long-cache-ttl"] = lh["audits"]["uses-long-cache-ttl"].clone();
    assert_eq!(
        get(&parse_lighthouse(&legacy), "Total Size Savings (Bytes)"),
        Some(&Cell::Num(999_999.0))
    );
}

// spec: INTEG-135
#[test]
fn crux_full_maps_url_and_origin_values_categories_and_assessment() {
    let out = parse_psi(&fx::psi_fast("https://site.test/crux-full", false));
    assert_golden(&out, &fx::expected_crux());
    assert_eq!(crux_category("FAST"), Some("Good"));
    assert_eq!(crux_category("AVERAGE"), Some("Needs Improvement"));
    assert_eq!(crux_category("SLOW"), Some("Poor"));
    // Desktop: alles FAST → Pass.
    let desktop = parse_psi(&fx::psi_fast("https://site.test/crux-full", true));
    assert_eq!(
        get(&desktop, "Core Web Vitals Assessment"),
        Some(&Cell::Text("Pass".into()))
    );
}

fn body_with_crux(variant: &str) -> serde_json::Value {
    // Der Mock baut die CrUX-Varianten selbst; hier dieselben Bausteine über den Router-Pfad.
    let mut body = fx::psi_fast(&format!("https://site.test/{variant}"), false);
    match variant {
        "crux-origin-only" => body["loadingExperience"] = json!({"id": "x"}),
        "crux-fallback" => {
            let origin = body["originLoadingExperience"].clone();
            body["loadingExperience"] = origin;
            body["loadingExperience"]["origin_fallback"] = json!(true);
        }
        "crux-none" => {
            let map = body.as_object_mut().unwrap();
            map.remove("loadingExperience");
            map.remove("originLoadingExperience");
        }
        _ => {}
    }
    body
}

// spec: INTEG-135
#[test]
fn crux_without_url_data_keeps_url_columns_empty_and_fills_origin() {
    for variant in ["crux-origin-only", "crux-fallback"] {
        let cells = parse_crux(&body_with_crux(variant));
        assert!(
            cells.iter().all(|(n, _)| n.starts_with("CrUX Origin")),
            "{variant}: keine URL-Spalten aus Origin-Daten: {cells:?}"
        );
        assert!(
            cells
                .iter()
                .any(|(n, c)| n == "CrUX Origin Largest Contentful Paint (sec)"
                    && *c == Cell::Num(4.2)),
            "{variant}"
        );
    }
    assert!(parse_crux(&body_with_crux("crux-none")).is_empty());
    // Ohne LCP bleibt die Gesamtbewertung leer.
    let mut body = fx::psi_fast(URL, false);
    body["loadingExperience"]["metrics"]
        .as_object_mut()
        .unwrap()
        .remove("LARGEST_CONTENTFUL_PAINT_MS");
    let cells = parse_crux(&body);
    assert!(!cells.iter().any(|(n, _)| n == "Core Web Vitals Assessment"));
    // INP fehlt: LCP und CLS Good genügen für Pass.
    let mut body = fx::psi_fast(URL, true);
    body["loadingExperience"]["metrics"]
        .as_object_mut()
        .unwrap()
        .remove("INTERACTION_TO_NEXT_PAINT");
    let cells = parse_crux(&body);
    assert!(cells
        .iter()
        .any(|(n, c)| n == "Core Web Vitals Assessment" && *c == Cell::Text("Pass".into())));
}

// spec: INTEG-136
#[test]
fn lab_data_scores_times_and_version_follow_the_column_units() {
    let heavy = parse_psi(&fx::psi_heavy(URL, false));
    let lab: Vec<_> = fx::expected_heavy()
        .into_iter()
        .filter(|(n, _)| os_integrations::psi::columns::group_of(n) == Some(Group::Lighthouse))
        .collect();
    assert_golden(&heavy, &lab);
    // Lighthouse 13 liefert kein Time to Interactive mehr → Spalten leer.
    assert_eq!(get(&heavy, "Time to Interactive (sec)"), None);
    assert_eq!(get(&heavy, "Time to Interactive Score"), None);
    // 0,985 → 99 (ganzzahlig gerundet); TTI vorhanden → Sekunden mit zwei Stellen.
    let fast = parse_psi(&fx::psi_fast(URL, false));
    assert_golden(&fast, &fx::expected_fast());
    for (_, name, _) in COLUMNS
        .iter()
        .filter(|(g, n, _)| *g == Group::Lighthouse && n.ends_with("Score"))
    {
        if let Some(Cell::Num(v)) = get(&heavy, name) {
            assert!((0.0..=100.0).contains(v) && v.fract() == 0.0, "{name}");
        }
    }
    assert_eq!(score_color(100), "green");
    assert_eq!(score_color(90), "green");
    assert_eq!(score_color(89), "orange");
    assert_eq!(score_color(50), "orange");
    assert_eq!(score_color(49), "red");
    assert_eq!(score_color(0), "red");
}

// spec: INTEG-137
#[test]
fn insight_columns_of_psi_heavy_match_the_golden_values() {
    let out = parse_psi(&fx::psi_heavy(URL, false));
    let insights: Vec<_> = fx::expected_heavy()
        .into_iter()
        .filter(|(n, _)| os_integrations::psi::columns::group_of(n) == Some(Group::Insights))
        .collect();
    assert_eq!(
        insights.len(),
        COLUMNS
            .iter()
            .filter(|(g, _, _)| *g == Group::Insights)
            .count(),
        "jede Insights-Spalte hat einen Golden-Wert"
    );
    assert_golden(&out, &insights);
    // Bestandenes Audit ohne Einsparangabe → 0; nicht bestanden ohne Angabe → leer.
    let fast = parse_psi(&fx::psi_fast(URL, false));
    assert_eq!(
        get(&fast, "Use Efficient Cache Lifetimes Savings (Bytes)"),
        Some(&Cell::Num(0.0))
    );
    let mut lh = fx::lighthouse_fast(URL, false);
    lh["audits"]["cache-insight"]["score"] = json!(0);
    assert_eq!(
        get(
            &parse_lighthouse(&lh),
            "Use Efficient Cache Lifetimes Savings (Bytes)"
        ),
        None
    );
}

// spec: INTEG-138
#[test]
fn diagnostics_columns_categories_and_legacy_cache_audit() {
    let out = parse_psi(&fx::psi_heavy(URL, false));
    let diag: Vec<_> = fx::expected_heavy()
        .into_iter()
        .filter(|(n, _)| os_integrations::psi::columns::group_of(n) == Some(Group::Diagnostics))
        .collect();
    assert_golden(&out, &diag);
    // Lighthouse 13 ohne uses-long-cache-ttl → leer, nicht 0.
    assert_eq!(get(&out, "Efficient Cache Policy Savings (Bytes)"), None);
    let legacy = parse_psi(&fx::psi_legacy_cache(URL, false));
    assert_eq!(
        get(&legacy, "Efficient Cache Policy Savings (Bytes)"),
        Some(&Cell::Num(120_000.0))
    );
    assert_eq!(score_category(0.9), "Good");
    assert_eq!(score_category(0.5), "Average");
    assert_eq!(score_category(0.4999), "Slow");
    assert_eq!(score_category(1.0), "Good");
    assert_eq!(score_category(0.0), "Slow");
    assert_eq!(get(&out, "DOM Size"), Some(&Cell::Num(2_345.0)));
}

// spec: INTEG-139
#[test]
fn psi_heavy_triggers_all_nineteen_filters_and_psi_fast_none() {
    assert_eq!(FILTERS.len(), 19);
    let heavy = parse_psi(&fx::psi_heavy(URL, false));
    let fast = parse_psi(&fx::psi_fast(URL, false));
    for (filter, _) in FILTERS {
        assert_eq!(
            get(&heavy, &fail_metric(filter)),
            Some(&Cell::Num(1.0)),
            "heavy: {filter}"
        );
        assert_eq!(
            get(&fast, &fail_metric(filter)),
            Some(&Cell::Num(0.0)),
            "fast: {filter}"
        );
    }
    // Spec-Reihenfolge und Gruppen stimmen mit der Fixture-Liste überein.
    let ours: Vec<(&str, &str)> = FILTERS.iter().map(|(f, g)| (*f, g.key())).collect();
    assert_eq!(ours, fx::FILTERS.to_vec());
    // Nicht anwendbare/manuelle Audits gelten als bestanden.
    let mut lh = fx::lighthouse_heavy(URL, false);
    lh["audits"]["forced-reflow-insight"]["scoreDisplayMode"] = json!("manual");
    lh["audits"]["dom-size-insight"]["scoreDisplayMode"] = json!("notApplicable");
    let out = parse_lighthouse(&lh);
    assert_eq!(
        get(&out, &fail_metric("Forced Reflow")),
        Some(&Cell::Num(0.0))
    );
    assert_eq!(
        get(&out, &fail_metric("Optimize DOM Size")),
        Some(&Cell::Num(0.0))
    );
}

// spec: INTEG-143
#[test]
fn runtime_error_yields_error_status_message_and_no_filter_results() {
    let out = parse_psi(&fx::errored_document("https://site.test/x", false));
    assert_eq!(out.get("PSI Status"), Some(&Cell::Text("Error".into())));
    match out.get("PSI Error") {
        Some(Cell::Text(t)) => assert!(t.starts_with("ERRORED_DOCUMENT_REQUEST "), "{t}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        out.cells.len(),
        2,
        "ein Fehler füllt keine Spalten und keinen Filter"
    );
    let ok = parse_psi(&fx::psi_fast(URL, false));
    assert_eq!(ok.get("PSI Status"), Some(&Cell::Text("Success".into())));
    assert_eq!(ok.get("PSI Error"), None);
}
