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

## Spec-Lücke (Vorschlag, nicht umgesetzt)

CRAWL-126/UI-003 regeln nicht, wie „Stoppen“ aus „Pausiert“ mit dem laufenden Hintergrund-Merge umgeht. Vorschlag für
eine OS-Festlegung:

> „Stoppen“ aus „Pausiert“ wartet weder auf die Rückführung des WAL noch bricht es sie ab. Der Stopp-Commit wartet
> höchstens auf die Schreibsperre (Busy-Timeout). Die Rückführung läuft danach im Hintergrund weiter bzw. wird nach dem
> Stopp-Commit neu angestoßen. Die Statusleiste zeigt bis dahin „Gestoppt – Datei wird zusammengeführt“. Beenden und
> „Im Finder zeigen“ warten wie nach der Pause (3 s bzw. 2 s).

Damit verbunden, ebenfalls offen und nicht umgesetzt: Nach einem Stop aus „Pausiert“ stößt `stop_checkpoint` – anders
als `settle` nach dem Stop eines laufenden Crawls – keinen neuen Merge an. Läuft beim Stop noch ein Merge (wie im Test),
führt er auch die Stopp-Frames zurück. Ist er schon beendet, bleiben die wenigen Stopp-Frames bis zum nächsten
Checkpoint oder Schließen im WAL. Ob das gewollt ist, gehört zur obigen Festlegung.

## Folgerisiko außerhalb dieses Pakets

Dasselbe Muster (verzögerte Transaktion, die erst liest und dann schreibt) findet sich noch in
`os_store::queries::api::invalidate` (`SELECT EXISTS` vor dem Löschen bzw. Aktualisieren). Laut CRAWL-126 dürfen
API-Daten während einer Pause angefordert werden, also während ein Merge laufen kann. Ob dieser Pfad dabei über die
Schreibwarteschlange läuft, wurde hier nicht geprüft. Empfehlung: in einem eigenen Paket auf `BEGIN IMMEDIATE`
umstellen oder belegen, dass der Pfad nicht betroffen ist.
