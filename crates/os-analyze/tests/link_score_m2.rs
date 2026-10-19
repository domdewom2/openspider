#[path = "tab_links_support.rs"]
mod support;
use os_analyze::{
    analysis::{
        steps::link_score::{self, Scaling},
        CancelToken,
    },
    views::{query_view, ViewQuery},
};
use os_model::{CrawlConfig, StepState, Value};
use support::*;

fn scores(file: &os_store::CrawlFile) -> Vec<(i64, f64, i64)> {
    let mut stmt = file.conn().prepare("SELECT a.url_id,a.v_real,b.v_int FROM url_ext a JOIN url_ext b ON b.url_id=a.url_id AND b.key='linkscore.score' WHERE a.key='linkscore.raw' ORDER BY a.url_id").unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
fn score_fixture(file: &os_store::CrawlFile) {
    nodes(file, 7);
    sql(
        file,
        include_str!("../../../tests/fixtures/tech-m2-links/link-score.sql"),
    );
}
// spec: TECH-145
#[test]
fn ten_iterations_resolve_redirects_and_canonicals_and_match_spec_goldens() {
    let (_dir, file) = file();
    score_fixture(&file);
    let cfg = CrawlConfig::default();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    let first = scores(&file);
    let expected = [
        (0.21601, 100),
        (0.09190, 7),
        (0.13147, 46),
        (0.08661, 1),
        (0.10450, 21),
    ];
    for ((_, raw, score), (gold, normalized)) in first.iter().zip(expected) {
        assert!((raw - gold).abs() < 1e-5, "{raw} != {gold}");
        assert_eq!(*score, normalized);
    }
    assert_eq!(first.len(), 5);
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(
        scores(&file)
            .iter()
            .map(|(_, r, s)| (r.to_bits(), *s))
            .collect::<Vec<_>>(),
        first
            .iter()
            .map(|(_, r, s)| (r.to_bits(), *s))
            .collect::<Vec<_>>()
    );
    link_score::run_with_scaling(&file, &cfg, &CancelToken::new(), Scaling::Linear).unwrap();
    assert_eq!(
        scores(&file).iter().map(|r| r.2).collect::<Vec<_>>(),
        [100, 5, 35, 1, 15]
    );
}
// spec: TECH-145
#[test]
fn singleton_equal_ranks_empty_graph_and_cancel_preserve_atomic_publication() {
    let (_dir, file) = file();
    let cfg = CrawlConfig::default();
    assert_eq!(
        link_score::run(&file, &cfg, &CancelToken::new()).unwrap(),
        StepState::Done
    );
    nodes(&file, 1);
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(scores(&file)[0].2, 100);
    let old = scores(&file);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        link_score::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
    assert_eq!(scores(&file), old);
    sql(&file, "UPDATE urls SET content_class='Images'");
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(scores(&file).is_empty());
    sql(&file,"UPDATE urls SET content_class='HTML'; INSERT INTO urls (id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length,content_class,status_code) VALUES(2,'https://site.test/2','https://site.test/2','https://site.test',1,'done','link',0,1,'HTML',200)");
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(
        scores(&file).iter().map(|r| r.2).collect::<Vec<_>>(),
        [100, 100]
    );
}
// spec: ONPAGE-017
// spec: UI-081
// spec: TECH-130 [M2]
#[test]
fn score_is_visible_in_links_internal_and_url_details_and_inlinks_is_a_standard_column() {
    let (_dir, file) = file();
    score_fixture(&file);
    let cfg = CrawlConfig::default();
    for tab in ["Internal", "Links"] {
        let p = query_view(&file, &cfg, &ViewQuery::new(tab, "All")).unwrap();
        let col = p.columns.iter().position(|c| c == "Link Score").unwrap();
        assert!(p.rows.iter().all(|r| r[col] == Value::Null));
    }
    os_analyze::analysis::runner::run_only(
        &file,
        &cfg,
        &CancelToken::new(),
        &|_, _| {},
        &[os_model::AnalysisStep::LinkScore],
    )
    .unwrap();
    for tab in ["Internal", "Links"] {
        let p = query_view(&file, &cfg, &ViewQuery::new(tab, "All")).unwrap();
        let col = p.columns.iter().position(|c| c == "Link Score").unwrap();
        assert_eq!(p.rows[0][col], Value::Int(100));
        if tab == "Links" {
            assert_eq!(p.columns[5], "Inlinks");
        }
    }
    assert_eq!(
        os_analyze::tabs::links::url_details(&file, &cfg, os_model::UrlId(1)).unwrap(),
        vec![("Link Score".into(), "100".into())]
    );
}
// spec: TECH-145
#[test]
fn seeded_random_graphs_match_independent_python_reference() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/tech-m2-links/random-ranks.json"
    ))
    .unwrap();
    for graph in fixture.as_array().unwrap() {
        let (_dir, file) = file();
        let cfg = CrawlConfig::default();
        nodes(&file, graph["ranks"].as_array().unwrap().len());
        for (i, edge) in graph["edges"].as_array().unwrap().iter().enumerate() {
            file.conn().execute("INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) VALUES(?1,?2,'/','Hyperlink',?3,?4)",rusqlite::params![edge[0].as_i64().unwrap(),edge[1].as_i64().unwrap(),i,edge[2].as_bool().unwrap()]).unwrap();
        }
        link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
        for ((_, actual, _), expected) in
            scores(&file).iter().zip(graph["ranks"].as_array().unwrap())
        {
            assert!((actual - expected.as_f64().unwrap()).abs() < 1e-14);
        }
    }
}

// spec: TECH-145
#[test]
fn resolution_stops_at_ten_hops_and_rejects_loops_and_unknown_destinations() {
    let (_dir, file) = file();
    nodes(&file, 20);
    // Nodes 1 and 2 participate. The ten-hop chain 3..12 resolves to 2;
    // the eleven-hop chain from 13 does not. Nodes 14/15 form a cycle.
    sql(&file,"UPDATE urls SET redirect_type='http', status_code=301, redirect_target_id=id+1 WHERE id BETWEEN 3 AND 11;
        UPDATE urls SET redirect_type='http',status_code=301,redirect_target_id=2 WHERE id=12;
        UPDATE urls SET redirect_type='http',status_code=301,redirect_target_id=3 WHERE id=13;
        UPDATE urls SET redirect_type='http',status_code=301,redirect_target_id=15 WHERE id=14;
        UPDATE urls SET redirect_type='http',status_code=301,redirect_target_id=14 WHERE id=15;
        UPDATE urls SET content_class='Images' WHERE id>=16;
        INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) VALUES
        (1,3,'/3','Hyperlink',1,1), (1,13,'/13','Hyperlink',2,1),
        (1,14,'/14','Hyperlink',3,1),(1,19,'/19','Hyperlink',4,1),
        (1,NULL,'/missing','Hyperlink',5,1), (1,1,'/1','Hyperlink',6,1)");
    link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).unwrap();
    let rank = scores(&file);
    assert_eq!(rank.len(), 2);
    assert!((rank[0].1 - 0.075).abs() < 1e-12);
    assert!((rank[1].1 - 0.13875).abs() < 1e-12);
}
// spec: TECH-145
#[test]
fn follow_wins_for_duplicate_resolved_targets_but_page_nofollow_blocks_rank_flow() {
    let (_dir, file) = file();
    score_fixture(&file);
    // A -> canonical alias K -> D supplies a follow path in addition to A -> D nofollow.
    sql(&file,"INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) VALUES (1,7,'/7','Hyperlink',7,1)");
    let cfg = CrawlConfig::default();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    let with_follow = scores(&file);
    assert!(with_follow[3].1 > 0.08661);
    sql(&file, "UPDATE urls SET effective_follow=0 WHERE id=1");
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert!(scores(&file)[1].1 < with_follow[1].1);
}
// spec: PERF-007
// spec: TECH-145
#[test]
fn cancellation_during_iterations_keeps_the_previous_complete_result() {
    let (_dir, file) = file();
    nodes(&file, 20_000);
    sql(&file,"INSERT INTO links(source_id,target_id,target_raw,link_type,position,follow) SELECT id,id%20000+1,'/','Hyperlink',1,1 FROM urls;
        INSERT INTO url_ext(url_id,key,v_int) VALUES (1,'linkscore.score',77)");
    file.conn().pragma_update(None, "temp_store", 2).unwrap();
    let cancel = CancelToken::new();
    let state = std::thread::scope(|scope| {
        scope.spawn(|| {
            while cancel.progress() == 0.0 {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            cancel.cancel();
        });
        link_score::run(&file, &CrawlConfig::default(), &cancel).unwrap()
    });
    assert_eq!(state, StepState::Pending);
    assert_eq!(
        file.conn()
            .query_row("PRAGMA temp_store", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT v_int FROM url_ext WHERE key='linkscore.score'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        77
    );
    assert_eq!(
        file.conn()
            .query_row(
                "SELECT COUNT(*) FROM url_ext WHERE key='linkscore.raw'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

// spec: UI-081
// spec: TECH-145
#[test]
fn pending_analysis_hides_previous_scores_in_all_three_sources() {
    let (_dir, file) = file();
    score_fixture(&file);
    let cfg = CrawlConfig::default();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::persist::begin_step(&file, os_model::AnalysisStep::LinkScore).unwrap();
    for tab in ["Links", "Internal"] {
        let page = query_view(&file, &cfg, &ViewQuery::new(tab, "All")).unwrap();
        let index = page.columns.iter().position(|c| c == "Link Score").unwrap();
        assert!(page.rows.iter().all(|r| r[index] == Value::Null));
    }
    assert_eq!(
        os_analyze::tabs::links::url_details(&file, &cfg, os_model::UrlId(1)).unwrap()[0].1,
        ""
    );
}

// spec: TECH-145
// spec: TECH-139
#[test]
fn identical_reanalysis_does_not_advance_the_change_feed() {
    let (_dir, file) = file();
    score_fixture(&file);
    let cfg = CrawlConfig::default();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::steps::links::run(&file, &cfg, &CancelToken::new()).unwrap();
    let seq = os_store::queries::change_seq(file.conn()).unwrap();
    link_score::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::steps::links::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(os_store::queries::change_seq(file.conn()).unwrap(), seq);
    assert!(os_store::queries::changed_since(file.conn(), seq, 500)
        .unwrap()
        .is_empty());
}

// spec: TECH-145
#[test]
fn temp_store_is_restored_after_success_and_sql_failure() {
    let (_dir, file) = file();
    score_fixture(&file);
    for previous in [0, 1, 2] {
        file.conn()
            .pragma_update(None, "temp_store", previous)
            .unwrap();
        link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).unwrap();
        let actual: i64 = file
            .conn()
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        assert_eq!(actual, previous);
        sql(&file, "CREATE TRIGGER fail_score BEFORE INSERT ON url_ext BEGIN SELECT RAISE(ABORT, 'test failure'); END;
            DELETE FROM url_ext WHERE key LIKE 'linkscore.%'");
        assert!(link_score::run(&file, &CrawlConfig::default(), &CancelToken::new()).is_err());
        let actual: i64 = file
            .conn()
            .query_row("PRAGMA temp_store", [], |r| r.get(0))
            .unwrap();
        assert_eq!(actual, previous);
        sql(&file, "DROP TRIGGER IF EXISTS fail_score");
    }
}
