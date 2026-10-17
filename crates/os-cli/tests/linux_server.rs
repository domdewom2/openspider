//! Linux-Serverbetrieb von Kern und CLI (OUT-273): Crawl mit dem Golden-Vergleich, MCP über STDIO und ein Befehl mit
//! Geheimnis, jeweils ohne Anzeige, ohne D-Bus-Sitzung und ohne Schlüsselbund.
//!
//! Die Tests tragen kein `cfg` und laufen auf allen Plattformen (die Kriterien sind plattformneutral formuliert);
//! den Linux-Nachweis liefert der Job `linux-core` (`.github/workflows/linux-core.yml`). Der Job setzt
//! `OPENSPIDER_BIN` auf die Release-Binärdatei `openspider`, damit der Crawl-Test sie statt des Debug-Binaries prüft.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use common::{column, golden_text, read_csv, site, Env};
use os_testsite::golden::assert_golden;
use serde_json::{json, Value};

/// Obergrenze für Antworten, die sofort kommen müssen (Kriterium „statt zu hängen“).
const FAST: Duration = Duration::from_secs(5);
/// Obergrenze für Abläufe mit Netzwerk und Prozessstart.
const SLOW: Duration = Duration::from_secs(120);

/// Das zu prüfende Binary: `OPENSPIDER_BIN` (Release-Binärdatei aus dem Job `linux-core`), sonst das Test-Binary.
fn binary() -> PathBuf {
    match std::env::var_os("OPENSPIDER_BIN").filter(|v| !v.is_empty()) {
        Some(path) => {
            let path = PathBuf::from(path);
            assert!(
                path.is_file(),
                "OPENSPIDER_BIN zeigt auf keine Datei: {}",
                path.display()
            );
            path
        }
        None => PathBuf::from(env!("CARGO_BIN_EXE_openspider")),
    }
}

/// Serverumgebung: eigener Datenordner, kein D-Bus, keine Anzeige, Speicher-Geheimnisspeicher (kein Schlüsselbund).
fn server_command(home: &Path, work: &Path) -> Command {
    let mut cmd = Command::new(binary());
    cmd.env("OPENSPIDER_HOME", home)
        .env("OPENSPIDER_SECRET_STORE", "memory")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("OPENSPIDER_LOG")
        .env_remove("RUST_LOG")
        .current_dir(work);
    cmd
}

/// Wartet höchstens `limit` auf das Ende des Prozesses; danach wird er beendet. `None` heißt: hängt.
fn wait_for_exit(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
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

// spec: OUT-273
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headless_crawl_of_the_basic_site_matches_the_internal_all_golden() {
    let site = site("out-basic-site").await;
    let base = site.base_url();
    let bases = site.golden_bases();
    let bases: Vec<&str> = bases.iter().map(String::as_str).collect();
    let env = Env::new();
    let out = env.out();

    let mut cmd = server_command(env.home.path(), env.work.path());
    cmd.args([
        "--crawl",
        &base,
        "--set",
        "threads=1",
        "--set",
        "extraction.http_headers=true",
        "--set",
        "robots.retries=0",
        "--export-tabs",
        "Internal:All",
        "--output-folder",
    ])
    .arg(&out)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let run = tokio::task::spawn_blocking(move || {
        let mut child = cmd.spawn().expect("Binary startbar");
        let stdout = spawn_reader(child.stdout.take().unwrap());
        let stderr = spawn_reader(child.stderr.take().unwrap());
        let status = wait_for_exit(&mut child, SLOW);
        let text = |rx: Receiver<String>| rx.try_iter().collect::<Vec<_>>().join("\n");
        (status, text(stdout), text(stderr))
    })
    .await
    .unwrap();
    let (status, stdout, stderr) = run;
    let status = status.unwrap_or_else(|| panic!("Crawl endet nicht\n{stdout}\n{stderr}"));
    assert_eq!(status.code(), Some(0), "{stdout}\n{stderr}");

    let rows = read_csv(&out.join("internal_all.csv"));
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/golden/out-basic-site/internal_all.csv");
    assert_golden(golden, &golden_text(&rows, &bases));
    // Kontrolle unabhängig vom Golden: die Seiten der Fixture sind erfasst.
    assert!(column(&rows, "Address").contains(&format!("{base}/about.html")));
}

// spec: OUT-273
#[test]
fn mcp_stdio_answers_initialize_and_tools_list_without_a_session_bus() {
    let env = Env::new();
    let mut cmd = server_command(env.home.path(), env.work.path());
    cmd.arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("Binary startbar");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = spawn_reader(child.stdout.take().unwrap());
    let _stderr = spawn_reader(child.stderr.take().unwrap());

    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "linux-server-test", "version": "0"}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    ];
    for r in &requests {
        writeln!(stdin, "{r}").unwrap();
    }
    stdin.flush().unwrap();

    // Zwei Antworten (die Benachrichtigung hat keine); stdin bleibt bis dahin offen.
    let mut by_id = std::collections::BTreeMap::new();
    for _ in 0..2 {
        let line = match stdout.recv_timeout(SLOW) {
            Ok(line) => line,
            Err(e) => {
                let _ = child.kill();
                panic!("keine Antwort des MCP-Servers ({e}); bekannt: {by_id:?}");
            }
        };
        let v: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("stdout-Zeile ist kein JSON ({e}): {line}"));
        by_id.insert(v["id"].to_string(), v);
    }
    drop(stdin);
    let status = wait_for_exit(&mut child, SLOW).expect("MCP-Server endet mit geschlossenem stdin");
    assert!(status.success(), "{status:?}");

    assert_eq!(by_id["1"]["result"]["serverInfo"]["name"], "openspider");
    let tools = by_id["2"]["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools/list ohne Werkzeugliste: {}", by_id["2"]));
    assert!(!tools.is_empty(), "der Server meldet keine Werkzeuge");
}

// spec: OUT-273
#[test]
fn a_command_that_needs_a_secret_reports_an_error_within_five_seconds() {
    let env = Env::new();
    // Basic-Auth verweist auf ein Geheimnis, das es in keinem Speicher gibt (CLAUDE.md/C13: Schlüsselbund nur per Kennung).
    let secret_ref = format!("linux-server-test-{}-missing", std::process::id());
    let config = env.work.path().join("secret.osconfig");
    std::fs::write(
        &config,
        format!(
            "schema_version = 1\n\n[[auth.basic]]\nprefix = \"http://127.0.0.1:1/\"\nusername = \"tester\"\nsecret_ref = \"{secret_ref}\"\n"
        ),
    )
    .unwrap();

    // Erster Start des Binaries (Laden, Betriebssystem-Prüfungen) bleibt außerhalb der 5-Sekunden-Messung.
    let mut warmup = server_command(env.home.path(), env.work.path());
    let mut warmup = warmup
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Binary startbar");
    assert!(
        wait_for_exit(&mut warmup, SLOW).is_some_and(|s| s.success()),
        "--version schlägt fehl oder hängt"
    );

    let mut cmd = server_command(env.home.path(), env.work.path());
    cmd.args(["--crawl", "http://127.0.0.1:1/", "--config"])
        .arg(&config)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = cmd.spawn().expect("Binary startbar");
    let _stdout = spawn_reader(child.stdout.take().unwrap());
    let stderr = spawn_reader(child.stderr.take().unwrap());
    let status = wait_for_exit(&mut child, FAST);
    let elapsed = start.elapsed();
    let log = stderr.try_iter().collect::<Vec<_>>().join("\n");

    let status = status.unwrap_or_else(|| panic!("hängt statt zu melden ({elapsed:?}): {log}"));
    assert!(!status.success(), "Fehler erwartet: {log}");
    assert!(
        log.contains("nicht verfügbar") && log.contains(&secret_ref),
        "klare Fehlermeldung zum fehlenden Geheimnis erwartet: {log}"
    );
    assert!(elapsed < FAST, "Fehler erst nach {elapsed:?}");
}
