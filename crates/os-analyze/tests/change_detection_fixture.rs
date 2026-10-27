#![allow(dead_code, clippy::disallowed_types, clippy::disallowed_methods)]
//! Synthetische SQLite-Fixture: zwei Site-Versionen mit denselben URLs und unterschiedlichen Messwerten.
use os_analyze::compare::{Comparison, CrawlSide};
use os_model::{
    CancelToken, ChangeDetectionConfig, ChangeElement, CrawlConfig, CrawlMode, IdSource, SeededIds,
    Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

pub fn fixture(dir: &std::path::Path, seed: u64, version: i64, config: CrawlConfig) -> CrawlFile {
    let at = Timestamp::from_db(if version == 1 {
        "2026-01-01T00:00:00.000Z"
    } else {
        "2026-02-01T00:00:00.000Z"
    })
    .unwrap();
    let id = SeededIds::new(seed).new_crawl_id(at);
    let file = CrawlFile::create(
        &dir.join(format!("{id}.ospider")),
        &CrawlInit {
            crawl_id: id,
            mode: CrawlMode::Spider,
            start_url: Some("https://site.test/".into()),
            list_source: None,
            project: "project".into(),
            name: format!("v{version}"),
            config,
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.set_meta("state", "Completed").unwrap();
    file.set_meta("end_reason", "queue_empty").unwrap();
    file.set_meta("started_at", &at.to_db()).unwrap();
    for (id, path) in [
        (1, "a"),
        (2, "b"),
        (3, "c"),
        (4, "d"),
        (5, "e"),
        (6, "f"),
        (7, "g"),
        (8, "h"),
        (9, "i"),
        (10, "j"),
        (11, "k"),
        (12, "l"),
        (13, "m"),
    ] {
        let address = format!("https://site.test/{path}");
        file.conn().execute("INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,title_1,h1_1,word_count,crawl_depth,indexability,indexability_status) VALUES(?1,?2,?2,'https://site.test',1,1,'done','link',0,30,'HTML',200,'Shop','Heading',100,1,'Indexable','')",(id, address)).unwrap();
        file.conn().execute("INSERT INTO sd_items(url_id,format,type_path,json) VALUES(?1,'json-ld','Product','{\"@type\":\"Product\"}')",[id]).unwrap();
        file.conn().execute("INSERT INTO url_ext(url_id,key,v_text) VALUES(?1,'content.text','one two three four five six seven')",[id]).unwrap();
    }
    if version == 2 {
        file.conn().execute_batch("UPDATE urls SET title_1='shop' WHERE id=1; UPDATE urls SET title_1='  Shop  ' WHERE id=2;UPDATE urls SET h1_1=NULL WHERE id=3;UPDATE urls SET meta_desc_1='New' WHERE id=4;UPDATE urls SET word_count=130 WHERE id=5;UPDATE urls SET crawl_depth=2 WHERE id=6;UPDATE url_counters SET inlinks=2,unique_inlinks=1 WHERE url_id=7;UPDATE sd_items SET json='{\"@type\":\"Article\"}' WHERE url_id=8;UPDATE urls SET indexability='Non-Indexable',indexability_status='noindex' WHERE id=9;UPDATE urls SET indexability_status='noindex; Canonicalised' WHERE id=10;UPDATE url_ext SET v_text='completely different new text tokens example here' WHERE url_id=11;UPDATE urls SET content_class='PDF' WHERE id=12;DELETE FROM urls WHERE id=13;").unwrap();
    } else {
        file.conn().execute_batch("UPDATE url_counters SET inlinks=1,unique_inlinks=1 WHERE url_id=7;UPDATE urls SET indexability_status='Canonicalised; noindex' WHERE id=10;").unwrap();
    }
    file
}
pub fn comparison(a: CrawlFile, b: CrawlFile) -> Comparison {
    let ap = a.path().to_path_buf();
    let bp = b.path().to_path_buf();
    Comparison::open(
        CrawlSide::from_file(a, ap, None).unwrap(),
        CrawlSide::from_file(b, bp, None).unwrap(),
        &[],
        &CancelToken::new(),
    )
    .unwrap()
}
pub fn all() -> ChangeDetectionConfig {
    ChangeDetectionConfig {
        elements: ChangeElement::ALL.to_vec(),
        content_threshold_pct: 10,
    }
}
