# M2 Integrationsrunde 24

Stand: 01.11.2026. Branch `m2/int-24`, Basis `df02958d` (origin/main: int-23 abgeschlossen). Bauen und Prüfen auf dem Linux-Server; Git-Operationen lokal. Kein Rebase, kein `cargo update` (Tokio bleibt 1.53.3), Schema bleibt v5.

## Integration

| Reihenfolge | Paket | Quellstand | Merge | Fable |
|---|---|---|---|---|
| 1 | FIX-STOP (`m2/fix-stop`) | `de4fd6db` | `12da0b2f` | approved |
| 2 | FIX-TX (`m2/fix-tx`) | `9b4810d8` | `8cf7bf68` | approved mit Auflagen, Auflagen erfüllt |
| 3 | Unit-Tests aus `m3/fix-e` (`dd02fe65`, Cherry-pick `-x`) | – | `42dddab0` | Teil der FIX-E-Freigabe |
| 4 | N24 (`m2/n24`) | `25c033b3` | `b8849080` | approved mit Auflagen, Auflagen erfüllt |

Alle Merges liefen mit `--no-ff` textuell konfliktfrei; der Cherry-pick ebenfalls. FIX-STOP und FIX-TX berühren keine gemeinsame Datei. N24 ändert nur das Frontend; FIX-STOP ändert in der Statusleiste `Statusbar.tsx`, N24 `McpMenu.tsx`.

## Inhalt

- **FIX-STOP (CRAWL-126):** „Stoppen“ aus „Pausiert“ blieb unter Windows in „Pausiert“ hängen („database is locked“): Der Stopp-Checkpoint las zuerst und stufte dann eine verzögerte Transaktion zum Schreiben hoch; SQLite meldet dabei sofort `SQLITE_BUSY`, der Busy-Timeout greift nicht. `stop_checkpoint` öffnet jetzt `BEGIN IMMEDIATE`, wartet bis 5 s auf die Schreibsperre und stößt danach die Rückführung an (`merge::start`, idempotent mit `again`-Flag). Statuszeile „Gestoppt – Datei wird zusammengeführt“. Spec: OS-Festlegung in `docs/spec/01-crawl.md` (CRAWL-126) mit Abnahmekriterium, Querverweis in `docs/spec/05-ui.md` (UI-003).
- **FIX-TX (INTEG-010, CRAWL-126):** Dasselbe Hochstufungsmuster an weiteren Stellen auf `BEGIN IMMEDIATE` umgestellt: `queries/api.rs` (`invalidate`, `replace_values`), `queries/detail_ext.rs` (`save_serp_edit`), Bildschritt der Analyse, `aggregates/status.rs::on_config_changed`; `api.setSettings` speichert Konfiguration und Invalidierung in einer IMMEDIATE-Transaktion. OS-Festlegung INTEG-010 in `docs/spec/04-custom-integrations.md`.
- **dd02fe65:** drei Unit-Tests für den `checkpointed`-Merker im Writer-Thread und ein präzisierter Kommentar (`crates/os-engine/src/writer_thread.rs`), aus der M3-Linie nachgezogen (Folgearbeit aus int-23/int-f).
- **N24 (UI-145):** Ursache des Testrennens `e2e/beta/n04-live.spec.ts:74` war Hauptthread-Arbeit im Klickpfad, nicht die Detailabfrage. Anfrage eines geklickten Tabs noch im Klick, kein zweiter Render des Detailrahmens, keine Rerenders versteckter zurückgehaltener Panels, Menümodelle nicht im Eingabeereignis und nicht je Live-Tick. WebKit `n04-live` 2/10 → 0/10 rot, Budget 1 s unverändert. Details in `M2-reports/n24.md`.

## Auflagen

**FIX-TX (Fable):**

1. CI und ui-e2e des Pakets grün – siehe Prüfungen.
2. Doc-Kommentar zur mitbenutzten Transaktion in `invalidate` – erledigt (`9b4810d8`).
3. Auf dem Integrationsbaum: `os-app --test pause_merge --test fix_tx_analysis --test fix_tx_api` sowie beide `fix_tx_immediate`-Tests – grün (6/6 bzw. 6/6).
4. Folgearbeit Sperrtest `replace_values_waits_for_a_held_write_lock` – im Folgepaket M2-FIX-WAL.

**N24 (Fable):**

- A1 K2-92-Tabelle mit Commit und Diff-Stelle in `M2-reports/n24.md` – bei der Integration ergänzt (`a528c411`).
- A2 Spec-Frage UI-145 ↔ K-74 und zwei Folgekandidaten in `M2-backlog.md` Abschnitt 6 – erledigt (`a528c411`).

**FIX-STOP (Fable):** Bedingung ci und ui-e2e grün, insbesondere windows-chromium – siehe Prüfungen.

## Planung

- `M2-FIXE-PORT` umfasst den Fix M2-FIX-STOP; Owns um `docs/spec/05-ui.md` (nur Querverweis UI-003) und `M2-reports/fix-stop.md` ergänzt.
- Neues Paket `M2-FIX-TX` (Welle 4n, Zusatz-ID INTEG-010); Fremddateien früherer Wellen laut K2-92-Tabelle in `M2-reports/fix-tx.md`.
- Neues Paket `M2-N24` (Welle 4n, Zusatz-ID UI-145); sequenzielle Überschneidungen N04/N24, N12/N24, FIXE-PORT/N24 in `ERLAUBT`.
- `python3 -I docs/plan/M2-check.py`: OK (140 Pakete, 590 Spec-IDs).

## Prüfungen

Server (Linux x86_64), Integrationsbaum:

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün |
| FIX-TX-Auflage 3 (`pause_merge`, `fix_tx_analysis`, `fix_tx_api`) | 6/6 |
| `fix_tx_immediate` (os-store, os-analyze) | 6/6 |
| `os-engine --lib writer_thread` | 15/15 |
| `cargo test --workspace --no-fail-fast` | grün, 0 fehlgeschlagen |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` (mit N24) | 100 Dateien, 886 Tests grün |

Im ersten Workspace-Lauf schlug `os-cli` `cli_basics::an_unreachable_start_url_is_a_valid_result_unless_fail_on_start_error` fehl: Der Test reserviert einen „geschlossenen“ Port, gibt ihn frei, und ein parallel laufender Test bekam denselben Port für seinen Testserver (19 URLs gecrawlt statt keiner). Danach `cli_basics` 3× einzeln grün und der volle Workspace grün. Vorbestehendes Testrennen ohne Bezug zu dieser Runde (Folgearbeit unten).

CI der Pakete (GitHub, ohne Intel):

| Paket | CI | ui-e2e |
|---|---|---|
| FIX-STOP | 41325638841 (Versuch 2): ci windows-x64/macos-arm64, crawlfile load, cargo-deny grün; spec-coverage nach 1 h abgebrochen (Runner-Zeitlimit) | 41325641287: windows-chromium, linux-chromium, macos-webkit grün; linux-webkit rot nur `n04-live:74` (durch N24 behoben), im zweiten Versuch wieder dort und flaky `detail-during-crawl` |
| FIX-TX | 41333352301: ci windows-x64/macos-arm64, crawlfile load, cargo-deny grün; spec-coverage nach 1 h abgebrochen (Runner-Zeitlimit) | 41333354811: windows-chromium, linux-chromium, linux-webkit grün; macos-webkit nach 1 h abgebrochen (Runner-Zeitlimit) |
| N24 | 41337341565: alle Jobs grün (inkl. spec-coverage) | 41337343906: windows-chromium, linux-chromium grün; linux-webkit `n04-live` grün, rot nur `config/dialog.spec.ts:135` (vorbestehender Flake, siehe unten); macos-webkit nach 1 h abgebrochen (Runner-Zeitlimit) |

Der erste FIX-TX-Lauf (41326365487/41326367795) und der erste Windows-Job von FIX-STOP wurden von GitHub mit „internal error“ abgebrochen; der erste spec-coverage-Job von FIX-STOP scheiterte am Runner („os-parity: cannot execute binary file“). Die späteren Abbrüche sind Zeitüberschreitungen auf macOS-Runnern (GitHub meldete Kapazitätsengpässe für macOS arm64). Die abgebrochenen Jobs laufen im Integrationsbaum erneut.

`config/dialog.spec.ts:135` (Regex-Tester, CRAWL-111) auf dem Server gegengeprüft, WebKit, `--repeat-each=20 --retries=0`: mit N24 3/20 rot, mit dem Frontend vor N24 4/20 rot (jeweils `regex-result` nicht binnen 5 s vorhanden). Vorbestehender Flake ohne Bezug zu N24.

CI und ui-e2e des Integrationsbaums: werden nach dem Push nachgetragen.

## Folgearbeiten

- **M2-FIX-WAL** (nach int-24, Festlegungen von Fable liegen vor): WAL leer nach Beenden (Schreibwarteschlange und View-Cache vor der Rückführung), Abschluss-Checkpoint der Analyse ≤ 100 ms mit Hintergrund-Merge (CRAWL-141), gemeinsamer Test Stopp → Analyse → Rückführung, Sperrtest `replace_values`.
- Testrennen `os-cli` `cli_basics` (Port-Wiederverwendung), `header.spec.ts:69` und WebKit-Flake `config/dialog.spec.ts:135` (Regex-Tester, ~15–20 % rot auf Linux) an die Spec-Besitzer.
- Spec-Frage UI-145 ↔ K-74 (`M2-backlog.md` Abschnitt 6).
- Aus int-23 weiter offen: Zählregeln je Filter, N20-Folgeauflage `select!`, Sortier-Peak ~1 GB (N22); M2-Stand in die M3-Linie integrieren.
