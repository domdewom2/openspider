//! Einzelinstanz und Dateizuordnung (OUT-232, OUT-272): Dateien aus Argumenten und URLs werden erkannt und als Meldung
//! `open-file` an die Oberfläche gegeben. Das Verhalten der Betriebssysteme (Doppelklick, zweiter Start) bleibt ein
//! manueller Test je OS (Spec); hier ist alles Prüfbare geprüft.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use openspider_desktop_lib::open::{
    forward, notice_for, requests_from_args, requests_from_urls, OpenKind, OpenRequest,
};
use os_app::dto::UiEvent;
use os_app::events::CollectingSink;
use os_app::{App, AppConfig, WindowId};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

// spec: OUT-232
#[test]
fn files_with_the_five_extensions_are_recognised_in_the_arguments() {
    let cwd = if cfg!(windows) {
        Path::new("C:\\arbeit")
    } else {
        Path::new("/arbeit")
    };
    let abs = if cfg!(windows) {
        "C:\\daten\\a.ospider"
    } else {
        "/daten/a.ospider"
    };
    let reqs = requests_from_args(
        &args(&[
            "openspider",
            abs,
            "relativ/b.OSCONFIG",
            "c.osauth",
            "--flag",
            "d.ospreset",
            "e.ostask",
            "bild.png",
            "ohne-endung",
        ]),
        Some(cwd),
    );
    let kinds: Vec<_> = reqs.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        vec![
            OpenKind::Crawl,
            OpenKind::Config,
            OpenKind::Auth,
            OpenKind::Preset,
            OpenKind::Task
        ]
    );
    assert_eq!(reqs[0].path, PathBuf::from(abs));
    // relative Pfade löst das Arbeitsverzeichnis der aufrufenden Instanz auf
    assert_eq!(reqs[1].path, cwd.join("relativ/b.OSCONFIG"));
    assert_eq!(reqs[2].path, cwd.join("c.osauth"));
}

// spec: OUT-232
#[test]
fn the_program_name_and_options_are_not_files() {
    assert!(
        requests_from_args(&args(&["/usr/bin/x.ospider"]), None).is_empty(),
        "args[0] ist das Programm"
    );
    assert!(requests_from_args(&args(&["x", "-a.ospider"]), None).is_empty());
    assert!(requests_from_args(&[], None).is_empty());
}

// spec: OUT-272
#[test]
fn file_urls_from_the_operating_system_are_recognised() {
    let ok = tauri::Url::parse(if cfg!(windows) {
        "file:///C:/daten/x.ospider"
    } else {
        "file:///daten/x.ospider"
    })
    .unwrap();
    let other_scheme = tauri::Url::parse("https://example.com/x.ospider").unwrap();
    let other_ext = tauri::Url::parse("file:///daten/x.txt").unwrap();
    let reqs = requests_from_urls(&[ok, other_scheme, other_ext]);
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].kind, OpenKind::Crawl);
    assert!(reqs[0].path.ends_with("x.ospider"));
}

// spec: OUT-272
#[test]
fn extension_mapping_is_case_insensitive_and_exact() {
    for (ext, kind) in [
        ("ospider", OpenKind::Crawl),
        ("OSPIDER", OpenKind::Crawl),
        ("osconfig", OpenKind::Config),
        ("osauth", OpenKind::Auth),
        ("ospreset", OpenKind::Preset),
        ("ostask", OpenKind::Task),
    ] {
        assert_eq!(OpenKind::from_extension(ext), Some(kind), "{ext}");
    }
    for ext in ["ospiderx", "spider", "", "db", "sqlite"] {
        assert_eq!(OpenKind::from_extension(ext), None, "{ext}");
    }
    assert_eq!(OpenKind::Crawl.as_str(), "crawl");
}

fn new_app() -> (App, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "os-desktop-open-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    (App::new(AppConfig::new(&dir)).unwrap(), dir)
}

// spec: OUT-232
#[tokio::test]
async fn forwarding_hands_each_file_to_the_running_instance_as_a_notice_in_the_next_tick() {
    let (app, dir) = new_app();
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), sink.clone());
    let reqs = vec![
        OpenRequest {
            kind: OpenKind::Crawl,
            path: PathBuf::from("/daten/a.ospider"),
        },
        OpenRequest {
            kind: OpenKind::Config,
            path: PathBuf::from("/daten/b.osconfig"),
        },
    ];
    assert_eq!(forward(&app, &reqs), 2);
    let events = sink.take();
    assert_eq!(events.len(), 1, "ein Tick für beide Dateien");
    let UiEvent::Tick { tick } = &events[0] else {
        panic!("{events:?}")
    };
    let codes: Vec<_> = tick
        .notices
        .iter()
        .map(|n| {
            (
                n.code.as_str(),
                n.args["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        codes,
        vec![
            ("open-file", "crawl".to_string()),
            ("open-file", "config".to_string())
        ]
    );
    assert_eq!(tick.notices[0].args["path"], "/daten/a.ospider");
    assert_eq!(forward(&app, &[]), 0);
    assert!(sink.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

// spec: OUT-232
#[tokio::test]
async fn a_file_that_starts_the_app_is_not_lost_before_the_window_subscribes() {
    let (app, dir) = new_app();
    let reqs = vec![OpenRequest {
        kind: OpenKind::Crawl,
        path: PathBuf::from("/daten/start.ospider"),
    }];
    forward(&app, &reqs);
    // K2-63: Wartende Startmeldungen werden seit fix-int15 schon beim Abonnieren
    // als Tick zugestellt. Entscheidend ist die genau einmalige Zustellung ans Fenster.
    let sink = Arc::new(CollectingSink::new());
    app.subscribe(WindowId("main".into()), sink.clone());
    let events = sink.take();
    assert_eq!(events.len(), 1);
    let UiEvent::Tick { tick } = &events[0] else {
        panic!()
    };
    assert_eq!(tick.notices, vec![notice_for(&reqs[0])]);
    assert_eq!(app.flush_tick(), 0);
    assert!(sink.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

// spec: OUT-272
#[test]
#[ignore = "P2: Dock-/Taskleistenfortschritt während eines Crawls (OUT-272, zweites Kriterium) ist P2"]
fn the_dock_icon_shows_the_crawl_progress_and_removes_it_at_the_end() {}
