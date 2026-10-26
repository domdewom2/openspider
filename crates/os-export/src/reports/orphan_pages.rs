//! Orphan Pages from GA, GSC and XML sitemaps (OUT-099, INTEG-015).

use crate::catalog::reports::ReportRequirement;
use crate::catalog::{TableEntry, TableJob};
use crate::error::ExportError;
use crate::types::TableWriter;

/// Die Einträge dieser Gruppe in Menüreihenfolge.
pub fn entries() -> &'static [TableEntry] {
    ENTRIES
}

const ENTRIES: &[TableEntry] = &[TableEntry {
    name: "Orphan Pages",
    aliases: &[],
    file_stem: "Orphan Pages",
    header: &["Address", "Source", "Status Code", "Indexability"],
    header_fn: None,
    requires: &[
        ReportRequirement::CrawlAnalysis,
        ReportRequirement::OrphanSourceAvailable,
    ],
    owner: "M2-D13",
    run,
}];

fn run(job: &TableJob<'_>, writer: &mut dyn TableWriter) -> Result<u64, ExportError> {
    use crate::reports::common::db;
    use os_analyze::ColumnKind;
    use os_model::Value;
    use std::collections::BTreeMap;
    if !os_analyze::views::state_of_in(job.tabs, job.file, job.cfg, "Sitemaps", "Orphan URLs")?
        .yields_rows()
    {
        return Err(ExportError::Unsupported(format!(
            "{} requires the Sitemaps crawl analysis",
            job.entry_name
        )));
    }
    let normalize = |address: &str| {
        url::Url::parse(address).ok().map(|mut u| {
            u.set_fragment(None);
            os_url::normalize(&u).encoded
        })
    };
    // Only source URLs are retained, rather than a crawl-wide address index.
    let raw_hosts: BTreeMap<_, std::collections::BTreeSet<String>> = ["ga4", "gsc"]
        .into_iter()
        .map(|source| {
            let value = db(job.file.get_meta(&format!("api_raw_hosts.{source}")))?;
            let hosts = value
                .and_then(|v| serde_json::from_str(&v).ok())
                .unwrap_or_default();
            Ok::<_, ExportError>((source, hosts))
        })
        .collect::<Result<_, _>>()?;
    let mut sources: BTreeMap<String, u8> = BTreeMap::new();
    let mut stmt = db(job.file.conn().prepare("SELECT url,source,0 FROM api_urls WHERE source IN ('ga4','gsc') UNION SELECT u.url,a.source,1 FROM api_values a JOIN urls u ON u.id=a.url_id WHERE a.source IN ('ga4','gsc') UNION SELECT u.url,'sitemap',0 FROM url_flags f JOIN urls u ON u.id=f.url_id JOIN sitemap_entries e ON e.url_id=COALESCE(CAST(NULLIF(f.value,'') AS INTEGER),u.id) WHERE f.flag='orphan' UNION SELECT u.url,'sitemap',0 FROM sitemap_entries e JOIN urls u ON u.id=e.url_id JOIN sitemaps s ON s.id=e.sitemap_id WHERE s.kind='urlset'"))?;
    let mut rows = db(stmt.query([]))?;
    while let Some(row) = db(rows.next())? {
        writer.checkpoint()?;
        let address: String = db(row.get(0))?;
        let source: String = db(row.get(1))?;
        let mapped: bool = db(row.get(2))?;
        if mapped
            && url::Url::parse(&address)
                .ok()
                .and_then(|u| u.host_str().map(str::to_lowercase))
                .is_some_and(|host| {
                    raw_hosts
                        .get(source.as_str())
                        .is_some_and(|hosts| hosts.contains(&host))
                })
        {
            continue; // OUT-099 uses raw URL provenance, not the looser INTEG-014 metric match.
        }
        if let Some(key) = normalize(&address) {
            *sources.entry(key).or_default() |= match source.as_str() {
                "ga4" => 1,
                "gsc" => 2,
                _ => 4,
            };
        }
    }
    // Normalize stored addresses as well: fragments and default ports must not hide an inlink.
    let mut facts = BTreeMap::new();
    let mut stmt = db(job.file.conn().prepare("SELECT u.url,u.address,u.status_code,u.indexability,((COALESCE(c.inlinks_all,0)-COALESCE(c.self_inlinks,0)>0 OR incoming.target_id IS NOT NULL) AND NOT EXISTS(SELECT 1 FROM url_flags f WHERE f.url_id=u.id AND f.flag='orphan')) OR u.discovered_via IN ('start','list_input','redirect') FROM urls u LEFT JOIN url_counters c ON c.url_id=u.id LEFT JOIN (SELECT DISTINCT l.target_id FROM links l JOIN urls src ON src.id=l.source_id WHERE src.is_internal=1 AND l.source_id<>l.target_id) incoming ON incoming.target_id=u.id WHERE u.is_internal=1"))?;
    let mut rows = db(stmt.query([]))?;
    while let Some(row) = db(rows.next())? {
        writer.checkpoint()?;
        let url: String = db(row.get(0))?;
        if let Some(key) = normalize(&url).filter(|key| sources.contains_key(key)) {
            let linked: bool = db(row.get(4))?;
            let entry =
                facts
                    .entry(key)
                    .or_insert((String::new(), None::<i64>, None::<String>, false));
            if entry.0.is_empty() || db(row.get::<_, Option<i64>>(2))?.is_some() {
                entry.0 = db(row.get(1))?;
                entry.1 = db(row.get(2))?;
                entry.2 = db(row.get(3))?;
            }
            entry.3 |= linked;
        }
    }
    let mut count = 0;
    for (address, mask) in sources {
        let fact = facts.get(&address);
        if fact.is_some_and(|f| f.3) {
            continue;
        }
        let source = [
            "",
            "GA",
            "GSC",
            "GA & GSC",
            "Sitemap",
            "GA & Sitemap",
            "GSC & Sitemap",
            "GA & GSC & Sitemap",
        ][mask as usize];
        writer.row(
            &[
                ColumnKind::Url,
                ColumnKind::Text,
                ColumnKind::Int,
                ColumnKind::Text,
            ],
            &[
                Value::Text(fact.map(|f| f.0.clone()).unwrap_or(address)),
                Value::Text(source.into()),
                fact.and_then(|f| f.1)
                    .filter(|v| *v != 0)
                    .map(Value::Int)
                    .unwrap_or(Value::Null),
                fact.and_then(|f| f.2.clone())
                    .map(Value::Text)
                    .unwrap_or(Value::Null),
            ],
        )?;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use os_engine::testing::{crawl_dir_with, test_deps};
    use os_integrations::credentials::{Credential, Credentials};
    use os_model::{ApiKind, CrawlConfig, FilterState};
    use os_testsite::mocks::{
        ga4::{Ga4Mock, MockRow, STREAM},
        MockServer,
    };
    use std::sync::Arc;

    // spec: INTEG-015
    // spec: INTEG-017
    // spec: OUT-099 [P1]
    #[tokio::test]
    async fn engine_discovers_orphans_through_normal_rules_and_waits_for_api_data() {
        let mock = Ga4Mock::default();
        mock.configure(|cfg| {
            cfg.rows = ["linked", "orphan", "blocked", "excluded"]
                .into_iter()
                .map(|p| MockRow::sessions(&format!("http://integ.test/{p}"), 10.0))
                .chain([MockRow::sessions("https://fremd.test/x", 20.0)])
                .collect()
        });
        let server = MockServer::start(vec![Arc::new(mock)]).await.unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("scenario.toml"),
            r#"
[[route]]
path = "/"
body = '<a href="/linked">linked</a>'
[[route]]
path = "/linked"
body = '<title>Linked</title>'
[[route]]
path = "/orphan"
body = '<title>Orphan</title>'
[[route]]
path = "/robots.txt"
content_type = "text/plain"
body = "User-agent: *\nDisallow: /blocked"
"#,
        )
        .unwrap();
        for crawl_new in [false, true] {
            let mut deps = test_deps();
            let store = Arc::new(os_fetch::MemorySecretStore::new());
            Credentials::new(store.clone())
                .put(
                    ApiKind::Ga4,
                    Credential {
                        account_id: "fixture".into(),
                        label: "Fixture".into(),
                        api_key: None,
                        access_token: Some("fixture-token".into()),
                        refresh_token: None,
                        expires_at: None,
                        client_secret: None,
                        scopes: vec![],
                    },
                    true,
                )
                .unwrap();
            deps.secrets = store;
            let mut cfg = CrawlConfig::default();
            cfg.api.ga4.enabled = true;
            cfg.api.ga4.base_url = Some(server.base_url("ga4"));
            cfg.api.ga4.property = Some(STREAM.into());
            cfg.api.ga4.crawl_new_urls = crawl_new;
            cfg.scope.exclude = vec![".*/excluded".into()];
            let fx = crawl_dir_with(dir.path(), "http://integ.test/", cfg.clone(), deps).await;
            assert_eq!(
                fx.site.count("integ.test", "/orphan"),
                usize::from(crawl_new)
            );
            assert_eq!(fx.site.count("integ.test", "/blocked"), 0);
            assert_eq!(fx.site.count("integ.test", "/excluded"), 0);
            assert_eq!(fx.site.count("fremd.test", "/x"), 0);
            let count: i64 = fx
                .file
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM api_values WHERE source='ga4'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                count > 0,
                "API results must be saved before wait() completes"
            );
            if crawl_new {
                let counts = os_analyze::views::filter_counts(&fx.file, &cfg).unwrap();
                assert_eq!(
                    counts
                        .iter()
                        .find(|c| c.tab == "Analytics" && c.filter == "Orphan URLs")
                        .unwrap()
                        .state,
                    FilterState::NeedsCrawlAnalysis
                );
            }
            os_analyze::analysis::run(&fx.file, &cfg, &Default::default(), &|_, _| {}).unwrap();
            let output = dir.path().join(if crawl_new { "on" } else { "off" });
            let result =
                crate::save_report(&fx.file, &cfg, "Orphan Pages", &output, crate::Format::Csv)
                    .unwrap();
            let mut reader = csv::Reader::from_path(&result.files[0].path).unwrap();
            let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
            assert!(rows
                .iter()
                .any(|r| r[0].ends_with("/orphan") && &r[1] == "GA"));
            assert!(!rows
                .iter()
                .any(|r| r[0].ends_with("/linked") || r[0].contains("fremd.test")));
            if crawl_new {
                let counts = os_analyze::views::filter_counts(&fx.file, &cfg).unwrap();
                assert!(
                    counts
                        .iter()
                        .find(|c| c.tab == "Analytics" && c.filter == "Orphan URLs")
                        .unwrap()
                        .count
                        .unwrap()
                        > 0
                );
            }
            let all = crate::export_tab(
                &fx.file,
                &cfg,
                &os_analyze::views::ViewQuery::new("Internal", "All"),
                &output,
                crate::Format::Csv,
            )
            .unwrap();
            let mut csv = csv::Reader::from_path(&all.path).unwrap();
            assert!(csv.headers().unwrap().iter().any(|h| h == "GA4 Sessions"));
        }
        server.stop().await;
    }
    // spec: OUT-099 [P1]
    #[tokio::test]
    async fn report_merges_all_sources_and_normalizes_fragments_but_preserves_slashes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<title>Fixture</title>").unwrap();
        let fx = crawl_dir_with(
            dir.path(),
            "http://integ.test/",
            CrawlConfig::default(),
            test_deps(),
        )
        .await;
        let conn = fx.file.conn();
        conn.execute("INSERT INTO sitemaps(id,url,kind,depth,found_via) VALUES(1,'http://integ.test/map.xml','urlset',0,'manual')",[]).unwrap();
        for mask in 1..=7 {
            let address = format!("http://integ.test/p{mask}");
            conn.execute("INSERT INTO urls(id,url,address,origin,is_internal,state,discovered_via,folder_depth,url_length) VALUES(?1,?2,?2,'http://integ.test',1,'discovered','link',1,25)",(100+mask,&address)).unwrap();
            for (source, bit) in [("ga4", 1), ("gsc", 2)] {
                if mask & bit != 0 {
                    conn.execute("INSERT INTO api_urls(url,source,first_seen) VALUES(?1,?2,'2026-10-19T12:00:00Z')",(format!("HTTP://INTEG.TEST:80/p{mask}#fragment"),source)).unwrap();
                }
            }
            if mask & 4 != 0 {
                conn.execute(
                    "INSERT INTO sitemap_entries(sitemap_id,url_id,loc_raw) VALUES(1,?1,?2)",
                    (100 + mask, &address),
                )
                .unwrap();
            }
        }
        conn.execute("INSERT INTO api_urls(url,source,first_seen) VALUES('http://integ.test/p1/','ga4','2026-10-19T12:00:00Z')",[]).unwrap();
        // INTEG-014 can assign /p4/ metrics to /p4; OUT-099 must keep their sources separate.
        conn.execute("INSERT INTO api_urls(url,source,first_seen) VALUES('http://integ.test/p4/','ga4','2026-10-19T12:00:00Z')",[]).unwrap();
        conn.execute("INSERT INTO api_values(url_id,source,metric,level,v_real,fetched_at) VALUES(104,'ga4','sessions','url',42,'2026-10-19T12:00:00Z')",[]).unwrap();
        conn.execute("INSERT INTO api_urls(url,source,first_seen) VALUES('https://integ.test/p6','ga4','2026-10-19T12:00:00Z')",[]).unwrap();
        conn.execute("INSERT INTO api_values(url_id,source,metric,level,v_real,fetched_at) VALUES(106,'ga4','sessions','url',42,'2026-10-19T12:00:00Z')",[]).unwrap();
        fx.file
            .set_meta("api_raw_hosts.ga4", r#"["integ.test"]"#)
            .unwrap();
        // A normalized variant with an inlink suppresses all source variants of that address.
        conn.execute("UPDATE url_counters SET inlinks_all=1 WHERE url_id=107", [])
            .unwrap();
        let mut cfg = CrawlConfig::default();
        cfg.api.ga4.enabled = true;
        cfg.api.gsc.enabled = true;
        os_analyze::analysis::run(&fx.file, &cfg, &Default::default(), &|_, _| {}).unwrap();
        let result = crate::save_report(
            &fx.file,
            &cfg,
            "Orphan Pages",
            &dir.path().join("report"),
            crate::Format::Csv,
        )
        .unwrap();
        let rows: Vec<_> = csv::Reader::from_path(&result.files[0].path)
            .unwrap()
            .records()
            .map(Result::unwrap)
            .collect();
        for (mask, source) in [
            (1, "GA"),
            (2, "GSC"),
            (3, "GA & GSC"),
            (4, "Sitemap"),
            (5, "GA & Sitemap"),
            (6, "GSC & Sitemap"),
        ] {
            assert!(rows
                .iter()
                .any(|r| r[0] == format!("http://integ.test/p{mask}") && &r[1] == source));
        }
        assert!(!rows.iter().any(|r| r[0].ends_with("/p7")));
        assert!(rows.iter().any(|r| r[0].ends_with("/p4/") && &r[1] == "GA"));
        assert!(rows
            .iter()
            .any(|r| &r[0] == "https://integ.test/p6" && &r[1] == "GA"));
        assert!(rows
            .iter()
            .any(|r| r[0].ends_with("/p1/") && r[2].is_empty()));
        conn.execute("UPDATE url_counters SET inlinks_all=0 WHERE url_id=107", [])
            .unwrap();
        let result = crate::save_report(
            &fx.file,
            &cfg,
            "Orphan Pages",
            &dir.path().join("all"),
            crate::Format::Csv,
        )
        .unwrap();
        let rows: Vec<_> = csv::Reader::from_path(&result.files[0].path)
            .unwrap()
            .records()
            .map(Result::unwrap)
            .collect();
        assert!(rows
            .iter()
            .any(|r| r[0].ends_with("/p7") && &r[1] == "GA & GSC & Sitemap"));
    }
}
