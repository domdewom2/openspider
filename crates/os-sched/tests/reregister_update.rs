use os_sched::{
    planner::{self, reregister, Backend, PlannerSpec},
    tasks::Schedule,
};
use std::path::{Path, PathBuf};
fn spec(binary: &str) -> PlannerSpec {
    PlannerSpec {
        task_id: "daily-0300".into(),
        task_name: "Täglich <03:00> & mehr".into(),
        schedule: Schedule::Daily {
            time: "03:00".into(),
            from: None,
        },
        binary: PathBuf::from(binary),
        home: None,
        run_if_missed: true,
        on_battery: true,
        wake: false,
    }
}
// spec: OUT-166 [M3]
// spec: OUT-251
#[test]
fn moved_cli_is_repaired_to_golden_and_stable_path_remains_unchanged() {
    let goldens =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/out-m2-tasks/golden");
    for (backend, binary, golden) in [
        (
            Backend::Launchd,
            "/Applications/OpenSpider.app/Contents/MacOS/openspider",
            "daily-0300.plist",
        ),
        (
            Backend::TaskScheduler,
            "C:\\Program Files\\OpenSpider\\openspider.exe",
            "daily-0300.xml",
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let planner = planner::dry_run(backend, dir.path());
        let target = spec(binary);
        assert!(!reregister::repair(planner.as_ref(), &target).unwrap());
        planner.install(&spec("/old/version/openspider")).unwrap();
        assert!(reregister::repair(planner.as_ref(), &target).unwrap());
        assert_eq!(
            planner.registered_binary("daily-0300"),
            Some(PathBuf::from(binary))
        );
        let preview = planner.preview(&target).unwrap();
        let expected = std::fs::read_to_string(goldens.join(golden))
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(preview.files[0].content, expected);
        assert!(!reregister::repair(planner.as_ref(), &target).unwrap());
    }
}
