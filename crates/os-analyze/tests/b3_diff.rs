//! B3-DIFF (Nutzerentscheidung 04.11.2026): Report „Crawl Changes“ (OUT-154), Change-Detection-Elemente „Canonical“ und
//! „Redirect Target“ (TECH-309, TECH-310) mit erweiterter Population (TECH-301) und die Invariante aus OUT-142 an der
//! Fixture `out-b3-diff` (`v1` Previous → `v2` Current, Inhalt im README dort).

#[path = "compare_common.rs"]
mod common;

use common::*;
use os_analyze::compare::changes::{self, ChangeKind, ChangeRow};
use os_analyze::compare::{change, CompareError, Comparison};
use os_model::{
    CancelToken, ChangeDetectionConfig, ChangeElement, CrawlConfig, CrawlMode, IdSource,
    MappingRule, SeededIds, Timestamp,
};
use os_store::{CrawlFile, CrawlInit, StorageProfile};

async fn b3_v1() -> os_engine::testing::FixtureCrawl {
    crawl_at(
        "out-b3-diff/v1",
        "http://www.site.test/",
        cfg(),
        "2026-01-01T00:00:00.000Z",
        61,
        StorageProfile::Balanced,
    )
    .await
}

async fn b3_v2() -> os_engine::testing::FixtureCrawl {
    crawl_at(
        "out-b3-diff/v2",
        "http://www.site.test/",
        cfg(),
        "2026-02-01T00:00:00.000Z",
        62,
        StorageProfile::Balanced,
    )
    .await
}

fn path(address: &str) -> String {
    address
        .strip_prefix("http://www.site.test")
        .unwrap_or(address)
        .to_string()
}

fn section(rows: &[ChangeRow], kind: ChangeKind) -> Vec<String> {
    rows.iter()
        .filter(|r| r.kind == kind)
        .map(|r| path(r.address()))
        .collect()
}

fn row<'a>(rows: &'a [ChangeRow], kind: ChangeKind, p: &str) -> &'a ChangeRow {
    rows.iter()
        .find(|r| r.kind == kind && path(r.address()) == p)
        .unwrap_or_else(|| panic!("{} {p} fehlt", kind.as_str()))
}

fn changed(c: &Comparison, filter: &str) -> Vec<String> {
    let ids = change::ids(c, filter, true).unwrap();
    addresses(&c.current().file, &ids)
        .into_iter()
        .map(|a| path(&a))
        .collect()
}

// spec: OUT-154
#[tokio::test(flavor = "multi_thread")]
async fn crawl_changes_list_exactly_the_deployment_changes_of_the_fixture() {
    let (old, new) = (b3_v1().await, b3_v2().await);
    let c = compare(&new, &old, &[]);
    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let expected: [(ChangeKind, &[&str]); 8] = [
        (ChangeKind::PageGone, &["/broken-now.html", "/gone.html"]),
        (ChangeKind::NewError, &["/broken-now.html", "/broken.html"]),
        (ChangeKind::NewRedirect, &["/c3", "/moved.html", "/r1b"]),
        (ChangeKind::RedirectTargetChanged, &["/c2", "/r1", "/rt"]),
        (ChangeKind::NewRedirectChain, &["/c2", "/r1"]),
        (ChangeKind::LongerRedirectChain, &["/c1"]),
        (ChangeKind::CanonicalChanged, &["/canon.html"]),
        (
            ChangeKind::IndexabilityChanged,
            &[
                "/broken-now.html",
                "/fixed.html",
                "/idx.html",
                "/moved.html",
            ],
        ),
    ];
    for (kind, paths) in expected {
        assert_eq!(section(&rows, kind), paths, "{}", kind.as_str());
    }
    // Reihenfolge: Abschnitte wie ChangeKind::ALL, darin Address aufsteigend.
    let order: Vec<_> = rows.iter().map(|r| r.kind).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted);
    // Unveränderte und neue fehlerfreie Seiten erscheinen nicht.
    assert!(!rows
        .iter()
        .any(|r| ["/keep.html", "/new.html"].contains(&path(r.address()).as_str())));

    // Werte vorher/nachher.
    let chain = row(&rows, ChangeKind::LongerRedirectChain, "/c1");
    assert_eq!(chain.previous.as_ref().unwrap().chain_length, 2);
    assert_eq!(chain.current.as_ref().unwrap().chain_length, 3);
    let canon = row(&rows, ChangeKind::CanonicalChanged, "/canon.html");
    assert_eq!(
        canon.previous.as_ref().unwrap().canonical.as_deref(),
        Some("http://www.site.test/canon-a.html")
    );
    assert_eq!(
        canon.current.as_ref().unwrap().canonical.as_deref(),
        Some("http://www.site.test/canon-b.html")
    );
    for side in [&canon.previous, &canon.current] {
        assert_eq!(side.as_ref().unwrap().indexability_status, "Canonicalised");
    }
    let gone = row(&rows, ChangeKind::PageGone, "/gone.html");
    assert!(
        gone.current.is_none(),
        "gelöschte Seite ohne Current-Partner"
    );
    assert_eq!(gone.previous.as_ref().unwrap().status_code, Some(200));
    let broken = row(&rows, ChangeKind::PageGone, "/broken-now.html");
    assert_eq!(broken.current.as_ref().unwrap().status_code, Some(404));
    let moved = row(&rows, ChangeKind::NewRedirect, "/moved.html");
    assert_eq!(
        moved.current.as_ref().unwrap().redirect_target.as_deref(),
        Some("http://www.site.test/keep.html")
    );
    assert_eq!(moved.previous.as_ref().unwrap().redirect_target, None);
    // Spaltenwerte in der Reihenfolge der Kopfzeile.
    assert_eq!(canon.values().len(), changes::HEADER.len());
    // Anzahl je Abschnitt (auch Abschnitte mit 0) in Reportreihenfolge.
    let counts = changes::counts(&rows);
    assert_eq!(counts.len(), ChangeKind::ALL.len());
    assert_eq!(counts[0], (ChangeKind::PageGone, 2));
}

// spec: OUT-154
// spec: OUT-142
#[tokio::test(flavor = "multi_thread")]
async fn page_gone_without_partner_is_missing_in_internal_html_and_the_invariant_holds() {
    let (old, new) = (b3_v1().await, b3_v2().await);
    let mut c = compare(&new, &old, &[]);
    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let missing = c
        .class_ids(
            "Internal",
            "HTML",
            os_analyze::compare::CompareClass::Missing,
        )
        .unwrap();
    let missing = addresses(&c.previous().file, &missing);
    for r in rows.iter().filter(|r| r.kind == ChangeKind::PageGone) {
        if r.current.is_none() {
            assert!(
                missing.contains(&r.address().to_string()),
                "{}",
                r.address()
            );
        }
    }
    // OUT-142 auch an dieser Fixture: Current − Previous = Added + New − Removed − Missing für jeden Filter.
    for f in c.overview(&CancelToken::new()).unwrap() {
        let (Some(cur), Some(prev), Some(k)) = (f.current, f.previous, f.classes) else {
            continue;
        };
        assert_eq!(
            cur as i64 - prev as i64,
            k[0] as i64 + k[1] as i64 - k[2] as i64 - k[3] as i64,
            "Invariante verletzt für {}:{}",
            f.tab,
            f.filter
        );
    }
}

// spec: OUT-154
// spec: OUT-141
#[tokio::test(flavor = "multi_thread")]
async fn after_a_swap_the_report_follows_the_new_direction() {
    let (old, new) = (b3_v1().await, b3_v2().await);
    let mut c = compare(&new, &old, &[]);
    c.swap();
    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let gone = section(&rows, ChangeKind::PageGone);
    assert!(gone.contains(&"/new.html".to_string()), "{gone:?}");
    assert!(!gone.contains(&"/gone.html".to_string()), "{gone:?}");
}

// spec: OUT-154
// spec: CRAWL-129
#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_report_yields_no_partial_result() {
    let (old, new) = (b3_v1().await, b3_v2().await);
    let c = compare(&new, &old, &[]);
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        changes::crawl_changes(&c, &cancel),
        Err(CompareError::Cancelled)
    ));
}

// spec: TECH-309
// spec: TECH-310
// spec: TECH-301
// spec: OUT-154
#[tokio::test(flavor = "multi_thread")]
async fn canonical_and_redirect_target_elements_match_the_report() {
    let (old, new) = (b3_v1().await, b3_v2().await);
    let c = compare(&new, &old, &[]);
    let cfg = ChangeDetectionConfig::default();
    assert!(cfg.elements.contains(&ChangeElement::Canonical));
    assert!(cfg.elements.contains(&ChangeElement::RedirectTarget));
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    assert_eq!(changed(&c, "Canonical"), ["/canon.html"]);
    assert_eq!(
        changed(&c, "Redirect Target"),
        ["/c2", "/moved.html", "/r1", "/rt"]
    );
    // Population: Weiterleitungen gehören dazu (TECH-301), auch ohne HTML-Content-Type; /c1 bleibt beim ersten Ziel.
    let all = changed(&c, "All");
    for p in ["/c1", "/moved.html", "/rt", "/keep.html"] {
        assert!(all.contains(&p.to_string()), "{p} fehlt in {all:?}");
    }
    // Für eine Weiterleitung sind nur Indexability und Redirect Target erhoben (TECH-301): /moved.html ist Seite →
    // Weiterleitung, Titel und Wortzahl werden nicht verglichen.
    assert!(!changed(&c, "Page Titles").contains(&"/moved.html".to_string()));
    assert!(!changed(&c, "Word Count").contains(&"/moved.html".to_string()));
    assert!(changed(&c, "Indexability").contains(&"/moved.html".to_string()));

    // Übereinstimmung mit dem Report für die Population (OUT-154).
    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let in_population = |p: &String| all.contains(p);
    let report = |kind| -> Vec<String> {
        let mut v: Vec<String> = section(&rows, kind)
            .into_iter()
            .filter(in_population)
            .collect();
        v.sort();
        v
    };
    assert_eq!(
        report(ChangeKind::CanonicalChanged),
        changed(&c, "Canonical")
    );
    assert_eq!(
        report(ChangeKind::IndexabilityChanged),
        changed(&c, "Indexability")
    );
    // Redirect Target Changed = Teil von „Redirect Target“, der in beiden Crawls weiterleitet.
    let both: Vec<String> = changed(&c, "Redirect Target")
        .into_iter()
        .filter(|p| p != "/moved.html")
        .collect();
    assert_eq!(report(ChangeKind::RedirectTargetChanged), both);

    // Spalten der neuen Elemente (TECH-309/310).
    let tab = change::tab(&cfg, true);
    for column in [
        "Canonical (Current)",
        "Canonical (Previous)",
        "Redirect Target (Current)",
        "Redirect Target (Previous)",
    ] {
        assert!(tab.columns.iter().any(|c| c.key == column), "{column}");
    }
    assert!(tab.filters.iter().any(|f| f.id == "Canonical"));
    assert!(tab.filters.iter().any(|f| f.id == "Redirect Target"));
}

// ---------------------------------------------------------------------------------------------
// Synthetische Paare für Randfälle (TECH-309, TECH-310)

fn side(
    dir: &std::path::Path,
    seed: u64,
    at: &str,
    host: &str,
    rows: &str,
) -> os_analyze::compare::CrawlSide {
    let at = Timestamp::from_db(at).unwrap();
    let id = SeededIds::new(seed).new_crawl_id(at);
    let path = dir.join(format!("{id}.ospider"));
    let file = CrawlFile::create(
        &path,
        &CrawlInit {
            crawl_id: id,
            mode: CrawlMode::Spider,
            start_url: Some(format!("https://{host}/")),
            list_source: None,
            project: "b3".into(),
            name: host.into(),
            config: CrawlConfig::default(),
            created_at: at,
        },
        StorageProfile::Balanced,
    )
    .unwrap();
    file.set_meta("started_at", &at.to_db()).unwrap();
    let rows = rows.replace("{host}", host);
    file.conn().execute_batch(&rows).unwrap();
    os_analyze::compare::CrawlSide::from_file(file, path, None).unwrap()
}

const URL: &str = "INSERT INTO urls(id,url,address,origin,is_internal,in_tables,state,discovered_via,folder_depth,url_length,content_class,status_code,indexability,indexability_status,canonical_url,redirect_type,redirect_url) VALUES";

// spec: TECH-309
// spec: TECH-310
#[test]
fn mapped_targets_body_canonicals_and_status_only_changes_are_no_change() {
    let dir = tempfile::tempdir().unwrap();
    // p: Canonical auf sich selbst (Host je Crawl), q: Body-Canonical geändert (canonical_url gleich), r: 301 → 308 mit
    // gleichem Ziel, s: Canonical-Ziel wirklich geändert, t: Weiterleitung neu (Seite → Weiterleitung).
    let previous = format!(
        "{URL}(1,'https://{{host}}/p','https://{{host}}/p','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','','https://{{host}}/p',NULL,NULL),\
         (2,'https://{{host}}/q','https://{{host}}/q','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','','https://{{host}}/q',NULL,NULL),\
         (3,'https://{{host}}/r','https://{{host}}/r','https://{{host}}',1,1,'done','link',0,20,'Unknown',301,'Non-Indexable','Redirected',NULL,'http','https://{{host}}/z'),\
         (4,'https://{{host}}/s','https://{{host}}/s','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Non-Indexable','Canonicalised','https://{{host}}/a',NULL,NULL),\
         (5,'https://{{host}}/t','https://{{host}}/t','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','',NULL,NULL,NULL);\
         INSERT INTO canonicals(url_id,source,position,raw,resolved,in_head) VALUES(2,'html',0,'/old-body','https://{{host}}/old-body',0);"
    );
    let current = format!(
        "{URL}(1,'https://{{host}}/p','https://{{host}}/p','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','','https://{{host}}/p',NULL,NULL),\
         (2,'https://{{host}}/q','https://{{host}}/q','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','','https://{{host}}/q',NULL,NULL),\
         (3,'https://{{host}}/r','https://{{host}}/r','https://{{host}}',1,1,'done','link',0,20,'Unknown',308,'Non-Indexable','Redirected',NULL,'http','https://{{host}}/z'),\
         (4,'https://{{host}}/s','https://{{host}}/s','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Non-Indexable','Canonicalised','https://{{host}}/b',NULL,NULL),\
         (5,'https://{{host}}/t','https://{{host}}/t','https://{{host}}',1,1,'done','link',0,20,'Unknown',301,'Non-Indexable','Redirected',NULL,'http','https://{{host}}/p');\
         INSERT INTO canonicals(url_id,source,position,raw,resolved,in_head) VALUES(2,'html',0,'/new-body','https://{{host}}/new-body',0);"
    );
    let rules = [MappingRule {
        pattern: r"^https://staging\.site\.test".into(),
        replacement: "https://www.site.test".into(),
    }];
    let c = Comparison::open(
        side(
            dir.path(),
            1,
            "2026-01-01T00:00:00.000Z",
            "staging.site.test",
            &previous,
        ),
        side(
            dir.path(),
            2,
            "2026-02-01T00:00:00.000Z",
            "www.site.test",
            &current,
        ),
        &rules,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(c.current().meta.name, "www.site.test");
    let cfg = ChangeDetectionConfig::default();
    change::calculate(&c, &cfg, &CancelToken::new()).unwrap();
    let ids = |filter| -> Vec<u32> { change::ids(&c, filter, true).unwrap().iter().collect() };
    assert_eq!(ids("Canonical"), [4], "nur das wirklich geänderte Ziel");
    assert_eq!(ids("Redirect Target"), [5], "nur die neue Weiterleitung");
    // Angezeigt wird der Rohwert des Previous-Crawls.
    let shown: String = c
        .current()
        .file
        .conn()
        .query_row(
            "SELECT previous FROM temp.c02_cells WHERE url_id=4 AND element='canonical'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(shown, "https://staging.site.test/a");

    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let kinds = |id: i64| -> Vec<ChangeKind> {
        rows.iter()
            .filter(|r| r.current.as_ref().is_some_and(|s| s.id == id))
            .map(|r| r.kind)
            .collect()
    };
    assert!(kinds(1).is_empty() && kinds(2).is_empty() && kinds(3).is_empty());
    assert_eq!(kinds(4), [ChangeKind::CanonicalChanged]);
    assert_eq!(
        kinds(5),
        [ChangeKind::NewRedirect, ChangeKind::IndexabilityChanged]
    );
}

// spec: OUT-154
#[test]
fn no_response_is_not_a_new_error_and_counts_as_an_indexability_change() {
    let dir = tempfile::tempdir().unwrap();
    // u: vorher keine Antwort, jetzt 404 → kein „New Error“ (vorher nicht „ohne Fehler geantwortet“).
    // v: vorher indexierbar, jetzt keine Antwort → „Page Gone“ und „Indexability Changed“, kein „New Error“.
    let previous = format!(
        "{URL}(1,'https://{{host}}/u','https://{{host}}/u','https://{{host}}',1,1,'done','link',0,20,'Unknown',0,'Non-Indexable','No Response',NULL,NULL,NULL),\
         (2,'https://{{host}}/v','https://{{host}}/v','https://{{host}}',1,1,'done','link',0,20,'HTML',200,'Indexable','',NULL,NULL,NULL);"
    );
    let current = format!(
        "{URL}(1,'https://{{host}}/u','https://{{host}}/u','https://{{host}}',1,1,'done','link',0,20,'Other',404,'Non-Indexable','Client Error',NULL,NULL,NULL),\
         (2,'https://{{host}}/v','https://{{host}}/v','https://{{host}}',1,1,'done','link',0,20,'Unknown',NULL,'Non-Indexable','No Response',NULL,NULL,NULL);"
    );
    let c = Comparison::open(
        side(
            dir.path(),
            1,
            "2026-01-01T00:00:00.000Z",
            "site.test",
            &previous,
        ),
        side(
            dir.path(),
            2,
            "2026-02-01T00:00:00.000Z",
            "site.test",
            &current,
        ),
        &[],
        &CancelToken::new(),
    )
    .unwrap();
    let rows = changes::crawl_changes(&c, &CancelToken::new()).unwrap();
    let kinds = |id: i64| -> Vec<ChangeKind> {
        rows.iter()
            .filter(|r| r.current.as_ref().is_some_and(|s| s.id == id))
            .map(|r| r.kind)
            .collect()
    };
    assert_eq!(kinds(1), [ChangeKind::IndexabilityChanged]);
    assert_eq!(
        kinds(2),
        [ChangeKind::PageGone, ChangeKind::IndexabilityChanged]
    );
}
