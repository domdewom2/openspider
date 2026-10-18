#[path = "graph_fixture.rs"]
mod fx;
use os_analyze::graph::{GraphOptions, Snapshot, WordOptions, WordUnit};
use std::collections::BTreeSet;

// spec: UI-091
// spec: UI-092
#[test]
fn shortest_discovery_paths_ties_cycles_and_force_graph_data() {
    let (_dir, file) = fx::basic();
    let snapshot = Snapshot::capture(&file).unwrap();
    let graph = snapshot
        .graph(&GraphOptions {
            expand_depth: 12,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(graph.edges, [(1, 2), (1, 3), (2, 4), (3, 5)]);
    assert_eq!(
        graph.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5]
    );
    assert_eq!(
        graph.nodes.iter().map(|n| n.depth).collect::<Vec<_>>(),
        [0, 1, 1, 2, 2]
    );
    assert!(!snapshot.provenance_missing);
    let all = snapshot
        .graph(&GraphOptions {
            include_non_html: true,
            expand_depth: 12,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all.nodes.len(), 6);
    assert_eq!(
        all.nodes
            .iter()
            .map(|n| n.id)
            .collect::<BTreeSet<_>>()
            .len(),
        6
    );
    file.conn()
        .execute(
            "UPDATE urls SET indexability = 'Non-Indexable' WHERE id = 2",
            [],
        )
        .unwrap();
    let filtered = Snapshot::capture(&file)
        .unwrap()
        .graph(&GraphOptions {
            include_non_indexable: false,
            expand_depth: 12,
            ..Default::default()
        })
        .unwrap();
    assert!(!filtered.nodes.iter().any(|n| n.id == 2));
    assert!(filtered.edges.contains(&(1, 4))); // hidden parent is promoted, HTML remains reachable
}

// spec: UI-091
#[test]
fn legacy_without_discovery_table_uses_bfs_and_reports_missing_provenance() {
    let (_dir, file) = fx::basic();
    file.conn()
        .execute("DROP TABLE discovery_edges", [])
        .unwrap();
    fx::link(&file, 1, 2, "a", "", true, "Hyperlink");
    fx::link(&file, 2, 3, "b", "", true, "Hyperlink");
    fx::link(&file, 3, 1, "cycle", "", true, "Hyperlink");
    let snapshot = Snapshot::capture(&file).unwrap();
    assert!(snapshot.provenance_missing);
    let graph = snapshot.graph(&GraphOptions::default()).unwrap();
    assert!(graph.edges.contains(&(1, 2)));
    assert!(graph.edges.contains(&(2, 3)));
    assert_eq!(graph.nodes.iter().find(|n| n.id == 3).unwrap().depth, 2);
}

// spec: UI-093
#[test]
fn directory_paths_holes_queries_and_trailing_slashes() {
    let (_dir, file) = fx::file();
    for (id, address) in [
        (1, "https://site.test/"),
        (2, "https://site.test/one/a"),
        (3, "https://site.test/two/b/"),
        (4, "https://site.test/one/a?q=1"),
        (5, "https://site.test/one/a?q=2"),
    ] {
        fx::url(&file, id, address, true);
    }
    let snapshot = Snapshot::capture(&file).unwrap();
    let graph = snapshot
        .graph(&GraphOptions {
            directory: true,
            expand_depth: 20,
            ..Default::default()
        })
        .unwrap();
    let holes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| n.address.is_none() && n.depth > 1)
        .collect();
    assert_eq!(
        holes
            .iter()
            .map(|n| n.label.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["one", "two"])
    );
    assert!(graph.edges.contains(&(2, 4)) && graph.edges.contains(&(2, 5)));
    assert_eq!(
        graph.nodes.iter().filter(|n| n.address.is_some()).count(),
        5
    );
    assert!(!graph
        .nodes
        .iter()
        .any(|n| n.label.is_empty() || n.label.contains('?') && n.address.is_none()));
}

// spec: UI-095
#[test]
fn numeric_metrics_are_independent_and_missing_values_remain_missing() {
    let (_dir, file) = fx::basic();
    file.conn()
        .execute(
            "UPDATE url_counters SET unique_inlinks = 17 WHERE url_id = 2",
            [],
        )
        .unwrap();
    file.conn()
        .execute(
            "INSERT INTO url_ext(url_id,key,v_real) VALUES (2, 'linkscore.score', 42)",
            [],
        )
        .unwrap();
    file.conn().execute("INSERT INTO api_values(url_id,source,metric,level,v_real,fetched_at) VALUES (2,'ga4','sessions','url',99,'2026-01-01')", []).unwrap();
    file.conn().execute("INSERT INTO custom_extraction_values(url_id,def_index,number,value) VALUES (2,0,1,'2.5'),(3,0,1,'not a number')", []).unwrap();
    let mut config = file.config().unwrap();
    config.segments.list.push(os_model::SegmentConfig {
        name: "Products".into(),
        color: "#12ab34".into(),
        ..Default::default()
    });
    file.set_config(&config).unwrap();
    file.conn()
        .execute(
            "INSERT INTO url_segments(url_id,segment_id) VALUES (2,0)",
            [],
        )
        .unwrap();
    let snapshot = Snapshot::capture(&file).unwrap();
    assert!(snapshot.available_metrics().contains("ga4.sessions"));
    assert!(!snapshot.available_metrics().contains("gsc.clicks"));
    let graph = snapshot
        .graph(&GraphOptions {
            metric: "Word Count".into(),
            color_metric: Some("ga4.sessions".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(graph.nodes[0].metric, Some(50.));
    assert_eq!(graph.nodes[1].metric, Some(2000.));
    assert_eq!(graph.nodes[0].color_value, None);
    assert_eq!(graph.nodes[1].color_value, Some(99.));
    assert_eq!(graph.nodes[1].unique_inlinks, Some(17));
    assert_eq!(graph.nodes[1].link_score, Some(42.));
    assert_eq!(graph.nodes[1].segment_id.as_deref(), Some("0"));
    assert_eq!(graph.nodes[1].segment_color.as_deref(), Some("#12ab34"));
    let custom = snapshot
        .graph(&GraphOptions {
            metric: "custom_extraction.0".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(custom.nodes[1].metric, Some(2.5));
    assert_eq!(custom.nodes[2].metric, None);
}

// spec: UI-096
#[test]
fn breadth_first_budget_badges_explore_focus_and_depth() {
    let (_dir, file) = fx::basic();
    let snapshot = Snapshot::capture(&file).unwrap();
    let graph = snapshot
        .graph(&GraphOptions {
            budget: 4,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        graph.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    let capped = graph.nodes.iter().find(|n| n.id == 3).unwrap();
    assert_eq!((capped.hidden_children, capped.hidden_descendants), (1, 1));
    assert_eq!(graph.nodes[0].hidden_descendants, 1);
    assert!(graph.truncated);
    let explored = snapshot
        .graph(&GraphOptions {
            budget: 4,
            explore_node: Some(3),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(explored.nodes.len(), 5);
    assert!(!explored.truncated);
    let focused = snapshot
        .graph(&GraphOptions {
            root: Some(3),
            budget: 2,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        focused.nodes.iter().map(|n| n.id).collect::<Vec<_>>(),
        [3, 5]
    );
    let collapsed = snapshot
        .graph(&GraphOptions {
            expand_depth: 0,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(collapsed.nodes.len(), 1);
    assert_eq!(
        (
            collapsed.nodes[0].hidden_children,
            collapsed.nodes[0].hidden_descendants
        ),
        (2, 4)
    );
    assert!(snapshot
        .graph(&GraphOptions {
            budget: 0,
            ..Default::default()
        })
        .is_err());
    assert!(snapshot
        .graph(&GraphOptions {
            root: Some(999),
            ..Default::default()
        })
        .is_err());
}

// spec: UI-099
#[test]
fn internal_phrases_alt_fallback_options_words_and_stopwords() {
    let (_dir, file) = fx::basic();
    for _ in 0..3 {
        fx::link(&file, 1, 2, "  Produkte\n  Kaufen ", "", true, "Hyperlink");
    }
    fx::link(&file, 3, 2, "Produkte Kaufen", "", false, "Hyperlink");
    fx::link(&file, 2, 2, "Produkte Kaufen", "", true, "Hyperlink");
    fx::link(&file, 3, 2, "produkte kaufen", "", true, "Hyperlink");
    fx::link(&file, 3, 2, "", "Logo", true, "Hyperlink");
    fx::link(&file, 3, 2, "", "", true, "Hyperlink");
    fx::link(&file, 3, 2, "ignore resource", "", true, "Image");
    fx::link(&file, 3, 2, "und die Produkte", "", true, "Hyperlink");
    file.conn()
        .execute("UPDATE urls SET is_internal = 0 WHERE id = 5", [])
        .unwrap();
    fx::link(&file, 5, 2, "external", "", true, "Hyperlink");
    let snapshot = Snapshot::capture(&file).unwrap();
    let phrases = snapshot.words(2, &WordOptions::default()).unwrap();
    assert_eq!(phrases[0], ("Produkte Kaufen".into(), 4));
    assert!(phrases.contains(&("Logo".into(), 1)));
    assert!(phrases.contains(&("(leer)".into(), 1)));
    assert!(!phrases
        .iter()
        .any(|(s, _)| s == "external" || s == "ignore resource"));
    let follow = snapshot
        .words(
            2,
            &WordOptions {
                include_nofollow: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(follow[0], ("Produkte Kaufen".into(), 3));
    let self_links = snapshot
        .words(
            2,
            &WordOptions {
                exclude_self_links: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(self_links[0], ("Produkte Kaufen".into(), 5));
    let words = snapshot
        .words(
            2,
            &WordOptions {
                unit: WordUnit::Word,
                language: "de-DE".into(),
                max_terms: 1,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(words, [("produkte".into(), 6)]);
}

// spec: UI-091
#[test]
fn same_depth_uses_global_discovery_sequence_before_parent_order() {
    let (_dir, file) = fx::basic();
    fx::url(&file, 7, "https://site.test/tie", true);
    fx::edge(&file, 10, Some(3), 7);
    fx::edge(&file, 11, Some(2), 7);
    let graph = Snapshot::capture(&file)
        .unwrap()
        .graph(&GraphOptions::default())
        .unwrap();
    assert!(graph.edges.contains(&(3, 7)));
    assert!(!graph.edges.contains(&(2, 7)));
}
