#[path = "session_common.rs"]
mod common;
use common::*;
use os_app::{
    dto::{Notice, NoticeLevel, UiEvent},
    events::{coalescer::Coalescer, CollectingSink, TickParts, TickSource},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
#[derive(Default)]
struct Changes(Mutex<Option<u64>>);
impl TickSource for Changes {
    fn collect(&self, p: &mut TickParts) {
        p.counts_version = self.0.lock().unwrap().take();
    }
}

// spec: UI-143
// spec: ONPAGE-193
// spec: UI-018 [M2]
#[tokio::test]
async fn sixty_seconds_at_200_urls_per_second_obeys_every_window_budget() {
    let trace: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixture().join("events.json")).unwrap()).unwrap();
    let url_interval = trace["urlIntervalMs"].as_u64().unwrap();
    let urls = trace["durationMs"].as_u64().unwrap() / url_interval;
    for case in trace["cases"].as_array().unwrap() {
        let interval = case["intervalMs"].as_u64().unwrap() as u32;
        let hidden = case["hidden"].as_bool().unwrap();
        let max = case["maxTicks"].as_u64().unwrap() as usize;
        let dir = tempfile::tempdir().unwrap();
        let (app, clock) = manual_app(dir.path());
        app.set_live_interval_ms(interval).unwrap();
        app.set_visibility(window(), !hidden);
        app.set_tick_policy(Arc::new(Coalescer::default()));
        let sink = Arc::new(CollectingSink::new());
        app.subscribe(window(), sink.clone());
        let changes = Arc::new(Changes::default());
        app.add_tick_source(changes.clone());
        app.flush_tick();
        for n in 1..=urls {
            clock.advance(Duration::from_millis(url_interval));
            *changes.0.lock().unwrap() = Some(n);
            app.flush_tick();
        }
        let events = sink.take();
        assert!(events.len() <= max, "{} > {max}", events.len());
        assert!(events.len() >= max - 1);
        let last = match events.last().unwrap() {
            UiEvent::Tick { tick } => tick,
            _ => panic!(),
        };
        assert_eq!(last.counts_version, urls);
        for _ in 0..100 {
            clock.advance(Duration::from_secs(1));
            assert_eq!(app.flush_tick(), 0);
        }
        assert!(sink.is_empty());
    }
}
// spec: UI-143
#[tokio::test]
async fn hidden_window_retains_notices_and_receives_latest_version() {
    let dir = tempfile::tempdir().unwrap();
    let (app, clock) = manual_app(dir.path());
    let hidden = os_app::WindowId("hidden".into());
    app.set_visibility(hidden.clone(), false);
    let a = Arc::new(CollectingSink::new());
    let b = Arc::new(CollectingSink::new());
    app.subscribe(window(), a.clone());
    app.subscribe(hidden, b.clone());
    app.set_live_interval_ms(250).unwrap();
    app.set_tick_policy(Arc::new(Coalescer::default()));
    let changes = Arc::new(Changes::default());
    app.add_tick_source(changes.clone());
    app.flush_tick();
    for n in 1..=4 {
        app.push_notice(Notice {
            level: NoticeLevel::Info,
            code: format!("fixture-{n}"),
            args: Default::default(),
        });
        *changes.0.lock().unwrap() = Some(n);
        clock.advance(Duration::from_millis(250));
        app.flush_tick();
    }
    assert_eq!(a.len(), 4);
    assert_eq!(b.len(), 1);
    let ev = b.take().pop().unwrap();
    let UiEvent::Tick { tick } = ev else { panic!() };
    assert_eq!(tick.notices.len(), 4);
    assert_eq!(tick.counts_version, 4);
}
