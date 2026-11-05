# Bericht B3-FLAKES: wackelnde Tests stabilisieren, zwei kleine Mängel beheben

Paket aus `docs/plan/beta3.md` §6 für den Release-Kandidaten beta.3. Branch `m3/flakes`, Basis `792b1741`
(origin/main). Grundsatz K2-63: keine Schwelle gesenkt, keine Assertion entfernt, kein `#[ignore]`, keine
Retries. Geändert wurden nur Testaufbau (Fixtures, Synchronisation, reine Wartefristen) und zwei echte
Produktmängel (Reparatur, MCP-Sicherung).

## Commits

| Commit | Inhalt |
|---|---|
| `c20099e1` | test: Geheimnis-Scan erst nach dem Schließen aller SQLite-Verbindungen (Punkt 2) |
| `26fac0cc` | test: Renderer-RSS frisch und stabil nach `E02_MEMORY_READY` gemessen (Punkt 3) |
| `7cc2515d` | store: Schreibfehler am Ziel der Reparatur sind kein Datenverlust (Punkt 4) |
| `13f2adf7` | mcp: Sicherung nach gescheitertem Ersetzen entfernen, eigener Fehler; Syntaxmeldung „z. B. Kommentare“ (Punkt 5) |
| `7088daf0` | docs: Handbuch de/en – Projektdatei `.cursor/mcp.json` überdeckt den globalen Eintrag (Punkt 5) |
| `b392561e` | test: Pause kommt garantiert vor dem Crawl-Ende (Punkt 1) |
| `1777ae92` | test: `lifecycle_crash` – Wartefrist 180 s, Tests seriell, sofortiger Abbruch mit Ursache (Nachtrag Orchestrator) |
| (dieser) | docs: Bericht |

## 1 Pause-vs.-Abschluss-Wettlauf

### `beta_resume.rs` `a_new_paused_crawl_is_listed_and_remembered_after_restart` (Windows)

- **Ursache:** Die Fixture (2 Seiten à 50 ms) ist nach rund 100 ms fertig. Erreicht `crawl.pause` die Engine
  später, ist der Crawl „completed“ (Windows-CI f725eeeb und 41965062474: `left: "completed"`, Zeile 47).
- **Warum nicht „Antworten bis zur Pause zurückhalten“:** CRAWL-126 verlangt, dass laufende Anfragen zu Ende
  geführt werden; die Pause wartet also auf jede gehaltene Antwort. Eine Fixture, die bis zur Pause hält,
  blockiert die Pause selbst. Deterministisch ist stattdessen eine Fixture, die vor der Pause nicht enden *kann*.
- **Fix:** Kette aus 400 Seiten, jede verlinkt nur die nächste, 50 ms je Antwort. Der Crawl kann unabhängig von
  der Thread-Zahl frühestens nach 400 × 50 ms = 20 s enden; die Pause folgt unmittelbar auf den Start (Zusage
  CRAWL-126: < 3 s). Alle Assertions unverändert.
- **Nachweis (Server):** alter Stand unter Last (`taskset -c 0`, sechs `yes`-Prozesse auf demselben Kern):
  **0/20 grün, 20/20 `completed`** – das Windows-Fehlerbild ist damit reproduziert. Neuer Stand: 20/20 ohne
  Last, 20/20 unter derselben Last (sechs `yes`) und 20/20 mit drei `yes`.

### `e2e/header/header.spec.ts:69` (zweite Pause, WebKit)

- **Ursache:** wie fix-e (Nachtrag 6) gemessen: 101 URLs à 200 ms sind auf langsamen Runnern vor der zweiten
  Pause fertig; `waitForPhase("Paused")` läuft dann 20 s gegen „Completed“.
- **Fix:** Die Fixture hat kein natürliches Ende: `/` verlinkt `/p/0 … /p/99`, jede `/p/<n>` verlinkt
  `/p/<n+100>`. Die Warteschlange bleibt bei rund 100 URLs, der Crawl kann nur durch Pause/Löschen enden.
  Gleiche Verzögerung (200 ms), gleiche Schritte und Wartefristen, gleiche Assertions.
- **Nachweis (Server, Bridge mit `--features e2e-bridge`, `--workers=1`, `--retries=0`):**
  - neuer Stand, beide Header-Tests, Chromium + WebKit, `--repeat-each=5`: **20/20 grün**;
  - Pausetest unter Last (`taskset -c 0-1` für Playwright, Vite, Bridge und Browser), Chromium + WebKit,
    `--repeat-each=5`: alter Stand **mindestens 6 von 10 rot** (Zeile 65 „Running“ nach dem Fortsetzen nie
    gesehen, Zeile 68 Pause-Knopf fehlt, Zeile 69 „Paused“ nie erreicht – jeweils weil der Crawl schon fertig
    war), neuer Stand **10/10 grün**.

### `e2e/beta/n04-live.spec.ts:74` – kein Pause-Wettlauf

Zeile 74 ist die Latenzprüfung „Detail-Tab Inlinks ≤ 1 s während des Live-Crawls“ (UI-145, Budget 1 s), nicht
die Pause. Belege aus dem Artefakt von Lauf 41953665387 (linux-webkit):

- Der Snapshot nach dem Fehlschlag zeigt `crawl-phase` = „Running“ und das Grid „Inlinks“ **mit Daten** – der
  Crawl war weder fertig noch pausiert, das Grid kam nur zu spät.
- Trace: Der Klick auf den Tab „Inlinks“ selbst dauerte **1 610 ms** (Hauptthread der Seite belegt), danach
  wartete `expect(grid)` die vollen 1 000 ms (+ Overhead 1 268 ms). Gleiches Bild wie fix-e Nachtrag 6
  (Tab-Klick bis 2,5 s).
- Das Pausieren in n04 (Zeile 80) liegt bei frühestens 33 s Crawl-Dauer (5 000 URLs, 150 URL/s) weit vor dem
  Ende; dort ist kein Wettlauf beobachtet.

Nicht geändert: Das 1-s-Budget bleibt (K2-63), eine Fixture-Änderung behebt Hauptthread-Last nicht. Empfehlung:
Backlog M2-N32 „Live-Latenz Detailfenster“ als eigenes Paket vor beta.4 (Ursache WebKit-Hauptthread im
Klickpfad, nicht die Kernabfrage).

### Spec-Frage CRAWL-001/CRAWL-126: Pause und Crawl-Ende fallen zusammen

Die Pause-Semantik steht in CRAWL-126 und UI-003 (CRAWL-001 beschreibt nur den Start); der Vorschlag gehört
daher zu CRAWL-126. Heutiges Produktverhalten (Code gelesen, `os-app/src/session/control.rs` `pause_or_stop`,
`settle`; `os-engine/src/crawl.rs` Steuerschleife; `Header.tsx`):

1. Ist der Crawl schon von selbst fertig, wenn die Pause ankommt (Engine beendet, Warteschlange leer), wird
   die Pause ohne Fehler quittiert; Endzustand „Fertig“ (`completed`, Grund `QueueEmpty`), Ereignis
   `Completed`, eine eingestellte Crawl-Analyse startet. Seit N18 kein Fehlerdialog mehr.
2. Sieht die Engine die Pause, bevor sie „Warteschlange leer“ feststellt (auch wenn nur noch die letzten
   Anfragen laufen), endet sie mit „Pausiert“, ggf. mit leerer Warteschlange; „Fortsetzen“ endet dann sofort
   regulär mit „Fertig“.
3. Die Kopfleiste verlässt „Pausiere …“, sobald der Kern einen anderen Zustand als „Läuft“ meldet – also auch
   bei „Fertig“; sie hängt nicht.

Das Verhalten ist widerspruchsfrei; Produktcode wurde dafür nicht geändert. **Vorschlag OS-Festlegung
(CRAWL-126, Akzeptanzkriterium):** „Fallen Pause und natürliches Crawl-Ende zusammen, gilt der Zustand, den die
Engine zuerst feststellt: War der Crawl schon fertig, ist der Endzustand ‚Fertig‘ und die Pause wird ohne
Meldung quittiert (die automatische Crawl-Analyse startet wie nach einem regulären Ende); hat die Engine die
Pause zuerst gesehen, ist der Zustand ‚Pausiert‘, und ‚Fortsetzen‘ endet sofort mit ‚Fertig‘. In keinem Fall
erscheint eine Fehlermeldung, und die Kopfleiste verlässt den Zwischenzustand ‚Pausiere …‘ mit dem gemeldeten
Zustand.“ Tests, die „Pausiert“ erwarten, brauchen deshalb eine Fixture, die vor der Pause nicht enden kann.

## 2 Windows: Datei-Sperre beim Durchsuchen (`cmd_profile_runtime.rs:270`)

- **Ursache:** Beim Start der App läuft die Aufbewahrung (`start_retention`) im Hintergrund und öffnet auf der
  Wartungsspur der Schreibwarteschlange `crawls/.registry.sqlite` und `crawls/.index.sqlite` (WAL). Der Test
  durchsuchte das App-Verzeichnis, solange die App lebte. Unter Windows hält eine offene WAL-Verbindung
  Bytebereichssperren in der `-shm`-Datei; `fs::read` scheitert dann mit Os-Fehler 33. Auf dem Server liegen zum
  Scanzeitpunkt nur `.registry.sqlite`/`.index.sqlite` ohne `-wal`/`-shm` vor (Linux war meist schon fertig,
  und Linux sperrt nicht verbindlich).
- **Fix:** Vor dem Scan `app.write_queue().shutdown()` (arbeitet eingereihte Aufträge ab, beendet Haupt- und
  Wartungsspur per `join`, schließt dabei alle Verbindungen; spätere Aufträge werden abgewiesen) und `drop(app)`.
  Andere SQLite-Verbindungen hält die App ohne geöffneten Crawl nicht (einzige `Connection::open` in `os-app`
  ist die Schreibwarteschlange; `Manager` lebt nur in deren Aufträgen). Der Scan liest weiterhin jede Datei.
- **Nachweis:** 20/20 grün auf dem Server; Windows ist hier nicht ausführbar, Review s. o. Die Windows-CI nach
  dem Merge ist der eigentliche Nachweis.

## 3 `memory_process.rs:89` `renderer_child_rss_triggers_pause_when_engine_alone_is_below_limit`

- **Befund:** CI linux-core las `renderer_bytes` 183 693 312 (175 MiB), auf dem Server sind es stets
  214–215 MiB (200 MiB Allokation + Testbinary).
- **Ursachenanalyse:** Die Vermutung „Seiten nicht berührt“ trifft nicht zu: Das Kind schreibt vor
  `E02_MEMORY_READY` in jede 4-KiB-Seite. Die zweite Vermutung „Sampling zu früh“ habe ich mit einer
  Diagnosevariante (Allokation künstlich auf 1–2,7 s verlangsamt, nur im Server-Worktree) geprüft: Der Test
  läuft auf dem Current-Thread-Runtime und blockiert beim Lesen von `READY`; die Engine misst während der
  Allokation also nicht, und die auslösende Messung lag in allen Läufen rund 1 s **nach** `READY`
  (2 × 20 Läufe des alten Tests mit verlangsamter Allokation plus 16 instrumentierte Läufe, 0 rot). `memory::sample()` liefert allerdings bis zu 1 s alte Werte aus einem Zwischenspeicher,
  und eine Messung, die vor `READY` begann, kann zurückkommen. Die genaue Ursache der 175 MiB auf dem
  CI-Runner ließ sich nicht reproduzieren.
- **Fix (Testaufbau, Grenzen unverändert):** Gemessen wird erst nach Ablauf des Zwischenspeichers
  (1,5 s nach `READY`) und so lange neu (höchstens 6 s), bis zwei aufeinanderfolgende frische Messungen der
  Kindprozesse übereinstimmen (± 1 MiB). Damit zählt nur ein stabiles RSS nach `READY`. Liegt es wirklich unter
  190 MiB, schlägt die unveränderte Assertion weiterhin fehl – dann liefert das Protokoll den stabilen Wert.
- **Nachweis:** 20/20 ohne Last, 20/20 unter `taskset -c 0` mit drei `yes`-Prozessen. Offen: Bleibt der
  CI-Fehler, ist die Messung selbst (sysinfo/Runner) zu untersuchen, nicht die Schwelle.

## 4 `os-store` Reparatur: Schreibfehler am Ziel (`files.rs`)

- **Ursache (Fable-Befund):** `Rescue::store` zählte jeden Fehler am Ziel (`unchecked_transaction`, `prepare`,
  `execute`, `commit`) als verlorene Zeilen. Ein vorübergehender Fehler des Ziels sah damit aus wie Datenverlust
  der Quelle (unter Last 1 362 statt ≥ 1 900 URLs).
- **Fix:** `store` liefert `Result`; ein Schreibblock läuft in einer Transaktion (`store_block`).
  - Vorübergehende Zielfehler (`SQLITE_BUSY`, `SQLITE_LOCKED`, `SQLITE_IOERR`): der zurückgerollte Block wird bis
    zu 4-mal geschrieben (Pausen 50/100/200 ms).
  - Jeder andere oder bleibende Zielfehler bricht die Reparatur mit `StoreError::Sqlite` ab; `repair_copy`
    entfernt die halbe Kopie (bestehender Pfad).
  - Als verloren zählen nur noch Lesefehler der Quelle (unverändert, mit Halbierung) und Zeilen, die das Schema
    des Ziels abweist (`SQLITE_CONSTRAINT`, `MISMATCH`, `TOOBIG`): deren Werte stammen aus der beschädigten
    Quelle und sind nicht zu retten.
- **Tests (`files::repair_target_tests`, OUT-230):** bleibende Sperre am Ziel (zweite Verbindung mit
  `BEGIN EXCLUSIVE`, `busy_timeout` 0) → `Err(DatabaseBusy)` nach genau 3 Wiederholungen, nichts gezählt, Block
  zurückgerollt, auch ohne Schlüsselspalte; vorübergehende Sperre (endet vor der ersten Wiederholung) → 25/25
  kopiert, 0 verloren; vom Schema abgewiesene Zeile → 2 kopiert, 1 verloren, keine Wiederholung. Deterministisch
  über einen Rückruf vor jeder Wiederholung statt über Zeitfenster.
- **Nachweis:** neue Tests grün; `crawl_files::damaged_file_is_detected_and_repaired_into_a_new_copy` 20/20.

## 5 MCP-Sicherung bei Fehlschlag (`os-app/src/mcp/clients.rs`)

- **Fix:** `write_guarded` → `write_guarded_with` mit austauschbarem letztem Schritt (`persist`). Scheitert
  nach dem Anlegen der Sicherung ein Schritt (Schreiben der Sicherung selbst, temporäre Datei, Rechte,
  `persist`), bleibt die Originaldatei unverändert, die Sicherung wird bestmöglich gelöscht und der Fehler
  `Io`/`mcp-client-write-failed` (wiederholbar) gemeldet. Lässt sich die Sicherung nicht löschen, lautet der
  Schlüssel `mcp-client-write-failed-backup` mit dem Pfad, weil dort fremde Geheimnisse liegen. Details
  enthalten nie Dateiinhalte.
- **i18n de/en:** neue Schlüssel `mcp-client-write-failed`, `mcp-client-write-failed-backup`;
  `mcp-client-problem-syntax` nennt jetzt „(Zeile n; z. B. Kommentare)“ bzw. „(line n; e.g. comments)“.
  `ClientsPanel` zeigt beide neuen Meldungen als feste Meldungen (mit `path`).
- **Handbuch de/en, Abschnitt MCP:** „Eine projektbezogene Datei `.cursor/mcp.json` überdeckt in Cursor einen
  gleichnamigen Eintrag der globalen Datei, die OpenSpider schreibt.“
- **Tests:** `a_failed_replace_removes_the_backup_and_reports_its_own_error` (simulierter `persist`-Fehler:
  Sicherung lag mit dem Originalinhalt vor, ist danach weg, Original unverändert, keine Reste, kein Testgeheimnis
  im Detail), `a_backup_that_cannot_be_removed_is_named_in_the_error` (Sicherung plattformneutral unlöschbar
  gemacht); Vitest: beide Meldungen im Panel, deutsche Texte, angepasste Syntaxmeldung.

## 6 macOS-Zeitgrenzen in `render_chromium.rs` (nur Analyse)

- `requires_chromium_screenshot_wait_captures_late_paint_pixels` (~635): 1 000 ms Färbung, 1 500 ms bzw. 0 ms
  Wartezeit sind **wörtlich Teil von CRAWL-082** (Kriterium „Inhalt, der 1 000 ms nach der Größenänderung
  nachgeladen wird, ist bei Wartezeit 1 500 ms im Bild sichtbar“, K2-82). Nicht geändert. Deterministischer
  ginge es nur im Produkt: Die Wartezeit beginnt heute direkt nach `Emulation.setDeviceMetricsOverride`
  (`os-render/src/page.rs`), nicht wenn die Seite das `resize` verarbeitet hat. Vorschlag für ein eigenes
  Render-Paket: vor dem Warten zwei `requestAnimationFrame` der Seite abwarten (begrenzt), dann zählt die
  Wartezeit ab der tatsächlichen Größenänderung. Das ist eine Verhaltensänderung des Renderers und gehört nicht
  in ein Flake-Paket kurz vor dem Merge-Schluss.
- `requires_chromium_emulates_devices_and_expands_google_viewport` (~213, kein `/pixel.png` im festen
  Viewport): keine zeitliche Grenze im Test, sondern eine Abwesenheitsprüfung nach `reset_request_log()`. Ein
  Fehlerprotokoll lag in den durchsuchten CI-Läufen nicht vor; die Testsite protokolliert beim Eingang, und
  `real_w` der ersten Darstellung belegt, dass deren Bildabruf vor dem Zurücksetzen einging. Ohne Fehlerbild
  keine Änderung; offen (siehe unten).

## 7 Weitere Flakes (nur Ursachenblick)

- **SMTP-Fixture (`cli_tests.rs` ~619):** Die Fixture wartet 30 s auf zwei Nachrichten. Im Fehlerbild
  (Backlog 03.11.) meldete der Client „Connection refused“; eine Nachricht kam nie an, die Fixture lief in ihr
  Zeitlimit. In den durchsuchten Läufen vom 02.11. scheiterten dieselben Tests stattdessen an der Startwarnung
  „Wenig freier Speicherplatz (0,9 GB)“ des Runners (CRAWL-124), nicht am SMTP. Keine Änderung; Ursache des
  „Refused“ offen.
- **`robots_step_m2`:** nur im parallelen Workspace-Lauf (`QueryReturnedNoRows`); nicht untersucht.
- **Intel:** nicht untersucht (niedrigste Priorität).

## Nachtrag Orchestrator: `lifecycle_crash.rs:133` (macOS-arm64, Lauf 41965062474)

- **Befund:** „das Kind meldet Fortschritt: Timeout“ nach 105 s Testdauer. Nominal braucht das Kind für
  700 URLs (10 ms je Antwort, 4 Threads) rund 2 s; davor lief im selben Prozess der Referenz-Crawl, und im selben
  Testbinary lief der zweite Kind-Crawl-Test parallel. Im Log steht kein Fehler des Kindes auf stderr; ob es vorzeitig `DONE` meldete (etwa durch eine automatische
  Pause), wertete der alte Test nicht aus. Am wahrscheinlichsten ist ein stark ausgelasteter Runner.
- **Fix:** Wartefrist bis zum Fortschrittspunkt 60 s → 180 s (auch für den zweiten Test); beide Tests laufen
  über eine `tokio::sync::Mutex` nacheinander statt gleichzeitig (Testaufbau); meldet das Kind `DONE` vor 700 URLs
  (etwa eine automatische Pause durch Speicher- oder Plattenwächter), scheitert der Test sofort mit dieser Zeile
  statt bis zur Frist zu warten; die Timeout-Meldung nennt den letzten Fortschritt. Abbruchpunkt (700),
  Konsistenz-, Heartbeat-, Duplikat- und Fortsetzungsprüfungen unverändert.
- **Nachweis:** 5/5 grün auf dem Server (Binary auf Kerne 4–15 gesetzt, 39–144 s je Lauf). Phasenmessung in einer
  Diagnosekopie (nur Server): Referenz-Crawl 25–67 s für 1 500 URLs (nominal ≈ 4 s), das Kind erreicht 700 URLs
  rund 4 s nach seinem Start, Fortsetzen bis Ende weitere 25 s. Der Test ist also schon auf dem Server langsam
  und schwankend; auf dem macOS-Runner brauchte das Kind mehr als 60 s. Warum der Referenz-Crawl das Vielfache der
  nominalen Zeit braucht (vermutlich das Ende des Crawls: 5xx-/404-Anteile, Abschluss-Checkpoint), ist ein
  offener Punkt; die Frist von 180 s deckt die gemessene Spanne mit Reserve.

## Testergebnisse (Server, Stand des Branches)

Server `openspider-build`, Worktree `m3-flakes`, Stand `1777ae92` (Bericht danach ohne Codeänderung).

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün |
| `cargo test --workspace` (nach `ws-slot`) | grün: 4 366 bestanden, 0 Fehler, 51 bestehende Ignores |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` (Vitest) | grün: 1 005/1 005 in 116 Dateien |
| `node website/check-site.mjs` | OK (20 Seiten) |
| `header.spec.ts` Chromium + WebKit, `--repeat-each=5`, `--workers=1`, `--retries=0` | 20/20 |
| Pausetest `header.spec.ts` unter `taskset -c 0-1`, Chromium + WebKit, 5× | alt ≥ 6/10 rot, neu 10/10 |
| `beta_resume` 20× / 20× unter Last (`taskset -c 0`, 3 bzw. 6 `yes`) | 20/20 / 20/20 / 20/20; alt unter 6 `yes`: 0/20 |
| `cmd_profile_runtime::secret_references…` 20× / 20× unter Last | 20/20 / 20/20 |
| `memory_process::renderer_child_rss…` 20× / 20× unter Last | 20/20 / 20/20 |
| `crawl_files::damaged_file…` 20× / 20× unter Last (6 `yes` auf Kern 0) | 20/20 / 20/20 |
| `files::repair_target_tests` (3 neu) | grün |
| `mcp::clients` Unit-Tests (2 neu) | 9/9 |
| `lifecycle_crash` (beide Tests) 5× | 5/5 |
| Vitest `features/mcp` | 29/29 |

Nicht ausführbar: Windows (Punkte 1/2/5) und macOS (Nachtrag `lifecycle_crash`, Punkt 6); dort zählt die CI nach
dem Merge. `n04-live.spec.ts` ist unverändert und wurde nicht erneut gemessen.

## Fremde Dateien (K2-92)

Das Paket hat keinen eigenen Crate; „fremd“ sind alle Dateien außerhalb der Tests der genannten Flakes.

| Datei | Änderung | Grund |
|---|---|---|
| `crates/os-store/src/files.rs` | Reparatur: Zielfehler wiederholen/abbrechen, Tests | Punkt 4 (Produktmangel) |
| `crates/os-app/src/mcp/clients.rs` | Sicherung bei gescheitertem Ersetzen entfernen, eigener Fehler, Tests | Punkt 5 (Produktmangel) |
| `apps/desktop/src/features/mcp/ClientsPanel.tsx`, `i18n/de.ftl`, `i18n/en.ftl`, `mcp.test.tsx` | zwei neue feste Meldungen, Syntaxmeldung | Punkt 5 |
| `website/site/handbuch/index.html`, `website/site/en/manual/index.html` | ein Satz im Abschnitt `#mcp` | Punkt 5 |
| `crates/os-app/tests/cmd_profile_runtime.rs`, `beta_resume.rs` | Testaufbau | Punkte 1, 2 |
| `crates/os-engine/tests/memory_process.rs`, `lifecycle_crash.rs` | Testaufbau, Wartefrist | Punkt 3, Nachtrag |
| `apps/desktop/e2e/header/header.spec.ts` | Fixture ohne natürliches Ende | Punkt 1 |

## Spec-Fragen und offene Punkte

1. **CRAWL-126 (Auftrag: CRAWL-001):** Festlegung „Pause und Crawl-Ende fallen zusammen“, Vorschlag oben.
2. **n04-live:74** ist eine Latenz- und keine Pause-Frage; Budget unverändert, Folgepaket M2-N32 empfohlen.
3. **memory_process:** Ursache der 175 MiB auf linux-core nicht reproduziert; Fix sichert eine stabile, frische
   Messung. Tritt der Fehler wieder auf, steht der stabile Wert im Fehlertext.
4. **render_chromium late_paint:** Zeitgrenzen sind Spec-Bestandteil (CRAWL-082); Produktvorschlag
   „Wartezeit ab verarbeitetem `resize`“ als eigenes Paket. `pixel.png`: ohne Fehlerprotokoll keine Änderung.
5. **SMTP „Connection refused“, `robots_step_m2`, Intel:** offen.
6. **Windows:** Punkte 1 (beta_resume), 2 und 5 (Löschen der Sicherung unter Windows) sind nur per Review und
   plattformneutralem Test geprüft; die Windows-CI nach dem Merge ist der Nachweis.
7. **AGENT-043 (Fable-Nacharbeit):** Verhalten „Ersetzen scheitert nach der Sicherung“ fehlte in der Spec; vom
   Orchestrator als Nachtrag samt Akzeptanzkriterium eingetragen, ebenso die Festlegung zu CRAWL-126 (Punkt 1) und die
   Zählweise der Reparatur in OUT-230.
8. **Referenz-Crawl in `lifecycle_crash`:** 25–67 s statt ≈4 s auf dem Server, Ursache offen; Folgepunkt im Backlog
   (Verdacht Crawl-Ende: 5xx-Backoff oder Abschluss-Checkpoint). Zweiter Test meldet bei Zeitüberschreitung jetzt
   die letzte Zeile des Kindes.

## Server und Aufräumen

- Kein Push, kein Workflow, kein Deploy; `gh` nur lesend (Logs der Läufe 41947513366, 41950839219, 41953665387,
  41965062474, ältere CI-Läufe; Artefakt `playwright-report-linux-webkit` von 41953665387 nur lokal im
  Scratchpad ausgewertet).
- Kein `cargo update`, keine neue Abhängigkeit; Tokio 1.53.3 und Lockfile unverändert.
- Diagnosekopien (verlangsamte Allokation, Phasenmessung `lifecycle_crash`, alte Stände von `beta_resume` und
  `header.spec.ts` für den Vorher-Vergleich) lagen nur im Server-Worktree und sind zurückgesetzt.
- `target/` und Playwright-Ausgaben des Server-Worktrees sind nach der Abgabe gelöscht.
