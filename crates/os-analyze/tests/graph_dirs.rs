//! M3-V02: Verzeichnisgraph (UI-094) und 3D-Knotengrenze (UI-098) auf Kernebene.
#[path = "graph_fixture.rs"]
mod fx;

use os_analyze::graph::{CaptureScope, GraphOptions, Snapshot, MAX_BUDGET_2D, MAX_BUDGET_3D};
use os_engine::testing::crawl_fixture;
use os_model::CrawlConfig;
use serde_json::Value;

fn expected() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/ui/fx-ui-dirs/expected.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn all(directory: bool) -> GraphOptions {
    GraphOptions {
        directory,
        expand_depth: 100,
        ..Default::default()
    }
}

// spec: UI-094
// spec: UI-093
#[tokio::test]
async fn directory_structure_of_fx_ui_dirs_has_two_hollow_directories_and_query_siblings() {
    let exp = expected();
    let crawl = crawl_fixture("ui/fx-ui-dirs", CrawlConfig::default()).await;
    let snapshot = Snapshot::capture(&crawl.file).unwrap();
    let graph = snapshot.graph(&all(true)).unwrap();
    assert_eq!(
        graph.nodes.len() as u64,
        exp["directoryNodes"].as_u64().unwrap()
    );
    assert_eq!(
        graph.edges.len() as u64,
        exp["directoryEdges"].as_u64().unwrap()
    );
    let hollow: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.address.is_none())
        .map(|n| n.label.clone())
        .collect();
    assert_eq!(hollow.len() as u64, exp["hollowNodes"].as_u64().unwrap());
    for dir in exp["hollowDirectories"].as_array().unwrap() {
        assert!(
            hollow.contains(&dir.as_str().unwrap().to_owned()),
            "{hollow:?}"
        );
    }
    // Query-Variante als Kind ihrer Seite, abschließender Slash ohne leeren Kindknoten.
    let id = |suffix: &str| {
        graph
            .nodes
            .iter()
            .find(|n| n.address.as_deref().is_some_and(|a| a.ends_with(suffix)))
            .unwrap()
            .id
    };
    let post = id(exp["querySiblingsUnder"].as_str().unwrap());
    assert!(graph.edges.contains(&(post, id("/blog/post?page=2"))));
    assert!(graph.nodes.iter().all(|n| !n.label.is_empty()));
    let about = id("/about/");
    assert!(graph.edges.contains(&(about, id("/about/team.html"))));
    // Jede URL genau einmal.
    let urls: Vec<_> = graph
        .nodes
        .iter()
        .filter_map(|n| n.address.clone())
        .collect();
    assert_eq!(urls.len(), exp["urls"].as_array().unwrap().len());
}

// spec: UI-094
// spec: UI-098
#[tokio::test]
async fn graph_and_three_d_captures_produce_the_same_directory_and_crawl_structures() {
    let crawl = crawl_fixture("ui/fx-ui-dirs", CrawlConfig::default()).await;
    let full = Snapshot::capture(&crawl.file).unwrap();
    let graph_2d =
        Snapshot::capture_scoped(&crawl.file, CaptureScope::graph(MAX_BUDGET_2D)).unwrap();
    let graph_3d =
        Snapshot::capture_scoped(&crawl.file, CaptureScope::graph(MAX_BUDGET_3D)).unwrap();
    for directory in [false, true] {
        let reference = full.graph(&all(directory)).unwrap();
        assert_eq!(graph_2d.graph(&all(directory)).unwrap(), reference);
        let mut three = all(directory);
        three.max_budget = MAX_BUDGET_3D;
        three.budget = MAX_BUDGET_3D;
        assert_eq!(graph_3d.graph(&three).unwrap(), reference);
    }
}

fn large(n: usize, links: usize) -> (tempfile::TempDir, os_store::CrawlFile) {
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
         WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i < {links})
         INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow)
          SELECT 1 + (i % {n}), 1 + ((i * 7) % {n}), '/', 'Hyperlink', 0, 1 FROM n WHERE {links} > 0;
         COMMIT;"
    )).unwrap();
    (dir, file)
}

// spec: UI-098
#[test]
fn three_d_scope_takes_one_hundred_thousand_urls_and_the_budget_is_capped_per_dimension() {
    let (_dir, file) = large(100_000, 0);
    let error = Snapshot::capture(&file).unwrap_err().to_string();
    assert!(error.contains("urls") && error.contains("50000"), "{error}");
    let start = std::time::Instant::now();
    let snapshot = Snapshot::capture_scoped(&file, CaptureScope::graph(MAX_BUDGET_3D)).unwrap();
    let graph = snapshot
        .graph(&GraphOptions {
            budget: MAX_BUDGET_3D,
            max_budget: MAX_BUDGET_3D,
            expand_depth: 100,
            ..Default::default()
        })
        .unwrap();
    eprintln!("100k capture + projection: {:?}", start.elapsed());
    assert_eq!(graph.nodes.len(), 100_000);
    assert_eq!(graph.edges.len(), 99_999);
    assert!(!graph.truncated);
    for (budget, max) in [
        (MAX_BUDGET_3D + 1, MAX_BUDGET_3D),
        (MAX_BUDGET_2D + 1, MAX_BUDGET_2D),
    ] {
        assert!(snapshot
            .graph(&GraphOptions {
                budget,
                max_budget: max,
                ..Default::default()
            })
            .is_err());
    }
    assert!(Snapshot::capture_scoped(&file, CaptureScope::graph(MAX_BUDGET_2D)).is_err());
}

// spec: UI-146 [M3]
#[test]
fn graphs_without_anchor_words_do_not_read_or_limit_the_links_table() {
    // 300 000 Links überschreiten die Grenze der Ankertext-Aufnahme, Graphen mit Herkunft lesen `links` nicht.
    let (_dir, file) = large(2_000, 300_000);
    let error = Snapshot::capture(&file).unwrap_err().to_string();
    assert!(error.contains("links"), "{error}");
    let snapshot = Snapshot::capture_scoped(&file, CaptureScope::graph(MAX_BUDGET_2D)).unwrap();
    assert_eq!(snapshot.graph(&all(false)).unwrap().nodes.len(), 2_000);
    // Ohne Entdeckungsherkunft greift der Rückfall über Links und damit wieder die Grenze.
    file.conn()
        .execute("DELETE FROM discovery_edges", [])
        .unwrap();
    assert!(Snapshot::capture_scoped(&file, CaptureScope::graph(MAX_BUDGET_2D)).is_err());
}
