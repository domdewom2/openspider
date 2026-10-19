#[path = "graph_fixture.rs"]
mod fx;
use os_analyze::graph::{GraphOptions, Snapshot};

fn large(n: usize) -> (tempfile::TempDir, os_store::CrawlFile) {
    let (dir, file) = fx::file();
    file.conn().execute_batch(&format!(
        "BEGIN;
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < {n})
         INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,indexability)
         SELECT i, CASE i WHEN 1 THEN 'https://site.test/' ELSE 'https://site.test/p/' || i END,
          CASE i WHEN 1 THEN 'https://site.test/' ELSE 'https://site.test/p/' || i END,
          'https://site.test',1,'done','Hyperlink',0,30,'HTML','Indexable' FROM n;
         INSERT INTO discovery_edges(seq,target_id,source_id,kind)
          SELECT id,id, CASE WHEN id=1 THEN NULL WHEN id<=41 THEN 1 WHEN id<=2041 THEN 2+(id-42)%40 ELSE 42+(id-2042)%2000 END,'link' FROM urls;
         COMMIT;"
    )).unwrap();
    (dir, file)
}

// spec: UI-096
#[test]
fn fifty_thousand_node_fixture_caps_by_level_and_counts_all_descendants() {
    let (_dir, file) = large(50_000);
    let snapshot = Snapshot::capture(&file).unwrap();
    let graph = snapshot.graph(&GraphOptions::default()).unwrap();
    assert_eq!(graph.nodes.len(), 10_000);
    let levels: Vec<_> = (0..=3)
        .map(|d| graph.nodes.iter().filter(|n| n.depth == d).count())
        .collect();
    assert_eq!(levels, [1, 40, 2000, 7959]);
    assert_eq!(graph.nodes[0].hidden_descendants, 40_000);
    let id = graph
        .nodes
        .iter()
        .rev()
        .find(|n| n.hidden_children > 0)
        .unwrap()
        .id;
    let direct = graph
        .nodes
        .iter()
        .find(|n| n.id == id)
        .unwrap()
        .hidden_children as usize;
    let explored = snapshot
        .graph(&GraphOptions {
            explore_node: Some(id),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(explored.nodes.len(), 10_000 + direct);
    let focus = snapshot
        .graph(&GraphOptions {
            root: Some(2),
            ..Default::default()
        })
        .unwrap();
    assert!(!focus.truncated);
    assert!(focus.nodes.len() < 2_000);
}

// spec: UI-096
#[test]
fn explore_refuses_more_than_one_thousand_over_budget() {
    let (_dir, file) = large(2041);
    file.conn()
        .execute(
            "UPDATE discovery_edges SET source_id = 1 WHERE target_id != 1",
            [],
        )
        .unwrap();
    let snapshot = Snapshot::capture(&file).unwrap();
    assert!(snapshot
        .graph(&GraphOptions {
            budget: 1,
            expand_depth: 0,
            explore_node: Some(1),
            ..Default::default()
        })
        .is_err());
}

// spec: UI-091
#[test]
fn depth_twelve_fixture_expands_and_collapses_globally() {
    let (_dir, file) = fx::depth_twelve();
    let snapshot = Snapshot::capture(&file).unwrap();
    assert_eq!(
        snapshot
            .graph(&GraphOptions::default())
            .unwrap()
            .nodes
            .len(),
        4
    );
    let all = snapshot
        .graph(&GraphOptions {
            expand_depth: 12,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all.nodes.len(), 13);
    assert_eq!(all.nodes.last().unwrap().depth, 12);
    assert_eq!(all.edges.last(), Some(&(12, 13)));
}

// spec: UI-146
#[test]
#[ignore = "Benchmark: K16-Aufbereitung und Projektion auf ruhiger Maschine"]
fn ten_thousand_node_preparation_and_collapse_budget() {
    let (_dir, file) = large(10_000);
    let start = std::time::Instant::now();
    let snapshot = Snapshot::capture(&file).unwrap();
    let initial = snapshot.graph(&GraphOptions::default()).unwrap();
    let preparation = start.elapsed();
    let start = std::time::Instant::now();
    let collapsed = snapshot
        .graph(&GraphOptions {
            expand_depth: 1,
            ..Default::default()
        })
        .unwrap();
    let collapse = start.elapsed();
    eprintln!("10k preparation: {preparation:?}; collapse: {collapse:?}");
    assert_eq!(initial.nodes.len(), 10_000);
    assert_eq!(collapsed.nodes.len(), 41);
    assert!(preparation <= std::time::Duration::from_millis(500));
    assert!(collapse <= std::time::Duration::from_millis(200));
}

// spec: UI-090
// spec: UI-096
#[test]
fn capture_refuses_crawls_above_fifty_thousand_rows_before_loading_data() {
    let (_dir, file) = large(50_001);
    let error = Snapshot::capture(&file).unwrap_err().to_string();
    assert!(error.contains("urls") && error.contains("50000"), "{error}");
}

// spec: UI-090
// spec: UI-095
#[test]
fn capture_also_limits_metric_rows_in_small_crawls() {
    let (_dir, file) = fx::basic();
    file.conn()
        .execute_batch(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<250001)
         INSERT INTO url_ext(url_id,key,v_int) SELECT 1,'metric.' || i,i FROM n;",
        )
        .unwrap();
    let error = Snapshot::capture(&file).unwrap_err().to_string();
    assert!(
        error.contains("url_ext") && error.contains("250000"),
        "{error}"
    );
}
