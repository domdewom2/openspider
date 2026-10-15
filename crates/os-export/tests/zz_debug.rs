#[path = "reports_common.rs"]
mod common;
use common::base_config;
use os_engine::testing::crawl_fixture_from;

#[tokio::test]
async fn dump() {
    let mut cfg = base_config();
    cfg.scope.all_subdomains = true;
    let fx = crawl_fixture_from("out-redirects", "http://site.test/", cfg.clone()).await;
    let c = fx.file.conn();
    let mut stmt = c.prepare("SELECT u.id, u.url, u.state, u.status_code, c.inlinks, c.inlinks_all, c.outlinks, (SELECT COUNT(*) FROM links l WHERE l.target_id = u.id), (SELECT COUNT(*) FROM links l WHERE l.target_id = u.id AND l.link_type='Hyperlink') FROM urls u LEFT JOIN url_counters c ON c.url_id = u.id ORDER BY u.id LIMIT 12").unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok(format!(
                "{} {} {} {:?} in={:?} all={:?} out={:?} L={} H={}",
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, Option<i64>>(5)?,
                r.get::<_, Option<i64>>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?
            ))
        })
        .unwrap();
    for r in rows {
        println!("{}", r.unwrap());
    }
    println!("verify: {:?}", os_store::counters::verify_all(c).unwrap());
}
