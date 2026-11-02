# M2 Integrationsrunde 23

Stand: 01.11.2026. Branch `m2/int-23`, Basis `bc964d71` (origin/main: int-22 abgeschlossen). Bauen und Prüfen auf dem Linux-Server (x86_64, Kernel 7.0.0-30-generic, Rust 1.99.1, Node 22.23.4, Playwright 1.64.1); Git-Operationen lokal. Kein Push, kein Rebase, kein `cargo update` (Tokio bleibt 1.53.3), Schema bleibt v5.

## Integration

| Reihenfolge | Paket | Quellstand | Merge | Fable |
|---|---|---|---|---|
| 1 | U31 (`build/m2-u31`) | `84de64dc` | `c093e259` | approved mit Auflagen |
| 2 | N21 (`m2/n21`) | `cf7af3d1` | `5347d8f6` | approved nach Nachprüfung |
| 3 | U31-MAC (`m2/u31-mac`, auf N21 `d09945c3`) | `c96fe256` | `55568ec5` | approved mit Auflagen (zu U31) |
| 4 | N22 (`build/m2-n22`) | `8a714fbd` | `c7a9f5f4` | approved mit Auflagen |
| 5 | N23 (`m2/n23`) | `7ed36ac3` | `64874b03` | approved mit Auflagen, Auflagen erfüllt |
| 6 | A03-plan (`m2/a03-plan`, nur Doku, A03-Build `bc964d71`) | `d3883e12` | `dc21cb49` | Abnahmeunterlagen |

Alle sechs Merges liefen mit `--no-ff` textuell konfliktfrei, wie in der Probe-Zusammenführung. N23 berührt `grid/DataGrid.tsx` nur im Zeilen-Rendering (Batch-Textmessung); N21 ändert diese Datei nicht. Beide Stände bleiben unverändert erhalten.

## FIX-E-Port aus der M3-Linie

Quelle `origin/m3/int-e..m3/fix-e` (30 Commits, Spitze `d9e96d9f`, Basis M3 int-e). Port per `git cherry-pick -x`: zuerst die übrigen FIX-E-Commits in Commit-Reihenfolge, danach der Pausen-/Merge-Kern in der von Fable empfohlenen Reihenfolge (`94db9704` dadurch nach `218c2a44`).

| Quelle | M2-Commit | Inhalt | Status |
|---|---|---|---|
| `04e552e8` | `93dda2dc` | Future-Stacks im Worker begrenzt (`Box::pin`) | portiert, Konflikt `worker.rs` |
| `208d482b` | `85d988c5` | Link-Generationen einmal materialisieren | portiert |
| `682663a9` | `87bba7c6` | Detail-Erstzeilen vor dem Grid laden | portiert |
| `2a04c504` | `7f57df5d` | Detailpanels stabil bei Fortschritts-Ticks | portiert |
| `d861cb54` | `c5bc2a9d` | erstes Link-Fenster ohne zweiten Lesezugriff | portiert |
| `ff16c5cb` | `cf287555` | `ViewHandle.initial_rows` (auch `compare.rs`, Stub, `gen/ViewHandle.ts`) | portiert |
| `b3b1ecd1` | `8b8808a0` | Auswahl-Entprellung nur für URL-Wechsel | portiert |
| `b1ed83de` | `5e95b706` | erste Overview-Antwort sofort veröffentlichen | portiert, Konflikt `sidebar.test.tsx` |
| `cec1f830` | `c1c0fbcf` | große Futures in kurzlebigen Frames bauen (`heap()`) | portiert, Konflikt `worker.rs` |
| `b2255ba3` | `e8b49f49` | jede Anfrage aus eigenem Task | portiert |
| `655c0d0e` | `80460a1f` | MCP `start_crawl`/`crawl_status` eigene Frames | portiert |
| `6a47d809` | `5ec79561` | MCP-Crawlstart geteilt, Read-Pool-Ergebnisse geboxt | portiert |
| `e3d33150` | `4bd03ad0` | Spec CRAWL-126: Quittung nach dauerhaftem Commit, WAL-Merge im Hintergrund | portiert |
| `858062d0` | `69e74c5a` | begrenzter Abschluss-Checkpoint, abbrechbarer WAL-Merge | portiert, angepasst (siehe unten) |
| `1505447e` | `1acd5f10` | Leser-Wartezeit per Wanduhr begrenzt | portiert |
| `218c2a44` | `d54c85d4` | End-Batch dauerhaft vor Pause/Ende | portiert, angepasst (siehe unten) |
| `94db9704` | `d746b2b1` | Start-Slot abbrechbar durch Pause | portiert |
| `c3d01b46` | `788dbd17` | App: WAL-Merge im Hintergrund nach Pause/Ende | portiert |
| `0953ecf0` | `30a4515a` | Statuszeile „Pausiert – Datei wird zusammengeführt“ | portiert |
| `86bb39db` | `ff8e915b` | Spec CRAWL-126: Abnahmepunkte Merge, Beenden, Zeigen | portiert |
| `deba2e77` | `d4271ff5` | „Im Finder zeigen“ wartet höchstens 2 s | portiert; **offen bis Fix-E-Regressionsfix** |
| `152f8c55` | `340f9e39` | Busy-Timeout bei Panic, kein doppelter Abschluss-Checkpoint | portiert, Konflikt `writer_thread.rs`; **offen bis Fix-E-Regressionsfix** |
| `a8a82e6c`, `e4fb1fe1`, `2f4a4cab`, `7cca5846`, `4963182a`, `324cd5a7`, `5f500611`, `d9e96d9f` | – | M3-Bericht `docs/plan/M3-reports/fix-e.md` | nicht übernommen (reiner M3-Bericht); Inhalt unten zusammengefasst |

Kein Commit hing an M3-only-Code so, dass er ausgelassen werden musste. Zwei Stellen setzten M3-Stand voraus und wurden im Anpassungscommit `77a22061` gelöst:

- `os_store::open::merge_wal` rief den M3-Helfer `open_sqlite` (lange Windows-Pfade, M3-Release) auf. In M2 jetzt `Connection::open_with_flags` mit denselben Flags und derselben Fehlerumwandlung.
- Der neue `Msg::Durable`-Zweig im Writer übergab `write_out` vier Argumente. N20 hat dort die Speicherbelastungen (`charges`) und die Stapelgröße ergänzt. Der Zweig übergibt jetzt `&mut charges` und setzt `batch_bytes` zurück, wie `Flush` und `Checkpoint`.

`dto_fixtures`: `CrawlStatus.merging` und `ViewHandle.initial_rows` tragen bereits `#[serde(default, skip_serializing_if = "Option::is_none")]` aus den portierten Commits; die Wire-Fixtures bleiben unverändert, der Test ist grün.

### Offen bis Fix-E-Regressionsfix

Hinweis des Orchestrators während dieser Runde: Auf `m3/fix-e` `d9e96d9f` ist `ui-e2e` rot (`beta/n04-live.spec.ts:74` Inlinks/Outlinks-Grid nicht binnen 1 s auf allen drei Plattformen; `header/header.spec.ts:69` zweites Pausieren erreicht „Paused“ nicht), auf `5f500611` grün. Verdacht: `152f8c55` (Merker `checkpointed` im Writer) oder `deba2e77`. Beide waren zu diesem Zeitpunkt schon portiert und bleiben drin. Ein separater Agent untersucht die Ursache in `c-fix-e`; der Orchestrator zieht den Fix nach.

Auf dem integrierten M2-Stand (Linux, `CI=true`, echte Bridge, 2 Worker, eigener Netzwerk-Namensraum) sind beide Specs grün:

- Volle Browserprüfung: `n04-live.spec.ts:17` Chromium 1,1 min, WebKit 1,3 min, beide bestanden.
- Gezielte Wiederholung `e2e/header/header.spec.ts` + `e2e/beta/n04-live.spec.ts`, Chromium + WebKit, `--repeat-each=3`: **18/18 bestanden**, 0 flaky (4,2 min).

Die CI-Matrix (macOS, Windows, linux-webkit) steht aus.

### Konflikte beim Port

Alle Konflikte wurden so gelöst, dass beide Seiten erhalten bleiben:

| Datei | Commit | M2-Seite | FIX-E-Seite | Lösung |
|---|---|---|---|---|
| `crates/os-engine/src/worker.rs` | `04e552e8` | N20: `parse_stage`-Speichererfassung und `send_admitted(batch, fetched_charge)` | `Box::pin(assemble(…))` | `Box::pin(assemble(…))`, danach unverändert `parse_stage.resize(…)` |
| `crates/os-engine/src/worker.rs` | `cec1f830` | wie oben | `crate::heap(|| assemble(…))`, `crate::heap(|| sh.writer.send(batch))` | `crate::heap(|| assemble(…))`, `parse_stage.resize(…)`, `crate::heap(|| sh.writer.send_admitted(batch, fetched_charge))`, `drop(parse_stage)` |
| `apps/desktop/src/features/sidebar/frame/sidebar.test.tsx` | `b1ed83de` | N21: zwei Tests zur Zählanzeige (UI-141/145) | neuer Test „first overview response“ (UI-018/061) | alle drei Tests nacheinander |
| `crates/os-engine/src/writer_thread.rs` | `152f8c55` | N20: `Msg::Batch(b, charge)` mit 32-MiB-Stapelgrenze; `write_out(…, &mut charges)` | `checkpointed`-Merker | N20-Zweig mit `checkpointed = false` am Anfang; Abschluss `write_out(…, &mut charges)` und `if !checkpointed && …` |

Die Future-Größentests (`url_worker_futures_have_bounded_size`, `crawl_future_size`) sind mit der N20-Speichererfassung grün.

### Zusammenfassung des M3-Berichts FIX-E

- **Stacküberlauf:** MCP-Crawl unter Windows (`STATUS_STACK_OVERFLOW`) und macOS (SIGBUS) durch große, ineinander geschachtelte Async-Zustandsmaschinen im Worker- und MCP-Startpfad. Lösung über `Box::pin` und den nicht inlinierten Helfer `heap()`; Produkt-Stackgrößen und Abhängigkeiten unverändert. Regressionstest mit 320-KiB-Runtime und Future-Größengrenzen (`run_item` ≤ 8 KiB, Runner ≤ 16 KiB).
- **Live-Link-Details und Overview:** Link-Auswahl wird bei Open/Update einmal materialisiert. Das erste Fenster (≤ 200 Zeilen) kommt mit der Open-Antwort (`initial_rows`), das Grid erscheint erst mit geladenen Zeilen. Die erste Overview-Antwort wird sofort veröffentlicht, statt im zurückgestellten Render unter Tick-Last zu verhungern (Ursache der roten WebKit-Beta-Tests `overview.spec.ts`).
- **Pause (CRAWL-126, Option a):** Quittung nach dauerhaftem Commit des End-Batches (`synchronous = FULL`). Abschluss-Checkpoint wartet höchstens 100 ms auf Leser, eigener Busy-Handler mit Wanduhr-Frist. WAL-Merge im Hintergrund, abbrechbar durch Fortsetzen/Löschen. Beenden wartet im 3-s-Budget, „Zeigen“ höchstens 2 s mit Hinweis. Statuszeile zeigt den Merge an. M3-Messung (zwei Kerne, WebKit, `control.spec.ts` 10×): Pausenlatenz Median 2,07/2,13 s statt 2,0–2,7 s, Maximum 2,5 s statt 3,2 s; Budget 3 s unverändert.
- **Grenzen laut M3-Bericht:** Unter „Maximale Geschwindigkeit“ schreiben Hintergrund-Checkpoints während des Crawls weiter ohne fsync (bestehendes Profilverhalten). Die native macOS-Bestätigung des SIGBUS-Befunds und die Mac-mini-WKWebView-Prüfung lagen beim Orchestrator. `auth/bridge.spec.ts:26` (linux-chromium) ist ein vorbestehender Kaltstart-Flake ohne Bezug zu FIX-E.

## Auflagen und Nachträge

**a) U31 (Fable):**

- UI-144-Budgets dezimal: `perf.bench.ts` Soak-Spitze `400_000_000`, `memory.bench.ts` Visualisierungszuwachs `300_000_000` B (Testname „300 MB“). Gleiche Grenze auch im nativen Windows-Soak `e2e-b/perf/run.mjs`, damit beide Harness-Teile dieselbe Spec-Grenze prüfen. Alle bekannten Mac-Werte bestehen (Kombilauf +260.685.824 B, Soak-Spitze 243.302.400 B).
- `playwright.config.ts`: `testIgnore` der Hauptprojekte um `perf/**/*.test.mjs` und `*.nodetest.mts` ergänzt; `report.test.mjs` läuft nicht mehr in `npm run e2e`.
- `e2e-b/perf/run.mjs`: `// spec: UI-147 [P1]` steht jetzt vor der Kaltstartmessung, der Soak-Block trägt `// spec: UI-144`. Veralteter SLACK-Kommentar in `viz.bench.ts:3-5` entfernt (die Budgets sind strikt).
- `u31.md`: neuer Abschnitt „Referenz A – Kombilauf Mac mini“ (Stand `82249d91`) mit den Messwerten, dem Kaltlauf-Hinweis, Rot/Offen und der K2-65-Abhängigkeit der Nightly-Abnahme (`visibilityState=hidden`; macOS-Nightly mit `OS_PERF_REQUIRE_NATIVE=1` deterministisch rot bis zur S02-Reparatur).

**b) Planung:** `owns` für N21 (K2-92-Tabelle in `n21.md`), N22 (Permutation, Suche, additives `sort_key` in `collation.rs`, Paritätstest, `os-loadtest/diagnostics/**`, Bericht und Daten) und N23 (K2-92-Tabelle in `n23.md`). U31 umfasst U31-MAC; neuer Eintrag **M2-FIXE-PORT** (Welle 4n, Zusatz-ID CRAWL-126, `depends_on` M2-N02). Zwölf gewollte Überschneidungen in `ERLAUBT`, je mit Grund (sequenzielle Integration auf bereits integrierten Paketen bzw. neuer Diagnose-Ordner). M2.md Welle 4n um den Abschnitt „Integrationsrunde 23“ ergänzt. `M2-check.py`: OK, 138 Pakete, 590 `spec_ids`.

**c)** `int-22.md`: Satz „werden nach Abschluss ergänzt“ ersetzt durch den Verweis auf den Orchestrator-Abschnitt.

## Prüfbedingungen

`CARGO_INCREMENTAL=0`, Dev-/Test-Debug `line-tables-only`, `OPENSPIDER_SECRET_STORE=memory`. Schwere Befehle mit `timeout 3600s nice -n 10`, Cargo `-j 6 --locked`, vor dem Workspace-Lauf `~/ws-slot.sh`. Playwright mit `CI=true`, zwei Workern, echter Bridge. Auf dem geteilten Server hielt ein fremder Vite-Dev-Server (Worktree `m2-n23`) Port 1420. Er wurde nicht angefasst. Die Browserprüfungen liefen stattdessen in einem eigenen Netzwerk-Namensraum (`unshare -rn`, darin Benutzer 1000), mit eigenem Loopback und eigenem Port 1420.

## Prüfergebnisse

| Prüfung | Ergebnis | Dauer |
|---|---|---|
| `cargo fmt --all --check` | grün | – |
| `cargo clippy --workspace --all-targets -- -D warnings`, ohne Ausschluss (inkl. Desktop) | grün nach Anpassungscommit `77a22061`; vorher zwei Kompilierfehler des Ports (`open_sqlite`, `write_out`-Argumente) | – |
| `cargo test --workspace -j 6 --locked`, ohne Ausschluss | **4.074 bestanden, 0 Fehler, 49 bestehende Ignores**, 551 Testprogramme inkl. Doc-Tests; kein Lastflake, keine Wiederholung | 09:09:56–09:35:42 UTC (25 min 46 s) |
| `npm run -w apps/desktop typecheck && npm run -w apps/desktop test` | grün; Vitest 100 Dateien, 882 Tests | – |
| Playwright e2e/beta, e2e/mcp, e2e/a11y, Chromium + WebKit, 2 Worker, echte Bridge (os-app-bridge, os-testsite, os-cli, os-parity vorher gebaut wie `ui-e2e.yml`) | **92 bestanden, 0 Fehler, 0 flaky**, einschließlich WebKit `beta/overview.spec.ts` | 27,6 min |
| `os-parity run --all` | grün, **218/218**, 10 Sites present, 0 Fehler, 0 Warnungen; drei bestehende unavailable-Skips | 254 s |
| `os-parity coverage --milestone M2` | verified 129/155 (83,2 %), unverändert zu int-22 | – |
| `python3 -I docs/plan/M2-check.py` | OK (138 Pakete, 590 `spec_ids`) | – |
| `bash scripts/check-neutral.sh` | keine Fundstellen | – |
| `cargo deny check licenses`; `cargo metadata --locked` | grün; grün | – |

Rohprotokolle: Server `/tmp/oi23`.

## WebKit-Grid-Gate-Endsprung

Auf dem integrierten Stand rot, Suite `e2e/grid/grid.gate.ts` (eigene Konfiguration `e2e/grid/playwright.config.ts`, Bridge mit `stub-views`). Chromium 8/8 grün, WebKit 7/8. Rot nur „native scrollbar, wheel, horizontal virtualization and controlled column gestures“ (`grid.gate.ts:143`): Nach dem Ziehen des Scrollbalkens bleibt `snapshot().last` bei `loading`, auch mit Retry.

**Keine Pflicht-Suite.** Der Test läuft nicht in `ui-e2e.yml` (Pflicht-Matrix e2e-a): Dort gilt die Standard-`testMatch`, `*.gate.ts` ist nicht enthalten. Er läuft nur im Workflow `spike-grid.yml` („Grid gate S-1 / S-2“). Dieser startet ausschließlich bei `pull_request` mit Pfaden unter `src/grid/**`, `src/data/**` u. a. sowie per `workflow_dispatch`. Die Matrix dort ist macOS-WebKit und Windows-Chromium (Skalierungen 1 bis 2), kein Linux-WebKit.

**Eingrenzung (Diagnose auf dem Server, temporäre Testdatei danach entfernt):**

| Engine | `scrollTop` nach dem Ziehen | max. `scrollTop` | erste sichtbare Zeile | `last` |
|---|---|---|---|---|
| Chromium | 999.252 | 999.252 | 4.999.973 | 5.000.000 |
| WebKit (Linux) | 995.239 | 999.252 | 4.979.893 | `loading` |

Der Daumenzug bis `box.height − 20` erreicht in WebKit wegen anderer Scrollbalken-Geometrie das Ende nicht. Die letzte Zeile ist dann nicht sichtbar und wird folgerichtig nicht geladen. Ein programmatischer Sprung ans Ende lädt sie sofort (`last` = 5.000.000). Das ist kein Produktfehler im Lade- oder Ansichtspfad, sondern die Testgeste: Der Zug endet in WebKit rund 4.000 px vor dem Ende. Kleine Lösung für den Testbesitzer: den Zug über das Gitterende hinausführen (Daumen wird geklemmt) oder auf `scrollTop === max` warten. Diese Testlogik ist hier nicht geändert (C10/K2-63). Ob macOS-WebKit in `spike-grid.yml` dasselbe zeigt, ist offen.

## Offene Folgearbeiten

- **Inkrementelle Zählregeln je Filter:** Overview/Issues laufen bei Dateien, deren Filter nie gezählt wurden, in den Timeout (Kombilauf Referenz A). Fable-Hinweise: Fallback ohne `persist_facts`, der Fortschrittsbalken steht im Fallback, `countCancel` soll warnen.
- **N20-Folgeauflage:** Steuer-Nachrichten in `crawl.rs` im `select!` nicht wartend senden.
- **Option (c):** Overview, Katalog und Export auf zwei Kernen beschleunigen.
- **Sortier-Peak** ~1 GB RSS je Sitzung (N22-Hinweis).
- **WebKit-Grid-Gate:** Testgeste im `grid.gate.ts` anpassen (Besitzer der Grid-Gate-Datei), siehe oben.
- **U31:** native WKWebView-Messung (Bildschirm entsperrt), 30-min-Soak auf dem Endstand, Windows-CI-Werte, sieben Nightly-Läufe; macOS-Nightly hängt an K2-65/S02.
- **FIX-E-Regression (`152f8c55`/`deba2e77`):** offen bis zum Fix aus `c-fix-e`; der Orchestrator zieht ihn nach.
- **FIX-E nativ:** linux-webkit im CI-Lauf `ui-e2e` auf diesem Stand (Fable int-22, Auflage a) und macOS-/Windows-Bestätigung der Stack-Korrektur; CI-Lauf und Push startet der Orchestrator.
- A03 (menschliche Runde) bleibt separat; der Merge der Unterlagen ist kein Abnahmenachweis.

Stand: **PAKET FERTIG** (Integration); CI und Push beim Orchestrator.
