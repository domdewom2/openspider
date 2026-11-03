//! M3-V02: Links eines Knotens einblenden (UI-097) gegen den echten Crawl von `fx-ui-words`.
use std::collections::HashSet;

use os_analyze::graph::{CaptureScope, GraphOptions, LinkDirection, LinkOverlayOptions, Snapshot};
use os_engine::testing::{crawl_fixture, FixtureCrawl};
use os_model::CrawlConfig;
use serde_json::Value;

fn expected() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/ui/fx-ui-words/expected.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn id_of(fx: &FixtureCrawl, path: &str) -> i64 {
    fx.file
        .conn()
        .query_row(
            "SELECT id FROM urls WHERE address LIKE '%' || ?1 AND in_tables = 1",
            [path],
            |r| r.get(0),
        )
        .unwrap()
}

fn counter(fx: &FixtureCrawl, column: &str, id: i64) -> u64 {
    fx.file
        .conn()
        .query_row(
            &format!("SELECT {column} FROM url_counters WHERE url_id = ?1"),
            [id],
            |r| r.get(0),
        )
        .unwrap()
}

fn options(node: i64, direction: LinkDirection) -> LinkOverlayOptions {
    LinkOverlayOptions {
        node,
        direction,
        directory: false,
        include_nofollow: true,
        exclude_nav_footer: false,
    }
}

fn shown(snapshot: &Snapshot, directory: bool) -> HashSet<i64> {
    snapshot
        .graph(&GraphOptions {
            directory,
            expand_depth: 100,
            ..Default::default()
        })
        .unwrap()
        .nodes
        .iter()
        .map(|n| n.id)
        .collect()
}

// spec: UI-097
#[tokio::test]
async fn inlink_and_outlink_edges_equal_the_unique_counters_for_every_option() {
    let exp = expected();
    let fx = crawl_fixture("ui/fx-ui-words", CrawlConfig::default()).await;
    let target = id_of(&fx, "/target.html");
    let instances: u64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM links l JOIN urls s ON s.id = l.source_id
             WHERE l.target_id = ?1 AND l.link_type = 'Hyperlink' AND s.is_internal = 1",
            [target],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(instances, exp["inlinks"].as_u64().unwrap());
    let unique = counter(&fx, "unique_inlinks", target);
    assert_eq!(unique, exp["uniqueInlinks"].as_u64().unwrap());

    let snapshot = Snapshot::capture_scoped(&fx.file, CaptureScope::three_d()).unwrap();
    let shown = shown(&snapshot, false);
    for (nofollow, nav_footer, key) in [
        (true, false, "uniqueInlinks"),
        (false, false, "uniqueInlinksWithoutNofollow"),
        (true, true, "uniqueInlinksWithoutNavFooter"),
        (false, true, "uniqueInlinksWithoutNofollowAndNavFooter"),
    ] {
        let overlay = snapshot
            .link_overlay(
                &fx.file,
                &LinkOverlayOptions {
                    include_nofollow: nofollow,
                    exclude_nav_footer: nav_footer,
                    ..options(target, LinkDirection::Inlinks)
                },
                &shown,
            )
            .unwrap();
        let want = exp[key].as_u64().unwrap();
        assert_eq!(overlay.total, want, "{key}");
        assert_eq!(overlay.edges.len() as u64, want, "{key}");
        assert!(overlay.edges.iter().all(|(_, t)| *t == target));
        // Eine Kante je Quellseite.
        let sources: HashSet<_> = overlay.edges.iter().map(|(s, _)| *s).collect();
        assert_eq!(sources.len() as u64, want, "{key}");
    }

    let outlinks = snapshot
        .link_overlay(&fx.file, &options(target, LinkDirection::Outlinks), &shown)
        .unwrap();
    assert_eq!(outlinks.total, counter(&fx, "unique_outlinks", target));
    assert_eq!(outlinks.total, exp["uniqueOutlinks"].as_u64().unwrap());
    assert_eq!(outlinks.edges.len() as u64, outlinks.total);
    assert!(
        outlinks.edges.contains(&(target, target)),
        "Selbstlink zählt"
    );
    let content_only = snapshot
        .link_overlay(
            &fx.file,
            &LinkOverlayOptions {
                exclude_nav_footer: true,
                ..options(target, LinkDirection::Outlinks)
            },
            &shown,
        )
        .unwrap();
    assert_eq!(
        content_only.total,
        exp["uniqueOutlinksWithoutNavFooter"].as_u64().unwrap()
    );

    // Kinder im Crawl-Baum: die nur von der Zielseite entdeckten Seiten.
    let children: Vec<i64> = exp["children"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| id_of(&fx, p.as_str().unwrap()))
        .collect();
    assert_eq!(snapshot.children_of(target, false), children);
    let child_in = snapshot
        .link_overlay(
            &fx.file,
            &options(target, LinkDirection::ChildInlinks),
            &shown,
        )
        .unwrap();
    assert_eq!(child_in.total, exp["childInlinks"].as_u64().unwrap());
    assert!(child_in.edges.iter().all(|(_, t)| children.contains(t)));
    let expected_child_in: u64 = children
        .iter()
        .map(|c| counter(&fx, "unique_inlinks", *c))
        .sum();
    assert_eq!(child_in.total, expected_child_in);
    let child_out = snapshot
        .link_overlay(
            &fx.file,
            &options(target, LinkDirection::ChildOutlinks),
            &shown,
        )
        .unwrap();
    assert_eq!(child_out.total, exp["childOutlinks"].as_u64().unwrap());
    assert!(child_out.edges.iter().all(|(s, _)| children.contains(s)));
}

// spec: UI-097
#[tokio::test]
async fn overlay_keeps_the_total_but_draws_only_edges_between_shown_nodes_also_in_directories() {
    let exp = expected();
    let fx = crawl_fixture("ui/fx-ui-words", CrawlConfig::default()).await;
    let target = id_of(&fx, "/target.html");
    let snapshot = Snapshot::capture(&fx.file).unwrap();
    // Nur Start-URL und erste Ebene teilweise: nicht alle Quellen sind dargestellt.
    let small: HashSet<i64> = snapshot
        .graph(&GraphOptions {
            budget: 5,
            ..Default::default()
        })
        .unwrap()
        .nodes
        .iter()
        .map(|n| n.id)
        .collect();
    let overlay = snapshot
        .link_overlay(&fx.file, &options(target, LinkDirection::Inlinks), &small)
        .unwrap();
    assert_eq!(overlay.total, exp["uniqueInlinks"].as_u64().unwrap());
    assert!(overlay.edges.len() < overlay.total as usize);
    assert!(overlay
        .edges
        .iter()
        .all(|(s, t)| small.contains(s) && small.contains(t)));

    // Verzeichnisgraph: URLs behalten ihre Kennung, dieselben Inlinks.
    let dirs = shown(&snapshot, true);
    let in_dirs = snapshot
        .link_overlay(
            &fx.file,
            &LinkOverlayOptions {
                directory: true,
                ..options(target, LinkDirection::Inlinks)
            },
            &dirs,
        )
        .unwrap();
    assert_eq!(in_dirs.total, exp["uniqueInlinks"].as_u64().unwrap());
    assert_eq!(in_dirs.edges.len() as u64, in_dirs.total);
    // Unbekannter Knoten → Eingabefehler.
    assert!(snapshot
        .link_overlay(&fx.file, &options(999_999, LinkDirection::Inlinks), &dirs)
        .is_err());
    assert_eq!(
        LinkDirection::parse("childOutlinks"),
        Some(LinkDirection::ChildOutlinks)
    );
    assert_eq!(LinkDirection::parse("sideways"), None);
}
