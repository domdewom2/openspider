//! Ereignisgerüst (M2-F03): ein Kanal je Fenster, Sichtbarkeit aus zwei Quellen, Taktregeln, Tick-Zusammenbau ohne
//! Nachricht im Leerlauf, `settingsChanged`, Meldungen.

#[path = "runtime_common.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::*;
use os_app::dto::{Notice, NoticeLevel, UiEvent, ViewChanged};
use os_app::events::{effective_period, CollectingSink, TickParts, TickSource};
use os_app::{ErrorCode, WindowId};
use serde_json::json;

fn win(s: &str) -> WindowId {
    WindowId(s.to_string())
}

fn tick_of(ev: UiEvent) -> os_app::dto::Tick {
    match ev {
        UiEvent::Tick { tick } => *tick,
        other => panic!("kein Tick: {other:?}"),
    }
}

// spec: UI-018
#[test]
fn the_period_is_the_interval_never_below_100_ms_and_at_least_one_second_when_hidden() {
    assert_eq!(effective_period(500, true), Duration::from_millis(500));
    assert_eq!(effective_period(250, true), Duration::from_millis(250));
    assert_eq!(effective_period(5000, true), Duration::from_millis(5000));
    // nie unter 100 ms (≤ 10 Nachrichten/s)
    assert_eq!(effective_period(10, true), Duration::from_millis(100));
    assert_eq!(effective_period(0, true), Duration::from_millis(100));
    // verdecktes Fenster: höchstens 1 Hz
    assert_eq!(effective_period(250, false), Duration::from_millis(1000));
    assert_eq!(effective_period(500, false), Duration::from_millis(1000));
    assert_eq!(effective_period(5000, false), Duration::from_millis(5000));
}

// spec: UI-143
#[tokio::test]
async fn a_window_is_visible_only_when_both_sources_say_so() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let w = win("main");
    // ohne Meldung gilt ein Fenster als sichtbar
    assert!(app.is_visible(&w));
    assert_eq!(app.window_period(&w), Duration::from_millis(500));

    // Quelle 2: Fensterereignisse der Hülle
    app.set_visibility(w.clone(), false);
    assert!(!app.is_visible(&w));
    assert_eq!(app.window_period(&w), Duration::from_millis(1000));

    // Quelle 1 meldet „sichtbar“ – reicht allein nicht
    app.dispatch(
        ctx(),
        "events.setVisibility",
        json!({ "window": "main", "visible": true }),
    )
    .await
    .unwrap();
    assert!(!app.is_visible(&w));

    app.set_visibility(w.clone(), true);
    assert!(app.is_visible(&w));

    // Quelle 1 meldet „verdeckt“ (visibilitychange)
    app.dispatch(
        ctx(),
        "events.setVisibility",
        json!({ "window": "main", "visible": false }),
    )
    .await
    .unwrap();
    assert!(!app.is_visible(&w));
    // andere Fenster bleiben unberührt
    assert!(app.is_visible(&win("settings")));
}

// spec: UI-018
#[tokio::test]
async fn set_interval_accepts_250_to_5000_ms_and_drives_the_window_period() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    for ms in [250u32, 1000, 5000] {
        app.dispatch(ctx(), "events.setInterval", json!({ "ms": ms }))
            .await
            .unwrap();
        assert_eq!(app.live_interval_ms(), ms);
        assert_eq!(
            app.window_period(&win("main")),
            Duration::from_millis(u64::from(ms))
        );
    }
    for ms in [0u32, 249, 5001] {
        let e = app
            .dispatch(ctx(), "events.setInterval", json!({ "ms": ms }))
            .await
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert_eq!(e.field.as_deref(), Some("ms"));
    }
    assert_eq!(app.live_interval_ms(), 5000);
}

// spec: UI-018
#[tokio::test]
async fn there_is_one_channel_per_window_and_a_new_subscription_replaces_the_old_one() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let (a1, a2, b) = (
        Arc::new(CollectingSink::new()),
        Arc::new(CollectingSink::new()),
        Arc::new(CollectingSink::new()),
    );
    let s1 = app.subscribe(win("main"), a1.clone());
    app.subscribe(win("settings"), b.clone());
    let s2 = app.subscribe(win("main"), a2.clone());
    assert_ne!(s1.id, s2.id);
    assert_eq!(app.subscribed_windows(), vec![win("main"), win("settings")]);

    let notice = UiEvent::Notice {
        notice: Notice {
            level: NoticeLevel::Info,
            code: "x".into(),
            args: Default::default(),
        },
    };
    assert_eq!(app.broadcast(notice.clone()), 2);
    assert!(a1.is_empty(), "das ersetzte Abonnement bekommt nichts mehr");
    assert_eq!((a2.len(), b.len()), (1, 1));

    assert!(app.send_event(&win("settings"), notice.clone()));
    assert_eq!(b.len(), 2);
    assert!(!app.send_event(&win("viz-1"), notice));

    // beenden
    assert!(app.unsubscribe(&s2));
    assert!(!app.unsubscribe(&s2));
    assert_eq!(app.subscribed_windows(), vec![win("settings")]);
}

// spec: UI-018
#[tokio::test]
async fn a_closed_sink_ends_the_subscription() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(win("viz-1"), sink.clone());
    sink.close();
    let ev = UiEvent::SettingsChanged { keys: vec![] };
    assert_eq!(app.broadcast(ev), 0);
    assert!(app.subscribed_windows().is_empty());
}

struct Once(std::sync::Mutex<Option<TickParts>>);
impl TickSource for Once {
    fn collect(&self, parts: &mut TickParts) {
        if let Some(p) = self.0.lock().unwrap().take() {
            *parts = p;
        }
    }
}

// spec: UI-143
#[tokio::test]
async fn without_changes_there_is_no_tick_and_sequence_numbers_only_advance_with_ticks() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(win("main"), sink.clone());
    for _ in 0..5 {
        assert!(app.assemble_tick().is_none());
        assert_eq!(app.flush_tick(), 0);
    }
    assert!(sink.is_empty());

    let parts = TickParts {
        views: vec![ViewChanged {
            view_id: "v1".into(),
            version: 3,
            total: Some(10),
        }],
        counts_version: Some(4),
        issues_version: Some(2),
        ..TickParts::default()
    };
    app.add_tick_source(Arc::new(Once(std::sync::Mutex::new(Some(parts)))));
    assert_eq!(app.flush_tick(), 1);
    let t = tick_of(sink.take().remove(0));
    assert_eq!(t.seq, 1);
    assert_eq!(t.views.len(), 1);
    assert_eq!(
        (t.counts_version, t.issues_version, t.sidebar_version),
        (4, 2, 0)
    );
    assert!(
        t.at.ends_with('Z') && t.at.len() == 24,
        "ISO 8601 UTC mit Millisekunden: {}",
        t.at
    );

    // Leerlauf: kein weiterer Tick, die Nummer bleibt
    assert_eq!(app.flush_tick(), 0);
    assert!(sink.is_empty());
}

// spec: UI-143
#[tokio::test]
async fn versions_persist_across_ticks_and_parts_from_all_sources_are_merged() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    let first = TickParts {
        counts_version: Some(7),
        ..TickParts::default()
    };
    app.add_tick_source(Arc::new(Once(std::sync::Mutex::new(Some(first)))));
    let t1 = app.assemble_tick().unwrap();
    assert_eq!(t1.counts_version, 7);

    app.push_notice(Notice {
        level: NoticeLevel::Warning,
        code: "disk-low".into(),
        args: Default::default(),
    });
    let t2 = app.assemble_tick().unwrap();
    // die Versionsstände bleiben erhalten, die Meldung erscheint genau einmal
    assert_eq!((t2.seq, t2.counts_version), (2, 7));
    assert_eq!(t2.notices.len(), 1);
    assert!(app.assemble_tick().is_none());
}

// spec: UI-143
#[tokio::test]
async fn notices_wait_for_a_window_instead_of_getting_lost() {
    let dir = tempfile::tempdir().unwrap();
    let app = new_app(dir.path());
    app.push_notice(Notice {
        level: NoticeLevel::Info,
        code: "open-file".into(),
        args: Default::default(),
    });
    // niemand hört zu: nichts wird verbraucht
    assert_eq!(app.flush_tick(), 0);
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(win("main"), sink.clone());
    assert_eq!(app.flush_tick(), 1);
    let t = tick_of(sink.take().remove(0));
    assert_eq!(t.notices[0].code, "open-file");
}
