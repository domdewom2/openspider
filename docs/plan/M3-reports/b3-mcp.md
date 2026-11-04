# B3-MCP – MCP mit einem Klick verbinden

Stand: 04.11.2026 · Branch `m3/mcp-connect` (Basis `origin/main` 79483063) · Build/Test: Linux-Build-Server
(Worktree `m3-mcp-connect`) · nicht nach origin gepusht, kein Merge.
Grundlage: `docs/plan/beta3.md` Abschnitt 1 (Nutzerentscheidung 04.11.2026), Spec AGENT-001, AGENT-003,
AGENT-043, UI-129.

## Commits

| Commit | Inhalt |
|---|---|
| `9c15b07f` docs: | Spec zuerst: AGENT-043 Verbindungsdialog, formatbewahrendes Schreiben, Vorschau ohne Geheimnisse; AGENT-003 Freigabe `crawl` beim Verbinden und Hilfetext; Verweis in UI-129 |
| `ee236eb8` mcp: | `os_mcp::client_config` formatbewahrend (Verbinden, Aktualisieren, Trennen, Maskieren), Fixtures, `toml_edit` |
| `de213102` app: | `mcp.clientConnect`/`mcp.clientDisconnect`, neue Vorschau, Freigabe `crawl`, Sicherung, Rechte; TS-Typen und Command-Katalog neu erzeugt |
| `e2ad8f6b` ui: | Dialog „Mit … verbinden“ je Programm, Reiter „Verbundene Programme“, Hilfetext `mcp-classes-help` |
| `8b002ec3` test: | E2E gegen Fixture-Dienst und echten Writer (Bridge) |
| `998dfc1c` web: | „MCP in 2 Minuten“ DE/EN, Link von der Startseite, Handbuch „MCP einrichten“ |
| (dieser) docs: | Bericht und Screenshots |

## Spec-Änderungen (Nutzerentscheidung 04.11.2026)

- **AGENT-043** (führend für „Verbundene Programme“): Dialog je Programm; Zustände „Nicht verbunden“ / „Bereits
  verbunden“ / „Verbunden, Eintrag veraltet“ / „Datei kann nicht geändert werden“; Verbinden/Aktualisieren in einem
  Schritt (Sicherung, Eintrag, Freigabe), Trennen nur des eigenen Eintrags; formatbewahrendes Schreiben mit
  Einfügeregeln für TOML/JSON; Ablehnungsgründe (Syntax mit Zeile, Aufbau, Symlink, geänderte Datei, Prüfung);
  Vorschau ohne Geheimnisse (Diff ohne Kontext aus maskierten Fassungen, Maskierungsregeln); Dateipfade je
  Programm; Cursor als offene Frage. Das alte Kriterium „Vor dem Schreiben wird ein Diff angezeigt“ ist ersetzt
  (Vorschau aufklappbar, nicht Pflicht). Neue Akzeptanzkriterien und Testfixture `tests/fixtures/agent-client-configs/`.
- **AGENT-003**: Haken setzt/entfernt `crawl` in `mcp.toml` (`read` immer, `write`/`cost` unberührt, gilt für alle
  Programme ohne Schalter); Trennen ändert die Freigabe nicht. Hilfetext: keine Kostenschätzung und keine
  Nutzerbestätigung zusagen, bis AGENT-030 umgesetzt ist. Je mit Kriterien und Testfixture.
- **UI-129**: Verweis auf AGENT-043/AGENT-003.

## Was der Nutzer jetzt klickt

| | Vorher | Nachher |
|---|---|---|
| Codex verbinden | MCP → „Mit Codex verbinden…“ → Reiter „Verbundene Programme“ → „Änderungen anzeigen“ → Haken „Ich habe die Änderungen geprüft…“ → „Konfiguration mit Sicherung schreiben“ → Reiter „Werkzeuge“ → Klasse „Crawlen“ einschalten (5 Schritte, 2 Reiter) | MCP → „Mit Codex verbinden…“ → **„Verbinden“** (Haken „Agent darf Crawls starten“ ist vorbelegt) → Meldung „Verbunden. Starte in Codex eine neue Sitzung, damit OpenSpider dort erscheint.“ + Pfad der Sicherung |
| Erneut öffnen | wieder volle Vorschau, Schreiben ohne Erkennung | „Bereits verbunden.“ mit „Aktualisieren“ / „Trennen“; veralteter Pfad → „Verbunden, aber der Eintrag ist veraltet“ |
| Vorschau | Pflicht, zeigte die ganze (umformatierte) Datei | aufklappbar „Änderungen anzeigen“, nur geänderte Zeilen, Geheimnisse maskiert |
| Kopieren | „Konfiguration kopieren“ kopierte die ganze fremde Datei | „Eintrag kopieren“ kopiert nur den eigenen Eintrag |

Screenshots (DE, Playwright Chromium, Fixture-Dienst): `docs/plan/M3-reports/b3-mcp/verbinden-codex-{light,dark}.png`,
`verbinden-codex-vorschau-{light,dark}.png` (Vorschau aufgeklappt), `verbunden-codex-{light,dark}.png` (nach dem Klick).
Der Knopf „Open MCP“ oben links gehört zum englischen Test-Harness, nicht zur App. Das Screenshot-Skript lief nur
auf dem Server und ist nicht eingecheckt (Harness-Parameter `?lang=de` ist eingecheckt).

## Umsetzung

- **`os_mcp::client_config`** (`client_config.rs`, `client_config/json_file.rs`, `client_config/toml_file.rs`):
  Jede Änderung ist genau ein eingefügter, ersetzter oder entfernter Ausschnitt; alle anderen Bytes werden
  nicht angefasst. JSON: eigener strenger RFC-8259-Parser mit Byte-Spannen (Escapes inkl. Surrogatpaare,
  Tiefe ≤ 256, BOM); Einfügen hinter dem letzten Mitglied mit Einrückung, Trenner `:` und Zeilenende der
  Datei; einzeilige Dateien bleiben einzeilig. `serde_json` (ohne `preserve_order`) dient nur der Prüfung.
  TOML: `toml_edit` nur als Parser (Feature `parse`, keine Ausgabe), liefert die Spannen; Anhängen am
  Dateiende bzw. Ersetzen von `[mcp_servers.openspider]` samt Untertabellen oder `openspider = { … }` an Ort
  und Stelle; Kommentare bleiben. Danach wird das Ergebnis neu geparst (`toml`/`serde_json` und eigener
  Parser) und mit der Ausgangsdatei verglichen – fremde Werte gleich, eigener Eintrag gleich `--print-config`.
  Gleichheit („Bereits verbunden“) vergleicht Werte, nicht Schreibweise. Fehler tragen nur Zeilennummer bzw.
  Grund, nie Dateiinhalt.
- **Maskierung** (`mask`): Werte unter `env`, `headers` und Schlüsseln mit `key`/`token`/`secret`/`password`/
  `passwd`/`authorization`/`cookie`/`credential` (Teilbaum), Zeichenketten mit `Bearer `/`Authorization:`,
  URL-Benutzerdaten und geheime Query-Parameter, Argumente nach `--token`/`--api-key`… bzw. `--token=…`;
  TOML-Kommentare mit solchen Wörtern ab `#`. Der eigene Eintrag bleibt vollständig. Der Diff entsteht in
  `os-app` aus zwei maskierten Fassungen mit `similar`, Kontextradius 0.
- **`os-app`** (`mcp/clients.rs`): `preview` (Zustand, Problem/Zeile, Diff Verbinden/Trennen, eigener Eintrag,
  aktuelle Freigabe), `connect` (Datei lesen → formatbewahrend ändern → unmittelbar vor dem Schreiben erneut
  lesen, sonst `Stale`/`mcp-client-changed` → Sicherung `*.<uuid>.bak` → atomarer Austausch mit den Rechten der
  alten Datei → Klasse `crawl` setzen; unveränderter Eintrag: keine Datei-Änderung, keine Sicherung),
  `disconnect`. Die Vorschau-Bindung an den Sitzungszustand (`State.previews`) entfällt. Die rohe fremde
  Datei verlässt den Kern nicht mehr (DTO ohne `before`/`after`).
- **App** (`features/mcp`): `ConnectPanel` (Dialog je Programm über `useMcpUi.connect`, Reiter mit
  Programmauswahl), feste Meldungsschlüssel (`mcp-client-*`), `redactText` als zweite Sicherung auf Diff und
  Pfaden. `configPreview.ts` (Redaktion ganzer Dokumente im Browser) entfällt, weil die Dokumente die
  Oberfläche nicht mehr erreichen.
- **Hilfetext** `mcp-classes-help` DE/EN: nennt Linkmetriken als einzige kostenpflichtige Klasse heute, den
  Vorab-Verbrauch in Anbieter-Einheiten (kein Geldbetrag) und dass der Agent den Abruf selbst mit einem zweiten
  Aufruf auslöst; keine Kostenschätzung, keine Nutzerbestätigung.
- **Website/Handbuch**: Funktionsseite Abschnitt `#mcp` mit „MCP in 2 Minuten“ (`#mcp-in-2-minuten`,
  EN `#mcp-in-2-minutes`), Link aus dem Agenten-Kasten der Startseiten; Handbuch `#mcp` DE/EN neu (Ablauf,
  Aktualisieren/Trennen, maskierte Vorschau, Freigabe für alle Programme, CLI `--print-config … --allow
  read,crawl`, Dateiorte). Kein CSS geändert, daher keine neue `?v=`-Kennung. Nicht deployt.

## Nachweis formatbewahrend (Byte-Vergleich der Fixtures)

Gemessen mit einem nicht eingecheckten Hilfstest auf dem Server (gleiche Funktionen wie die Tests, Programmpfad
`/Applications/OpenSpider.app/Contents/MacOS/openspider`). „Anfang/Ende gleich“ = gemeinsame Bytes am Anfang und
Ende von Ausgangsdatei und Ergebnis. „Alt“ = bisheriges `client_config::merged` (Parsen und neu schreiben).

| Fixture | Größe / Zeilen | Anfang gleich | Ende gleich | entfernte Bytes | eingefügte Bytes | Zeilen −/+ | Trennen = Ausgangsdatei | Alt: Zeilen −/+ |
|---|---|---|---|---|---|---|---|---|
| `codex-config.toml` | 2302 B / 85 | 2302 B | 0 B (Anhängen) | 0 | 108 | 0 / 4 | ja | 52 / 77 |
| `claude-code.json` | 2221 B / 85 | 1998 B | 223 B | 0 | 163 | 0 / 7 | ja | 56 / 70 |
| `claude-desktop.json` | 548 B / 21 | 505 B | 43 B | 0 | 174 | 0 / 6 | ja | 18 / 27 |

Anfang + Ende ergeben jeweils die ganze Ausgangsdatei: Es wird nur eingefügt, kein fremdes Byte geändert. Bei
JSON ist die erste eingefügte Zeile das Komma hinter dem vorigen Server (`    }` → `    },` erscheint im Diff als
eine geänderte Zeile). Die Tests prüfen darüber hinaus byte-genau: Aktualisieren eines veralteten Eintrags mitten
in der Codex-Datei (samt Untertabelle `env`, Kommentar), Inline-Eintrag, CRLF, Datei ohne Zeilenumbruch am Ende,
leere/fehlende Datei, einzeiliges JSON, Tabs, vier Leerzeichen, leeres `mcpServers`, fehlendes `mcpServers`.

## Tests (Build-Server)

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| `cargo test --workspace` (mit `~/ws-slot.sh`) | 4327 bestanden, 0 fehlgeschlagen, 51 ignoriert (600 Suiten). Erster Lauf brach im Linker mit „No space left on device“ ab (Platte durch parallele Pakete voll), Wiederholung grün |
| `cargo deny check` | OK (`toml_edit 0.25.17` stand schon im Lockfile; Lockfile-Änderung: eine Zeile, Abhängigkeit von `os-mcp`) |
| `cargo test -p os-mcp --lib client_config` | 13 Tests (Fixtures, Byte-Vergleich, Ablehnungen, Maskierung, Regeln) |
| `cargo test -p os-app --lib mcp::` / `--test mcp_clients` | 3 + 4 Tests (Stale, Rechte, Diff ohne Kontext; Ein-Klick-Ablauf je Programm mit Fixtures, Klasse `crawl`, ungültig/veraltet, Symlink) |
| `npm run -w apps/desktop typecheck` | OK |
| `npm run -w apps/desktop test` | 114 Dateien, 986 Tests bestanden (inkl. CSS-Lint, i18n-Vollständigkeit) |
| `npx playwright test -c e2e/mcp/playwright.config.ts --project=chromium --workers=1 dialog.spec.ts bridge.spec.ts` | 12 bestanden (10,6 min; u. a. „real writer connects all clients in one click, keeps every foreign byte and backs up“ gegen `os-app-bridge`) |
| `node website/check-site.mjs`, `bash scripts/check-neutral.sh` | OK (20 Seiten), keine Fundstellen |

Alle neuen Tests tragen `// spec: AGENT-043` bzw. `AGENT-003`.

**Ersetzte Tests (K2-63, keine Aufweichung):** Die Tests des alten Ablaufs (Pflicht-Vorschau, Bestätigungshaken,
Redaktion ganzer Dokumente im Browser: `configPreview.test.ts`, fünf Fälle in `mcp.test.tsx`, je zwei in
`dialog.spec.ts`/`bridge.spec.ts`, `client_config_requires_preview…` in `cmd_mcp_service.rs`) prüften Verhalten,
das die Nutzerentscheidung abschafft. Ihre Schutzziele sind weiter abgedeckt und verschärft: keine Testgeheimnisse
in der Oberfläche (jetzt kommt die fremde Datei gar nicht mehr dort an; Rust-Tests auf der Maskierung, Vitest/E2E
auf dem DOM), Sicherung byte-gleich, fremde Einträge erhalten (jetzt byte-genau statt inhaltlich), geänderte Datei
wird nicht überschrieben (`Stale`-Unit-Test), ungültige Dateien werden nicht geschrieben.

## Spec-Fragen und Prüfpunkte

1. **Cursor (Spec-Frage):** Die Spec sieht Cursor nur für `--print-config cursor` vor (AGENT-001), nicht in der
   App (AGENT-043). Cursors Installationslink (`cursor://…`-Schema mit kodierter Konfiguration) ist in der Spec
   nicht beschrieben; ihn zu bauen hieße, Format und Parameter zu raten, außerdem bräuchte die App eine Freigabe
   zum Öffnen eines Nicht-`https`-Schemas (heute nur `https`, UI-130). Deshalb ausgelassen. Vorschlag zur
   Entscheidung: statt Link direkt `~/.cursor/mcp.json` (Wurzel-`mcpServers`, global) schreiben – Ort und
   Format stehen schon im Platzierungshinweis von `--print-config cursor`, der JSON-Writer ist dafür fertig und
   getestet (`Client::Cursor` läuft durch denselben Pfad). Auf der Website steht Cursor über die CLI.
2. **Claude-Desktop-Erweiterungspaket (Prüfpunkt, nicht gebaut):** Ein Paket (`.mcpb`, früher `.dxt`) würde den
   Programmpfad und die Freigaben im Paket festschreiben; die Installation geschieht in Claude Desktop per
   Doppelklick. Zu klären: Signatur/Herkunft des Pakets, wie es die installierte Binary findet (macOS/Windows,
   verschobene App), ob Update der App das Paket nachzieht, und ob es gegenüber dem jetzt formatbewahrenden
   Dateischreiben einen Mehrwert hat. Empfehlung: erst nach 1.0 bewerten.
3. **Haken aus = `crawl` entfernen:** Die Spec legt jetzt fest, dass „Verbinden“ mit abgewähltem Haken `crawl`
   global entfernt (sonst könnte ein Agent trotz abgewähltem Haken crawlen). Weil die Freigabe für alle Programme
   gilt, steht das im Hinweistext. Alternative wäre eine Freigabe je Programm über `--allow crawl` im Eintrag –
   bitte bestätigen, ob die globale Lösung gewünscht ist.
4. **Reihenfolge:** Erst Datei, dann Freigabe. Schlägt das Speichern der Freigabe nach geschriebener Datei fehl,
   erscheint eine Fehlermeldung; die Datei ist dann schon verbunden (erneutes „Aktualisieren“ setzt die Freigabe).
5. **Auftragstext „Reiter Zugang“:** Die Klassen-Schalter liegen im Reiter „Werkzeuge“, nicht „Zugang“ (dort nur
   der Token). Der neue Hinweistext verweist auf „Werkzeuge“.
6. **JSON mit BOM:** Der eigene Parser liest eine BOM, `serde_json` (Prüfung) lehnt sie ab; solche Dateien werden
   deshalb als „nicht änderbar“ gemeldet statt geschrieben. Kein bekannter Client schreibt eine BOM.
7. **Historische Checkliste:** `crates/os-app/F05b-checklist.md` (M2-Startliste) nennt noch `mcp.clientConfigWrite`;
   bewusst nicht geändert (historisches Dokument).

## Fremde Dateien (K2-92)

Dateien außerhalb der im Auftrag genannten eigenen Pfade, die das Paket ändern musste:

| Datei | Änderung | Grund |
|---|---|---|
| `Cargo.toml` (Workspace) | eine Abhängigkeit `toml_edit` (nur `parse`) + Kommentar | Spannen für formatbewahrendes TOML |
| `Cargo.lock` | eine Zeile (`toml_edit` in den Abhängigkeiten von `os-mcp`), keine Versionsänderung, kein `cargo update` | folgt aus oben |
| `crates/os-mcp/Cargo.toml` | `toml_edit.workspace = true` | dito |
| `crates/os-app/src/commands/mcp.rs` | `mcp.clientConfigWrite` → `mcp.clientConnect`, `mcp.clientDisconnect`; `preview` ohne Sitzungszustand | neue Commands |
| `crates/os-app/src/dto/mcp.rs`, `crates/os-app/src/dto/mod.rs` | DTOs `McpConnectArgs`, `McpClientState`, `McpClientProblem`, neue `McpClientPreview`/`McpClientWritten`; Exportlisten | IPC-Vertrag |
| `crates/os-app/tests/manifest_contract.rs` | Command-Liste (2 Zeilen) | Vertragstest |
| `crates/os-app/tests/cmd_mcp_service.rs` | alten Vorschau-Test entfernt (Verweis auf neue Datei) | ersetzt durch `tests/mcp_clients.rs` |
| `crates/os-app/tests/mcp_clients.rs` | neu | Integrationstests AGENT-043/003 |
| `apps/desktop/src/ipc/commands.json`, `apps/desktop/src/ipc/mcp.ts`, `apps/desktop/src/ipc/gen/Mcp{ClientArgs,ClientPreview,ClientWritten,ClientProblem,ClientState,ConnectArgs}.ts` | erzeugt (`os-app-manifest`, `gen-ipc.mjs`) | generierte Verträge |
| `apps/desktop/e2e/mcp/{dialog.spec.ts,bridge.spec.ts,harness.tsx}` | neuer Ablauf; Harness `?lang=de`, `setClientPreview` | E2E |
| `tests/fixtures/agent-client-configs/` | neu (README, drei Fixtures) | Testfixture AGENT-043 |
| `website/site/index.html`, `website/site/en/index.html` | nur ein Link im Agenten-Kasten | „verlinkt von der Startseite“ (beta3.md) |
| `website/site/funktionen/index.html`, `website/site/en/features/index.html` | nur Abschnitt MCP | „MCP in 2 Minuten“ |

Nicht angefasst: `crates/os-analyze/**`, `crates/os-store/**`, `crates/os-cli/**`, `crates/os-mcp/src/tools/**`,
Report-Code, andere Handbuch-Abschnitte, `site.css`.

## Nachbesserung nach Fable-Review (needs_work 82/100)

| Befund | Behebung | Commit |
|---|---|---|
| hoch: `.bak` mit umask-Rechten (0644/0664), Originale 0600 mit fremden Geheimnissen | Sicherung wird mit dem Modus der Originaldatei angelegt (`OpenOptionsExt::mode`) und vor dem Schreiben des Inhalts per `set_permissions` umask-unabhängig auf deren Rechte gesetzt (außerhalb von Unix nach dem Schreiben); neue Datei wie bisher. Test `permissions_of_the_previous_file_are_kept` prüft Original, neue Datei und Sicherung für 0600, 0644, 0640 | `cbbb1f0d` |
| mittel: fremde Zeile vor dem Einfügepunkt (Komma) bzw. beliebiger Wurzelwert erscheint im Diff | Vorschau (`preview_diff`, ersetzt `masked_diff`) zeigt eine geänderte Zeile nur vollständig, wenn sie ganz zum eigenen Eintrag gehört (`os_mcp::client_config::own_lines`) **und** wörtlich im neu erzeugten Eintrag vorkommt; Leerzeilen als Leerzeilen; alles andere nur als `…` bzw. `…,`. Beim Testen fiel zusätzlich auf, dass ein vorhandener eigener Eintrag Zusätze des Nutzers mit Geheimnissen (`env`) tragen kann – auch diese Zeilen (und ein alter Programmpfad) erscheinen deshalb nur als `…`. Tests mit Werten ohne Geheimwort (`"lastHandle": "ZZOPAQUE-41"` ohne `mcpServers`, Argument eines fremden Servers in der letzten Zeile, einzeiliges JSON, TOML ohne Zeilenumbruch, `env` im eigenen Eintrag) | `900562c4`, `cbbb1f0d` |
| niedrig a: `--print-config claude-code` nannte `.mcp.json`/`claude mcp add-json` | Hinweis nennt `~/.claude.json` (bzw. `$CLAUDE_CONFIG_DIR/.claude.json`), Abschnitt `mcpServers`, Benutzerebene; Codex `~/.codex/config.toml` bzw. `$CODEX_HOME/config.toml`; beide verweisen auf den App-Dialog. Test `placement_hints_name_the_same_files_as_the_app` | `5046888b` |
| niedrig b: `CODEX_HOME`, `CLAUDE_CONFIG_DIR` | `client_path` verwendet sie, wenn absolut gesetzt; leer/relativ → Standardpfad; mit ausdrücklichem App-Heimordner (portabel, Tests) gelten sie nicht, damit Tests nie echte Benutzerdateien treffen. Reine Funktion mit Test `codex_home_and_claude_config_dir_are_honoured_when_absolute`; Spec AGENT-043 ergänzt, Handbuch DE/EN nennt beide Orte | `674eb4fc`, `cbbb1f0d`, `cb04cb97` |
| niedrig c: Zusätze im eigenen Eintrag gehen beim Aktualisieren verloren | `own_extra_keys` liefert die zusätzlichen Schlüsselnamen (alphabetisch, nie Werte); Vorschau-Feld `extraKeys`; der Dialog zeigt „Der vorhandene Eintrag enthält eigene Zusätze (env, startup_timeout_sec). „Aktualisieren“ ersetzt den ganzen Eintrag; die Zusätze stehen danach nur noch in der Sicherung.“ Tests in `os-mcp`, `tests/mcp_clients.rs`, Vitest | `900562c4`, `cbbb1f0d`, `65f14cc8` |

Entscheidungen übernommen: Freigabe `crawl` bleibt global (in AGENT-003 vermerkt, Hinweistext unverändert);
Cursor nicht in diesem Branch (AGENT-043: „folgt als eigenes Paket“). Spec-Nachträge: `674eb4fc`, `6f5ddf52`.
Die Maskierungsfunktion `client_config::mask` bleibt als Baustein mit ihren Tests erhalten; die Vorschau nutzt
jetzt die strengere Zeilenregel.

Prüfungen nach der Nachbesserung (Build-Server):

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | OK |
| `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| `cargo test --workspace` (mit `~/ws-slot.sh`) | 4333 bestanden, 0 fehlgeschlagen, 51 ignoriert (600 Suiten). Der erste Durchlauf brach erneut mit „No space left on device“ ab (Platte durch parallele Pakete voll); danach eigener `target` gelöscht und mit `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` wiederholt – grün |
| `cargo deny check` | advisories, bans, licenses, sources OK |
| `cargo test -p os-mcp --lib` | 82 bestanden (darunter `own_lines_cover_exactly_the_own_entry`, `user_additions_in_the_own_entry_are_named_without_values`, `placement_hints_name_the_same_files_as_the_app`) |
| `cargo test -p os-app --lib mcp::` / `--test mcp_clients` | 7 + 4 bestanden (Rechte 0600/0644/0640 inkl. Sicherung, „…“-Vorschau, Zusätze im eigenen Eintrag, `CODEX_HOME`/`CLAUDE_CONFIG_DIR`) |
| `npm run -w apps/desktop typecheck` / `npm run -w apps/desktop test` | OK / 114 Dateien, 987 Tests bestanden |
| Playwright MCP-Suite (Chromium, `--workers=1`, `dialog.spec.ts`, `bridge.spec.ts`) | 12 bestanden (10,6 min) |
| `node website/check-site.mjs`, `bash scripts/check-neutral.sh` | OK (20 Seiten), keine Fundstellen |
