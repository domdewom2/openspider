# agent-client-configs (AGENT-043)

Realistische Konfigurationsdateien fremder MCP-Clients für die Tests des formatbewahrenden Schreibens
(`crates/os-mcp/src/client_config/tests.rs`, `crates/os-app/tests/mcp_clients.rs`):

- `codex-config.toml` – wie `~/.codex/config.toml`: viele fremde Server, Inline-Tabellen, mehrzeilige
  Arrays, Untertabellen, Kommentare, Projekte mit macOS- und Windows-Pfaden, Literal-Strings.
- `claude-code.json` – wie `~/.claude.json`: Projekte mit Pfaden als Schlüssel und eigenen `mcpServers`,
  Wurzel-`mcpServers` mitten in der Datei, Escapes (`\u…`, `\/`, Surrogatpaar), Konto- und Schlüsselwerte.
- `claude-desktop.json` – wie `claude_desktop_config.json`: vier Leerzeichen Einrückung, ohne
  Zeilenumbruch am Dateiende.

Alle Werte mit `TESTSECRET` sind Testgeheimnisse: Sie dürfen in keiner Vorschau erscheinen.
CRLF-, einzeilige und ungültige Varianten erzeugen die Tests selbst.
