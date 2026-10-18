use os_engine::{recompute, testing::crawl_fixture};
use os_model::{CancelToken, CrawlConfig, UrlId};

// spec: TECH-068
// spec: TECH-051 [M2]
#[tokio::test]
async fn switching_refresh_and_respect_recomputes_persisted_facts_and_visibility() {
    let old = CrawlConfig::default();
    let mut fx = crawl_fixture("tech-m2-directives", old.clone()).await;
    let conn = fx.file.conn();
    // Eine fremde Ausblendung wird beim Umschalten nicht aufgehoben.
    conn.execute(
        "UPDATE urls SET in_tables = 0 WHERE address LIKE '%/ok.html'",
        [],
    )
    .unwrap();
    // Selbst-Refresh + noindex: Refresh-Status hat Vorrang, daher bei Respect Noindex sichtbar.
    conn.execute(
        "UPDATE urls SET effective_index = 0 WHERE address LIKE '%/refresh-bare.html'",
        [],
    )
    .unwrap();
    // Canonical-Quelle hängt am Selbst-Refresh-Ziel und benötigt ebenfalls einen Feed-Stempel.
    conn.execute(
        "UPDATE urls SET canonical_url = ?1 WHERE address LIKE '%/noarchive.html'",
        [format!("{}/refresh-self.html", fx.site.base_url())],
    )
    .unwrap();
    conn.execute("INSERT INTO canonicals(url_id, source, position, raw, resolved, target_id, in_head) SELECT s.id, 'html', 0, t.url, t.url, t.id, 1 FROM urls s, urls t WHERE s.address LIKE '%/noarchive.html' AND t.address LIKE '%/refresh-self.html'", []).unwrap();
    conn.execute("INSERT INTO pagination(url_id, rel, source, position, raw, url, target_id, in_head) SELECT s.id, 'next', 'html', 0, t.url, t.url, t.id, 1 FROM urls s, urls t WHERE s.address LIKE '%/ok.html' AND t.address LIKE '%/refresh-self.html'", []).unwrap();
    let mut new = old.clone();
    new.advanced.respect_self_ref_meta_refresh = true;
    new.advanced.respect_noindex = true;
    new.advanced.respect_canonical = true;
    let seq: i64 = fx
        .file
        .get_meta("change_seq")
        .unwrap()
        .unwrap()
        .parse()
        .unwrap();
    let out = recompute::run_triggered(&mut fx.file, &old, &new, &CancelToken::new()).unwrap();
    assert_eq!(out.ran, ["M2-D06"]);
    assert!(!out.changed.is_empty());
    for (path, status, visible) in [
        ("refresh-self", "Self Referencing Meta Refresh", true),
        ("refresh-bare", "Self Referencing Meta Refresh", true),
        ("header-self", "Self Referencing Meta Refresh", true),
        ("refresh-query", "Redirected", true),
        ("body", "noindex", false),
        ("header", "noindex", false),
        ("none", "noindex", false),
        ("gone", "Client Error", true),
        ("noarchive", "Canonicalised", false),
    ] {
        let (id, actual, shown, changed): (i64, String, bool, i64) = fx.file.conn().query_row(
            "SELECT id, indexability_status, in_tables, change_seq FROM urls WHERE address = ?1",
            [format!("{}/{path}.html", fx.site.base_url())], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap();
        assert_eq!((actual.as_str(), shown), (status, visible), "{path}");
        if path != "gone" && path != "refresh-query" {
            assert!(out.changed.contains(&UrlId(id)), "{path}");
            assert!(changed > seq, "Änderungsfeed {path}");
        }
    }
    let (dependent_id, dependent_seq): (i64, i64) = fx
        .file
        .conn()
        .query_row(
            "SELECT id, change_seq FROM urls WHERE address LIKE '%/ok.html'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(dependent_seq > seq);
    assert!(out.changed.contains(&UrlId(dependent_id)));
    assert_eq!(
        fx.file.get_meta("analysis_stale").unwrap().as_deref(),
        Some("1")
    );
    let unchanged = recompute::d06::run(&mut fx.file, &old, &new, &CancelToken::new()).unwrap();
    assert!(unchanged.is_empty(), "wiederholter Lauf ist idempotent");
    let reversed = recompute::run_triggered(&mut fx.file, &new, &old, &CancelToken::new()).unwrap();
    assert!(!reversed.changed.is_empty());
    let count: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags WHERE flag = 'respect_hidden'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
    let visible: bool = fx
        .file
        .conn()
        .query_row(
            "SELECT in_tables FROM urls WHERE address LIKE '%/ok.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!visible);
    let visible: bool = fx
        .file
        .conn()
        .query_row(
            "SELECT in_tables FROM urls WHERE address LIKE '%/body.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(visible);
}

// spec: TECH-068
#[tokio::test]
async fn cancelled_recompute_preserves_database_and_change_sequence() {
    let old = CrawlConfig::default();
    let mut fx = crawl_fixture("tech-m2-directives", old.clone()).await;
    let before = fx.file.get_meta("change_seq").unwrap();
    let mut new = old.clone();
    new.advanced.respect_self_ref_meta_refresh = true;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        recompute::d06::run(&mut fx.file, &old, &new, &cancel),
        Err(os_engine::EngineError::Cancelled)
    ));
    assert_eq!(fx.file.get_meta("change_seq").unwrap(), before);
    let count: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM urls WHERE indexability_status = 'Self Referencing Meta Refresh'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

// spec: TECH-068
#[tokio::test]
async fn write_failure_rolls_back_visibility_status_and_change_feed_together() {
    let old = CrawlConfig::default();
    let mut fx = crawl_fixture("tech-m2-directives", old.clone()).await;
    let before = fx.file.get_meta("change_seq").unwrap();
    let mut new = old.clone();
    new.advanced.respect_noindex = true;
    new.advanced.respect_self_ref_meta_refresh = true;
    // Der Fehler tritt nach den vorangehenden Noindex-Ausblendungen ein.
    fx.file.conn().execute_batch("CREATE TEMP TRIGGER reject_refresh_update BEFORE UPDATE OF indexability ON urls WHEN NEW.indexability_status = 'Self Referencing Meta Refresh' BEGIN SELECT RAISE(ABORT, 'fixture write failure'); END;").unwrap();
    assert!(recompute::d06::run(&mut fx.file, &old, &new, &CancelToken::new()).is_err());
    assert_eq!(fx.file.get_meta("change_seq").unwrap(), before);
    let hidden: i64 = fx
        .file
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM url_flags WHERE flag = 'respect_hidden'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(hidden, 0);
    let indexability: String = fx
        .file
        .conn()
        .query_row(
            "SELECT indexability FROM urls WHERE address LIKE '%/refresh-self.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(indexability, "Indexable");
    let visible: bool = fx
        .file
        .conn()
        .query_row(
            "SELECT in_tables FROM urls WHERE address LIKE '%/body.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(visible);
}
