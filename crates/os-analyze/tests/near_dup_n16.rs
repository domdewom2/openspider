//! Differential oracle: the N14 disk implementation and the optimized implementation
//! must publish exactly the same pairs and f64 bits.
#[path = "near_dup_support/baseline.rs"]
mod baseline;
#[path = "framework_common.rs"]
mod common;
use common::*;
use os_analyze::{
    analysis::{steps::near_duplicates, CancelToken},
    minhash,
};
use os_model::CrawlConfig;

fn dump(file: &os_store::CrawlFile) -> Vec<(i64, i64, u64)> {
    file.conn()
        .prepare("SELECT url_id,other_id,similarity FROM near_duplicates ORDER BY 1,2")
        .unwrap()
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get::<_, f64>(2)?.to_bits()))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// spec: ONPAGE-144
// spec: PERF-007
#[test]
fn n16_pairs_and_similarity_bits_match_disk_oracle_at_every_threshold() {
    let dir = tempfile::tempdir().unwrap();
    let (_, file) = create(dir.path(), "differential.ospider", 0x16);
    let mut seed = 0x16_u64;
    for id in 1..=96 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let group = id / 6;
        let text = (0..48)
            .map(|word| {
                if word < id % 6 {
                    format!("change{seed}x{word}")
                } else {
                    format!("group{group}x{word}")
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let hash = match id % 6 {
            0 | 1 => Some(format!("exact{group}")),
            2 => None,
            3 => Some(String::new()),
            _ => Some(format!("hash{id}")),
        };
        let mut item = row(id, &format!("{id}.html"))
            .text("content_class", if id % 19 == 0 { "PDF" } else { "HTML" })
            .int("status_code", if id % 17 == 0 { 404 } else { 200 })
            .text(
                "indexability",
                if id % 13 == 0 {
                    "Non-Indexable"
                } else {
                    "Indexable"
                },
            );
        if let Some(hash) = hash {
            item = item.text("hash_md5", &hash);
        }
        insert(&file, &[item]);
        // Missing text is intentionally valid input for comparison (empty shingles).
        if id % 23 != 0 {
            file.conn()
                .execute(
                    "INSERT INTO url_ext(url_id,key,v_text) VALUES (?1,'content.text',?2)",
                    (id, &text),
                )
                .unwrap();
        }
        let sig = if id % 29 == 0 {
            vec![0; 7]
        } else {
            os_store::encode_minhash(&minhash::signature(&minhash::shingles(&text)))
        };
        file.conn()
            .execute("INSERT INTO minhash VALUES (?1,?2)", (id, sig))
            .unwrap();
    }
    let mut cfg = CrawlConfig::default();
    cfg.duplicates.near_enabled = true;
    for threshold in 50..=100 {
        cfg.duplicates.near_threshold_pct = threshold;
        for only_indexable in [false, true] {
            cfg.duplicates.only_indexable = only_indexable;
            baseline::run(&file, &cfg, &CancelToken::new()).unwrap();
            let expected = dump(&file);
            near_duplicates::run(&file, &cfg, &CancelToken::new()).unwrap();
            assert_eq!(
                dump(&file),
                expected,
                "threshold={threshold} indexable={only_indexable}"
            );
        }
    }
}
