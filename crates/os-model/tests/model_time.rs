//! Zeit, Uhren und Kennungsquellen (Regel 2.8 des M1-Plans; CRAWL-128).

use std::time::Duration;

use os_model::time::{
    Clock, FixedClock, IdSource, ManualClock, RandomIds, SeededIds, SystemClock, Timestamp,
};
use os_model::{CrawlId, ModelError};
use time::UtcOffset;

fn ts(s: &str) -> Timestamp {
    Timestamp::from_db(s).unwrap()
}

// spec: OUT-002
#[test]
fn timestamp_db_round_trip_is_utc_with_millis() {
    let t = ts("2026-10-13T07:35:00.123Z");
    assert_eq!(t.to_db(), "2026-10-13T07:35:00.123Z");
    // Andere Offsets werden nach UTC umgerechnet.
    assert_eq!(
        ts("2026-10-13T09:35:00+02:00").to_db(),
        "2026-10-13T07:35:00.000Z"
    );
    // Sekundenbruchteile unter einer Millisekunde werden abgeschnitten.
    assert_eq!(
        ts("2026-10-13T07:35:00.123987Z").to_db(),
        "2026-10-13T07:35:00.123Z"
    );
    assert!(matches!(
        Timestamp::from_db("gestern"),
        Err(ModelError::InvalidTimestamp(_))
    ));
}

// spec: OUT-002
#[test]
fn timestamp_export_uses_offset_without_fraction() {
    let t = ts("2026-10-13T07:35:00.123Z");
    let plus2 = UtcOffset::from_hms(2, 0, 0).unwrap();
    assert_eq!(t.to_export_with(plus2), "2026-10-13T09:35:00+02:00");
    assert_eq!(
        t.to_export_with(UtcOffset::UTC),
        "2026-10-13T07:35:00+00:00"
    );
    let minus = UtcOffset::from_hms(-5, -30, 0).unwrap();
    assert_eq!(t.to_export_with(minus), "2026-10-13T02:05:00-05:30");
    // to_export() hängt vom lokalen Offset ab, hat aber immer die feste Form.
    os_model::time::init_local_offset();
    os_model::time::init_local_offset();
    let e = t.to_export();
    assert_eq!(e.len(), 25, "{e}");
}

#[test]
fn timestamp_serde_is_db_string() {
    let t = ts("2026-10-13T07:35:00.123Z");
    let json = serde_json::to_string(&t).unwrap();
    assert_eq!(json, "\"2026-10-13T07:35:00.123Z\"");
    assert_eq!(serde_json::from_str::<Timestamp>(&json).unwrap(), t);
    assert_eq!(Timestamp::from_unix_millis(t.unix_millis()), t);
}

// spec: CRAWL-130
#[test]
fn manual_clock_stands_still_until_advanced() {
    let start = ts("2026-10-13T00:00:00.000Z");
    let c = ManualClock::new(start);
    assert_eq!(c.now(), start);
    assert_eq!(c.now(), start);
    assert_eq!(c.monotonic(), Duration::ZERO);
    c.advance(Duration::from_secs(20));
    c.advance(Duration::from_millis(500));
    assert_eq!(c.monotonic(), Duration::from_millis(20_500));
    assert_eq!(c.now().to_db(), "2026-10-13T00:00:20.500Z");
}

#[test]
fn fixed_clock_steps_on_every_query() {
    let start = ts("2026-10-13T00:00:00.000Z");
    let c = FixedClock::new(start, Duration::from_millis(250));
    assert_eq!(c.now().to_db(), "2026-10-13T00:00:00.000Z");
    assert_eq!(c.now().to_db(), "2026-10-13T00:00:00.250Z");
    assert_eq!(c.monotonic(), Duration::from_millis(500));
    assert_eq!(c.now().to_db(), "2026-10-13T00:00:00.750Z");
    // Zwei Uhren mit gleichen Parametern liefern dieselbe Folge.
    let a = FixedClock::new(start, Duration::from_secs(1));
    let b = FixedClock::new(start, Duration::from_secs(1));
    assert_eq!(a.now(), b.now());
    assert_eq!(a.now(), b.now());
}

#[test]
fn system_clock_is_monotonic() {
    let c = SystemClock;
    let a = c.monotonic();
    let b = c.monotonic();
    assert!(b >= a);
    assert!(c.now().unix_millis() > 1_700_000_000_000);
}

// spec: CRAWL-128
#[test]
fn seeded_ids_are_deterministic_uuid_v7() {
    let now = ts("2026-10-13T07:35:00.123Z");
    let a = SeededIds::new(42);
    let b = SeededIds::new(42);
    let (a1, a2) = (a.new_crawl_id(now), a.new_crawl_id(now));
    let (b1, b2) = (b.new_crawl_id(now), b.new_crawl_id(now));
    assert_eq!((a1, a2), (b1, b2));
    assert_ne!(a1, a2);
    assert_ne!(SeededIds::new(43).new_crawl_id(now), a1);
    assert_eq!(a1.0.get_version_num(), 7);
    // Zeitanteil = Unix-Millisekunden der übergebenen Zeit.
    let (secs, _) = a1.0.get_timestamp().unwrap().to_unix();
    assert_eq!(secs as i64, now.0.unix_timestamp());
}

// spec: CRAWL-128
#[test]
fn random_ids_are_unique_v7_and_display_lowercase_hyphenated() {
    let now = Timestamp::now_utc();
    let ids = RandomIds;
    let (x, y) = (ids.new_crawl_id(now), ids.new_crawl_id(now));
    assert_ne!(x, y);
    assert_eq!(x.0.get_version_num(), 7);
    let s = x.to_string();
    assert_eq!(s.len(), 36);
    assert_eq!(s, s.to_lowercase());
    assert_eq!(s.parse::<CrawlId>().unwrap(), x);
    assert!("kein-uuid".parse::<CrawlId>().is_err());
}
