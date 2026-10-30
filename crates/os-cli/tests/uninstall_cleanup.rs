use os_sched::planner::{CommandOutput, CommandRunner};
use std::sync::Mutex;

struct FixtureRunner {
    calls: Mutex<Vec<Vec<String>>>,
    fail_delete: bool,
    fail_query: bool,
}
impl CommandRunner for FixtureRunner {
    fn run(&self, program: &str, args: &[String]) -> std::io::Result<CommandOutput> {
        assert_eq!(program, "schtasks");
        self.calls.lock().unwrap().push(args.to_vec());
        let query = args[0] == "/Query";
        Ok(CommandOutput {
            status: i32::from(if query {
                self.fail_query
            } else {
                self.fail_delete
            }),
            stdout: if query {
                concat!(
                    "\"\\OpenSpider\\daily\",\"N/A\",\"Ready\"\r\n",
                    "\"\\Other\\keep\",\"N/A\",\"Ready\"\r\n",
                    "\"\\OpenSpiderOther\\keep\",\"N/A\",\"Ready\"\r\n",
                    "\"\\OpenSpider\\with\"\"quote\",\"N/A\",\"Ready\"\r\n"
                )
                .into()
            } else {
                String::new()
            },
            stderr: String::new(),
        })
    }
}
fn fixture(fail_delete: bool, fail_query: bool) -> FixtureRunner {
    FixtureRunner {
        calls: Mutex::new(Vec::new()),
        fail_delete,
        fail_query,
    }
}

// spec: OUT-166 [M3]
#[test]
fn cleanup_removes_only_os_namespace_even_without_local_xml_and_preserves_data() {
    let runner = fixture(false, false);
    let data = tempfile::tempdir().unwrap();
    let crawl = data.path().join("keep.ospider");
    std::fs::write(&crawl, b"user crawl fixture").unwrap();
    os_cli::uninstall::cleanup_windows_with(&runner).unwrap();
    let calls = runner.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1], ["/Delete", "/TN", "\\OpenSpider\\daily", "/F"]);
    assert_eq!(
        calls[2],
        ["/Delete", "/TN", "\\OpenSpider\\with\"quote", "/F"]
    );
    assert_eq!(std::fs::read(crawl).unwrap(), b"user crawl fixture");
}

// spec: OUT-166 [M3]
// spec: OUT-266
#[test]
fn installer_receives_failure_when_task_cleanup_fails() {
    assert!(os_cli::uninstall::cleanup_windows_with(&fixture(true, false)).is_err());
    assert!(os_cli::uninstall::cleanup_windows_with(&fixture(false, true)).is_err());
}

// spec: OUT-272 [M3]
#[test]
fn installer_configuration_retains_file_associations_and_start_menu() {
    let common: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/src-tauri/tauri.conf.json"
    ))
    .unwrap();
    let windows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/src-tauri/tauri.windows.conf.json"
    ))
    .unwrap();
    let associations = common["bundle"]["fileAssociations"].as_array().unwrap();
    assert!(associations.iter().any(|a| a["ext"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e == "ospider")));
    assert_eq!(
        windows["bundle"]["windows"]["nsis"]["startMenuFolder"],
        "OpenSpider"
    );
    assert_eq!(
        windows["bundle"]["windows"]["nsis"]["installMode"],
        "currentUser"
    );
    assert_eq!(common["bundle"]["externalBin"][0], "binaries/openspider");
}
