//! OS-Planer (OUT-166): plist und Task-XML gegen Golden-Dateien, Registrierung mit Fake-Befehlsausführung.
//! Es wird nie ein echter Systemdienst angesprochen.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use os_sched::planner::{
    self, launchd, windows, Backend, CommandOutput, CommandRunner, OsPlanner, PlannerError,
    PlannerSpec,
};
use os_sched::tasks::Schedule;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/out-m2-tasks/golden")
}

/// Vergleicht mit der Golden-Datei; `OPENSPIDER_UPDATE_GOLDEN=1` schreibt sie neu.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_dir().join(name);
    if std::env::var_os("OPENSPIDER_UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .replace("\r\n", "\n");
    assert_eq!(actual, expected, "Abweichung von {name}");
}

fn spec(id: &str, schedule: Schedule) -> PlannerSpec {
    PlannerSpec {
        task_id: id.into(),
        task_name: "Täglich <03:00> & mehr".into(),
        schedule,
        // Plattformunabhängig: Goldens enthalten feste Pfade.
        binary: PathBuf::from("/Applications/OpenSpider.app/Contents/MacOS/openspider"),
        home: None,
        run_if_missed: true,
        on_battery: true,
        wake: false,
    }
}

fn daily_0300() -> PlannerSpec {
    // Golden-Dateien sind plattformunabhängig: Programmpfade als feste Texte.
    let mut s = spec(
        "daily-0300",
        Schedule::Daily {
            time: "03:00".into(),
            from: None,
        },
    );
    s.binary = PathBuf::from("/Applications/OpenSpider.app/Contents/MacOS/openspider");
    s
}

fn daily_0300_windows() -> PlannerSpec {
    let mut s = daily_0300();
    s.binary = PathBuf::from("C:\\Program Files\\OpenSpider\\openspider.exe");
    s
}

#[derive(Default)]
struct Recorder {
    calls: Mutex<Vec<Vec<String>>>,
    fail: bool,
}

impl CommandRunner for Recorder {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<CommandOutput> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().cloned());
        self.calls.lock().unwrap().push(call);
        Ok(CommandOutput {
            status: i32::from(self.fail),
            stdout: String::new(),
            stderr: if self.fail {
                "denied".into()
            } else {
                String::new()
            },
        })
    }
}

// spec: OUT-166
#[test]
fn daily_0300_plist_matches_the_golden_file() {
    let plist = launchd::plist(&daily_0300()).unwrap();
    assert_golden("daily-0300.plist", &plist);
    assert!(plist.contains("<string>--run-task</string>"));
    assert!(plist.contains("StartCalendarInterval"));
    assert!(!plist.contains("RunAtLoad"));
}

// spec: OUT-166
#[test]
fn daily_0300_task_xml_matches_the_golden_file_and_never_limits_runtime_or_battery() {
    let xml = windows::task_xml(&daily_0300_windows()).unwrap();
    assert_golden("daily-0300.xml", &xml);
    for needle in [
        "<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>",
        "<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>",
        "<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>",
        "<StartWhenAvailable>true</StartWhenAvailable>",
        "<LogonType>InteractiveToken</LogonType>",
        "<Arguments>--run-task daily-0300</Arguments>",
        "<URI>\\OpenSpider\\daily-0300</URI>",
    ] {
        assert!(xml.contains(needle), "{needle} fehlt");
    }
    // Sonderzeichen im Namen sind maskiert.
    assert!(xml.contains("Täglich &lt;03:00&gt; &amp; mehr"));
}

// spec: OUT-166 [P2]
#[test]
fn wake_and_catch_up_options_change_the_task_xml_fields() {
    let mut s = daily_0300_windows();
    s.wake = true;
    s.run_if_missed = false;
    s.on_battery = false;
    let xml = windows::task_xml(&s).unwrap();
    assert!(xml.contains("<WakeToRun>true</WakeToRun>"));
    assert!(xml.contains("<StartWhenAvailable>false</StartWhenAvailable>"));
    assert!(xml.contains("<DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>"));
    assert_golden("daily-0300-wake.xml", &xml);
}

// spec: OUT-166
#[test]
fn weekly_and_monthly_schedules_map_to_calendar_entries() {
    let weekly = launchd::plist(&spec(
        "w",
        Schedule::Weekly {
            days: vec![7, 1, 3],
            time: "06:15".into(),
            from: None,
        },
    ))
    .unwrap();
    // Sonntag (7) wird zu launchd-Wochentag 0, Reihenfolge aufsteigend nach Eingabe 1, 3, 7.
    let weekdays: Vec<&str> = weekly.lines().filter(|l| l.contains("<integer>")).collect();
    assert_eq!(weekdays.len(), 9);
    assert_golden("weekly-mwu-0615.plist", &weekly);

    let month_31 = launchd::plist(&spec(
        "m",
        Schedule::Monthly {
            day: 31,
            time: "01:00".into(),
            from: None,
        },
    ))
    .unwrap();
    assert!(month_31.contains("<integer>30</integer>"), "April/Juni: 30");
    assert!(month_31.contains("<integer>28</integer>"), "Februar: 28");
    assert_eq!(month_31.matches("<key>Month</key>").count(), 12);
    assert_golden("monthly-31-0100.plist", &month_31);

    let month_xml = windows::task_xml(&spec(
        "m",
        Schedule::Monthly {
            day: 31,
            time: "01:00".into(),
            from: None,
        },
    ))
    .unwrap();
    // Monate mit 31 Tagen: Tag 31; die übrigen: letzter Tag.
    assert!(month_xml.contains("<Day>31</Day>"));
    assert!(month_xml.contains("<Day>Last</Day>"));
    assert_golden("monthly-31-0100.xml", &month_xml);

    let weekly_xml = windows::task_xml(&spec(
        "w",
        Schedule::Weekly {
            days: vec![1, 5],
            time: "06:15".into(),
            from: None,
        },
    ))
    .unwrap();
    assert!(weekly_xml.contains("<DaysOfWeek><Monday/><Friday/></DaysOfWeek>"));
}

// spec: OUT-166
#[test]
fn once_hourly_and_unsupported_schedules() {
    let once = launchd::plist(&spec(
        "o",
        Schedule::Once {
            at: "2026-12-24T18:30:00+01:00".into(),
        },
    ))
    .unwrap();
    for needle in [
        "<integer>12</integer>",
        "<integer>24</integer>",
        "<integer>18</integer>",
        "<integer>30</integer>",
    ] {
        assert!(once.contains(needle), "{needle}");
    }
    let hourly = launchd::plist(&spec(
        "h",
        Schedule::Hourly {
            start: "2026-10-20T08:15".into(),
        },
    ))
    .unwrap();
    assert!(hourly.contains("<key>Minute</key>") && !hourly.contains("<key>Hour</key>"));
    let every3 = spec(
        "e",
        Schedule::EveryDays {
            every: 3,
            start: "2026-10-20T08:15".into(),
        },
    );
    assert!(matches!(
        launchd::plist(&every3),
        Err(PlannerError::Unsupported(_))
    ));
    assert!(windows::task_xml(&every3)
        .unwrap()
        .contains("<DaysInterval>3</DaysInterval>"));
    let bad = spec(
        "b",
        Schedule::Daily {
            time: "xx".into(),
            from: None,
        },
    );
    assert!(matches!(
        launchd::plist(&bad),
        Err(PlannerError::Invalid(_))
    ));
}

// spec: OUT-166
#[test]
fn launchd_install_writes_the_agent_file_loads_it_and_uninstall_removes_everything() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(Recorder::default());
    let planner = launchd::Launchd::new(dir.path().to_path_buf(), runner.clone());
    assert!(!planner.is_registered("daily-0300"));
    let preview = planner.preview(&daily_0300()).unwrap();
    assert!(
        !dir.path()
            .join("de.openspider.task.daily-0300.plist")
            .exists(),
        "Vorschau schreibt nichts"
    );
    assert!(runner.calls.lock().unwrap().is_empty());
    assert_eq!(preview.backend, Backend::Launchd);
    assert_eq!(
        preview.files[0].path,
        dir.path().join("de.openspider.task.daily-0300.plist")
    );
    assert_eq!(preview.command[..3], ["launchctl", "load", "-w"]);

    planner.install(&daily_0300()).unwrap();
    assert!(planner.is_registered("daily-0300"));
    assert_eq!(planner.registered_ids(), ["daily-0300"]);
    assert_eq!(
        planner.registered_binary("daily-0300").unwrap(),
        PathBuf::from("/Applications/OpenSpider.app/Contents/MacOS/openspider")
    );
    // Neuinstallation entlädt zuerst.
    planner.install(&daily_0300()).unwrap();
    let calls = runner.calls.lock().unwrap().clone();
    let verbs: Vec<&str> = calls.iter().map(|c| c[1].as_str()).collect();
    assert_eq!(verbs, ["load", "unload", "load"]);

    planner.uninstall("daily-0300").unwrap();
    assert!(!planner.is_registered("daily-0300"));
    planner.uninstall("daily-0300").unwrap();
}

// spec: OUT-166
#[test]
fn a_failing_system_command_is_reported_as_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(Recorder {
        fail: true,
        ..Recorder::default()
    });
    let planner = launchd::Launchd::new(dir.path().to_path_buf(), runner);
    match planner.install(&daily_0300()) {
        Err(PlannerError::Command {
            program,
            status,
            stderr,
        }) => {
            assert_eq!(program, "launchctl");
            assert_eq!(status, 1);
            assert_eq!(stderr, "denied");
        }
        other => panic!("{other:?}"),
    }
}

// spec: OUT-166
#[test]
fn task_scheduler_install_writes_utf16_xml_and_calls_schtasks() {
    let dir = tempfile::tempdir().unwrap();
    let runner = Arc::new(Recorder::default());
    let planner = windows::TaskScheduler::new(dir.path().to_path_buf(), runner.clone());
    planner.install(&daily_0300_windows()).unwrap();
    let bytes = std::fs::read(dir.path().join("daily-0300.xml")).unwrap();
    assert_eq!(&bytes[..2], &[0xFF, 0xFE], "UTF-16LE mit Byte-Order-Mark");
    assert!(planner.is_registered("daily-0300"));
    assert_eq!(
        planner.registered_binary("daily-0300").unwrap(),
        PathBuf::from("C:\\Program Files\\OpenSpider\\openspider.exe")
    );
    let calls = runner.calls.lock().unwrap().clone();
    assert_eq!(
        calls[0][..4],
        ["schtasks", "/Create", "/TN", "\\OpenSpider\\daily-0300"]
    );
    planner.uninstall("daily-0300").unwrap();
    assert!(!planner.is_registered("daily-0300"));
    assert!(runner
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|c| c[1] == "/Delete"));
}

// spec: OUT-166
#[test]
fn the_dry_run_planner_writes_files_into_its_folder_and_runs_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let planner = planner::dry_run(Backend::Launchd, dir.path());
    planner.install(&daily_0300()).unwrap();
    assert!(planner.is_registered("daily-0300"));
    assert!(dir
        .path()
        .join("de.openspider.task.daily-0300.plist")
        .is_file());
    let windows = planner::dry_run(Backend::TaskScheduler, dir.path());
    windows.install(&daily_0300_windows()).unwrap();
    assert!(windows.is_registered("daily-0300"));
    assert_eq!(
        planner::native_backend() == Backend::TaskScheduler,
        cfg!(windows)
    );
}

// spec: OUT-166
#[test]
fn an_updated_binary_path_is_visible_for_reregistration() {
    let dir = tempfile::tempdir().unwrap();
    let planner = planner::dry_run(Backend::Launchd, dir.path());
    planner.install(&daily_0300()).unwrap();
    let mut moved = daily_0300();
    moved.binary = PathBuf::from("/Applications/OpenSpider 2.app/Contents/MacOS/openspider");
    assert_ne!(
        planner.registered_binary("daily-0300").unwrap(),
        moved.binary
    );
    planner.install(&moved).unwrap();
    assert_eq!(
        planner.registered_binary("daily-0300").unwrap(),
        moved.binary
    );
}

// spec: OUT-166
#[test]
fn a_home_override_is_passed_to_the_scheduled_process() {
    let mut s = daily_0300();
    s.home = Some(PathBuf::from("/tmp/os-home"));
    let plist = launchd::plist(&s).unwrap();
    assert!(plist.contains("<key>OPENSPIDER_HOME</key>"));
    assert!(plist.contains("<string>/tmp/os-home</string>"));
}
