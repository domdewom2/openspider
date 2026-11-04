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

async fn call(app: &App, name: &str, args: Value) -> Value {
    app.dispatch(CallCtx::for_window("main"), name, args)
        .await
        .unwrap()
}

fn clients(home: &Path) -> [(&'static str, PathBuf, &'static str); 3] {
    [
        ("codex", home.join(".codex").join("config.toml"), CODEX),
        ("claudeCode", home.join(".claude.json"), CLAUDE_CODE),
        (
            "claudeDesktop",
            home.join("Claude").join("claude_desktop_config.json"),
            CLAUDE_DESKTOP,
        ),
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
    let diff = preview["connectDiff"].as_str().unwrap();
    assert!(
        diff.contains("-command = \"/Volumes/alt/openspider\""),
        "{diff}"
    );
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
