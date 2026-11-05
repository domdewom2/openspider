//! Verbinden mit einem Klick über die Commands der App (AGENT-043, AGENT-003; Nutzerentscheidung
//! 04.11.2026), gegen die Fixtures `tests/fixtures/agent-client-configs/` in einem temporären Heimordner.
use std::path::{Path, PathBuf};

use os_app::{App, AppConfig, CallCtx};
use serde_json::{json, Value};

const CODEX: &str = include_str!("../../../tests/fixtures/agent-client-configs/codex-config.toml");
const CLAUDE_CODE: &str =
    include_str!("../../../tests/fixtures/agent-client-configs/claude-code.json");
const CLAUDE_DESKTOP: &str =
    include_str!("../../../tests/fixtures/agent-client-configs/claude-desktop.json");
const CURSOR: &str = include_str!("../../../tests/fixtures/agent-client-configs/cursor-mcp.json");

async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}

fn clients(home: &Path) -> [(&'static str, PathBuf, &'static str); 4] {
    [
        ("codex", home.join(".codex").join("config.toml"), CODEX),
        ("claudeCode", home.join(".claude.json"), CLAUDE_CODE),
        (
            "claudeDesktop",
            home.join("Claude").join("claude_desktop_config.json"),
            CLAUDE_DESKTOP,
        ),
        ("cursor", home.join(".cursor").join("mcp.json"), CURSOR),
    ]
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Dateien im Ordner der Konfiguration (Sicherungen eingeschlossen).
fn files_beside(path: &Path) -> usize {
    std::fs::read_dir(path.parent().unwrap()).unwrap().count()
}

fn classes(home: &Path) -> Vec<os_mcp::ToolClass> {
    os_mcp::settings::load(&home.join("mcp.toml"))
        .unwrap()
        .classes
}

/// Gemeinsamer Anfang/Ende: Länge des Anfangs und die beiden abweichenden Mittelstücke.
fn changed<'a>(before: &'a str, after: &'a str) -> (&'a str, &'a str) {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let max = before.len().min(after.len()) - prefix;
    let suffix = before
        .bytes()
        .rev()
        .zip(after.bytes().rev())
        .take(max)
        .take_while(|(a, b)| a == b)
        .count();
    (
        &before[prefix..before.len() - suffix],
        &after[prefix..after.len() - suffix],
    )
}

// spec: AGENT-043 [P1]
// spec: AGENT-003 [M2]
#[tokio::test]
async fn one_click_connect_update_and_disconnect_keep_every_foreign_byte() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    assert_eq!(classes(home.path()), [os_mcp::ToolClass::Read]);
    for (client, path, original) in clients(home.path()) {
        write(&path, original);
        let preview = call(&app, "mcp.clientConfigPreview", json!({ "client": client })).await;
        assert_eq!(preview["state"], "notConnected", "{client}");
        assert!(preview["problem"].is_null());
        assert_eq!(preview["disconnectDiff"], "");
        let text = preview.to_string();
        assert!(!text.contains("TESTSECRET"), "{client}: {text}");
        assert!(preview.get("before").is_none() && preview.get("after").is_none());
        let diff = preview["connectDiff"].as_str().unwrap();
        assert!(diff.contains("+") && diff.contains("openspider"), "{diff}");
        assert!(!diff.contains("github"), "no context lines: {diff}");
        assert!(preview["entry"].as_str().unwrap().contains("openspider"));

        // Ein Klick: Sicherung, Eintrag, Freigabe – ohne vorherige Vorschau.
        let fresh_home = home.path().join(format!("direct-{client}"));
        let direct = App::new(AppConfig::new(&fresh_home)).unwrap();
        let (_, direct_path, _) = clients(&fresh_home)
            .into_iter()
            .find(|(c, _, _)| *c == client)
            .unwrap();
        write(&direct_path, original);
        let written = call(
            &direct,
            "mcp.clientConnect",
            json!({ "client": client, "allowCrawl": true }),
        )
        .await;
        assert_eq!(written["changed"], true);
        assert!(classes(&fresh_home).contains(&os_mcp::ToolClass::Crawl));
        drop(direct);

        let written = call(
            &app,
            "mcp.clientConnect",
            json!({ "client": client, "allowCrawl": true }),
        )
        .await;
        assert_eq!(written["changed"], true);
        let backup = written["backupPath"].as_str().unwrap();
        assert_eq!(std::fs::read_to_string(backup).unwrap(), original);
        let after = std::fs::read_to_string(&path).unwrap();
        let (removed, added) = changed(original, &after);
        assert_eq!(removed, "", "{client}: foreign bytes changed");
        assert!(added.contains("openspider") && !added.contains("TESTSECRET"));

        let preview = call(&app, "mcp.clientConfigPreview", json!({ "client": client })).await;
        assert_eq!(preview["state"], "connected");
        assert_eq!(preview["connectDiff"], "");
        assert_eq!(preview["crawlAllowed"], true);
        assert!(!preview.to_string().contains("TESTSECRET"));

        // Aktualisieren bei gleichem Eintrag: Datei unangetastet, keine weitere Sicherung.
        let count = files_beside(&path);
        let again = call(
            &app,
            "mcp.clientConnect",
            json!({ "client": client, "allowCrawl": true }),
        )
        .await;
        assert_eq!(again, json!({ "backupPath": null, "changed": false }));
        assert_eq!(files_beside(&path), count);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), after);

        // Trennen: genau die Ausgangsdatei, mit Sicherung; die Freigabe bleibt.
        let removed = call(&app, "mcp.clientDisconnect", json!({ "client": client })).await;
        assert_eq!(removed["changed"], true);
        assert_eq!(
            std::fs::read_to_string(removed["backupPath"].as_str().unwrap()).unwrap(),
            after
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
        assert!(classes(home.path()).contains(&os_mcp::ToolClass::Crawl));
        let nothing = call(&app, "mcp.clientDisconnect", json!({ "client": client })).await;
        assert_eq!(nothing, json!({ "backupPath": null, "changed": false }));
    }
}

// spec: AGENT-003 [M2]
// spec: AGENT-043 [P1]
#[tokio::test]
async fn the_crawl_checkbox_sets_only_the_crawl_class() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    call(
        &app,
        "mcp.setClasses",
        json!({ "classes": ["read", "write"] }),
    )
    .await;
    let codex = home.path().join(".codex").join("config.toml");
    call(
        &app,
        "mcp.clientConnect",
        json!({ "client": "codex", "allowCrawl": true }),
    )
    .await;
    use os_mcp::ToolClass::{Crawl, Read, Write};
    assert_eq!(classes(home.path()), [Read, Crawl, Write]);
    // Neue Datei ohne Vorgänger: keine Sicherung.
    assert_eq!(files_beside(&codex), 1);
    // Ein ohne Schalter gestartetes `openspider mcp` übernimmt die gespeicherte Freigabe (AGENT-001).
    let saved = os_mcp::settings::load(&home.path().join("mcp.toml")).unwrap();
    let permissions = os_mcp::permissions::Permissions::resolve(&[], false, &saved).unwrap();
    assert!(permissions.classes().contains(&Crawl));
    assert!(!permissions.classes().contains(&os_mcp::ToolClass::Cost));
    let tools = call(&app, "mcp.tools", Value::Null).await;
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["class"] == "crawl" && t["enabled"] == true));

    call(
        &app,
        "mcp.clientConnect",
        json!({ "client": "codex", "allowCrawl": false }),
    )
    .await;
    assert_eq!(classes(home.path()), [Read, Write]);
    call(&app, "mcp.clientDisconnect", json!({ "client": "codex" })).await;
    assert_eq!(classes(home.path()), [Read, Write]);
    let prefs = call(&app, "settings.get", Value::Null).await;
    assert_eq!(prefs["mcp"]["classes"], json!(["read", "write"]));
}

// spec: AGENT-043 [P1]
#[tokio::test]
async fn invalid_and_outdated_files_are_reported_without_content() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let desktop = home
        .path()
        .join("Claude")
        .join("claude_desktop_config.json");
    let broken = "{\n  // TESTSECRET-90\n  \"mcpServers\": {}\n}\n";
    write(&desktop, broken);
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "claudeDesktop" }),
    )
    .await;
    assert_eq!(preview["problem"], "syntax");
    assert_eq!(preview["line"], 2);
    assert!(!preview.to_string().contains("TESTSECRET"));
    let err = app
        .dispatch(
            CallCtx::for_window("main"),
            "mcp.clientConnect",
            json!({ "client": "claudeDesktop", "allowCrawl": true }),
        )
        .await
        .unwrap_err();
    assert_eq!(err.message_key, "mcp-client-problem-syntax");
    assert_eq!(err.args["line"], 2);
    assert!(!format!("{err:?}").contains("TESTSECRET"));
    assert_eq!(std::fs::read_to_string(&desktop).unwrap(), broken);
    assert_eq!(files_beside(&desktop), 1, "no backup for a refused write");
    // Die Freigabe bleibt bei einem abgelehnten Schreiben unverändert.
    assert_eq!(classes(home.path()), [os_mcp::ToolClass::Read]);

    let codex = home.path().join(".codex").join("config.toml");
    write(&codex, "mcp_servers = { foo = { command = \"foo\" } }\n");
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "codex" }),
    )
    .await;
    assert_eq!(preview["problem"], "unsupported");
    let err = app
        .dispatch(
            CallCtx::for_window("main"),
            "mcp.clientDisconnect",
            json!({ "client": "codex" }),
        )
        .await
        .unwrap_err();
    assert_eq!(err.message_key, "mcp-client-problem-unsupported");

    write(
        &codex,
        "[mcp_servers.openspider]\ncommand = \"/Volumes/alt/openspider\"\nargs = [\"mcp\"]\n",
    );
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "codex" }),
    )
    .await;
    assert_eq!(preview["state"], "outdated");
    assert_eq!(preview["extraKeys"], json!([]));
    let diff = preview["connectDiff"].as_str().unwrap();
    // Der alte Wert gehört zum vorhandenen Eintrag des Nutzers: nur „…“, die neue Zeile vollständig.
    assert!(diff.contains("-\u{2026}\n+command = \""), "{diff}");
    assert!(!diff.contains("/Volumes/alt/openspider"), "{diff}");
    assert!(!diff.contains("-args"), "only changed lines: {diff}");
    let disconnect = preview["disconnectDiff"].as_str().unwrap();
    assert!(disconnect.contains("-[mcp_servers.openspider]"));
    call(
        &app,
        "mcp.clientConnect",
        json!({ "client": "codex", "allowCrawl": false }),
    )
    .await;
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "codex" }),
    )
    .await;
    assert_eq!(preview["state"], "connected");

    // Zusätze des Nutzers im eigenen Eintrag: nur die Namen, nie die Werte (Nachtrag Review).
    write(
        &codex,
        "[mcp_servers.openspider]\ncommand = \"/old\"\nargs = [\"mcp\"]\nstartup_timeout_sec = 30\nenv = { API_KEY = \"TESTSECRET-91\" }\n",
    );
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "codex" }),
    )
    .await;
    assert_eq!(preview["state"], "outdated");
    assert_eq!(preview["extraKeys"], json!(["env", "startup_timeout_sec"]));
    assert!(!preview.to_string().contains("TESTSECRET"));
}

// spec: AGENT-043 [P1]
#[cfg(unix)]
#[tokio::test]
async fn symlinked_client_files_are_never_written() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let real = home.path().join("elsewhere.json");
    std::fs::write(&real, "{}\n").unwrap();
    std::os::unix::fs::symlink(&real, home.path().join(".claude.json")).unwrap();
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "claudeCode" }),
    )
    .await;
    assert_eq!(preview["problem"], "symlink");
    let err = app
        .dispatch(
            CallCtx::for_window("main"),
            "mcp.clientConnect",
            json!({ "client": "claudeCode", "allowCrawl": true }),
        )
        .await
        .unwrap_err();
    assert_eq!(err.message_key, "mcp-client-problem-symlink");
    assert_eq!(std::fs::read_to_string(real).unwrap(), "{}\n");
}

// spec: AGENT-043 [P1]
// spec: AGENT-001
#[tokio::test]
async fn cursor_gets_the_print_config_entry_in_the_global_file() {
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    // Ohne `.cursor`-Ordner: nicht verbunden, Verbinden legt Ordner und Datei an (keine Sicherung).
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "cursor" }),
    )
    .await;
    assert_eq!(preview["state"], "notConnected");
    assert!(preview["problem"].is_null());
    let path = home.path().join(".cursor").join("mcp.json");
    assert_eq!(preview["path"], path.to_string_lossy().as_ref());
    let entry: Value = serde_json::from_str(preview["entry"].as_str().unwrap()).unwrap();
    let written = call(
        &app,
        "mcp.clientConnect",
        json!({ "client": "cursor", "allowCrawl": true }),
    )
    .await;
    assert_eq!(written, json!({ "backupPath": null, "changed": true }));
    let file: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    // „Eintrag kopieren“ (= `--print-config cursor` mit dem Programmpfad der App) und Datei sind wertgleich.
    assert_eq!(file, entry);
    let own = &file["mcpServers"]["openspider"];
    assert_eq!(own["type"], "stdio");
    assert_eq!(own["args"], json!(["mcp"]));
    assert!(Path::new(own["command"].as_str().unwrap()).is_absolute());
    let preview = call(
        &app,
        "mcp.clientConfigPreview",
        json!({ "client": "cursor" }),
    )
    .await;
    assert_eq!(preview["state"], "connected");
}

/// Schreibschutz setzen bzw. aufheben (Windows-Attribut, Unix-Schreibbits).
#[allow(clippy::permissions_set_readonly_false)] // Aufräumen: Windows löscht schreibgeschützte Dateien nicht.
fn set_readonly(path: &Path, value: bool) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(value);
    std::fs::set_permissions(path, permissions).unwrap();
}

/// Verbinden und Trennen einer schreibgeschützten Datei: gesperrt, kein Schreiben, keine Sicherung.
async fn assert_read_only_is_refused(
    app: &App,
    home: &Path,
    client: &str,
    path: &Path,
    text: &str,
) {
    let preview = call(app, "mcp.clientConfigPreview", json!({ "client": client })).await;
    assert_eq!(preview["problem"], "readOnly", "{client}");
    assert!(!preview.to_string().contains("TESTSECRET"));
    // `~/.claude.json` liegt direkt im Heimordner: Dateien vorher zählen statt „genau eine“.
    let count = files_beside(path);
    for (command, args) in [
        (
            "mcp.clientConnect",
            json!({ "client": client, "allowCrawl": true }),
        ),
        ("mcp.clientDisconnect", json!({ "client": client })),
    ] {
        let err = app
            .dispatch(CallCtx::for_window("main"), command, args)
            .await
            .unwrap_err();
        assert_eq!(
            err.message_key, "mcp-client-problem-readOnly",
            "{client} {command}"
        );
        assert!(!format!("{err:?}").contains("TESTSECRET"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), text);
        assert_eq!(files_beside(path), count, "no backup for a refused write");
        assert!(std::fs::metadata(path).unwrap().permissions().readonly());
    }
    // Ein abgelehntes Verbinden ändert die Freigabe nicht.
    assert_eq!(classes(home), [os_mcp::ToolClass::Read]);
}

// spec: AGENT-043 [P1]
#[tokio::test]
async fn read_only_client_files_are_never_written() {
    // Plattformneutral: `set_readonly` setzt unter Windows das Attribut, unter Unix entfernt es die Schreibbits.
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    for (client, path, original) in clients(home.path()) {
        // Mit eigenem Eintrag, damit auch Trennen etwas zu schreiben hätte.
        let connected = os_mcp::client_config::connect(
            os_mcp::print_config::Client::parse(match client {
                "claudeDesktop" => "claude-desktop",
                "claudeCode" => "claude-code",
                other => other,
            })
            .unwrap(),
            original,
            "/x/openspider",
            &["mcp".to_string()],
        )
        .unwrap();
        write(&path, &connected);
        set_readonly(&path, true);
        assert_read_only_is_refused(&app, home.path(), client, &path, &connected).await;
        set_readonly(&path, false);
    }
}

// spec: AGENT-043 [P1]
#[cfg(windows)]
#[tokio::test]
async fn windows_read_only_attribute_is_kept_and_nothing_is_written() {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
    let home = tempfile::tempdir().unwrap();
    let app = App::new(AppConfig::new(home.path())).unwrap();
    let path = home.path().join(".cursor").join("mcp.json");
    write(&path, CURSOR);
    set_readonly(&path, true);
    let attributes = std::fs::metadata(&path).unwrap().file_attributes();
    assert_ne!(attributes & FILE_ATTRIBUTE_READONLY, 0);
    assert_read_only_is_refused(&app, home.path(), "cursor", &path, CURSOR).await;
    assert_eq!(
        std::fs::metadata(&path).unwrap().file_attributes(),
        attributes
    );
    set_readonly(&path, false);
    // Ohne Attribut wird wie gewohnt verbunden; die neue Datei trägt kein Schreibschutz-Attribut.
    let written = call(
        &app,
        "mcp.clientConnect",
        json!({ "client": "cursor", "allowCrawl": false }),
    )
    .await;
    assert_eq!(written["changed"], true);
    assert_eq!(
        std::fs::metadata(&path).unwrap().file_attributes() & FILE_ATTRIBUTE_READONLY,
        0
    );
}
