# Bericht B3-CURSOR: Cursor mit einem Klick verbinden, schreibgeschützte Konfigurationsdateien

Stand: 05.11.2026 · Branch `m3/cursor` (Basis origin/main `f90bb133`) · Worktree `c-cursor`, Server `m3-cursor`.
Grundlage: Bericht `b3-mcp.md` (Spec-Frage 1, Backlog Windows-Readonly), `docs/plan/beta3.md` §1, Spec AGENT-001,
AGENT-003, AGENT-043, UI-129. Nichts gepusht, kein Workflow gestartet, kein Website-Deploy.

## Commits

| Commit | Inhalt |
|---|---|
| `ef15600c` docs: | Spec zuerst: AGENT-043 Cursor (Datei, Format, Erkennung, Meldung, Kriterien, Fixture) und schreibgeschützte Dateien; AGENT-001 `type` für Cursor; UI-129 Menüeintrag |
| `a9645292` test: | Fixture `tests/fixtures/agent-client-configs/cursor-mcp.json` + README |
| `b90c87c0` mcp: | `--print-config cursor` mit `type: "stdio"`, Platzierungshinweis auf globale Datei und App-Dialog, Goldens, Cursor-Tests in `client_config` |
| `ad86ac0b` app: | `McpClientKind::Cursor`, Pfad `~/.cursor/mcp.json`, `McpClientProblem::ReadOnly`, Ablehnung vor der Sicherung; Unit- und Integrationstests (inkl. `#[cfg(windows)]`); TS-Typen erzeugt |
| `f3ad7bf1` ui: | Menü „Mit Cursor verbinden…“, Meldungen „MCP-Server neu laden“ (DE/EN), Hinweis schreibgeschützt; Vitest |
| `5b5f005e` test: | Playwright `dialog.spec.ts`/`bridge.spec.ts` mit Cursor |
| `b7d571a5` docs: | Handbuch „MCP einrichten“ und „MCP in 2 Minuten“ DE/EN |
| (dieser) docs: | Bericht |

## Ergebnis

| Auftrag | Stand |
|---|---|
| 1 Cursor als viertes Programm | umgesetzt: Menüeintrag „Mit Cursor verbinden…“, gleicher Dialog/Zustände/Vorschau/Sicherung, Datei `~/.cursor/mcp.json` |
| 1 `--print-config cursor` passt zum geschriebenen Eintrag | ja, beide über `print_config::render`; Test vergleicht wertgleich. Neu mit `"type": "stdio"` (siehe Entscheidung) |
| 1 optional Installationslink | nicht gebaut (Begründung unten), als Idee notiert |
| 2 Windows-Readonly | entschieden: **nicht schreiben, klar melden** (Zustand „Datei kann nicht geändert werden“, Grund „schreibgeschützt“), für alle Programme und beide Plattformen |
| 3 Hilfetext `mcp-classes-help` | bereits durch B3-MCP korrigiert (keine Kostenschätzung, keine Bestätigung); Vitest „does not promise a cost estimate or user confirmation before AGENT-030“ prüft das. Nur bestätigt, nicht geändert |
| 4 Spec | AGENT-043 (Cursor, Datei, Erkennung, Meldung, Readonly, 3 Kriterien, Fixture), AGENT-001 (`type` bei Cursor, Platzierungshinweis), UI-129 (Menüeintrag) |
| 5 Handbuch/Website | „MCP einrichten“ und „MCP in 2 Minuten“ DE/EN um Cursor und Schreibschutz ergänzt; nicht deployt |

## Pfade je Plattform

| Programm | macOS | Windows | Quelle im Code |
|---|---|---|---|
| Cursor (neu) | `~/.cursor/mcp.json` | `%USERPROFILE%\.cursor\mcp.json` | `dirs::home_dir()` + `.cursor` + `mcp.json` (`Path::join`, keine Trenner) |
| Mit App-Heimordner (portabel, Tests) | `<Heim>/.cursor/mcp.json` | `<Heim>\.cursor\mcp.json` | wie die übrigen Programme |

Keine Umgebungsvariable (Cursor dokumentiert keine). Projektbezogene `.cursor/mcp.json` in Projektordnern werden
nicht angefasst. Grundlage ist ausschließlich Cursors öffentliche Doku (`cursor.com/docs/context/mcp` und
`…/mcp/install-links`, gelesen am 05.11.2026): globale Datei `~/.cursor/mcp.json`, Wurzelschlüssel `mcpServers`,
STDIO-Felder `type`, `command`, `args`, `env`, `envFile`; Interpolation `${env:…}`, `${userHome}` u. a. Windows nennt
die Doku nicht ausdrücklich; `~` entspricht dort dem Benutzerordner, den `dirs::home_dir()` liefert.

## Verhalten

- **Menü und Dialog:** MCP → „Mit Cursor verbinden…“ (EN „Connect Cursor…“) öffnet denselben Dialog wie bei den
  anderen Programmen; im Reiter „Verbundene Programme“ ist Cursor in der Programmauswahl.
- **Zustände:** „Noch nicht verbunden“ / „Bereits verbunden“ / „Verbunden, aber der Eintrag ist veraltet“ /
  „Die Datei kann nicht geändert werden“ – unverändert aus B3-MCP. Ein Cursor-Eintrag ohne `type` (wie ihn
  `--print-config cursor` bisher ausgab) gilt als veraltet; „Aktualisieren“ ersetzt ihn an Ort und Stelle.
- **Schreiben:** derselbe JSON-Writer wie Claude Desktop/Claude Code (Einfügen am Ende der Wurzel-`mcpServers`,
  Einrückung/Zeilenende der Datei, Sicherung `*.<uuid>.bak` mit den Rechten des Originals, Stale-Prüfung,
  Symlink-Ablehnung, Trennen ergibt die Ausgangsdatei byte-gleich).
- **Vorschau:** dieselbe Zeilenregel (`…`/`…,` für fremde Zeilen); die Fixture `cursor-mcp.json` enthält Geheimnisse
  in `env`, `headers` (`Bearer`), URL-Benutzerdaten und `--api-key=`; keiner erscheint (Tests in `os-mcp`, `os-app`,
  Vitest, Playwright-Bridge).
- **Erkennung:** Wie bei den übrigen drei Programmen prüft der Dialog nicht, ob Cursor installiert ist (die
  Vorgabe „wie die anderen Programme behandeln“; dort gibt es keine Installationserkennung). Fehlt `~/.cursor`
  oder die Datei, lautet der Zustand „Noch nicht verbunden“; „Verbinden“ legt Ordner und Datei an (ohne Sicherung).
- **Meldung danach** (Spec-Stil, eigene Schlüssel `mcp-client-done-*-reload`, nur für Cursor):
  - DE „Verbunden. Lade in Cursor die MCP-Server neu oder starte Cursor neu, damit OpenSpider dort erscheint.“ /
    „Bereits verbunden, die Datei ist unverändert. Lade in Cursor die MCP-Server neu, falls OpenSpider dort noch
    fehlt.“ / „Getrennt. Lade in Cursor die MCP-Server neu.“
  - EN „Connected. Reload the MCP servers in Cursor or restart Cursor so that OpenSpider appears there.“ /
    „Already connected; the file is unchanged. Reload the MCP servers in Cursor if OpenSpider is still missing
    there.“ / „Disconnected. Reload the MCP servers in Cursor.“
  - Bewusst ohne Menüpfad in Cursor (die Doku nennt die Einstellungen „Customize“, das kann sich ändern; der
    Neustart ist der sichere Weg).
- **CLI:** `--print-config cursor` gibt jetzt `type`, `command`, `args` aus; der Platzierungshinweis nennt
  `~/.cursor/mcp.json` (Windows `%USERPROFILE%\.cursor\mcp.json`) und den App-Dialog statt zuerst der
  Projektdatei. Goldens `tests/golden/mcp-print-config/{macos,windows}/cursor.json` angepasst (Grund: Feldtabelle
  der Cursor-Doku führt `type` für STDIO als Pflichtfeld; die Beispiele dort lassen es weg, Cursor nimmt also beides).

### Entscheidung `"type": "stdio"` für Cursor

Die Cursor-Doku ist in sich uneinheitlich: Feldtabelle „`type` – Required: Yes“, Beispiele ohne `type`. Mit `type`
ist der Eintrag nach beiden Lesarten gültig. Folge: Wer den Eintrag früher von Hand aus `--print-config cursor`
übernommen hat, sieht „veraltet“ und kann mit einem Klick aktualisieren. In AGENT-001/043 festgehalten.

## Entscheidung Windows-Readonly

**Befund (vorher, `clients.rs` `write_guarded`):** Unter Windows wurde zuerst die Sicherung angelegt, dann die
temporäre Datei mit den Rechten des Originals (also schreibgeschützt) versehen und per `persist` (MoveFileEx mit
Ersetzen) über das Original gelegt. Das Ersetzen einer schreibgeschützten Zieldatei scheitert mit „Zugriff
verweigert“ – nach dem Anlegen der Sicherung, d. h. es blieb eine zusätzliche Kopie mit fremden Geheimnissen und
eine Fehlermeldung ohne verständlichen Grund. Unter Unix hätte `rename` eine `0444`-Datei dagegen stillschweigend
ersetzt (Schreibschutz ignoriert).

**Entscheidung:** Schreibgeschützte Dateien werden **nicht geschrieben**, auf beiden Plattformen und für alle vier
Programme. Prüfung über `std::fs::Permissions::readonly()` (Windows: Attribut `FILE_ATTRIBUTE_READONLY`; Unix: kein
Schreibrecht-Bit gesetzt) – plattformneutral, ohne `cfg`-Zweig im Produktcode.
- Vorschau: `problem: "readOnly"` → Zustand „Die Datei kann nicht geändert werden“ und Meldung „Die
  Konfigurationsdatei ist schreibgeschützt. OpenSpider ändert sie nicht. Den Schreibschutz aufheben oder den Eintrag
  kopieren und von Hand einfügen.“; Verbinden/Trennen gesperrt, „Eintrag kopieren“ bleibt.
- `write_guarded`: nach der Stale-Prüfung und **vor** der Sicherung → `CmdError` `Permission`,
  `mcp-client-problem-readOnly`; keine Sicherung, keine temporäre Datei, Inhalt und Attribut unverändert.

**Begründung gegen „Attribut übernehmen“:** Der Schreibschutz ist eine bewusste Entscheidung des Nutzers oder einer
Verwaltung (z. B. verwaltete Rechner). Ihn aufzuheben, zu ersetzen und wieder zu setzen wäre drei Schritte statt
eines atomaren Austauschs; ein Abbruch dazwischen hinterließe eine ungeschützte Datei. Die Ablehnung passt zur
Symlink-Regel aus B3-MCP (im Zweifel nicht schreiben, Eintrag kopieren).

## Tests

Alle Läufe auf dem Build-Server (Worktree `m3-cursor`), Stand der Commits oben:

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| `cargo test --workspace` (nach `~/ws-slot.sh`) | **4358 bestanden, 0 fehlgeschlagen, 51 ignoriert** (606 Suiten) |
| `cargo test -p os-mcp --lib` | 84 bestanden (vorher 82) |
| `cargo test -p os-mcp --test server_print_config` | 3 bestanden (Goldens macOS/Windows) |
| `cargo test -p os-app --lib mcp::` / `--test mcp_clients` | 8 / 6 bestanden (Linux; der `#[cfg(windows)]`-Test läuft erst in der Windows-CI nach dem Merge) |
| `os-app-manifest` (neu erzeugt) / `gen-ipc.mjs --check` | nur die zwei TS-Typen geändert, identisch mit dem Commit / 29 Wrapper aktuell |
| `npm run -w apps/desktop typecheck` | OK |
| `npm run -w apps/desktop test` | **114 Dateien, 991 Tests bestanden** (vorher 987; inkl. i18n-Vollständigkeit, CSS-Lint) |
| `npx playwright test -c e2e/mcp/playwright.config.ts --workers=1 dialog.spec.ts bridge.spec.ts`, Projekte Chromium und WebKit | **24 bestanden** (21,3 min; u. a. „real writer connects all clients in one click…“ jetzt mit Cursor, gegen `os-app-bridge` mit Feature `e2e-bridge`) |
| `node website/check-site.mjs`, `bash scripts/check-neutral.sh` | OK (20 Seiten), keine Fundstellen |

Hinweis: Der erste Playwright-Lauf scheiterte nur an der Bridge-Binärdatei (ohne `--features e2e-bridge` gebaut);
nach korrektem Bau grün. Ein Zwischenstand von `read_only_client_files_are_never_written` scheiterte an einer zu
strengen Zählung (`~/.claude.json` liegt direkt im Heimordner neben anderen Dateien); die Prüfung vergleicht jetzt
die Dateianzahl vorher/nachher – nicht gelockert, sondern ortsunabhängig.

Neue bzw. erweiterte Tests (alle mit `// spec: AGENT-043`, teils zusätzlich `AGENT-001`):

| Datei | Test | prüft |
|---|---|---|
| `crates/os-mcp/src/client_config/tests.rs` | `cursor_connect_appends_to_root_servers_and_keeps_every_byte` | Fixture `cursor-mcp.json`, macOS- und Windows-Programmpfad: nur Einfügen, Anfang/Ende byte-gleich, Idempotenz, Trennen = Ausgangsdatei, Eintrag wertgleich mit `--print-config cursor`, fremde Server wertgleich |
| | `cursor_entry_without_type_is_outdated_and_updated_in_place` | alter Eintrag ohne `type` → veraltet, Ersetzen an Ort und Stelle |
| | `masking_hides_every_test_secret_and_keeps_everything_else` (+Cursor) | keine `TESTSECRET` nach Maskierung, Platzhalter/URL/Windows-Pfad sichtbar |
| `crates/os-mcp/src/print_config.rs` | `json_clients_…`, `placement_hints_name_the_same_files_as_the_app` | `type` bei Claude Code und Cursor, nicht bei Claude Desktop; Hinweis nennt globale Datei, `%USERPROFILE%`, App-Dialog |
| `crates/os-mcp/tests/server_print_config.rs` | Goldens macOS/Windows | neue `cursor.json` |
| `crates/os-app/src/mcp/clients.rs` | `read_only_files_are_refused_before_any_backup` | plattformneutral über `set_readonly`: Fehler `Permission`/`mcp-client-problem-readOnly`, keine Sicherung, Schutz bleibt, Stale-Reihenfolge, fehlende Datei nicht schreibgeschützt |
| | `foreign_lines_next_to_the_own_entry_appear_only_as_ellipsis` (+Cursor) | Vorschau ohne `ZZOPAQUE` |
| | `codex_home_and_claude_config_dir_are_honoured_when_absolute` (+Cursor) | Cursor-Pfad, unabhängig von Umgebungsvariablen |
| `crates/os-app/tests/mcp_clients.rs` | `one_click_connect_update_and_disconnect_keep_every_foreign_byte` (+Cursor) | Ein-Klick-Ablauf über die Commands mit der Fixture |
| | `cursor_gets_the_print_config_entry_in_the_global_file` | ohne `.cursor`-Ordner: nicht verbunden → Datei angelegt, gleich „Eintrag kopieren“, `type`/`args`, absoluter Pfad |
| | `read_only_client_files_are_never_written` | alle vier Programme mit verbundener Fixture, schreibgeschützt: Vorschau `readOnly`, Verbinden und Trennen abgelehnt, Inhalt/Anzahl Dateien/Schutz/Freigabe unverändert, keine Testgeheimnisse im Fehler |
| | `windows_read_only_attribute_is_kept_and_nothing_is_written` (`#[cfg(windows)]`) | Attribut `FILE_ATTRIBUTE_READONLY` an der Cursor-Fixture bleibt bitgleich; ohne Attribut wird normal verbunden. Läuft erst in der Windows-CI nach dem Merge |
| `apps/desktop/src/features/mcp/mcp.test.tsx` | Ein-Klick-Schleife (+Cursor), `tells Cursor users to reload…`, `blocks a read-only configuration file…` | Meldungen EN/DE, gesperrte Knöpfe, Fehler des Dienstes nur als fester Text, kein Testtoken im DOM |
| `apps/desktop/e2e/mcp/dialog.spec.ts`, `bridge.spec.ts` | Menüeintrag `mcp.cursor`, Ein-Klick-Schleife (+Cursor); echter Writer über `os-app-bridge` mit Cursor-Datei im Bridge-Heim | Bytes vor/nach dem Eintrag, Sicherung byte-gleich, Trennen = Original, kein Geheimnis im DOM |

## Fremde Dateien (K2-92)

Dateien außerhalb von `crates/os-mcp`, `crates/os-app/src/mcp`, `apps/desktop/src/features/mcp`, Spec und Bericht:

| Datei | Änderung | Grund |
|---|---|---|
| `crates/os-app/src/dto/mcp.rs` | `McpClientKind::Cursor`, `McpClientProblem::ReadOnly` | IPC-Vertrag |
| `crates/os-app/tests/mcp_clients.rs` | Cursor in der Programmliste, drei neue Tests | Integrationstests |
| `apps/desktop/src/ipc/gen/McpClientKind.ts`, `McpClientProblem.ts` | je ein Literal mehr (erzeugt mit `os-app-manifest`, Ausgabe gleich) | generierte Typen |
| `apps/desktop/e2e/mcp/dialog.spec.ts`, `bridge.spec.ts` | Cursor in den Schleifen, Menüeintrag | E2E |
| `tests/fixtures/agent-client-configs/cursor-mcp.json` (neu), `README.md` | neue Fixture, Beschreibung | Testfixture AGENT-043 |
| `tests/golden/mcp-print-config/{macos,windows}/cursor.json` | `"type": "stdio"` | Snapshot AGENT-001 |
| `website/site/handbuch/index.html`, `website/site/en/manual/index.html` | nur Abschnitt `#mcp` | Handbuch „MCP einrichten“ |
| `website/site/funktionen/index.html`, `website/site/en/features/index.html` | nur Abschnitt „MCP in 2 Minuten“ | Website |
| `docs/spec/05-ui.md` | UI-129: ein Satz (Menüeintrag) | Verweis |

Nicht angefasst: `commands.json` (keine neuen Commands), `Cargo.toml`/`Cargo.lock` (keine neuen Abhängigkeiten),
Startseiten, `site.css`, andere Crates.

## Spec-Fragen und Ideen

1. **Installationslink (Idee, nicht gebaut):** Cursor dokumentiert `cursor://anysphere.cursor-deeplink/mcp/install?name=<Name>&config=<Base64 von JSON.stringify(Server-Objekt)>`;
   eine `https`-Variante ist nicht dokumentiert. Geheimnisrisiko gäbe es keines (der Eintrag enthält nur
   Programmpfad und `mcp`). Dagegen: (a) Auf der Website ist der absolute Programmpfad unbekannt (Windows-Installationsort
   frei wählbar, macOS-App verschiebbar), ein Link wäre dort also geraten; (b) in der App müsste UI-130 (nur
   `https`) um das Schema `cursor:` erweitert werden; (c) das direkte Schreiben erledigt dasselbe mit einem Klick,
   mit Sicherung und Zustandserkennung. Sinnvoll wäre höchstens ein zweiter Knopf „In Cursor installieren“ in der
   App, falls Nutzer die Bestätigung in Cursor bevorzugen – bitte entscheiden, ob gewünscht.
2. **Neue Server in Cursor ein- oder ausgeschaltet?** Die Doku sagt nicht, ob ein neu eingetragener Server sofort
   aktiv ist oder erst unter „Customize“ eingeschaltet werden muss. Die Meldung nennt deshalb „neu laden oder neu
   starten“. Falls der Nutzer eine genauere Anweisung möchte, braucht es eine Prüfung an einer echten Cursor-Installation
   (nicht in diesem Paket; fremde Konfigurationen auf dem Mac wurden bewusst nicht gelesen).
3. **Unix `0444` wird jetzt ebenfalls abgelehnt** (vorher stillschweigend ersetzt). Das ist eine Verhaltensänderung
   für macOS-Nutzer mit bewusst schreibgeschützter Datei; gewollt im Sinn der Entscheidung, bitte bestätigen.
4. **Erkennung „installiert“:** Für keines der vier Programme gibt es eine Installationserkennung. Falls der Dialog
   künftig „Cursor scheint nicht installiert“ zeigen soll, wäre das ein eigener Spec-Punkt für alle Programme
   (Prüfgrundlage je Plattform festlegen).
5. **Projektdatei `.cursor/mcp.json`:** bleibt bewusst unberührt (Spec); die CLI nennt sie weiter als Alternative.

## Server und Aufräumen

Server-`target/` von `m3-cursor` (122 GB) sowie `test-results/` gelöscht; `node_modules` bleibt für die
Nachprüfung. Keine fremden Worktrees oder deren `target/` benutzt. Fremde Konfigurationsdateien auf dem Mac
(`~/.cursor/mcp.json` usw.) wurden weder gelesen noch verändert; gearbeitet wurde nur mit Fixtures.
