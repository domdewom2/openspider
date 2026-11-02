# M2 Integrationsrunde 25

Stand: 02.11.2026. Branch `m2/int-25`, Basis `fdcf9578` (origin/main: int-24 abgeschlossen). Bauen und Prüfen auf dem Linux-Server; Git-Operationen lokal. Kein Rebase, kein `cargo update` (Tokio bleibt 1.53.3), Schema bleibt v5.

## Nachtrag int-24

Fable-Auflage 1 (vollständiger macos-webkit-Lauf auf dem main-Stand): ui-e2e 41350394982 auf `fdcf9578` komplett grün, macos-webkit 53 min. Auflage 2 (CI-Pflege) ist das Paket M2-CI-E2E dieser Runde; auf dessen Branch lief macos-webkit 59:50 min (Lauf 41350460596, grün) und damit knapp unter dem alten Limit.

## Integration

| Reihenfolge | Paket | Quellstand | Merge | Fable |
|---|---|---|---|---|
| 1 | FIX-WAL (`m2/fix-wal`, Spec `ae2c58b6`) | `15bb2c6f` | `0bf3d579` | approved |
| 2 | N26 (`m2/n26`) | `8a975a37` | `f5b78f6f` | approved mit Auflagen, Auflagen erfüllt |
| 3 | FIX-ABOUT (`m2/fix-about`) | `20494c6f` | `aa531627` | Orchestrator-Fix (UI-127), Tests grün |
| 4 | CI-E2E (`m2/ci-e2e-limit`) | – | `3c8450eb` | int-24-Auflage 2 |
| 5 | N25 (`m2/n25`) | `f585094f` | `668b6651` | approved mit Auflagen, Auflagen erfüllt |

Konflikt: `crates/os-analyze/src/tabs/hreflang.rs` beim Merge von N25. N25 und N26 haben denselben Fix (feste Join-Reihenfolge der Hreflang-Beziehungsfilter, `CROSS JOIN`) unabhängig gebaut. Übernommen ist die N26-Fassung: Sie enthält zusätzlich den Filter „Non-Canonical Return Links“ und den Abfrageplan-Test. Der N25-Test `n25_hreflang_scaling.rs` bleibt und ist mit der N26-Fassung grün. Alle übrigen Merges liefen textuell konfliktfrei.

## Inhalt

- **FIX-WAL (CRAWL-126, CRAWL-141):** Beenden führt zuerst die Schreibwarteschlange und wartende View-Cache-Aufträge aus, stößt dann je beschreibbarer Datei die Rückführung an und wartet im Restbudget (3 s gesamt). Ab Beginn des Beendens keine neuen View-Cache-Speicherungen. Der abschließende Checkpoint der Crawl-Analyse wartet höchstens 100 ms auf Leser; den Rest führt der Hintergrund-Merge zurück. Tests: WAL nach Beenden 4.152 B → 0; Analyse-Abschluss 5,02 s → 0,20 s; Stopp → Analyse → Rückführung; Sperrtest `replace_values` (FIX-TX-Auflage 4).
- **N26 (UI-145, UI-147):** „Top-Probleme“/„Befunde“ luden nach Crawl-Ende 9–16 s. Ursache: vier Hreflang-Beziehungsfilter mit quadratischem Abfrageplan. `sidebar.issues` 9,1 s → 0,57 s.
- **N25 (Beta-Befunde):** `export.catalog` und die Report-Verfügbarkeit bestimmen analyseabhängige Filter ohne Vollzählung (1.358 → 56 ms je Katalogabfrage); `crawl.open` stellt steuerbare Crawls vor dem Öffnen der Leser auf WAL um (behebt das CI-Rennen `pending_w1_o02` „database is locked“); Diagnosewerkzeuge unter `crates/os-loadtest/diagnostics/n25-*`. Der gemeldete Ratenabfall nach dem Fortsetzen ist auf Testsites nicht reproduziert (Backlog).
- **FIX-ABOUT (UI-127):** „Über OpenSpider“ zeigte im nativen Build „unerwarteter Fehler“: Der Tauri-Transport rief `plugin:app|info` und traf das Tauri-Kern-Plugin `app`; die Hülle registriert die Domäne als `os-app`. Der Transport wendet jetzt dieselbe Kern-Plugin-Liste an; ein Test vergleicht sie mit `src-tauri/src/plugin_names.rs`.
- **CI-E2E:** Job-Limit nur für macos-webkit auf 90 min (`matrix.timeout`); nachträglich in dieser Runde auch `spec-coverage` (`ci.yml`) auf 90 min, weil grüne Läufe 50–58 min brauchen und der Job bei 60 min wiederholt abbrach. Keine Änderung an Specs oder Budgets.

## Auflagen

- **FIX-WAL:** keine. Offene Fragen entschieden: (1) Zweitflush in `runtime/mod.rs` bleibt, (3) Flush ohne eigene Frist bleibt; (2) Zwischen-Checkpoints der Analyse und der Hinweis zur View-Cache-Sperre im Pfad ohne Rückfrage stehen in `M2-backlog.md` Abschnitt 8.
- **N26:** A1 Zeitgrenze des Zähltests 2 → 10 s (`8a975a37`); A2 Planeintrag (`a2f4a58f`).
- **N25:** A1 `tracing::warn!` statt `debug!` beim WAL-Wechsel in `crawl.open`; A2 `prereq::availability` ohne Vollzählung je Report (beide `f585094f`).

## Planung

- Neue Pakete in Welle 4n: M2-FIX-WAL, M2-N25, M2-N26, M2-FIX-ABOUT, M2-CI-E2E; Owns aus den K2-92-Tabellen der Berichte.
- `ERLAUBT` (M2-check.py) um elf sequenzielle Überschneidungen mit Grund ergänzt.
- Folgepakete in `M2.md` (Integrationsrunde 25) und `M2-backlog.md` Abschnitt 8: M2-N27 (gemeinsame, gedrosselte Zählung je Datenstand, Spec-Festlegung UI-018), M2-N28 (UI-061 „gefunden“ ohne Hilfsabrufe, robots.txt im Feed, Pagination-Join), M2-N29 (Kennzeichnung nicht gebauter Funktionen); nach der Beta inkrementelle Zähler (UI-145).
- `python3 -I docs/plan/M2-check.py`: OK (145 Pakete, 590 Spec-IDs).

## Prüfungen

Server (Linux x86_64), Integrationsbaum `a2f4a58f`:

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün |
| os-app: `pause_merge`, `fix_tx_analysis`, `fix_tx_api`, `stop_analysis_merge`, `n25_resume_after_open`, `n25_export_catalog_cost`, `pending_w1_o02` | 12/12 |
| os-analyze: `n26_hreflang_plans`, `n25_hreflang_scaling`, `fix_wal_checkpoint`, `tab_hreflang`, `tab_hreflang_m2` | 32/32 |
| `cargo test --workspace --no-fail-fast` | 4.098 bestanden, 0 fehlgeschlagen |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | 100 Dateien, 888 Tests grün |

CI der Pakete (GitHub, ohne Intel):

| Paket | CI | ui-e2e |
|---|---|---|
| FIX-WAL | 41355678156 grün | 41355680651 grün |
| N26 | 41363213894 (nach Auflage A1) | 41362730621: windows-chromium rot nur `beta/n04-live.spec.ts:74` (siehe unten), flaky `keyboard-suite/shortcuts.spec.ts:28`; übrige Plattformen grün |
| FIX-ABOUT | – (nur Frontend) | 41357285881 grün |
| N25 | Läufe durch den Auflagen-Push abgebrochen; Nachweis über den Integrationsbaum | – |

`n04-live.spec.ts:74` (Detailtab-Grid ≤ 1 s während des Live-Crawls): N24 hat das Rennen unter WebKit behoben (2/10 → 0/10); auf windows-chromium trat es im N26-Lauf in beiden Versuchen wieder auf. N26 ändert kein Frontend. Wird es im Integrationslauf erneut rot, folgt ein eigenes Paket für den Windows-Pfad (Budget unverändert, K2-63).

CI und ui-e2e des Integrationsbaums (`a2f4a58f` + Bericht):

| Lauf | Ergebnis |
|---|---|
| ui-e2e 41368453747 | grün auf allen vier Plattformen (macos-webkit 59 min, windows-chromium inkl. `n04-live`) |
| CI 41368451527 | rustfmt, cargo-deny, spec-coverage (43 min), ci macos-arm64, crawlfile load grün; ci windows-x64 rot nur `os-app` `views::n21_tests::filter_open_reuses_the_tab_session_but_keeps_closed_handles_revoked` (`tests/n21/views.rs:230`, Tab-Sitzung nicht wiederverwendet) – im FIX-WAL-Lauf auf Windows grün; Wiederholung des Jobs siehe unten |

Windows-Zeitabhängigkeiten: In den Läufen dieser Runde fielen auf Windows drei verschiedene Tests einmalig aus (`pending_w1_o02` – durch N25 behoben; `cmd_config_runtime.rs:391` 2-s-Warten auf Threadwechsel; `n21/views.rs:230` Tab-Sitzung nach Dateiänderung neu). Folgepaket M2-N30: zeitabhängige os-app-Tests auf Windows stabilisieren (Ursache je Test belegen, keine Budgetlockerung).

## Folgearbeiten

- M2-N27, M2-N28 (laufen, Basis `m2/int-25`), M2-N29.
- Inkrementelle Zähler nach UI-145 (nach der Beta).
- `n04-live.spec.ts:74` auf windows-chromium (im Integrationslauf grün, beobachten).
- M2-N30 Windows-Zeitabhängigkeiten in os-app-Tests.
- Auswertung der Nutzer-Crawl-Datei mit `n25-tail.py` (Ratenabfall nach Fortsetzen).
- Aus int-24 weiter offen: Testrennen `cli_basics` (Port), `header.spec.ts:69`, `config/dialog.spec.ts:135`; macos-webkit in Shards teilen.
