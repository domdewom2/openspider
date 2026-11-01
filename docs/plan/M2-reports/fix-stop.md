# M2-FIX-STOP – „Stop“ aus „Pausiert“ scheitert während des Hintergrund-Merges

Basis: `main` df02958d. Auslöser: ui-e2e-Lauf 41303595347, Job windows-chromium (124839385175),
`apps/desktop/e2e/beta/control.spec.ts:59` – nach Pause → „Resume“ aktiv → Klick auf „Stop“ bleibt `crawl-phase`
15 s auf „Paused“ (beide Versuche).

## Ursache (mit Beleg)

**Es ist kein Warten auf den Merge und kein Rennen in UI oder Tick, sondern ein sofortiger SQLite-Fehler.** Der Stop-Befehl
scheitert mit `SQLITE_BUSY` („database is locked“), weil der Hintergrund-Merge die Schreibsperre des WAL hält.

Belege aus dem CI-Lauf:

- Call-Log der fehlgeschlagenen Erwartung: nach dem Stop-Klick 5× (Retry: 9×) `Paused – merging file`, danach nur noch
  `Paused`. Der Merge lief also zum Zeitpunkt des Stop-Klicks.
- Letzter DOM-Snapshot im Trace (`trace.zip`, `1-trace.trace`): Dialog „Crawl notice“ mit `Cause: Busy` und den
  technischen Details `database is locked`. Der Stop-Befehl kam also mit einem Fehler zurück; die Phase blieb deshalb
  dauerhaft „Paused“.

Mechanismus:

1. Nach der Pause nimmt `settle` den Engine-Handle weg. Ein Stop aus „Pausiert“ läuft deshalb über
   `pause_or_stop` → `settle(.., Some(true))` → `stop_checkpoint` (`crates/os-app/src/session/control.rs`).
2. `stop_checkpoint` öffnete eine **verzögerte** Transaktion (`unchecked_transaction()` = `BEGIN DEFERRED`), las
   zuerst `crawl_meta` (`f.meta()`) und schrieb danach fünf Metadaten.
3. Der Merge (`os_store::open::merge_wal`, CRAWL-126) ruft in einer Schleife `PRAGMA wal_checkpoint(TRUNCATE)` auf, je
   mit höchstens 100 ms Wartezeit auf Leser, dann 50 ms Pause. Ein TRUNCATE-Checkpoint hält während dieser Wartezeit die
   WAL-Schreibsperre (SQLite: Modus FULL/RESTART/TRUNCATE „blocks new database writers while it is pending“). Solange
   ein Leser einen Snapshot hält (hier die UI-Leser von Feed/Übersicht während „Pausiert“), hält der Merge die
   Schreibsperre also etwa zwei Drittel der Zeit.
4. Eine Verbindung, die bereits eine Lesetransaktion hält und auf Schreiben hochstufen will, ruft in SQLite den
   Busy-Handler **nicht** auf (`sqlite3BtreeBeginTrans` wiederholt nur bei `inTransaction == TRANS_NONE`). Der
   Busy-Timeout von 5 s der Schreibverbindung wirkt hier nicht; das UPDATE scheitert sofort mit `SQLITE_BUSY`.
   Ein erneuter Klick hätte zufällig funktionieren können, der Test klickt aber nur einmal.

Reproduktion ohne E2E: neuer Rust-Test (unten) auf dem ungefixten Stand **10 von 10 Läufen rot**, jeweils mit
`CmdError { code: Busy, message_key: "err-db-busy", detail: Some("database is locked") }`, also exakt dem Fehler aus
dem Windows-Lauf.

### Warum vor allem Windows (und früher linux-webkit)

Ob der Fehler auftritt, hängt allein davon ab, ob der Merge beim Stop-Klick noch läuft. Er läuft, solange UI-Leser
Snapshots halten. Auf dem Windows-Runner sind diese Lesezugriffe deutlich langsamer (NTFS, Virenscanner, langsamere
Runner, `Sleep`-Granularität von etwa 15,6 ms in den Busy-Schleifen beider Seiten). Der Merge war dort noch 1–2 s nach
dem Klick sichtbar aktiv („Paused – merging file“ über 5–9 Abfragen). Auf schnellen Linux-Läufen ist er nach den 500 ms,
die der Test vor jedem IPC-Aufruf wartet, meist schon fertig. Die SQLite-Sperrsemantik selbst (`LockFileEx` auf der
`-shm`-Datei unter Windows, `fcntl` unter Unix) ist auf allen Plattformen gleich. Der Fehler ist plattformneutral und
wird auf Windows nur viel häufiger getroffen. WebKit liest langsamer als Chromium, daher der frühere Treffer auf
linux-webkit.

## Was die Spezifikation verlangt

- UI-003 (K2-77): „Stoppen“ beendet einen laufenden **oder pausierten** Crawl endgültig; die Schaltfläche bleibt im
  Zwischenzustand „Stoppe …“, bis der Kern den neuen Zustand meldet. Ein Stop aus „Pausiert“ muss also gelingen.
- CRAWL-126 (OS-Festlegung 01.11.2026): Der Merge läuft im Hintergrund; „Beenden der App“ wartet bis 3 s, „Im Finder
  zeigen“ bis 2 s. **Zu „Stoppen“ sagt die Spezifikation nichts**: weder, ob Stop den Merge abbricht, noch, ob er auf ihn
  wartet (siehe Spec-Lücke).

Der Fix beschränkt sich deshalb auf das eindeutig Spec-konforme: Der Stop-Schreibvorgang darf an der Sperre des Merges
nicht scheitern. Er bricht den Merge nicht ab und wartet nicht auf dessen Ende.

## Fix

`stop_checkpoint` öffnet seine Transaktion jetzt mit `BEGIN IMMEDIATE`
(`rusqlite::Transaction::new_unchecked(.., TransactionBehavior::Immediate)`). Die Schreibsperre wird damit vor dem
Lesen angefordert. Dabei greift der normale Busy-Timeout (5 s) der Schreibverbindung, die in der kurzen Pause zwischen
zwei Merge-Versuchen (≤ 100 ms Sperre, 50 ms frei) zum Zug kommt. Dasselbe Muster nutzt bereits
`os_store::profile::config_transaction` (Kommentar zu `SQLITE_BUSY_SNAPSHOT`). Ein Commit mit fünf Metadaten bleibt
erhalten (eine Platten-Synchronisierung). Budgets, Timeouts und Testlogik wurden nicht geändert (K2-63).

Commits (lokal, Branch `m2/fix-stop`):

- `app: Stop aus Pausiert holt die Schreibsperre vor dem Lesen (CRAWL-126)` – `crates/os-app/src/session/control.rs`
- `test: Stop während des WAL-Merges mit gehaltenem Leser (CRAWL-126, UI-003)` – `crates/os-app/tests/pause_merge.rs`
- `docs: M2-FIX-STOP-Bericht` – dieser Bericht

## Test

`crates/os-app/tests/pause_merge.rs::stop_from_paused_succeeds_while_the_merge_waits_for_a_reader`
(`// spec: CRAWL-126`, `// spec: UI-003`): sechs Runden mit je eigenem Crawl auf `crawl-site-100` (30 ms je Seite,
1 Thread). Ein fremder Leser hält einen älteren Snapshot, dann „Pause“ (WAL nicht leer, Merge läuft). Darauf folgt
„Stop“ mit 0–100 ms Versatz. Erwartet: Stop gelingt, die Datei steht auf `Completed` mit `end_reason = stopped`.
Danach endet der Leser, und der Test wartet auf den Merge. Mehrere Runden machen das Treffen des Sperrfensters praktisch
sicher; vor dem Fix scheiterte schon Runde 0 oder 1.

## Messungen (Server, Linux, 16 Kerne)

Rust-Test `stop_from_paused…`:

| Stand | Läufe | Ergebnis |
|---|---|---|
| vor dem Fix | 10 | 10 rot (`err-db-busy`, „database is locked“; 9× Runde 0, 1× Runde 1) |
| nach dem Fix | 10 + 10 | 20 grün (ganze Datei `pause_merge`: 3/3) |

E2E `beta/control.spec.ts`, Chromium + WebKit, `--repeat-each=10`, `--retries=0`, `CI=true`, eigener Vite-Port:

| Konfiguration | vor dem Fix | nach dem Fix |
|---|---|---|
| ohne Begrenzung, 2 Worker | 20/20 grün | 20/20 grün |
| `taskset -c 0-3`, 1 Worker | 20/20 grün | 19/20 (1× Zeile 44, siehe unten) |
| `taskset -c 0-3`, 4 Worker (Überlast) | 7/20; darunter **1× Zeile 59 mit `Cause: Busy`** | 10/20; **0× Zeile 59, 0× „database is locked“** |
| `taskset -c 0-1`, 2 Worker (Überlast) | 1/20, alle Fehler vor dem Stop-Schritt | – |

Unter Linux trifft der E2E-Test den Fehler nur selten (1 Treffer mit Busy in 40 begrenzten Läufen). Der Nachweis
stützt sich deshalb auf den deterministischen Rust-Test und den Trace aus dem Windows-Lauf. Die übrigen Fehler unter
Überlast liegen **vor** dem Stop-Schritt und kommen vor wie nach dem Fix vor: Zeile 56 (Pause-Bestätigung > 3000 ms),
Zeilen 42–44 (Zwischenzustand „Pausing …“/„Stop“ gesperrt, bereits bestätigt, bevor der Matcher prüft) und Zeile 11. Das
ist Lastrauschen dieser Konfigurationen und keine Folge des Fixes. Zeile 44 ist ein eigenes, seltenes Rennen im Test
(die Pause wird schon bestätigt, bevor die Prüfung „Stop gesperrt“ läuft). Es wurde nicht angefasst.

Abnahme-Gates nach dem Fix (Server):

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`: grün.
- `cargo test --workspace` (über `~/ws-slot.sh`): grün, 4075 Tests bestanden, 0 fehlgeschlagen.
- `npm run -w apps/desktop typecheck`: grün. `npm run -w apps/desktop test`: 100 Dateien, 882 Tests grün.
- `beta/control.spec.ts` + `header/header.spec.ts`, Chromium + WebKit, `--repeat-each=5`, `--retries=0`: 30/30 grün.

Hinweis zum Vorgehen: Der erste Lauf „nach dem Fix“ prüfte noch den alten Stand. `srv-sync.sh` überträgt mit
`rsync -a` die lokale mtime, und die war älter als der gerade laufende Server-Build. Cargo hielt `os-app` deshalb für
aktuell. Alle Zahlen oben stammen aus Läufen nach einem nachweislich neuen Build (Bridge-Binärdatei vorher/nachher
verglichen). Für künftige Pakete: nach lokalen Änderungen während eines Server-Builds die geänderten Dateien vor dem
Sync per `touch` neu stempeln.

## Spec-Lücke → OS-Festlegung (entschieden)

Die im ersten Bericht gemeldete Lücke ist entschieden: Fable bewertete das Paket mit „APPROVED MIT AUFLAGEN“, und der
Orchestrator hat die Festlegung in f026087c in die Spezifikation übernommen. CRAWL-126 hat jetzt den Absatz
„OS-Festlegung ‚Stoppen‘ während der Rückführung“ und ein neues Akzeptanzkriterium; UI-003 verweist darauf. Kern:

- „Stoppen“ aus „Pausiert“ bricht eine laufende Rückführung weder ab noch wartet es auf sie. Der Stopp-Commit wartet
  höchstens auf die Schreibsperre (Busy-Timeout der Schreibwarteschlange, 5 s).
- Verbleiben danach Frames im WAL, auch nur die des Stopp-Commits, wird die Rückführung fortgesetzt bzw. neu
  angestoßen. Die Statusleiste zeigt bis dahin „Gestoppt – Datei wird zusammengeführt“.
- Beenden (3 s), „Im Finder zeigen“ (2 s) und Verschieben warten wie nach einer Pause; Löschen bricht ab. Dasselbe gilt
  für „Stoppen“ aus „Läuft“.

## Auflagen und Umsetzung (zweite Runde)

**Auflage B – Rückführung nach dem Stopp anstoßen.** `stop_checkpoint` ruft nach dem dauerhaften Commit
`super::merge::start(app, &s.key, s.path.clone())` auf, wie `settle`. `merge::start` ist dabei echt idempotent geworden:

- Läuft schon ein Merge (der von der Pause), wird kein zweiter gestartet. Der laufende erhält das neue Flag `again` und
  macht nach seinem Ende eine weitere Runde.
- Prüfen und Setzen von `again` in `start` und das Prüfen plus Fertigmelden im Merge-Task geschehen unter derselben
  Sperre der Merge-Tabelle. Dadurch geht keine Anforderung verloren.
- Ohne das Flag gäbe es ein kleines Rennen: Der laufende Merge leert das WAL, der Stopp-Commit schreibt danach neue
  Frames, `start` sieht den Merge noch als „läuft“ und tut nichts. Die Stopp-Frames blieben dann im WAL, was dem neuen
  Akzeptanzkriterium widerspräche.
- Ein Abbruch (`cancel`: Fortsetzen, MCP, Löschen) beendet den Merge wie bisher ohne weitere Runde.

**Beenden, Finder, Verschieben, Löschen nach einem Stopp.** Alle vier hängen am selben Merge-Eintrag je Crawl. Nach
Auflage B gibt es diesen Eintrag auch nach dem Stopp; es war keine weitere Änderung nötig.

| Aktion | Code | Verhalten |
|---|---|---|
| Beenden | `quit.rs`, `merge::wait_all` | höchstens 3 s |
| „Im Finder zeigen“ | `commands/crawls.rs`, `wait_merged_within` | höchstens 2 s, danach Hinweis `crawl-merge-pending` |
| Verschieben | `commands/crawls.rs`, `crawls.move`, `wait_merged` | wartet ohne Frist, wie nach einer Pause |
| Löschen | `commands/crawls.rs`, `crawls.delete`, `merge::cancel` | bricht ab |

Verschieben war also schon vorhanden; es gibt keine Lücke zu melden. Für den Stopp-Fall belegt das der Code, nicht ein
eigener Test: Die Pfade sind identisch, und die bestehenden Tests für Beenden und Finder decken den Merge-Eintrag ab.

**Statustext.** Der Kern meldet `merging` bereits unabhängig von der Phase (`session/mod.rs`, Tick). `Statusbar.tsx`
zeigt bei `endReason === "stopped"` und laufendem Merge jetzt `status-stopped-merging`:
„Gestoppt – Datei wird zusammengeführt“ bzw. „Stopped – merging file“ in `i18n/{de,en}.ftl`. Ohne Merge bleibt es bei
„Gestoppt“/„Stopped“.

**Tests.**

- `stop_from_paused_succeeds_while_the_merge_waits_for_a_reader` prüft zusätzlich: Nach Ende des Lesers und
  `wait_merged` ist `wal_bytes == 0`.
- Neu und deterministisch: `stop_from_paused_waits_for_the_writer_lock_and_merges_its_own_commit`
  (`// spec: CRAWL-126`, `// spec: UI-003`).
  - Ablauf: Pause, Merge abwarten, dann hält eine zweite Verbindung 200 ms lang `BEGIN IMMEDIATE`, währenddessen „Stop“.
  - Erwartet: Stop gelingt nach ≥ 150 ms Wartezeit, die Datei steht auf `Completed`/`stopped`.
  - Danach startet der Stopp einen neuen Merge, und das WAL wird ohne weitere Aktion leer.
- `Statusbar.test.tsx`: neuer Test (`// spec: CRAWL-126`, `// spec: UI-003`) für „Stopped – merging file“ →
  „Stopped“ und dieselbe Abfolge auf Deutsch.

Gegenproben auf dem Server mit absichtlich zurückgebautem Code:

- **Ohne `merge::start` in `stop_checkpoint`:** Der deterministische Test scheitert an `wal_bytes == 0` (3 von 3
  Läufen, WAL 8272 Byte). Die übrigen drei Tests bleiben grün.
- **Mit `TransactionBehavior::Deferred` statt `Immediate`:** Beide Stop-Tests scheitern mit `err-db-busy`,
  „database is locked“.

Commits der zweiten Runde (lokal, Branch `m2/fix-stop`, auf f026087c):

- `app: Stopp stößt die WAL-Rückführung an, laufender Merge macht eine weitere Runde (CRAWL-126)` –
  `crates/os-app/src/session/control.rs`, `crates/os-app/src/session/merge.rs`
- `test: Stopp mit gehaltener Schreibsperre und leerem WAL danach (CRAWL-126, UI-003)` –
  `crates/os-app/tests/pause_merge.rs`
- `ui: Statusleiste „Gestoppt – Datei wird zusammengeführt“ (CRAWL-126)` – `apps/desktop/src/features/statusbar/`
  (`Statusbar.tsx`, `Statusbar.test.tsx`, `i18n/de.ftl`, `i18n/en.ftl`)
- `docs: M2-FIX-STOP-Bericht um Auflagen und Festlegung ergänzen`

**K2-92 (Owns):** Alle in diesem Paket geänderten Dateien liegen in den Owns von M2-FIXE-PORT; der Orchestrator
vermerkt das in der Planung.

- `crates/os-app/src/session/control.rs`
- `crates/os-app/src/session/merge.rs`
- `crates/os-app/tests/pause_merge.rs`
- `apps/desktop/src/features/statusbar/Statusbar.tsx`
- `apps/desktop/src/features/statusbar/Statusbar.test.tsx`
- `apps/desktop/src/features/statusbar/i18n/de.ftl`
- `apps/desktop/src/features/statusbar/i18n/en.ftl`
- `docs/plan/M2-reports/fix-stop.md`

**Abnahme-Gates der zweiten Runde (Server):** grün.

- `cargo fmt --all --check` und `cargo clippy --workspace --all-targets -- -D warnings`: grün.
- `cargo test --workspace` (über `~/ws-slot.sh`): 4076 bestanden, 0 fehlgeschlagen.
- `npm run -w apps/desktop typecheck`: grün. `npm run -w apps/desktop test`: 100 Dateien, 883 Tests grün.
- `pause_merge` vollständig: 4/4 grün.
- E2E mit `--retries=0`, Chromium + WebKit:
  - `beta/control.spec.ts` + `header/header.spec.ts` je 5×: 30/30 grün.
  - `beta/control.spec.ts` 10×: 20/20 grün.
  - In keinem Lauf „database is locked“.
- Platte: Nach dem Workspace-Lauf war der Server knapp (18 GB frei). Ich habe die Testbinärdateien in
  `target/debug/deps` und `target/debug/incremental` gelöscht und danach nur Bridge und Werkzeuge neu gebaut. Kein Lauf
  scheiterte an Platzmangel.

Hinweis E2E: `control.spec.ts:59` wartet bis 15 s auf den exakten Text „Stopped“. Läuft der Merge nach dem Stopp noch,
steht zwischenzeitlich „Stopped – merging file“ da; das ist laut Festlegung richtig und bleibt im Zeitbudget, solange
die UI-Leser das WAL innerhalb von 15 s freigeben (im Windows-Lauf war der Merge nach 1–2 s beendet).

## Folgerisiko außerhalb dieses Pakets

Dasselbe Muster (verzögerte Transaktion, die erst liest und dann schreibt) findet sich noch in
`os_store::queries::api::invalidate` (`SELECT EXISTS` vor dem Löschen bzw. Aktualisieren). Laut CRAWL-126 dürfen
API-Daten während einer Pause angefordert werden, also während ein Merge laufen kann. Ob dieser Pfad dabei über die
Schreibwarteschlange läuft, wurde hier nicht geprüft. Empfehlung: in einem eigenen Paket auf `BEGIN IMMEDIATE`
umstellen oder belegen, dass der Pfad nicht betroffen ist.
