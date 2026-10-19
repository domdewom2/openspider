static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[path = "view_session_fixture.rs"]
mod fixture;
use os_analyze::views::session::{release_unused, Budget, ViewSession};
use os_model::CrawlConfig;

// spec: UI-142
#[test]
fn shared_budget_releases_least_recently_used_view_and_close_drops_ownership() {
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture::synthetic(1000);
    let cfg = CrawlConfig::default();
    let first = ViewSession::open(
        &file,
        &cfg,
        fixture::specification("Address", false),
        Budget::default(),
    )
    .unwrap();
    first.prepare(&file, &Default::default()).unwrap();
    assert!(first.memory_bytes() > 0);
    let second = ViewSession::open(
        &file,
        &cfg,
        fixture::specification("Address", false),
        Budget::default(),
    )
    .unwrap();
    second.prepare(&file, &Default::default()).unwrap();
    let second_bytes = second.memory_bytes();
    release_unused(second_bytes);
    assert_eq!(first.memory_bytes(), 0);
    assert_eq!(second.memory_bytes(), second_bytes);
    let pin = second.snapshot(&file).unwrap();
    second.close();
    assert_eq!(pin.locate(os_model::UrlId(1)), Some(0));
    drop(pin);
    release_unused(0);
    // A released session still has a stable generation and can reconstruct its derived data.
    assert_eq!(first.prepare(&file, &Default::default()).unwrap(), 1000);
    assert_eq!(first.version(), 1);
}

// spec: UI-142
#[test]
fn idle_expiration_uses_injected_monotonic_clock() {
    use os_model::time::{ManualClock, Timestamp};
    use std::{sync::Arc, time::Duration};
    let _serial = SERIAL.lock().unwrap();
    let (_dir, file) = fixture::synthetic(100);
    let clock = Arc::new(ManualClock::new(Timestamp::UNIX_EPOCH));
    let view = ViewSession::open_with_clock(
        &file,
        &CrawlConfig::default(),
        fixture::specification("Address", false),
        Budget::default(),
        clock.clone(),
    )
    .unwrap();
    view.prepare(&file, &Default::default()).unwrap();
    let bytes = view.memory_bytes();
    assert!(bytes > 0);
    clock.advance(Duration::from_secs(59));
    release_unused(u64::MAX);
    assert_eq!(view.memory_bytes(), bytes);
    clock.advance(Duration::from_secs(1));
    release_unused(u64::MAX);
    assert_eq!(view.memory_bytes(), 0);
    assert_eq!(view.prepare(&file, &Default::default()).unwrap(), 100);
    assert_eq!(view.version(), 1);
}
