# M2-FIX-TX – Lesen-dann-Schreiben-Transaktionen neben der WAL-Rückführung (CRAWL-126)

Basis: `origin/main` `df02958d`, Branch `m2/fix-tx`, lokaler Worktree `~/Projects/openspider-wt/c-fix-tx`, Server-Ordner `m2-fix-tx`. Stand: 01.11.2026. Kein Push. `crates/os-app/src/session/control.rs` und `session/merge.rs` sind unverändert (M2-FIX-STOP).

## Ausgangslage

Seit der OS-Festlegung vom 01.11.2026 zu CRAWL-126 führt `os_store::open::merge_wal` das WAL nach Pause und Ende im Hintergrund zurück: `PRAGMA wal_checkpoint(TRUNCATE)` über `checkpoint_within`, bis zu 100 ms je Versuch, danach 50 ms Pause, solange ein Leser einen älteren Stand hält. Während eines Versuchs hält die Rückführung die WAL-Schreibsperre. Eine verzögerte Transaktion, die zuerst liest und dann schreibt, bekommt beim Hochstufen sofort `SQLITE_BUSY`; SQLite ruft den Busy-Handler nur, solange die Verbindung noch keine Lesetransaktion hat. Transaktionen, deren erste Anweisung schreibt, und `BEGIN IMMEDIATE` warten dagegen im Busy-Timeout.

## 1. `os_store::queries::api::invalidate` – belegt, behoben

**Befund:** `crates/os-store/src/queries/api.rs:93–113` (Basis): `unchecked_transaction()` → `SELECT EXISTS` → UPDATE/DELETE/INSERT. Aufrufer `crates/os-app/src/commands/api.rs:913–923` (`api.setSettings`: `file.set_config` im Autocommit, danach `invalidate`) und `:1763` (`on_config_changed`, Hintergrund, Fehler wird verworfen).

**Beleg (vorher):** neuer Test `crates/os-store/tests/fix_tx_immediate.rs`. Eine zweite Verbindung hält `BEGIN IMMEDIATE` 200 ms; `invalidate` auf einer Verbindung mit Busy-Timeout 5 s:
- `invalidate_waits_for_a_held_write_lock_instead_of_failing_busy`: `Err(DatabaseBusy "database is locked") after 3.0 ms`.
- `invalidate_succeeds_while_the_wal_merge_waits_for_a_reader` (echtes `merge_wal` mit gehaltenem Leser, 10 Aufrufe): bereits der erste Aufruf `DatabaseBusy`.

**Änderung (`eedbf334`):** `invalidate` öffnet `Transaction::new_unchecked(conn, TransactionBehavior::Immediate)`. Läuft auf der Verbindung schon eine Transaktion, wird sie mitbenutzt (vorher: „cannot start a transaction within a transaction“). Damit schreibt `api.setSettings` Konfiguration und Invalidierung jetzt in **einem** IMMEDIATE-Commit (`bf7ec8b5`). Spec-Bezug INTEG-010: Bei „Behalten“ muss der Hinweis „Daten stammen aus einer früheren Konfiguration“ zur gespeicherten Konfiguration passen. Vorher konnte ein `err-db-busy` die neue Konfiguration ohne Hinweis bzw. ohne Löschung zurücklassen. Die Atomarität ist eine Folgerung aus INTEG-010, kein ausdrücklicher Spec-Satz (siehe offene Fragen).

**Nachher:** alle vier Store-Tests grün (wartet ≥ 100 ms, Ergebnis korrekt; Rollback des Aufrufers nimmt die Invalidierung mit).

**App-Test** `crates/os-app/tests/fix_tx_api.rs`: acht `api.setSettings`-„Behalten“-Änderungen während `merge_wal` mit gehaltenem Leser, danach Merge abgeschlossen, Konfiguration und Hinweis gespeichert. **Ehrliche Einordnung:** Dieser Test war auch vor dem Fix 3/3 grün. Das vorausgehende `set_config` schreibt im Autocommit und wartet dabei auf die Sperre; die Invalidierung folgt dann in derselben 50-ms-Lücke. Das Rennfenster auf diesem Weg ist schmal; deterministisch belegt ist der Mechanismus im Store-Test. Der App-Test sichert ab, dass die verschachtelte Transaktion (App-Commit + `invalidate`) im echten Pfad funktioniert. Auf dem zweiten Aufrufer `on_config_changed` (`:1763`) gibt es kein vorausgehendes Schreiben; dort ist das Fenster voll offen.

## 2. Crawl-Analyse vs. WAL-Rückführung – belegt (nur Bildschritt), behoben mit IMMEDIATE

**Test zuerst:** `crates/os-app/tests/fix_tx_analysis.rs`. Echte Session über `start_with_fetcher` mit `SimFetcher` (Startseite mit `style="background-image:url(/hero.png)"` und Bildlink, 20 Seiten à 40 ms, 1 Thread), `analysis.auto_at_end = true`, `images.parse_inline_css_backgrounds = true`. Nach 3 URLs hält ein fremder Leser einen Snapshot. Der Crawl endet, `settle` startet `merge::start` und die automatische Analyse. Erwartet: Analyse-Operation `Done`, `analysis_state = completed`, Bildschritt `done`, Flag `image_background` für `hero.png`. Nach dem Ende des Lesers schließt der weiterlaufende Merge ab, und die WAL-Datei ist kleiner als vorher (Analyse-Frames mit zurückgeführt).

**Beleg (vorher):** 6 Läufe, **5 rot** mit `CmdError { code: Internal, detail: "SQLite-Fehler: database is locked" }` (Analyse `Failed` über `first_error`, `runner.rs:257`), einer zufällig grün (Hochstufen in die 50-ms-Lücke).

**Entkräftet für die übrigen Schritte:** In den ersten vier Vorher-Läufen wurde `hero.png` mangels Link nicht gecrawlt. Der Bildschritt fand deshalb nichts zu schreiben, und alle übrigen Standardschritte liefen trotz laufendem Merge durch (4/4 grün). Code-Befund: `canonicals`, `sitemaps`, `pagination` (`BEGIN IMMEDIATE`), `hreflang`, `links`, `link_score` (IMMEDIATE-Veröffentlichung; ihre verzögerten Transaktionen schreiben nur in TEMP), `near_duplicates` (SAVEPOINT, erste Anweisung `DELETE`) und `persist::*` (einzelne Autocommit-Anweisungen) sind nicht betroffen. Die in der Aufgabe genannten `aggregates/live.rs:511` und `steps/link_score.rs:116` lesen MAIN und schreiben nur TEMP. Sie stufen MAIN nie hoch und sind unkritisch. Betroffen ist allein `steps/images.rs:17`.

**Entscheidung: IMMEDIATE im Bildschritt statt `merge::cancel` beim Analyse-Start.** Begründung:
1. Nur eine Analyse-Transaktion ist betroffen; die übrigen Schreiber warten schon heute im Busy-Timeout.
2. `merge::cancel` vor der Analyse ließe den Merge unvollendet. Der abschließende TRUNCATE-Checkpoint der Analyse (`runner.rs:277`) wartet höchstens 5 s und meldet bei weiter gehaltenem Leser nur `busy=1`, ohne Fehler. Das WAL bliebe voll, ohne dass jemand die Rückführung neu anstößt. Dafür müsste `merge.rs` geändert werden, das gerade M2-FIX-STOP umbaut („again“-Runde).
3. Ein Abbruch schützt die übrigen Schreiber während der Pause nicht (API, SERP-Snippet, Konfigurationsänderung; siehe 3.).
4. Nebeneffekt: Die bisherige verzögerte Transaktion konnte auch ohne Merge mit `SQLITE_BUSY_SNAPSHOT` scheitern, wenn ein paralleler Schritt zwischen Lesen und Schreiben veröffentlichte (vgl. Kommentar in `near_duplicates.rs`). Mit IMMEDIATE hat die Veröffentlichung einen frischen Snapshot. Das ist nicht eigens getestet.

**Änderung (`14809036`):** `steps/images.rs` sammelt `d03_background` im Autocommit ohne Schreibsperre. `image_instances` sind Crawl-Daten, die kein Analyseschritt ändert. Vergleich (`d03_changed` gegen `url_flags`) und Veröffentlichung laufen unter `BEGIN IMMEDIATE`. Ein `Drop`-Wächter entfernt die TEMP-Tabellen auch bei Abbruch oder Fehler. Die Schreibsperre wird damit nicht während der Sammelabfrage über `image_instances` gehalten.

**Nachher:** `fix_tx_analysis` 6/6 grün (je ≈ 7 s). Deterministischer Zusatztest `crates/os-analyze/tests/fix_tx_immediate.rs::image_step_publishes_after_a_held_write_lock_instead_of_failing_busy`: vorher `database is locked after 0.9 ms`, nachher wartet er und veröffentlicht 2 Hintergrundbilder. Danach sind keine TEMP-Reste vorhanden. `tab_images_m2` (5 Tests, u. a. atomare Re-Analyse und Abbruch) unverändert grün.

**WAL nach der Analyse:** Der Merge wird nicht abgebrochen. Er läuft weiter, solange der Leser hält, und führt danach auch die Analyse-Frames zurück (im Test: 1.104.192 Byte vor dem Ende des Lesers, danach 0 Byte bzw. nur noch spätere Schreibzugriffe, siehe Zusatzbefund A).

## 3. Weitere verzögerte „lesen, dann schreiben“-Transaktionen

Suche: `grep -rn 'transaction()\|unchecked_transaction\|BEGIN\|new_unchecked\|TransactionBehavior\|savepoint()' crates/ --include='*.rs'` (ohne `tests/`), zusätzlich `conn.transaction()` (rusqlite: ebenfalls DEFERRED).

| Stelle | Bewertung |
|---|---|
| `os-store/src/queries/api.rs:93` `invalidate` | **betroffen** → behoben (IMMEDIATE, Punkt 1) |
| `os-store/src/queries/api.rs:123` `replace_values` | **betroffen im Randfall**: ohne erfolgreiche URL und ohne Werte erst `SELECT change_seq`, dann `UPDATE crawl_meta`. API-Abruf während der Pause ist ausdrücklich erlaubt → behoben (IMMEDIATE) |
| `os-store/src/queries/detail_ext.rs:46` `save_serp_edit` | **betroffen**: `SELECT EXISTS`, dann Schreiben (SERP-Snippet bearbeiten, UI-037, während der Rückführung) → behoben (IMMEDIATE), Test `serp_edit_waits_…`: vorher `database is locked after 14 ms` |
| `os-analyze/src/aggregates/status.rs:98` `on_config_changed` | **betroffen**: `get_meta`/`read_stale`, dann `set_meta` (Konfiguration während der Pause → Analyse-Schritte veraltet, UI-082; Fehler wurde nur geloggt) → behoben (IMMEDIATE), Test `config_change_marks_steps_stale_…`: vorher `database is locked after 9 ms` |
| `os-analyze/src/analysis/steps/images.rs:17` | **betroffen** → behoben (Punkt 2) |
| `os-app/src/session/control.rs:692` `stop_checkpoint` | **betroffen** (`f.meta()` vor `set_meta`) → Paket M2-FIX-STOP, hier nicht angefasst |
| `os-store/src/queries/api.rs:45` `record_error`, `:215` `store_input_rows` | unkritisch: erste Anweisung schreibt (`DELETE` bzw. `INSERT`) |
| `os-integrations/src/crawl_glue/runtime.rs:203`, `:411` | unkritisch: erste Anweisung `DELETE` |
| `os-analyze/src/tabs/content.rs:796` | unkritisch: erste Anweisung `DELETE` (die Ergebnisliste ist vorher materialisiert) |
| `os-analyze/src/analysis/steps/near_duplicates.rs` (SAVEPOINT `d02_publish`) | unkritisch: erste Anweisung `DELETE` |
| `os-analyze/src/analysis/steps/links.rs:85`, `link_score.rs:116` | unkritisch: schreiben nur TEMP; Veröffentlichung `:115`/`:196` IMMEDIATE |
| `os-analyze/src/aggregates/live.rs:334`, `:511`, `:601`, `:607` | unkritisch: lesen MAIN, schreiben nur TEMP (`temp.k03_*`) |
| `os-analyze/src/compare/change/mod.rs:112` | unkritisch: nur TEMP |
| `os-analyze/src/compare/change/auto.rs:107` | unkritisch: eigene Vergleichsdatenbank, erste Anweisung DDL |
| `hreflang.rs:32`, `canonicals.rs:45`, `sitemaps.rs:229`, `pagination.rs:74`, `os-store/src/writer.rs:676`, `profile.rs:450`, `os-engine/src/{rescan.rs:330,crawl.rs:496}` | bereits IMMEDIATE |
| `os-store/src/view_cache.rs:47` | schreibt zuerst, aber absichtlich mit Busy-Timeout 0: scheitert während der Rückführung still und speichert später (siehe Zusatzbefund A) |
| `os-store/src/open.rs:183` (neue Datei), `schema.rs:93` (Migration beim Öffnen), `files.rs:493` (Kopierziel) | unkritisch: neue bzw. noch nicht geteilte Datei, erste Anweisung schreibt |
| `os-store/src/registry/{index.rs:26,management.rs:83}` | unkritisch: Registry-Datenbank, nicht die Crawl-Datei |
| `os-app/src/pool.rs:250`, `session/feed.rs:214`, `commands/{detail.rs,detail_ext.rs,view.rs,catalog.rs}`, `views/mod.rs`, `os-analyze/src/{graph/load.rs,search/view.rs,views/*}`, `os-export/src/pipeline.rs:150`, `os-cli/src/segment_workbook.rs:19` | unkritisch: Lese-Snapshots (ReadPool nur `ReadOnly`), höchstens TEMP |
| `os-cli/src/upload.rs:38` | unkritisch: erste Anweisung `UPDATE` |
| `os-loadtest/src/gen_ui/mod.rs`, `os-engine/examples/crawl_child.rs`, `os-analyze/benches/near_dup.rs`, `analysis/runner.rs:412/467` (Testcode) | unkritisch: Generator/Beispiel/Benchmark/Test |

Nicht per Grep auffindbar: Autocommit-Schreibzugriffe, während auf derselben Verbindung noch eine Leseanweisung offen ist (Cursor-Schleife mit Schreiben im Rumpf). Dieses Muster ist auf den Analyse- und App-Schreibpfaden nicht systematisch geprüft. Die Analyse-Schritte materialisieren nach Durchsicht vor dem Schreiben bzw. schreiben nur TEMP.

## Zusatzbefunde (nicht behoben, nicht dieses Paket)

**A – WAL-Inhalt nach „Beenden“ trotz abgeschlossenem Merge.** Im App-Test blieb nach `quit::confirm` in 3 von 6 Läufen ein WAL von 57–66 KB. Diagnose: Die N21-Vorberechnung (`view_cache`-Schlüssel `n21:counts:{External,Internal,Response Codes,Security}`, `n21:summary`) speichert mit Busy-Timeout 0. Während der Rückführung scheitert sie, danach schreibt sie, also **nach** dem Ende des Merges. `quit` wartet nur auf laufende Merges (`quit.rs:108`). Damit ist das Akzeptanzkriterium „Beenden … danach hat die WAL-Datei keinen Inhalt“ (CRAWL-126) bei späteren Schreibern nicht gesichert. Vorschlag: nach `write_queue().flush()` in `quit::confirm` je beschreibbarer Datei noch eine Merge-Runde (`merge::start` + `wait_all` im Restbudget), Besitz K04/M2-FIX-STOP. Der Test prüft deshalb nur, dass der Merge die Analyse-Frames zurückführt (WAL kleiner), nicht „0 nach Beenden“.

**B – Abschluss-Checkpoint der Analyse blockiert bis 5 s.** `runner.rs:309–319` setzt für den finalen TRUNCATE-Checkpoint `busy_timeout = 5000`. Bei gehaltenem Leser verlängert das jede Analyse um ≈ 5 s (im Test sichtbar), obwohl der Hintergrund-Merge den Rest ohnehin übernimmt. Funktional unkritisch; optional auf `checkpoint_within(CHECKPOINT_READER_WAIT)` umstellen.

**C – Stopp aus „Pausiert“ + Auto-Analyse** ist hier nicht getestet: Der Pfad läuft über `stop_checkpoint` (M2-FIX-STOP). Nach der Integration beider Pakete sollte die Kombination „gehaltener Leser, Pause, Stopp, Auto-Analyse mit Bildschritt“ einmal gemeinsam laufen.

## Fremdänderungen (K2-92)

Keine der Dateien gehört einem parallel laufenden Paket. M2-FIX-STOP ändert `session/{control,merge}.rs`, `tests/pause_merge.rs` und die Statusleiste, M2-N24 Detail-/Hilfe-UI und `commands/view.rs`.

| Datei | Besitzer | Änderung (minimal) | Spec |
|---|---|---|---|
| `crates/os-store/src/queries/api.rs` | M2-I01 (Welle 1) | `invalidate`: IMMEDIATE bzw. Mitbenutzung einer offenen Transaktion; `replace_values`: `transaction_with_behavior(Immediate)` | CRAWL-126, INTEG-010 |
| `crates/os-store/src/queries/detail_ext.rs` | M2-K09 (K2-74) | `save_serp_edit`: `transaction_with_behavior(Immediate)` | CRAWL-126, UI-037 |
| `crates/os-analyze/src/analysis/steps/images.rs` | M2-D03 | Sammeln im Autocommit, Vergleich/Veröffentlichung IMMEDIATE, TEMP-Wächter | CRAWL-126, CRAWL-141, ONPAGE-178 |
| `crates/os-analyze/src/aggregates/status.rs` | M2-K03 | `on_config_changed`: IMMEDIATE statt `unchecked_transaction` | CRAWL-126, UI-082 |
| `crates/os-app/src/commands/api.rs` | M2-I01 | `api.setSettings`: `set_config` + `invalidate` in einer IMMEDIATE-Transaktion | INTEG-010, CRAWL-126 |

Neue eigene Dateien: `crates/os-store/tests/fix_tx_immediate.rs`, `crates/os-analyze/tests/fix_tx_immediate.rs`, `crates/os-app/tests/fix_tx_api.rs`, `crates/os-app/tests/fix_tx_analysis.rs`, dieser Bericht.

## Commits

- `eedbf334` store: Lesen-dann-Schreiben-Transaktionen mit BEGIN IMMEDIATE (CRAWL-126)
- `14809036` analyze: Bildschritt und Konfigurations-Invalidierung warten auf die Schreibsperre (CRAWL-126)
- `bf7ec8b5` app: API-Einstellungen und Invalidierung in einer IMMEDIATE-Transaktion (INTEG-010, CRAWL-126)
- docs: dieser Bericht

Server: `m2-fix-tx/target` (158 GB nach Workspace-Lauf) gelöscht; Platte danach 264 GB frei.

## Testzahlen

| Lauf | vorher (Basis + neue Tests) | nachher |
|---|---|---|
| `os-store --test fix_tx_immediate` | 0/4 grün (3× `DatabaseBusy`, 1× verschachtelte Transaktion) | 4/4 |
| `os-analyze --test fix_tx_immediate` | 0/2 grün (`database is locked`) | 2/2 |
| `os-app --test fix_tx_analysis`, 6 Wiederholungen | 1/6 grün (5× Analyse `Failed`: `database is locked`) | 6/6 |
| `os-app --test fix_tx_api`, 3 Wiederholungen | 3/3 grün (schmales Rennfenster, siehe 1.) | 3/3 (+ Gesamtlauf) |
| `cargo test --workspace` (ws-slot) | int-23: 4.074 bestanden, 0 Fehler, 49 Ignores | **4.082 bestanden, 0 Fehler, 49 Ignores**, 554 Testprogramme (neue Tests: 4 Store, 2 Analyse, 2 App) |
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | – | grün |
| `npm run -w apps/desktop typecheck && npm run -w apps/desktop test` | int-23: 100 Dateien, 882 Tests | grün; Vitest 100 Dateien, 882 Tests |

## Offene Spec-Fragen

1. **INTEG-010:** Müssen Konfigurationsänderung und Invalidierung atomar sein? Umgesetzt als ein Commit, weil sonst die gespeicherte Konfiguration ohne Hinweis „Daten stammen aus einer früheren Konfiguration“ bzw. ohne Löschung möglich war. Ein ausdrücklicher Spec-Satz fehlt.
2. **CRAWL-126:** Gilt „Beenden … danach hat die WAL-Datei keinen Inhalt“ auch für Schreibzugriffe, die erst **nach** dem Ende der Hintergrund-Rückführung stattfinden (View-Cache-Vorberechnung, Analyse-Folgeschreiber)? Wenn ja, fehlt eine abschließende Merge-Runde beim Beenden (Zusatzbefund A).
