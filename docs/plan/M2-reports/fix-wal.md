# M2-FIX-WAL – WAL leer nach Beenden, Abschluss-Checkpoint der Analyse ≤ 100 ms (CRAWL-126, CRAWL-141)

Basis: `main` fdcf9578 + Spec-Commit ae2c58b6 (OS-Festlegungen vom 01.11.2026), Branch `m2/fix-wal`, lokaler Worktree
`~/Projects/openspider-wt/c-fix-wal`, Server-Ordner `m2-fix-wal`. Kein Push.

## Ursachen

**A – WAL nach „Beenden“ nicht leer** (Zusatzbefund A aus `fix-tx.md`). `quit::confirm` wartete zuerst auf laufende
Merges (`merge::wait_all`) und arbeitete die Schreibwarteschlange erst danach ab (`write_queue().flush()`). Jeder Auftrag,
der dabei noch lief oder wartete, schrieb also nach der letzten Rückführung neue WAL-Frames. Typischer Fall: die
N21-Vorberechnung (`view_cache`, Busy-Timeout 0) scheitert während eines Merges still und speichert danach. Ohne
laufende Engine startete `confirm` außerdem gar keinen Merge. Derselbe Fehler steckte im Pfad ohne Rückfrage
(`SessionQuit::request_quit`, `reasons.is_empty()`): nur `wait_all`, danach flushte `App::request_quit` die Warteschlange.

**B – Abschluss-Checkpoint der Analyse** (Zusatzbefund B). `runner.rs::checkpoint_phase(.., always = true)` setzte für den
finalen `wal_checkpoint(TRUNCATE)` einen Busy-Timeout von 5 s. Hielt ein Leser einen älteren Stand, dauerte jede Analyse
≈ 5 s länger, und niemand stieß die Rückführung danach an. Die Analyse-Frames blieben im WAL, bis zufällig ein späterer
Merge lief.

## Fix

**A** (`crates/os-app/src/quit.rs`, Aufrufer der View-Cache-Speicherung)

- `quit::drain`, gemeinsam für `confirm` und `request_quit` (ohne Rückfrage):
  1. Ein Verwaltungsauftrag auf dem Schreib-Thread läuft nach allen bis dahin eingereihten Aufträgen (wirkt wie `flush`).
     Ausstehende eigene Schreibaufträge werden also zuerst ausgeführt.
  2. Danach `merge::start` je beschreibbarer offener Datei. `start` prüft selbst `wal_bytes > 0`; läuft schon ein Merge,
     macht er eine weitere Runde (`again`).
  3. `merge::wait_all` im Restbudget von insgesamt 3 s ab Beginn des Beendens.
  4. Läuft danach noch ein Merge, wird das protokolliert. Die Datei bleibt konsistent, und das nächste Öffnen führt das WAL
     zurück.
- `confirm`: Reihenfolge jetzt Operationen abbrechen → Engines pausieren und `settle` → `mcp.stop` → `drain`. `mcp.stop`
  steht damit vor der Rückführung, weil auch MCP-Aufträge über die Warteschlange schreiben können.
- **Keine neuen View-Cache-Speicherungen ab Beginn des Beendens.** `quit::cache_ticket` liefert ab dem Beginn des Beendens
  `None`; die drei Aufrufer (`views::persist_facts`, `sidebar::count_steps`, Sidebar-Vorberechnung in
  `sidebar_ext::on_startup`) reichen dann nichts mehr ein. Ein Ticket, das kurz vor Beginn geholt wurde, prüft der Auftrag
  auf dem Schreib-Thread erneut (`CacheTicket::open`). Der Verwaltungsauftrag aus Schritt 1 „versiegelt“ dort, deshalb
  wird auch ein solcher Nachzügler übersprungen. Bereits eingereihte Speicherungen laufen noch, wie die Spec verlangt.
- Scheitert `confirm` (etwa `err-quit-operation-running`), ist der View-Cache wieder offen.
- Hinweis zum Lint: os-app darf `os_store::CrawlFile` außerhalb von `pool.rs`/`write_queue.rs` nicht benennen. Deshalb
  prüfen die Aufrufer das Ticket in ihrem eigenen `submit_file`-Auftrag, statt dass ein gemeinsamer Wrapper das tut.

**B** (`crates/os-analyze/src/analysis/runner.rs`, `crates/os-store/src/writer.rs`, `crates/os-app/src/session/analysis.rs`)

- Der finale Checkpoint nutzt `os_store::writer::checkpoint_within(.., CHECKPOINT_READER_WAIT = 100 ms, ..)` mit dem
  Wanduhr-Busy-Handler. Danach wird der vorherige Busy-Timeout der Verbindung wiederhergestellt.
- `checkpoint_within` ist dafür additiv `pub` geworden (K2-61).
- Die Zwischen-Checkpoints (WAL ≥ 64 MB zwischen zwei Schritten) bleiben unverändert, weil die Spec nur den
  abschließenden Checkpoint regelt.
- `session::analysis::execute` ruft nach dem Analyse-Auftrag `merge::start` auf, noch unter dem Lebenszyklus-Wächter.
  Ein „Fortsetzen“ danach bricht diesen Merge also vor dem ersten Engine-Schreibzugriff ab. Bei `busy = 0` ist das WAL
  leer, und `start` tut nichts. Die Spec-Bedingung „bei busy = 1 und wal_bytes > 0“ ist damit erfüllt, ohne `busy` über
  die API von `AnalysisOutcome` zu reichen.
- Erfasst sind auch Schreibzugriffe nach dem Checkpoint, etwa `write_run_state(Incomplete)` bei unvollständigem Crawl.
- Die Analyse meldet „abgeschlossen“, sobald ihr letzter Commit dauerhaft ist.

**C/D** – nur Tests (siehe unten).

## Tests

| Test | Spec | Basis (fdcf9578 + Tests) | mit Fix |
|---|---|---|---|
| `os-app/tests/stop_analysis_merge.rs::quit_runs_a_pending_view_cache_write_first_and_leaves_an_empty_wal` – wartender View-Cache-Auftrag in der Warteschlange, Freigabe nach Beginn von `confirm` | CRAWL-126 | **rot**: WAL 4152 Byte nach Beenden | grün; Eintrag liegt in der Hauptdatei, WAL 0 |
| `os-app/src/quit.rs` → `tests/fix_wal/gate.rs::view_cache_writes_queued_before_quitting_run_and_later_ones_are_skipped` (Unit-Test, deterministisch über `now_or_never`) | CRAWL-126 | – (neue Funktion) | grün |
| `os-analyze/tests/fix_wal_checkpoint.rs::final_checkpoint_waits_at_most_briefly_for_a_held_reader` | CRAWL-141, CRAWL-126 | **rot**: Analyse wartete 5,02 s | grün (Lauf 0,20 s) |
| `stop_analysis_merge.rs::analysis_leaves_its_frames_to_a_background_merge_when_a_reader_holds_the_wal` – manuelle Analyse bei gehaltenem Leser ohne laufenden Merge | CRAWL-141, CRAWL-126 | **rot**: WAL 156.592 Byte nach Leserende | grün |
| `stop_analysis_merge.rs::stop_from_paused_with_a_held_reader_then_analysis_and_an_empty_wal` (Teil C) – Crawl → Pause bei gehaltenem Leser → Stopp → Auto-Analyse `Done`, Bildschritt `done` → nach Leserende WAL 0 ohne Nutzeraktion | CRAWL-126, CRAWL-141, ONPAGE-178 | grün (siehe unten) | grün |
| `os-store/tests/fix_tx_immediate.rs::replace_values_waits_for_a_held_write_lock` (Teil D) | CRAWL-126, INTEG-009 | grün (Fix stammt aus FIX-TX); **Gegenprobe** mit `transaction()` statt IMMEDIATE: rot, `database is locked` nach 48 µs | grün |

Einordnung C: Auf der Basis grün, weil der Merge des Stopps bei gehaltenem Leser noch läuft. Er nimmt nach dem Leserende
auch die Analyse-Frames mit. Der gemeinsame Test sichert die Kombination aus FIX-STOP, FIX-TX und diesem Paket ab
(Zusatzbefund C aus `fix-tx.md`). Seine Laufzeit sinkt durch B um ≈ 5 s.

Synchronisation: Leser und Schreibsperren werden ohne Schlafpausen gehalten. Die einzige Pause (50 ms vor der Freigabe des
wartenden Auftrags im Quit-Test) ist nicht ergebnisrelevant: Die Erwartung gilt für jede Reihenfolge. Auf der Basis
scheitert der Test immer, weil dort nach dem Auftrag kein Merge mehr folgt.

Nebenwirkung: `fix_tx_analysis` (FIX-TX) läuft jetzt 1,98 s statt ≈ 7 s, weil die Analyse nicht mehr 5 s auf den Leser
wartet.

## Gates und Testzahlen (Server, Linux, 16 Kerne)

| Lauf | Ergebnis |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | grün |
| `cargo test --workspace --no-fail-fast` (über `~/ws-slot.sh`) | **4.093 bestanden, 0 fehlgeschlagen, 49 ignoriert**, 557 Testprogramme. Davon 6 Tests neu (3 App-Integration, 1 App-Unit, 1 Analyse, 1 Store); auf der Basis rechnerisch 4.087 |
| `cargo test -p os-app --test pause_merge --test fix_tx_analysis --test fix_tx_api --test stop_analysis_merge`, 3× wiederholt | 3/3 grün (je 1 + 1 + 4 + 3 Tests) |
| `npm run -w apps/desktop typecheck` / `npm run -w apps/desktop test` | grün; Vitest 100 Dateien, 886 Tests |
| E2E `beta/control.spec.ts` + `header/header.spec.ts`, Chromium + WebKit, `--repeat-each=3 --retries=0 --workers=2`, `CI=true`, Bridge, eigener Vite-Port 1437 (Konfiguration nur auf dem Server, nicht eingecheckt) | 18/18 grün in 57 s; kein „database is locked“ im Log |

Zwischenbefund beim ersten Workspace-Lauf: `os-app/tests/runtime_lint.rs` erlaubt die Lint-Ausnahme
`clippy::disallowed_*` in `src` nur für `pool.rs` und `write_queue.rs`. Der Unit-Test der Sperre liegt deshalb in
`tests/fix_wal/gate.rs` und wird per `#[path]` eingebunden.

## Fremdänderungen (K2-92) außerhalb von `session/**` und `quit.rs`

`session/analysis.rs` und `quit.rs` gehören zu M2-FIXE-PORT und stehen deshalb nicht in der Tabelle.

| Datei | Besitzer-Paket | Änderung | Spec |
|---|---|---|---|
| `crates/os-store/src/writer.rs` | M2-F05a (`crates/os-store/**`, Datei eingefroren) | `checkpoint_within`: `pub(crate)` → `pub`, Doku-Satz; Verhalten unverändert (K2-61, von Fable freigegeben) | CRAWL-141 |
| `crates/os-analyze/src/analysis/runner.rs` | M2-F05a / M2-F02 | `checkpoint_phase`: finaler Checkpoint über `checkpoint_within` (100 ms Wanduhr), Busy-Timeout danach wiederhergestellt; Zwischen-Checkpoints unverändert | CRAWL-141, CRAWL-126 |
| `crates/os-app/src/views/mod.rs` | M2-N21 (zuvor M2-K05) | `persist_facts`: ohne `cache_ticket` keine Einreichung; Ticket im Auftrag geprüft | CRAWL-126 |
| `crates/os-app/src/commands/sidebar.rs` | M2-N21 | `count_steps`: View-Cache-Speicherung nur mit `cache_ticket`, wie oben | CRAWL-126 |
| `crates/os-app/src/commands/sidebar_ext.rs` | M2-N21 (zuvor M2-K03) | Sidebar-Vorberechnung (`on_startup`): Speicherung der Aggregate nur mit `cache_ticket`, wie oben | CRAWL-126 |
| `crates/os-store/tests/fix_tx_immediate.rs` | M2-FIX-TX | neuer Test `replace_values_waits_for_a_held_write_lock` (Auftrag Teil D) | CRAWL-126, INTEG-009 |

Neue eigene Dateien: `crates/os-app/tests/stop_analysis_merge.rs`, `crates/os-app/tests/fix_wal/gate.rs` (per `#[path]`
aus `quit.rs`, Muster `tests/n21/views.rs`), `crates/os-analyze/tests/fix_wal_checkpoint.rs`, dieser Bericht.

Nicht geändert: `crates/os-app/src/runtime/mod.rs` (`App::request_quit` flusht nach dem Hook noch einmal, höchstens 2 s).
Dieser Flush findet nach `drain` nichts mehr vor, außer einem Auftrag, den jemand genau in diesem Fenster einreicht (siehe
offene Fragen).

## Commits

- `182212be` store: checkpoint_within öffentlich für den Abschluss-Checkpoint der Analyse (CRAWL-141)
- `fb97ddd1` analyze: abschließender Checkpoint wartet höchstens 100 ms auf Leser (CRAWL-141, CRAWL-126)
- `95b4b351` app: Analyse-Ende stößt die WAL-Rückführung an (CRAWL-141, CRAWL-126)
- `53fbc232` app: Beenden führt eigene Schreibaufträge zuerst aus, dann WAL-Rückführung (CRAWL-126)
- `68f9710f` test: Stopp aus Pausiert mit Leser, Analyse und leeres WAL; Beenden mit wartendem Schreibauftrag
- `318b3618` test: replace_values wartet auf gehaltene Schreibsperre (Testlücke FIX-TX)
- `262f660a` app: View-Cache-Sperre beim Beenden als Ticket im eigenen submit_file-Auftrag (Lint)
- `93e6c7cc` test: Unit-Test der View-Cache-Sperre nach tests/fix_wal (runtime_lint)
- docs: dieser Bericht

## Offene Fragen

1. **Schreibaufträge nach dem Drain im Pfad ohne Rückfrage:** `App::request_quit` (`runtime/mod.rs`, nicht in diesem
   Paket) flusht nach dem Hook erneut. Ein Nicht-Cache-Auftrag, der zwischen `drain` und diesem Flush eingereicht wird
   (z. B. `config.set` im selben Augenblick), landet im WAL. Das nächste Öffnen führt ihn zurück. Soll die Warteschlange
   ab Beginn des Beendens alle neuen Aufträge abweisen? Das wäre eine Verhaltensänderung für alle Schreiber und ist nicht
   umgesetzt.
2. **Zwischen-Checkpoints der Analyse** (WAL ≥ 64 MB zwischen zwei Schritten) warten weiterhin bis 5 s auf Leser. Die
   Spec regelt nur den abschließenden Checkpoint (CRAWL-141 Punkt 7). Sollen sie ebenfalls auf 100 ms begrenzt werden?
3. **Budget des Flush beim Beenden:** Der Flush in `drain` hat keine eigene Frist (wie bisher in `confirm`). Ein sehr
   langer, nicht abbrechbarer Auftrag verbraucht daher das 3-s-Budget, und die Rückführung wird nur noch protokolliert.
   Laut Spec gehen ausstehende Aufträge vor; das ist so umgesetzt.
