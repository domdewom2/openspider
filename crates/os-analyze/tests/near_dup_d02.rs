#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::{
    analysis::{steps::near_duplicates, CancelToken},
    minhash,
};
use os_model::{CrawlConfig, StepState};
fn fixture() -> (tempfile::TempDir, os_store::CrawlFile) {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "dups.ospider", 42);
    for (id, text, hash) in [
        (1, "The cat sat on the mat.", "a"),
        (2, "The cat sat on the mat.", "b"),
        (3, "The cat sat on the mat.", "a"),
        (4, "unrelated different words and material", "c"),
    ] {
        insert(
            &file,
            &[row(id, &format!("{id}.html"))
                .text("content_class", "HTML")
                .int("status_code", 200)
                .text("hash_md5", hash)
                .text("indexability", "Indexable")],
        );
        file.conn()
            .execute(
                "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.text',?2)",
                rusqlite::params![id, text],
            )
            .unwrap();
        file.conn()
            .execute(
                "INSERT INTO minhash VALUES (?1,?2)",
                rusqlite::params![
                    id,
                    os_store::encode_minhash(&minhash::signature(&minhash::shingles(text)))
                ],
            )
            .unwrap();
    }
    (dir, file)
}
// spec: ONPAGE-144
#[test]
fn shingling_jaccard_and_candidate_probability() {
    assert_eq!(
        minhash::shingles("Der Hund bellt laut.")
            .into_iter()
            .collect::<Vec<_>>(),
        ["der hund bellt", "hund bellt laut"]
    );
    assert_eq!(minhash::shingles("Hund bellt").len(), 1);
    let a = (0..9).map(|n| n.to_string()).collect();
    let b = (0..10).map(|n| n.to_string()).collect();
    assert_eq!(minhash::jaccard(&a, &b), 0.9);
    for threshold in 50..=100 {
        let r = minhash::band_rows(threshold);
        assert!(
            1.0 - (1.0 - (f64::from(threshold) / 100.0).powi(r as i32)).powi((128 / r) as i32)
                >= 0.999
        );
    }
}
// spec: ONPAGE-144
#[test]
fn analysis_is_deterministic_excludes_exact_pairs_and_can_cancel() {
    let (_dir, file) = fixture();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let cancel = CancelToken::new();
    assert_eq!(
        near_duplicates::run(&file, &cfg, &cancel).unwrap(),
        StepState::Done
    );
    let dump = || {
        file.conn()
            .prepare("SELECT url_id,other_id,similarity FROM near_duplicates ORDER BY 1,2")
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    let before = dump();
    assert_eq!(before, [(1, 2, 1.0), (2, 1, 1.0), (2, 3, 1.0), (3, 2, 1.0)]);
    assert_eq!(cancel.progress(), 1.0);
    near_duplicates::run(&file, &cfg, &cancel).unwrap();
    assert_eq!(dump(), before);
    cancel.cancel();
    assert_eq!(
        near_duplicates::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
    assert_eq!(dump(), before);
}
// spec: ONPAGE-145
#[test]
fn duplicate_details_and_word_diff() {
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let params = serde_json::json!({"filter":"Exact"});
    let req = os_analyze::details::DetailReq {
        url_ids: &[os_model::UrlId(1)],
        params: &params,
        sort: &[],
        offset: 0,
        limit: 10,
    };
    let rows = os_analyze::tabs::duplicates::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(rows.total, 1);
    assert_eq!(rows.rows[0][2], os_model::Value::Text("Exact".into()));
    let mut cfg = cfg.clone();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            os_model::AnalysisStep::NearDuplicates,
            &cfg,
        )),
    )
    .unwrap();
    let near_params = serde_json::json!({"filter":"Near"});
    let near_req = os_analyze::details::DetailReq {
        params: &near_params,
        ..req
    };
    let near = os_analyze::tabs::duplicates::detail_rows(&file, &cfg, &near_req).unwrap();
    assert_eq!(near.total, 1);
    assert_eq!(near.rows[0][1], os_model::Value::Int(100));
    assert_eq!(
        minhash::word_diff("one two three", "one four three"),
        (vec![1], vec![1])
    );
    assert_eq!(minhash::word_diff("a a b", "a b"), (vec![1], vec![]));
}
// spec: ONPAGE-144
#[test]
fn lsh_recall_on_two_thousand_seeded_documents() {
    use std::collections::{HashMap, HashSet};
    let mut buckets = HashMap::<(usize, Vec<u32>), Vec<usize>>::new();
    let mut pairs = HashSet::new();
    let mut docs = Vec::new();
    for id in 0..2000usize {
        let group = if id < 600 { id / 2 } else { id };
        let mut words = (0..120)
            .map(|n| format!("word{group}x{n}"))
            .collect::<Vec<_>>();
        if id < 600 && id % 2 == 1 {
            for n in 0..(group % 3) {
                words[20 + n * 45] = format!("changed{group}x{n}");
            }
        }
        let shingles = minhash::shingles(&words.join(" "));
        let sig = minhash::signature(&shingles);
        for (band, part) in sig.chunks(minhash::band_rows(90)).enumerate() {
            let bucket = buckets.entry((band, part.to_vec())).or_default();
            for &candidate in bucket.iter() {
                pairs.insert((candidate, id));
            }
            bucket.push(id);
        }
        docs.push(shingles);
    }
    let mut found = 0;
    for n in 0..300 {
        let (a, b) = (n * 2, n * 2 + 1);
        assert!(minhash::jaccard(&docs[a], &docs[b]) >= 0.9);
        if pairs.contains(&(a, b)) {
            found += 1;
        }
    }
    assert!(found >= 299, "recall {found}/300");
    for (a, b) in pairs {
        if minhash::jaccard(&docs[a], &docs[b]) >= 0.9 {
            assert_eq!(a / 2, b / 2);
        }
    }
}
// spec: ONPAGE-144
#[test]
fn non_indexable_documents_do_not_change_counts_when_excluded() {
    let (_dir, file) = fixture();
    file.conn()
        .execute(
            "UPDATE urls SET indexability='Non-Indexable' WHERE id=2",
            [],
        )
        .unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    let count = || {
        file.conn()
            .query_row("SELECT COUNT(*) FROM near_duplicates", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(count(), 0);
    cfg.duplicates.only_indexable = false;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(count(), 4);
}
// spec: ONPAGE-144
#[tokio::test]
async fn parser_signatures_match_analysis_contract_on_fixture() {
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    cfg.pdf.store = true;
    let fx = os_engine::testing::crawl_fixture("onpage-m2-content", cfg).await;
    let mut stmt=fx.file.conn().prepare("SELECT m.sig,e.v_text FROM minhash m JOIN url_ext e ON e.url_id=m.url_id AND e.key='content.text'").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?))
        })
        .unwrap();
    let mut count = 0;
    for row in rows {
        let (sig, text) = row.unwrap();
        assert_eq!(
            os_store::decode_minhash(&sig).unwrap(),
            minhash::signature(&minhash::shingles(&text))
        );
        count += 1;
    }
    assert!(count >= 5);
}
// spec: ONPAGE-145
#[test]
fn duplicate_details_fixture_has_exact_near_and_below_threshold_pages() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/ui/fx-ui-dup/expected.json"
    ))
    .unwrap();
    let pages = fixture["pages"].as_array().unwrap();
    assert_eq!(pages[0]["hash"], pages[1]["hash"]);
    let c = minhash::shingles(pages[2]["text"].as_str().unwrap());
    let d = minhash::shingles(pages[3]["text"].as_str().unwrap());
    let e = minhash::shingles(pages[4]["text"].as_str().unwrap());
    assert!(minhash::jaccard(&c, &d) >= 0.9);
    assert!(minhash::jaccard(&c, &e) < 0.9);
    assert_eq!(
        (minhash::jaccard(&c, &d) * 100.0).round() as i64,
        fixture["display_similarity_C_D"].as_i64().unwrap()
    );
    assert_eq!(
        (minhash::jaccard(&c, &e) * 100.0).round() as i64,
        fixture["display_similarity_C_E"].as_i64().unwrap()
    );
}

// spec: ONPAGE-144
#[test]
fn changing_comparison_population_invalidates_near_results() {
    let (_dir, file) = fixture();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            os_model::AnalysisStep::NearDuplicates,
            &cfg,
        )),
    )
    .unwrap();
    assert_eq!(
        os_analyze::views::state_of(&file, &cfg, "Content", "Near Duplicates").unwrap(),
        os_model::FilterState::Ready
    );
    cfg.duplicates.only_indexable = false;
    assert_eq!(
        os_analyze::views::state_of(&file, &cfg, "Content", "Near Duplicates").unwrap(),
        os_model::FilterState::Stale
    );
    let mut query = os_analyze::views::ViewQuery::new("Content", "All");
    query.columns = os_analyze::views::ColumnSelection::Named(vec![
        "Closest Similarity Match".into(),
        "No. Near Duplicates".into(),
    ]);
    let page = os_analyze::views::query_view(&file, &cfg, &query).unwrap();
    assert!(page
        .rows
        .iter()
        .all(|row| row == &[os_model::Value::Null, os_model::Value::Null]));
}

// spec: ONPAGE-145
#[test]
fn details_page_in_sql_and_reject_non_html_source_pages() {
    let (_dir, file) = fixture();
    let cfg = CrawlConfig::default();
    let params = serde_json::json!({"filter":"Exact"});
    let mut req = os_analyze::details::DetailReq {
        url_ids: &[os_model::UrlId(1)],
        params: &params,
        sort: &[],
        offset: 1,
        limit: 1,
    };
    let result = os_analyze::tabs::duplicates::detail_rows(&file, &cfg, &req).unwrap();
    assert_eq!(result.total, 1);
    assert!(result.rows.is_empty());
    req.offset = 0;
    assert_eq!(
        os_analyze::tabs::duplicates::detail_rows(&file, &cfg, &req)
            .unwrap()
            .rows
            .len(),
        1
    );
    file.conn()
        .execute("UPDATE urls SET content_class='PDF' WHERE id=1", [])
        .unwrap();
    assert_eq!(
        os_analyze::tabs::duplicates::detail_rows(&file, &cfg, &req)
            .unwrap()
            .total,
        0
    );
}

// spec: PERF-007
#[test]
fn cancelled_empty_analysis_does_not_report_completion() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "empty.ospider", 7);
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert_eq!(
        near_duplicates::run(&file, &cfg, &cancel).unwrap(),
        StepState::Pending
    );
    assert_eq!(cancel.progress(), 0.0);
}

// spec: ONPAGE-019
// spec: ONPAGE-144
#[test]
fn stale_and_disabled_analysis_does_not_leak_old_similarity_columns() {
    let (_dir, file) = fixture();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            os_model::AnalysisStep::NearDuplicates,
            &cfg,
        )),
    )
    .unwrap();
    for marker in [
        "0",
        "1",
        "near_duplicates",
        "sitemaps,near_duplicates",
        "sitemaps",
    ] {
        file.set_meta("analysis_stale", marker).unwrap();
        for tab in ["Internal", "Content"] {
            let mut query = os_analyze::views::ViewQuery::new(tab, "All");
            query.columns = os_analyze::views::ColumnSelection::Named(vec![
                "Closest Similarity Match".into(),
                "No. Near Duplicates".into(),
            ]);
            let page = os_analyze::views::query_view(&file, &cfg, &query).unwrap();
            let row = &page.rows[0];
            if marker == "0" || marker == "sitemaps" {
                assert_eq!(
                    row,
                    &[os_model::Value::Float(100.0), os_model::Value::Int(1)],
                    "{marker}: {tab}"
                );
            } else {
                assert_eq!(
                    row,
                    &[os_model::Value::Null, os_model::Value::Null],
                    "{marker}: {tab}"
                );
            }
        }
    }
}

// spec: ONPAGE-144
#[test]
fn disabled_analysis_leaves_similarity_columns_empty() {
    let (_dir, file) = fixture();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    os_analyze::analysis::persist::write_step(
        &file,
        os_model::AnalysisStep::NearDuplicates,
        StepState::Done,
        None,
        Some(&os_analyze::state::step_config_hash(
            os_model::AnalysisStep::NearDuplicates,
            &cfg,
        )),
    )
    .unwrap();
    cfg.analysis.near_duplicates = false;
    let mut query = os_analyze::views::ViewQuery::new("Content", "All");
    query.columns = os_analyze::views::ColumnSelection::Named(vec![
        "Closest Similarity Match".into(),
        "No. Near Duplicates".into(),
    ]);
    let page = os_analyze::views::query_view(&file, &cfg, &query).unwrap();
    assert!(page
        .rows
        .iter()
        .all(|row| row == &[os_model::Value::Null, os_model::Value::Null]));
}

// spec: ONPAGE-144
// spec: ONPAGE-145
#[test]
fn threshold_fixture_rebands_saved_signatures_and_preserves_exact_word_diffs() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/onpage-m2-content/thresholds");
    let expected: std::collections::BTreeMap<String, f64> =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected_jaccard.json")).unwrap())
            .unwrap();
    let diffs: std::collections::BTreeMap<String, Vec<usize>> =
        serde_json::from_str(&std::fs::read_to_string(root.join("expected_diff.json")).unwrap())
            .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "thresholds.ospider", 91);
    let names = ["base", "95", "91", "89", "80"];
    let texts: Vec<_> = names
        .iter()
        .map(|name| {
            let html = std::fs::read_to_string(root.join(format!("near-{name}.html"))).unwrap();
            html.split_once("<main>")
                .unwrap()
                .1
                .split_once("</main>")
                .unwrap()
                .0
                .to_owned()
        })
        .collect();
    for (n, (name, text)) in names.iter().zip(&texts).enumerate() {
        let id = n as i64 + 1;
        insert(
            &file,
            &[row(id, &format!("near-{name}.html"))
                .text("content_class", "HTML")
                .int("status_code", 200)],
        );
        file.conn()
            .execute(
                "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.text',?2)",
                (id, text),
            )
            .unwrap();
        file.conn()
            .execute(
                "INSERT INTO minhash VALUES (?1,?2)",
                (
                    id,
                    os_store::encode_minhash(&minhash::signature(&minhash::shingles(text))),
                ),
            )
            .unwrap();
        if *name != "base" {
            assert_eq!(
                minhash::word_diff(&texts[0], text),
                (diffs[*name].clone(), diffs[*name].clone())
            );
        }
    }
    for (pair, similarity) in expected {
        let (a, b) = pair.split_once(':').unwrap();
        let a = names.iter().position(|n| *n == a).unwrap();
        let b = names.iter().position(|n| *n == b).unwrap();
        assert!(
            (minhash::jaccard(&minhash::shingles(&texts[a]), &minhash::shingles(&texts[b]))
                - similarity)
                .abs()
                < 1e-12
        );
    }
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    let pairs = || {
        file.conn().prepare("SELECT other_id,CAST(ROUND(similarity*100) AS INTEGER) FROM near_duplicates WHERE url_id=1 ORDER BY similarity DESC").unwrap()
        .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?))).unwrap().collect::<Result<Vec<_>, _>>().unwrap()
    };
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(pairs(), [(2, 95), (3, 91)]);
    cfg.duplicates.near_threshold_pct = 75;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(pairs(), [(2, 95), (3, 91), (4, 89), (5, 80)]);
}

// spec: ONPAGE-144
// spec: PERF-007
#[test]
fn failed_publication_restores_previous_pairs_and_removes_scratch_tables() {
    let (_dir, file) = fixture();
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
    file.conn().execute_batch("CREATE TRIGGER reject_pairs BEFORE INSERT ON near_duplicates BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert!(near_duplicates::run(&file, &cfg, &CancelToken::new()).is_err());
    let count: i64 = file
        .conn()
        .query_row("SELECT COUNT(*) FROM near_duplicates", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 4);
    assert!(file.conn().is_autocommit());
    let scratch: i64 = file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name LIKE 'd02_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(scratch, 0);
}
