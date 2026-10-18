//! Full cross-package assertion; M2-N01 removes the placeholder once D08 is integrated.
#[path = "graph_fixture.rs"]
mod fx;
use os_analyze::graph::{Snapshot, WordOptions};
use os_model::{UrlId, Value};

// spec: UI-099
#[test]
#[ignore = "wartet auf M2-D08"]
fn phrases_equal_the_anchors_detail_aggregation() {
    let (_dir, file) = fx::basic();
    for _ in 0..3 {
        fx::link(&file, 1, 2, " Produkte ", "", true, "Hyperlink");
    }
    fx::link(&file, 3, 2, "Produkte", "", false, "Hyperlink");
    fx::link(&file, 2, 2, "Produkte", "", true, "Hyperlink");
    fx::link(&file, 3, 2, "produkte", "", true, "Hyperlink");
    fx::link(&file, 3, 2, "", "Logo", true, "Hyperlink");
    let words = Snapshot::capture(&file)
        .unwrap()
        .words(2, &WordOptions::default())
        .unwrap();
    let anchors = os_store::queries::anchors::detail_rows(&file, &[UrlId(2)], 0, 100).unwrap();
    let actual: Vec<_> = anchors
        .rows
        .iter()
        .map(|row| match (&row[0], &row[1]) {
            (Value::Text(term), Value::Int(count)) => (term.clone(), *count as u64),
            other => panic!("unexpected Anchors columns: {other:?}"),
        })
        .collect();
    assert_eq!(words, actual);
    assert_eq!(words[0], ("Produkte".into(), 4));
}
