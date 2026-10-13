//! Indexierbarkeits-Stub (nur Statuscode-Regeln; Vollausbau M1-P24).

use os_model::*;

fn eq(a: &str, b: &str) -> bool {
    a == b
}

fn facts(status: u16) -> FetchFacts {
    FetchFacts {
        status_code: status,
        ..FetchFacts::default()
    }
}

fn eval(f: &FetchFacts) -> (Indexability, Option<IndexabilityStatus>) {
    let cfg = CrawlConfig::default();
    evaluate(&IndexabilityInput {
        fetch: f,
        page: None,
        page_url: "https://a.test/",
        refresh: &[],
        cfg: &cfg,
        same_url: eq,
    })
}

// spec: TECH-001
#[test]
fn status_code_rules() {
    use Indexability::*;
    use IndexabilityStatus::*;
    assert_eq!(eval(&facts(200)), (Indexable, None));
    assert_eq!(eval(&facts(204)), (Indexable, None));
    assert_eq!(eval(&facts(0)), (NonIndexable, Some(NoResponse)));
    assert_eq!(eval(&facts(301)), (NonIndexable, Some(Redirected)));
    assert_eq!(eval(&facts(399)), (NonIndexable, Some(Redirected)));
    assert_eq!(eval(&facts(404)), (NonIndexable, Some(ClientError)));
    assert_eq!(eval(&facts(429)), (NonIndexable, Some(ClientError)));
    assert_eq!(eval(&facts(500)), (NonIndexable, Some(ServerError)));
    assert_eq!(eval(&facts(503)), (NonIndexable, Some(ServerError)));
}

// spec: TECH-001
#[test]
fn robots_block_wins_over_no_response() {
    let mut f = facts(0);
    f.robots_blocked = true;
    assert_eq!(
        eval(&f),
        (
            Indexability::NonIndexable,
            Some(IndexabilityStatus::BlockedByRobots)
        )
    );
    let mut g = facts(0);
    g.no_response_reason = Some(NoResponseReason::BlockedByRobotsUnreachable);
    assert_eq!(
        eval(&g),
        (
            Indexability::NonIndexable,
            Some(IndexabilityStatus::BlockedByRobots)
        )
    );
}

// spec: CRAWL-130
#[test]
fn progress_json_field_names_are_fixed() {
    let p = Progress {
        state: CrawlState::Running,
        end_reason: None,
        crawled: 12,
        queued: 30,
        percent: 28,
        rate: 7.5,
        avg_response_time: Some(0.12),
        elapsed_s: 20,
        eta_s: None,
        active_threads: 5,
        started_at: Timestamp::from_db("2026-10-13T07:35:00.000Z").unwrap(),
        avg_rate: 6.0,
        paused_s: 0,
        discovered: 42,
    };
    let v = serde_json::to_value(&p).unwrap();
    for key in [
        "crawled",
        "queued",
        "percent",
        "rate",
        "avg_response_time",
        "elapsed_s",
        "eta_s",
        "state",
    ] {
        assert!(v.get(key).is_some(), "Feld {key} fehlt");
    }
    assert_eq!(v["state"], "Running");
    assert_eq!(v["started_at"], "2026-10-13T07:35:00.000Z");
    let back: Progress = serde_json::from_value(v).unwrap();
    assert_eq!(back, p);
}

#[test]
fn value_serializes_as_plain_json() {
    let row = vec![
        Value::Null,
        Value::Bool(true),
        Value::Int(3),
        Value::Float(1.5),
        Value::Text("a".into()),
    ];
    assert_eq!(
        serde_json::to_string(&row).unwrap(),
        "[null,true,3,1.5,\"a\"]"
    );
    assert_eq!(Value::from(None::<i64>), Value::Null);
    assert_eq!(Value::from(Some("x")), Value::Text("x".into()));
}

#[test]
fn write_batch_is_empty_by_default() {
    let b = WriteBatch::default();
    assert!(b.new_urls.is_empty() && b.pages.is_empty() && b.queue.is_empty());
    assert_eq!(UrlId(7).to_string(), "7");
}
