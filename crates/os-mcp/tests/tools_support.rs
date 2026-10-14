//! Gemeinsame Hilfen der Werkzeugtests (M1-P27); als `#[path]`-Modul eingebunden. Eigenes Testziel ohne Tests.
//!
//! * [`World`] legt einen Crawl-Ordner an, speichert gecrawlte Fixtures darin (`<crawl_id>.ospider`, nach der
//!   Crawl-Analyse wie bei `openspider --crawl`) und startet den Server mit den echten Werkzeugen.
//! * [`Client`] ist ein rmcp-Client über einen In-Memory-Kanal; [`Reply`] zerlegt die Antwortblöcke
//!   (Zusammenfassung, Untrusted-Block, Kürzungshinweis).
//! * [`Table`] liest Exportdateien (CSV), damit MCP-Antworten gegen die Exporte derselben Crawls geprüft
//!   werden können.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use os_analyze::analysis::{run_after_crawl, CancelToken};
use os_analyze::views::ViewQuery;
use os_engine::testing::{crawl_fixture_with, test_deps, FixtureCrawl};
use os_export::{export_tab, Format};
use os_mcp::cli::{build_server, McpArgs};
use os_mcp::server::{McpServer, UNTRUSTED_BEGIN, UNTRUSTED_END, UNTRUSTED_PREAMBLE};
use os_mcp::tools;
use os_model::{CrawlConfig, SeededIds};
use os_store::paths::app_paths_with_home;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};

/// Wurzel der Fixtures im Repository.
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
}

/// Ein gecrawltes und gespeichertes Fixture.
pub struct Stored {
    /// Kennung des Crawls (Dateiname im Crawl-Ordner).
    pub id: String,
    /// Der Crawl samt Testsite und Arbeitsdatei.
    pub fx: FixtureCrawl,
    /// Die Konfiguration aus der Crawl-Datei.
    pub cfg: CrawlConfig,
}

/// Crawl-Ordner mit gespeicherten Crawls.
pub struct World {
    /// Der Crawl-Ordner.
    pub dir: tempfile::TempDir,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> World {
        World {
            dir: tempfile::tempdir().expect("temporärer Crawl-Ordner"),
        }
    }

    /// Crawlt das Szenario, führt wie die CLI die Crawl-Analyse aus und speichert die Datei unter ihrer
    /// Kennung im Crawl-Ordner. `seed` bestimmt die Crawl-Kennung (verschiedene Seeds = verschiedene Crawls).
    pub async fn add(&self, scenario: &str, start: &str, cfg: CrawlConfig, seed: u64) -> Stored {
        self.add_with_state(scenario, start, cfg, seed, None).await
    }

    /// Wie [`World::add`]; `state` überschreibt den gespeicherten Zustand (`"Paused"`) vor dem Speichern.
    pub async fn add_with_state(
        &self,
        scenario: &str,
        start: &str,
        cfg: CrawlConfig,
        seed: u64,
        state: Option<&str>,
    ) -> Stored {
        let mut deps = test_deps();
        deps.ids = Arc::new(SeededIds::new(seed));
        let fx = crawl_fixture_with(scenario, start, cfg, deps).await;
        run_after_crawl(fx.file.path(), &CancelToken::new()).expect("Crawl-Analyse");
        if let Some(state) = state {
            fx.file.set_meta("state", state).unwrap();
            if state == "Paused" {
                fx.file
                    .set_meta("paused_at", "2026-01-01T00:00:10.000Z")
                    .unwrap();
                fx.file.set_meta("ended_at", "").unwrap();
            }
        }
        let id = fx.file.meta().unwrap().crawl_id.to_string();
        fx.file
            .save_as(&self.dir.path().join(format!("{id}.ospider")))
            .expect("Crawl speichern");
        let cfg = fx.file.config().unwrap();
        Stored { id, fx, cfg }
    }

    /// Server mit den echten Werkzeugen (Standardfreigabe: nur `read`) über diesen Crawl-Ordner.
    pub fn server(&self) -> McpServer {
        let home = tempfile::tempdir().unwrap();
        let paths = app_paths_with_home(Some(home.path().as_os_str()));
        let args = McpArgs {
            crawl_dir: Some(self.dir.path().to_path_buf()),
            ..McpArgs::default()
        };
        let server = build_server(&args, &paths, tools::all()).unwrap();
        // Der Einstellungsordner wird nur beim Bau gelesen.
        drop(home);
        server
    }

    pub async fn client(&self) -> Client {
        Client::connect(self.server()).await
    }
}

/// rmcp-Client gegen einen Server im selben Prozess.
pub struct Client {
    inner: RunningService<RoleClient, ()>,
}

impl Client {
    pub async fn connect(server: McpServer) -> Client {
        let (server_io, client_io) = tokio::io::duplex(1 << 22);
        tokio::spawn(async move {
            let running = server.serve(server_io).await.expect("Server-Handshake");
            let _ = running.waiting().await;
        });
        let inner = ().serve(client_io).await.expect("Client-Handshake");
        Client { inner }
    }

    pub fn from_running(inner: RunningService<RoleClient, ()>) -> Client {
        Client { inner }
    }

    pub async fn tool_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .inner
            .list_all_tools()
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        names.sort();
        names
    }

    pub async fn call(&self, name: &'static str, args: Value) -> Reply {
        let mut params = CallToolRequestParams::new(name);
        if let Value::Object(map) = args {
            params = params.with_arguments(map);
        }
        let result = self.inner.call_tool(params).await.expect("tools/call");
        Reply::from_result(&result)
    }

    pub async fn cancel(self) {
        self.inner.cancel().await.unwrap();
    }
}

/// Zerlegte Antwort eines Werkzeugs.
pub struct Reply {
    pub is_error: bool,
    /// Alle Textblöcke in Reihenfolge.
    pub blocks: Vec<String>,
    /// Erster Block als JSON (`{"summary", "total", "next_cursor"}`); bei Fehlern `null`.
    pub envelope: Value,
    /// Inhalt des Untrusted-Blocks als JSON.
    pub untrusted: Option<Value>,
}

impl Reply {
    pub fn from_result(result: &CallToolResult) -> Reply {
        let blocks: Vec<String> = result
            .content
            .iter()
            .map(|c| c.as_text().expect("Textblock").text.clone())
            .collect();
        let is_error = result.is_error == Some(true);
        let envelope = if is_error {
            Value::Null
        } else {
            serde_json::from_str(&blocks[0]).expect("Zusammenfassung ist JSON")
        };
        let untrusted = blocks
            .get(1)
            .filter(|b| b.starts_with(UNTRUSTED_BEGIN))
            .map(|b| {
                let data = b
                    .strip_prefix(&format!("{UNTRUSTED_BEGIN}\n{UNTRUSTED_PREAMBLE}\n"))
                    .and_then(|r| r.strip_suffix(&format!("\n{UNTRUSTED_END}")))
                    .expect("Rahmen des Untrusted-Blocks");
                match serde_json::from_str(data) {
                    Ok(v) => v,
                    // Eine gekürzte Antwort schneidet den Datenblock mitten im JSON ab.
                    Err(_)
                        if blocks
                            .last()
                            .is_some_and(|b| b.starts_with("[Antwort gekürzt")) =>
                    {
                        Value::Null
                    }
                    Err(e) => panic!("Untrusted-Block ist JSON: {e}"),
                }
            });
        Reply {
            is_error,
            blocks,
            envelope,
            untrusted,
        }
    }

    /// Die Zusammenfassung (`summary`).
    pub fn summary(&self) -> &Value {
        assert!(!self.is_error, "Fehler: {:?}", self.blocks);
        &self.envelope["summary"]
    }

    pub fn total(&self) -> Option<u64> {
        self.envelope["total"].as_u64()
    }

    pub fn next_cursor(&self) -> Option<String> {
        self.envelope["next_cursor"].as_str().map(str::to_string)
    }

    /// Meldung eines Fehlers.
    pub fn error_text(&self) -> &str {
        assert!(self.is_error, "kein Fehler: {:?}", self.blocks);
        &self.blocks[0]
    }

    /// Texte außerhalb des Untrusted-Blocks (Zusammenfassung, Hinweise).
    pub fn trusted_text(&self) -> String {
        self.blocks
            .iter()
            .filter(|b| !b.starts_with(UNTRUSTED_BEGIN))
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Der Untrusted-Block als Text (leer, wenn es keinen gibt).
    pub fn untrusted_text(&self) -> String {
        self.blocks
            .iter()
            .find(|b| b.starts_with(UNTRUSTED_BEGIN))
            .cloned()
            .unwrap_or_default()
    }

    /// Zeilen von `query_urls` aus dem Untrusted-Block.
    pub fn rows(&self) -> Vec<Vec<Value>> {
        self.untrusted.as_ref().expect("Untrusted-Block")["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .map(|r| r.as_array().expect("Zeile").clone())
            .collect()
    }
}

/// Gelesene CSV-Datei (BOM verworfen).
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    pub fn read(path: &Path) -> Table {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let text = String::from_utf8(bytes).expect("UTF-8");
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let mut all = parse_csv(text);
        let header = all.remove(0);
        Table { header, rows: all }
    }

    pub fn col(&self, name: &str) -> usize {
        self.header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("Spalte {name:?} fehlt in {:?}", self.header))
    }

    pub fn column(&self, name: &str) -> Vec<String> {
        let c = self.col(name);
        self.rows.iter().map(|r| r[c].clone()).collect()
    }
}

/// RFC-4180-Parser (Anführungszeichen, doppelte Anführungszeichen, Zeilenumbrüche in Feldern).
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => in_quotes = false,
                other => field.push(other),
            }
            continue;
        }
        match c {
            '"' => in_quotes = true,
            ',' => row.push(std::mem::take(&mut field)),
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Exportiert eine Ansicht wie `--export-tabs` (CSV) und liest sie ein.
pub fn export_view(s: &Stored, q: &ViewQuery) -> Table {
    let tmp = tempfile::tempdir().unwrap();
    let info = export_tab(&s.fx.file, &s.cfg, q, tmp.path(), Format::Csv).expect("Tab-Export");
    Table::read(&info.path)
}

/// Eine Zelle der MCP-Antwort in der Textform des CSV-Exports (bis auf Dezimalstellen).
pub fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Stimmen MCP-Zelle und CSV-Zelle überein? Zahlen vergleichen numerisch (Export rundet auf feste Stellen).
pub fn cells_equal(mcp: &Value, csv: &str) -> bool {
    match mcp {
        Value::Number(n) if n.is_f64() => csv
            .parse::<f64>()
            .is_ok_and(|c| (c - n.as_f64().unwrap_or(f64::NAN)).abs() < 1e-6),
        other => cell_text(other) == csv,
    }
}

/// Alle Seiten von `query_urls`, bis `next_cursor` leer ist: Zeilen, Spaltennamen und Gesamtzahl.
pub async fn query_all(
    client: &Client,
    base_args: Value,
    limit: u64,
) -> (Vec<Vec<Value>>, Vec<String>, u64) {
    let mut rows = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..10_000 {
        let mut args = base_args.clone();
        args["limit"] = json!(limit);
        if let Some(c) = &cursor {
            args["cursor"] = json!(c);
        }
        let reply = client.call("query_urls", args).await;
        assert!(!reply.is_error, "{:?}", reply.blocks);
        rows.extend(reply.rows());
        cursor = reply.next_cursor();
        if cursor.is_none() {
            let columns = reply.summary()["columns"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| c.as_str().unwrap().to_string())
                .collect();
            return (rows, columns, reply.total().unwrap());
        }
    }
    panic!("query_urls hört nicht auf zu blättern");
}

/// Basiskonfiguration der Tests.
pub fn base_config() -> CrawlConfig {
    CrawlConfig::default()
}
