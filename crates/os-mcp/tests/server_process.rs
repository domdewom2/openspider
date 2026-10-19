//! Prozesstests des Starters `openspider-mcp` (M1-P18): stdout enthält nur JSON-RPC, Logs auf stderr,
//! Exit-Codes, `--print-config` mit absolutem Pfad, rmcp-Client über Kindprozess.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use rmcp::model::CallToolRequestParams;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::ServiceExt;
use serde_json::{json, Value};

const BIN: &str = env!("CARGO_BIN_EXE_openspider-mcp");
const WAIT: Duration = Duration::from_secs(30);

fn command(home: &Path) -> Command {
    let mut cmd = Command::new(BIN);
    // Die Einstellungen des Entwicklers dürfen die Tests nie beeinflussen.
    cmd.env("OPENSPIDER_HOME", home)
        .env_remove("OPENSPIDER_LOG")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn spawn_reader(stream: impl std::io::Read + Send + 'static) -> Receiver<String> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stream).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    rx
}

fn wait_exit(child: &mut Child) -> std::process::ExitStatus {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if start.elapsed() > WAIT {
            let _ = child.kill();
            panic!("Prozess endet nicht");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn drain(rx: &Receiver<String>) -> Vec<String> {
    // Der Leser-Thread endet mit dem Prozess; danach ist der Kanal geschlossen.
    rx.iter().collect()
}

/// Startet mit geschlossener stdin, wartet auf das Ende und liefert (Exit-Code, stdout, stderr).
fn run_to_end(args: &[&str], home: &Path) -> (Option<i32>, String, String) {
    let mut child = command(home).args(args).spawn().unwrap();
    drop(child.stdin.take());
    let out = spawn_reader(child.stdout.take().unwrap());
    let err = spawn_reader(child.stderr.take().unwrap());
    let status = wait_exit(&mut child);
    (
        status.code(),
        drain(&out).join("\n"),
        drain(&err).join("\n"),
    )
}

// spec: AGENT-001
#[test]
fn stdout_carries_only_json_rpc_and_logs_go_to_stderr() {
    let home = tempfile::tempdir().unwrap();
    let mut child = command(home.path()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = spawn_reader(child.stdout.take().unwrap());
    let stderr = spawn_reader(child.stderr.take().unwrap());

    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "prozesstest", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "prompts/list"}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
               "params": {"name": "gibt_es_nicht", "arguments": {}}}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "ping"}),
    ];
    for r in &requests {
        writeln!(stdin, "{r}").unwrap();
    }
    stdin.flush().unwrap();

    // Sechs Antworten (die Benachrichtigung hat keine); stdin bleibt bis dahin offen.
    let mut responses = Vec::new();
    for _ in 0..6 {
        let line = stdout.recv_timeout(WAIT).expect("Antwort auf stdout");
        responses.push(line);
    }
    drop(stdin);
    let status = wait_exit(&mut child);
    assert!(status.success(), "{status:?}");
    // Nach den Antworten darf nichts Weiteres auf stdout stehen.
    let rest = drain(&stdout);
    responses.extend(rest);

    let mut by_id = std::collections::BTreeMap::new();
    for line in &responses {
        let v: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("stdout-Zeile ist kein JSON ({e}): {line}"));
        assert_eq!(v["jsonrpc"], "2.0", "keine JSON-RPC-Nachricht: {line}");
        by_id.insert(v["id"].to_string(), v);
    }
    assert_eq!(by_id.len(), 6, "{responses:?}");
    assert_eq!(by_id["1"]["result"]["serverInfo"]["name"], "openspider");
    assert!(by_id["2"]["result"]["tools"].is_array());
    let resources = by_id["3"]["result"]["resources"].as_array().unwrap();
    assert_eq!(
        resources
            .iter()
            .map(|r| r["uri"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["openspider://schema", "openspider://tools"]
    );
    assert_eq!(by_id["4"]["result"]["prompts"].as_array().unwrap().len(), 6);
    assert_eq!(by_id["5"]["result"]["isError"], true);

    let log = drain(&stderr).join("\n");
    assert!(
        log.contains("gestartet"),
        "Startmeldung gehört auf stderr: {log:?}"
    );
    assert!(log.contains("read"), "{log:?}");
}

// spec: AGENT-001
#[test]
fn read_only_with_another_class_exits_with_two_and_names_both_switches() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, err) = run_to_end(&["--read-only", "--allow", "crawl"], home.path());
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty(), "stdout bleibt leer: {out:?}");
    assert!(
        err.contains("--read-only") && err.contains("--allow"),
        "{err}"
    );

    let (code, _, err) = run_to_end(&["--read-only", "--allow", "read,write"], home.path());
    assert_eq!(code, Some(2), "{err}");
}

// spec: AGENT-001
#[test]
fn read_only_with_allow_read_starts_normally() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, err) = run_to_end(&["--read-only", "--allow", "read"], home.path());
    assert_eq!(code, Some(0), "{err}");
    assert!(out.is_empty(), "{out:?}");
    assert!(err.contains("gestartet"), "{err}");
}

// spec: AGENT-001
#[test]
fn unknown_allow_class_exits_with_two() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, err) = run_to_end(&["--allow", "admin"], home.path());
    assert_eq!(code, Some(2), "{err}");
    assert!(out.is_empty());
    assert!(err.contains("admin") && err.contains("crawl"), "{err}");
}

// spec: AGENT-001
#[test]
fn settings_file_classes_show_up_in_the_start_log_and_read_only_overrides_them() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join("mcp.toml"),
        "classes = [\"read\", \"write\"]\n",
    )
    .unwrap();
    let (code, _, err) = run_to_end(&[], home.path());
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("classes=[\"read\", \"write\"]"),
        "Klasse aus mcp.toml: {err}"
    );
    let (code, _, err) = run_to_end(&["--read-only"], home.path());
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("classes=[\"read\"]"),
        "--read-only überstimmt die Datei: {err}"
    );
}

// spec: AGENT-001
#[test]
fn unreadable_settings_file_is_a_runtime_error_with_exit_one() {
    let home = tempfile::tempdir().unwrap();
    std::fs::write(home.path().join("mcp.toml"), "classes = \"read\" ===").unwrap();
    let (code, out, err) = run_to_end(&[], home.path());
    assert_eq!(code, Some(1), "{err}");
    assert!(out.is_empty());
    assert!(err.contains("mcp.toml"), "{err}");
}

// spec: AGENT-001
#[test]
fn print_config_prints_valid_config_with_the_absolute_binary_path() {
    let home = tempfile::tempdir().unwrap();
    for client in ["claude-desktop", "claude-code", "cursor"] {
        let (code, out, err) = run_to_end(&["--print-config", client], home.path());
        assert_eq!(code, Some(0), "{client}: {err}");
        let v: Value =
            serde_json::from_str(&out).unwrap_or_else(|e| panic!("{client}: {e}: {out}"));
        let server = &v["mcpServers"]["openspider"];
        let command = server["command"].as_str().unwrap();
        assert!(Path::new(command).is_absolute(), "{client}: {command}");
        assert!(Path::new(command).exists(), "{client}: {command}");
        assert!(command.contains("openspider-mcp"), "{command}");
        // Der schlanke Starter hat keinen Unterbefehl `mcp`.
        assert_eq!(server["args"], json!([]), "{client}");
        assert!(!err.is_empty(), "Hinweis zum Einfügen auf stderr");
    }
    let (code, out, _) = run_to_end(&["--print-config", "codex"], home.path());
    assert_eq!(code, Some(0));
    let t: toml::Table = toml::from_str(&out).unwrap();
    let command = t["mcp_servers"]["openspider"]["command"].as_str().unwrap();
    assert!(Path::new(command).is_absolute(), "{command}");
}

// spec: AGENT-001
#[test]
fn print_config_passes_the_given_switches_on() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, _) = run_to_end(
        &[
            "--print-config",
            "cursor",
            "--allow",
            "crawl",
            "--crawl-dir",
            "/data/crawls",
        ],
        home.path(),
    );
    assert_eq!(code, Some(0));
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["mcpServers"]["openspider"]["args"],
        json!(["--crawl-dir", "/data/crawls", "--allow", "crawl"])
    );
    // Auch --print-config prüft Widersprüche.
    let (code, out, _) = run_to_end(
        &[
            "--print-config",
            "cursor",
            "--read-only",
            "--allow",
            "write",
        ],
        home.path(),
    );
    assert_eq!(code, Some(2));
    assert!(out.is_empty());
}

// spec: AGENT-001
#[test]
fn print_config_rejects_unknown_clients() {
    let home = tempfile::tempdir().unwrap();
    let (code, out, _) = run_to_end(&["--print-config", "vscode"], home.path());
    assert_eq!(code, Some(2));
    assert!(out.is_empty());
}

// spec: AGENT-001
#[tokio::test]
async fn rmcp_client_talks_to_the_binary_over_stdio() {
    let home = tempfile::tempdir().unwrap();
    let transport = TokioChildProcess::new(tokio::process::Command::new(BIN).configure(|cmd| {
        cmd.env("OPENSPIDER_HOME", home.path())
            .env_remove("OPENSPIDER_LOG");
    }))
    .unwrap();
    let client = ().serve(transport).await.expect("initialize");
    let info = client.peer_info().expect("Server-Info");
    assert_eq!(
        info.server_info.as_ref().map(|i| i.name.as_str()),
        Some("openspider")
    );
    let tools = client.list_all_tools().await.unwrap();
    // Im Auslieferungszustand nur `read`-Werkzeuge; welche, bestimmt M1-P27.
    assert!(tools
        .iter()
        .all(|t| { t.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true) }));
    assert_eq!(client.list_all_resources().await.unwrap().len(), 2);
    assert_eq!(client.list_all_prompts().await.unwrap().len(), 6);
    let result = client
        .call_tool(CallToolRequestParams::new("gibt_es_nicht"))
        .await
        .unwrap();
    assert_eq!(result.is_error, Some(true));
    client.cancel().await.unwrap();
}
