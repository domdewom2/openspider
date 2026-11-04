//! Formatbewahrendes Schreiben und Maskieren gegen realistische Fixtures (AGENT-043).
use super::*;

const CODEX: &str =
    include_str!("../../../../tests/fixtures/agent-client-configs/codex-config.toml");
const CLAUDE_CODE: &str =
    include_str!("../../../../tests/fixtures/agent-client-configs/claude-code.json");
const CLAUDE_DESKTOP: &str =
    include_str!("../../../../tests/fixtures/agent-client-configs/claude-desktop.json");

const MAC: &str = "/Applications/OpenSpider.app/Contents/MacOS/openspider";
const WIN: &str = "C:\\Program Files\\OpenSpider\\openspider.exe";

fn mcp() -> Vec<String> {
    vec!["mcp".to_string()]
}

/// Gemeinsamer Anfang und gemeinsames Ende zweier Texte; liefert die abweichenden Mittelstücke.
fn changed<'a>(before: &'a str, after: &'a str) -> (usize, &'a str, &'a str) {
    let prefix = before
        .bytes()
        .zip(after.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let max_suffix = before.len().min(after.len()) - prefix;
    let suffix = before
        .bytes()
        .rev()
        .zip(after.bytes().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();
    (
        prefix,
        &before[prefix..before.len() - suffix],
        &after[prefix..after.len() - suffix],
    )
}

fn assert_no_secrets(text: &str) {
    assert!(!text.contains("TESTSECRET"), "secret visible:\n{text}");
}

// --- Codex (TOML) ----------------------------------------------------------------------------------

// spec: AGENT-043
#[test]
fn codex_connect_appends_only_the_own_table_and_keeps_every_byte() {
    for exe in [MAC, WIN] {
        let after = connect(Client::Codex, CODEX, exe, &mcp()).unwrap();
        assert_eq!(
            &after[..CODEX.len()],
            CODEX,
            "prefix must be byte-identical"
        );
        let escaped = exe.replace('\\', "\\\\");
        assert_eq!(
            &after[CODEX.len()..],
            format!("\n[mcp_servers.openspider]\ncommand = \"{escaped}\"\nargs = [\"mcp\"]\n")
        );
        assert_eq!(
            entry_state(Client::Codex, &after, exe, &mcp()).unwrap(),
            EntryState::Current
        );
        // Ein zweites Verbinden ändert nichts.
        assert_eq!(connect(Client::Codex, &after, exe, &mcp()).unwrap(), after);
        // Trennen ergibt die Ausgangsdatei byte-gleich.
        assert_eq!(disconnect(Client::Codex, &after).unwrap(), CODEX);
    }
    assert_eq!(
        entry_state(Client::Codex, CODEX, MAC, &mcp()).unwrap(),
        EntryState::Absent
    );
}

// spec: AGENT-043
#[test]
fn codex_update_replaces_an_outdated_table_in_place() {
    let old = "[mcp_servers.openspider]\ncommand = \"/Volumes/alt/openspider\" # alte Beta\nargs = [\n  \"mcp\",\n]\n\n[mcp_servers.openspider.env]\nRUST_LOG = \"debug\"\n\n";
    let before = CODEX.replace(
        "[mcp_servers.github]\n",
        &format!("{old}[mcp_servers.github]\n"),
    );
    assert_eq!(
        entry_state(Client::Codex, &before, MAC, &mcp()).unwrap(),
        EntryState::Outdated
    );
    let after = connect(Client::Codex, &before, MAC, &mcp()).unwrap();
    let k = CODEX.find("[mcp_servers.github]\n").unwrap();
    let block = format!("[mcp_servers.openspider]\ncommand = \"{MAC}\"\nargs = [\"mcp\"]\n");
    // Ersetzt wird genau der alte Eintrag samt Untertabelle; die Leerzeile davor und danach bleibt.
    assert_eq!(after, format!("{}{block}\n{}", &CODEX[..k], &CODEX[k..]));
    let (_, removed, added) = changed(&before, &after);
    assert!(removed.contains("Volumes/alt/openspider") && removed.contains("RUST_LOG"));
    assert!(
        added.contains(&MAC[1..]) && !added.contains("github"),
        "{added}"
    );
    assert_eq!(
        entry_state(Client::Codex, &after, MAC, &mcp()).unwrap(),
        EntryState::Current
    );
    // Trennen entfernt genau die eigene Tabelle (mitten in der Datei bleibt ihre Leerzeile stehen).
    assert_eq!(
        disconnect(Client::Codex, &after).unwrap(),
        format!("{}\n{}", &CODEX[..k], &CODEX[k..])
    );
}

// spec: AGENT-043
#[test]
fn codex_inline_own_entry_is_replaced_and_removed_in_place() {
    let before = "[mcp_servers]\nfoo = { command = \"foo\" }\nopenspider = { command = \"/old\", args = [] } # eigener Eintrag\nbar = { command = \"bar\" }\n";
    assert_eq!(
        entry_state(Client::Codex, before, MAC, &mcp()).unwrap(),
        EntryState::Outdated
    );
    let after = connect(Client::Codex, before, MAC, &mcp()).unwrap();
    assert_eq!(
        after,
        format!("[mcp_servers]\nfoo = {{ command = \"foo\" }}\nopenspider = {{ command = \"{MAC}\", args = [\"mcp\"] }} # eigener Eintrag\nbar = {{ command = \"bar\" }}\n")
    );
    assert_eq!(
        disconnect(Client::Codex, &after).unwrap(),
        "[mcp_servers]\nfoo = { command = \"foo\" }\nbar = { command = \"bar\" }\n"
    );
}

// spec: AGENT-043
#[test]
fn codex_line_endings_and_missing_final_newline_are_kept() {
    let crlf = CODEX.replace('\n', "\r\n");
    let after = connect(Client::Codex, &crlf, MAC, &mcp()).unwrap();
    assert!(after.starts_with(&crlf));
    assert_eq!(
        &after[crlf.len()..],
        format!("\r\n[mcp_servers.openspider]\r\ncommand = \"{MAC}\"\r\nargs = [\"mcp\"]\r\n")
    );
    assert_eq!(disconnect(Client::Codex, &after).unwrap(), crlf);

    let bare = CODEX.trim_end();
    let after = connect(Client::Codex, bare, MAC, &mcp()).unwrap();
    assert!(after.starts_with(bare));
    assert!(after[bare.len()..].starts_with("\n\n[mcp_servers.openspider]\n"));

    let empty = connect(Client::Codex, "", MAC, &mcp()).unwrap();
    assert_eq!(
        empty,
        format!("[mcp_servers.openspider]\ncommand = \"{MAC}\"\nargs = [\"mcp\"]\n")
    );
    assert_eq!(disconnect(Client::Codex, &empty).unwrap(), "");
    assert_eq!(disconnect(Client::Codex, CODEX).unwrap(), CODEX);
}

// spec: AGENT-043
#[test]
fn codex_files_that_cannot_be_edited_in_place_are_rejected() {
    assert_eq!(
        connect(Client::Codex, "model = \"x\"\n[broken\n", MAC, &mcp()),
        Err(ConfigError::Syntax { line: 2 })
    );
    assert_eq!(
        connect(
            Client::Codex,
            "mcp_servers = { foo = { command = \"foo\" } }\n",
            MAC,
            &mcp()
        ),
        Err(ConfigError::Unsupported(Unsupported::InlineServers))
    );
    assert_eq!(
        connect(Client::Codex, "mcp_servers = 1\n", MAC, &mcp()),
        Err(ConfigError::Unsupported(Unsupported::ServersNotObject))
    );
    assert_eq!(
        connect(
            Client::Codex,
            "[mcp_servers]\nopenspider.command = \"x\"\n",
            MAC,
            &mcp()
        ),
        Err(ConfigError::Unsupported(Unsupported::OwnEntryShape))
    );
    // Untertabelle des eigenen Eintrags hinter fremden Tabellen: kein zusammenhängender Ausschnitt.
    let split = "[mcp_servers.openspider]\ncommand = \"x\"\n[other]\na = 1\n[mcp_servers.openspider.env]\nA = \"b\"\n";
    assert_eq!(
        connect(Client::Codex, split, MAC, &mcp()),
        Err(ConfigError::Unsupported(Unsupported::OwnEntryShape))
    );
    assert_eq!(
        disconnect(Client::Codex, split),
        Err(ConfigError::Unsupported(Unsupported::OwnEntryShape))
    );
    // Der Fehlertext enthält nie den Inhalt der Datei.
    let err = connect(Client::Codex, "token = \"TESTSECRET-99\n", MAC, &mcp()).unwrap_err();
    assert!(matches!(err, ConfigError::Syntax { line: 1 }));
    assert!(!err.to_string().contains("TESTSECRET"));
}

// --- JSON (Claude Code, Claude Desktop) ------------------------------------------------------------

// spec: AGENT-043
#[test]
fn claude_code_connect_inserts_into_root_servers_and_keeps_every_byte() {
    for exe in [MAC, WIN] {
        let after = connect(Client::ClaudeCode, CLAUDE_CODE, exe, &mcp()).unwrap();
        let (at, removed, added) = changed(CLAUDE_CODE, &after);
        assert_eq!(removed, "", "nothing outside may be removed");
        let exe_json = serde_json::to_string(exe).unwrap();
        assert_eq!(
            added,
            format!(",\n    \"openspider\": {{\n      \"args\": [\n        \"mcp\"\n      ],\n      \"command\": {exe_json},\n      \"type\": \"stdio\"\n    }}")
        );
        // Eingefügt am Ende der Wurzel-`mcpServers` (hinter dem Eintrag `proxy`), nicht in einem Projekt.
        assert!(CLAUDE_CODE[..at].ends_with("region=eu\"\n    }"));
        assert_eq!(
            entry_state(Client::ClaudeCode, &after, exe, &mcp()).unwrap(),
            EntryState::Current
        );
        assert_eq!(
            connect(Client::ClaudeCode, &after, exe, &mcp()).unwrap(),
            after
        );
        assert_eq!(disconnect(Client::ClaudeCode, &after).unwrap(), CLAUDE_CODE);
        let v: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(
            v["projects"]["/Users/anna/Code/shop"]["mcpServers"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
    }
}

// spec: AGENT-043
#[test]
fn claude_desktop_keeps_four_space_indent_and_missing_final_newline() {
    let after = connect(Client::ClaudeDesktop, CLAUDE_DESKTOP, MAC, &mcp()).unwrap();
    let (_, removed, added) = changed(CLAUDE_DESKTOP, &after);
    assert_eq!(removed, "");
    assert_eq!(
        added,
        format!(",\n        \"openspider\": {{\n            \"args\": [\n                \"mcp\"\n            ],\n            \"command\": \"{MAC}\"\n        }}")
    );
    assert!(after.ends_with("\"Alt+Space\"\n}"));
    assert_eq!(
        disconnect(Client::ClaudeDesktop, &after).unwrap(),
        CLAUDE_DESKTOP
    );
}

// spec: AGENT-043
#[test]
fn json_update_replaces_only_the_own_value() {
    let before = CLAUDE_CODE.replace(
        "  \"mcpServers\": {\n    \"github\"",
        "  \"mcpServers\": {\n    \"openspider\": {\"command\": \"/old/openspider\", \"args\": [\"mcp\"], \"type\": \"stdio\"},\n    \"github\"",
    );
    assert_eq!(
        entry_state(Client::ClaudeCode, &before, MAC, &mcp()).unwrap(),
        EntryState::Outdated
    );
    let after = connect(Client::ClaudeCode, &before, MAC, &mcp()).unwrap();
    let (at, removed, added) = changed(&before, &after);
    assert!(
        before[..at].ends_with("\"openspider\": {"),
        "{}",
        &before[at.saturating_sub(40)..at]
    );
    assert!(removed.contains("/old/openspider"));
    assert!(added.contains(MAC) && !added.contains("github"));
    // Erstes Mitglied entfernen: das folgende Komma geht mit, alles andere bleibt.
    assert_eq!(disconnect(Client::ClaudeCode, &after).unwrap(), CLAUDE_CODE);
}

// spec: AGENT-043
#[test]
fn json_single_line_empty_missing_and_crlf_files() {
    let single = r#"{"mcpServers":{"foreign":{"command":"foreign-helper"}}}"#;
    let after = connect(Client::ClaudeDesktop, single, MAC, &mcp()).unwrap();
    assert_eq!(
        after,
        format!(
            r#"{{"mcpServers":{{"foreign":{{"command":"foreign-helper"}},"openspider":{{"args":["mcp"],"command":"{MAC}"}}}}}}"#
        )
    );
    assert_eq!(disconnect(Client::ClaudeDesktop, &after).unwrap(), single);

    let spaced = r#"{ "mcpServers": { "a": 1, "b": 2 } }"#;
    let after = connect(Client::ClaudeDesktop, spaced, MAC, &mcp()).unwrap();
    assert!(
        after.starts_with(r#"{ "mcpServers": { "a": 1, "b": 2, "openspider": {"#),
        "{after}"
    );

    let no_servers = "{\n  \"theme\": \"dark\"\n}\n";
    let after = connect(Client::ClaudeCode, no_servers, MAC, &mcp()).unwrap();
    assert!(after.starts_with("{\n  \"theme\": \"dark\",\n  \"mcpServers\": {\n    \"openspider\": {\n      \"args\": ["), "{after}");
    assert!(after.ends_with("\n    }\n  }\n}\n"));
    assert_eq!(
        disconnect(Client::ClaudeCode, &after).unwrap(),
        "{\n  \"theme\": \"dark\",\n  \"mcpServers\": {}\n}\n"
    );

    let empty_servers = "{\n  \"mcpServers\": {}\n}";
    let after = connect(Client::ClaudeDesktop, empty_servers, MAC, &mcp()).unwrap();
    assert_eq!(
        after,
        format!("{{\n  \"mcpServers\": {{\n    \"openspider\": {{\n      \"args\": [\n        \"mcp\"\n      ],\n      \"command\": \"{MAC}\"\n    }}\n  }}\n}}")
    );
    assert_eq!(
        disconnect(Client::ClaudeDesktop, &after).unwrap(),
        empty_servers
    );

    for empty in ["", "\n"] {
        let after = connect(Client::ClaudeCode, empty, MAC, &mcp()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(v["mcpServers"]["openspider"]["command"], MAC);
        assert!(after.ends_with('\n'));
    }
    assert_eq!(
        entry_state(Client::ClaudeCode, "", MAC, &mcp()).unwrap(),
        EntryState::Absent
    );

    let crlf = CLAUDE_CODE.replace('\n', "\r\n");
    let after = connect(Client::ClaudeCode, &crlf, MAC, &mcp()).unwrap();
    let (_, removed, added) = changed(&crlf, &after);
    assert_eq!(removed, "");
    assert!(added.starts_with(",\r\n    \"openspider\": {\r\n      \"args\": [\r\n"));
    assert!(!added.replace("\r\n", "").contains('\n'));
    assert_eq!(disconnect(Client::ClaudeCode, &after).unwrap(), crlf);

    let tabs = "{\n\t\"mcpServers\": {\n\t\t\"a\": {}\n\t}\n}\n";
    let after = connect(Client::ClaudeDesktop, tabs, MAC, &mcp()).unwrap();
    assert!(
        after.contains(",\n\t\t\"openspider\": {\n\t\t\t\"args\": [\n\t\t\t\t\"mcp\"\n\t\t\t],"),
        "{after}"
    );
}

// spec: AGENT-043
#[test]
fn json_files_that_cannot_be_edited_in_place_are_rejected() {
    let cases = [
        (
            "{\n  // Kommentar\n  \"a\": 1\n}",
            ConfigError::Syntax { line: 2 },
        ),
        ("{\n  \"a\": 1,\n}", ConfigError::Syntax { line: 3 }),
        ("{\n  \"a\": [1, 2\n}", ConfigError::Syntax { line: 3 }),
        ("{\"a\": \"TESTSECRET-98", ConfigError::Syntax { line: 1 }),
        ("{\"a\": 01}", ConfigError::Syntax { line: 1 }),
        ("{\"a\": \"\\ud800\"}", ConfigError::Syntax { line: 1 }),
        ("[]", ConfigError::Unsupported(Unsupported::RootNotObject)),
        (
            "{\"mcpServers\": []}",
            ConfigError::Unsupported(Unsupported::ServersNotObject),
        ),
        (
            "{\"mcpServers\": {\"openspider\": {}, \"openspider\": {}}}",
            ConfigError::Unsupported(Unsupported::DuplicateKey),
        ),
        (
            "{\"mcpServers\": {}, \"mcpServers\": {}}",
            ConfigError::Unsupported(Unsupported::DuplicateKey),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(
            connect(Client::ClaudeCode, text, MAC, &mcp()),
            Err(expected),
            "{text}"
        );
        assert_eq!(
            entry_state(Client::ClaudeCode, text, MAC, &mcp()).err(),
            Some(expected),
            "{text}"
        );
        assert!(!expected.to_string().contains("TESTSECRET"));
    }
}

// spec: AGENT-043
#[test]
fn json_parser_accepts_the_full_grammar_of_the_fixture() {
    let root = json_file::parse(CLAUDE_CODE).unwrap();
    let ours: serde_json::Value = serde_json::from_str(CLAUDE_CODE).unwrap();
    assert!(matches!(root.kind, json_file::Kind::Object(_)));
    assert!(ours["cachedChangelog"]
        .as_str()
        .unwrap()
        .contains("\u{1f41b}"));
    let bom = format!("\u{feff}{CLAUDE_DESKTOP}");
    let after = connect(Client::ClaudeDesktop, &bom, MAC, &mcp());
    // serde_json lehnt eine BOM ab; die Prüfung schlägt dann fehl, statt still umzuschreiben.
    assert!(after.is_err() || after.unwrap().starts_with('\u{feff}'));
    let deep = format!("{}{}", "[".repeat(300), "]".repeat(300));
    assert!(matches!(
        json_file::parse(&format!("{{\"a\": {deep}}}")),
        Err(ConfigError::Syntax { .. })
    ));
}

// --- Maskierung -------------------------------------------------------------------------------------

// spec: AGENT-043
#[test]
fn masking_hides_every_test_secret_and_keeps_everything_else() {
    for (client, text) in [
        (Client::Codex, CODEX),
        (Client::ClaudeCode, CLAUDE_CODE),
        (Client::ClaudeDesktop, CLAUDE_DESKTOP),
    ] {
        let masked = mask(client, text).unwrap();
        assert_no_secrets(&masked);
        assert_eq!(masked.lines().count(), text.lines().count(), "{client:?}");
        for visible in ["npx", "/Users/anna"] {
            assert!(masked.contains(visible), "{client:?}: {visible}");
        }
        let after = connect(client, text, MAC, &mcp()).unwrap();
        let masked_after = mask(client, &after).unwrap();
        assert_no_secrets(&masked_after);
        // Der eigene Eintrag erscheint vollständig.
        assert!(masked_after.contains(MAC), "{client:?}");
        if client == Client::Codex {
            assert!(masked_after.starts_with(&masked));
        }
    }
    let codex = mask(Client::Codex, CODEX).unwrap();
    for visible in [
        "GITHUB_HOST = \"\u{2022}\u{2022}\u{2022}\u{2022}\"",
        "PGUSER = \"\u{2022}\u{2022}\u{2022}\u{2022}\"",
        "\"X-Client\" = \"codex\"",
        "\"--region=de\"",
        "[projects.'C:\\Users\\Anna\\Projekte\\Größe & Co']",
        "model_reasoning_effort = \"high\"   # Kommentar am Zeilenende",
    ] {
        assert!(codex.contains(visible), "{visible}\n{codex}");
    }
    let code = mask(Client::ClaudeCode, CLAUDE_CODE).unwrap();
    for visible in [
        "\"emailAddress\": \"anna@example.test\"",
        "\"url\": \"https://mcp.example.test/mcp\"",
        "\"Authorization\": \"\u{2022}\u{2022}\u{2022}\u{2022}\"",
        "\"--root\", \"D:\\\\Daten mit Leerzeichen\"",
    ] {
        assert!(code.contains(visible), "{visible}\n{code}");
    }
}

// spec: AGENT-043
#[test]
fn secret_detection_rules() {
    for key in [
        "env",
        "ENV",
        "headers",
        "OPENAI_API_KEY",
        "accessToken",
        "client_secret",
        "Password",
        "Authorization",
        "cookie",
        "credentials",
    ] {
        assert!(is_secret_key(key), "{key}");
    }
    for key in ["command", "args", "url", "type", "enabled", "projects"] {
        assert!(!is_secret_key(key), "{key}");
    }
    for flag in [
        "--token",
        "--api-key",
        "--password=x",
        "-apikey",
        "--client-secret",
    ] {
        assert!(is_secret_flag(flag), "{flag}");
    }
    for flag in ["--root", "-y", "--region=de", "token"] {
        assert!(!is_secret_flag(flag), "{flag}");
    }
    for s in [
        "Bearer abc",
        "authorization: x",
        "--token=abc",
        "https://user:pw@host/x",
        "https://host/x?api_key=1&b=2",
        "postgresql://reader:pw@db:5432/x",
    ] {
        assert!(is_secret_string(s), "{s}");
    }
    for s in [
        "https://host/x?region=eu",
        "@modelcontextprotocol/server-github",
        "anna@example.test",
        "npx",
    ] {
        assert!(!is_secret_string(s), "{s}");
    }
}

// --- Nachtrag Review: eigene Zeilen und Zusätze ---------------------------------------------------

/// Zeilen eines Textes mit Index (wie `similar::TextDiff::from_lines`).
fn line(text: &str, index: usize) -> &str {
    text.split_inclusive('\n')
        .nth(index)
        .unwrap()
        .trim_end_matches(['\n', '\r'])
}

// spec: AGENT-043
#[test]
fn own_lines_cover_exactly_the_own_entry() {
    for (client, text) in [
        (Client::Codex, CODEX),
        (Client::ClaudeCode, CLAUDE_CODE),
        (Client::ClaudeDesktop, CLAUDE_DESKTOP),
    ] {
        assert_eq!(own_lines(client, text).unwrap(), None, "{client:?}");
        let after = connect(client, text, MAC, &mcp()).unwrap();
        let own = own_lines(client, &after).unwrap().unwrap();
        let shown: Vec<&str> = own.clone().map(|i| line(&after, i)).collect();
        assert!(
            shown.iter().any(|l| l.contains(MAC)),
            "{client:?}: {shown:?}"
        );
        assert!(shown.iter().all(|l| !l.contains("TESTSECRET")));
        // Die Zeile davor ist fremd (bei JSON trägt sie das angehängte Komma).
        let before_line = line(&after, own.start - 1);
        assert!(
            !before_line.contains("openspider"),
            "{client:?}: {before_line}"
        );
        if client != Client::Codex {
            assert!(before_line.ends_with(','), "{before_line}");
        }
    }
    // Einzeiliges JSON: keine Zeile gehört allein dem eigenen Eintrag.
    let single = connect(Client::ClaudeDesktop, r#"{"mcpServers":{}}"#, MAC, &mcp()).unwrap();
    assert_eq!(own_lines(Client::ClaudeDesktop, &single).unwrap(), None);
    // Inline-Eintrag mit Kommentar auf derselben Zeile: der Kommentar ist fremd.
    let inline =
        "[mcp_servers]\nopenspider = { command = \"x\" } # Notiz\nfoo = { command = \"y\" }\n";
    assert_eq!(own_lines(Client::Codex, inline).unwrap(), None);
    let inline = "[mcp_servers]\nopenspider = { command = \"x\" }\nfoo = { command = \"y\" }\n";
    assert_eq!(own_lines(Client::Codex, inline).unwrap(), Some(1..2));
    // Ungültige Dateien melden den Fehler.
    assert!(own_lines(Client::ClaudeCode, "{").is_err());
}

// spec: AGENT-043
#[test]
fn user_additions_in_the_own_entry_are_named_without_values() {
    let codex = "[mcp_servers.openspider]\ncommand = \"/old\"\nargs = [\"mcp\"]\nstartup_timeout_sec = 30\n\n[mcp_servers.openspider.env]\nRUST_LOG = \"TESTSECRET-77\"\n";
    assert_eq!(
        own_extra_keys(Client::Codex, codex, MAC, &mcp()).unwrap(),
        ["env", "startup_timeout_sec"]
    );
    assert_eq!(
        entry_state(Client::Codex, codex, MAC, &mcp()).unwrap(),
        EntryState::Outdated
    );
    let json = r#"{"mcpServers": {"openspider": {"type": "stdio", "command": "/old", "args": [], "env": {"A": "TESTSECRET-78"}}}}"#;
    assert_eq!(
        own_extra_keys(Client::ClaudeCode, json, MAC, &mcp()).unwrap(),
        ["env"]
    );
    // `type` gehört bei Claude Code zum erzeugten Eintrag, bei Claude Desktop nicht.
    assert_eq!(
        own_extra_keys(Client::ClaudeDesktop, json, MAC, &mcp()).unwrap(),
        ["env", "type"]
    );
    let current = connect(Client::Codex, CODEX, MAC, &mcp()).unwrap();
    assert!(own_extra_keys(Client::Codex, &current, MAC, &mcp())
        .unwrap()
        .is_empty());
    assert!(own_extra_keys(Client::Codex, CODEX, MAC, &mcp())
        .unwrap()
        .is_empty());
}
