# B4-LISTFIX – Beschädigung von Crawl-Dateien neben der Crawl-Verwaltung – Bericht

Stand 06.11.2026. Branch `fix/list-crawls-corruption` (Basis `main` e3c10e72), Mac-Worktree `openspider-wt/c-listfix`,
Server-Worktree `m3-listfix`. Befund 1 aus `M3-reports/b4-resume-tests.md` (`m3/resume-tests`). Der Fix hängt nicht von
`m3/resume` ab.

## Ursache (belegt)

Zwei Fehler, die erst zusammen schaden:

1. **Sperrverlust beim Besitzer (POSIX):** `check_sqlite_header`, `probe_header` und der Fingerprint des Metadaten-Index lasen
   die Crawl-Datei per `std::fs::File::open`. Schließt ein Prozess irgendeinen Deskriptor einer Datei, gibt POSIX **alle** seine
   Sperren darauf frei, auch die gemeinsame Sperre seiner laufenden SQLite-Verbindung. Das geschieht im Betrieb ständig:
   Analyse-Worker und `crawl_status` (`CrawlFile::open`), `--json-progress`, `list_crawls` und die Verwaltung im eigenen Prozess.
2. **Aufräumen beim anderen Prozess:** `open_raw_metadata` öffnete mit Schreibrechten („als letzte Verbindung entfernt SQLite
   `-wal`/`-shm`“). Beim Schließen bekommt diese Verbindung die Exklusivsperre, weil der Besitzer seine verloren hat, hält sich
   für die letzte, führt das WAL zurück und löscht `-wal`/`-shm`.

Der Besitzer schreibt in das gelöschte WAL weiter; seine Commits gehen verloren, die Datei ist beschädigt (`wrong # of entries in
index`, `btreeInitPage() returns error code 11`, `database disk image is malformed`, `disk I/O error`).

**Beleg ohne Zeitabhängigkeit** (`crates/os-store/tests/wal_live_owner.rs`, zwei echte Prozesse), Stand vor dem Fix:

| Test | Vor dem Fix | Aussage |
|---|---|---|
| `another_process_listing_keeps_the_wal_of_a_live_owner` | grün | Fehler 2 allein schadet nicht |
| `…_after_the_owner_process_reopened_the_file` | rot | Fehler 1 + 2: `-wal` weg |
| `…_after_the_owner_process_listed_its_folder` | rot | ebenso, Auslöser ist die Verwaltung im Besitzerprozess |
| `reading_metadata_never_writes_or_merges_a_foreign_wal` | rot | Der Listenlauf führt das fremde WAL zurück |

Korrektur zum Zwischenbericht: `…_listed_its_folder` schlug zunächst aus einem Testfehler fehl (Dateiname ohne `<id>.ospider`).
Nach der Korrektur ging er auf dem alten Stand zuerst durch, weil der zweite Prozess den Index-Treffer nutzte und die Datei gar nicht
öffnete. Er liest deshalb jetzt auch ohne Index. Erst damit ist er auf dem alten Stand aus dem richtigen Grund rot. Die übrigen
Aussagen des Zwischenberichts bleiben gültig.

## Betroffenheit

Rot auf dem ungefixten Stand e3c10e72 (`crates/os-cli/tests/listing_live_owner.rs`, `crates/os-app/tests/wal_live_owner_app.rs`,
je echte Prozesse):

| Weg | Vor dem Fix | Nach dem Fix |
|---|---|---|
| `--list-crawls` neben endendem MCP-Crawl (Test unverändert aus `m3/resume-tests`) | rot | grün |
| `--list-crawls` neben MCP-Stopp (Test unverändert) | rot | grün |
| `--list-crawls` plus `crawl_status`-Polling mitten im Crawl | rot | grün |
| `list_crawls` im zweiten MCP-Server | rot, `integrity_check` schlägt an | grün |
| `list_crawls` im selben Server, kein zweiter Prozess | grün | grün |
| CLI-Crawl plus `--list-crawls` | rot, `integrity_check` schlägt an | grün |
| CLI-Crawl mit `--json-progress` plus `--list-crawls` | rot | grün |
| App: Crawl-Verwaltung neben Fremdprozess (Besitzer simuliert, echter zweiter Prozess) | rot, `-wal` weg | grün |
| App-eigener Crawl: App listet, anderer Prozess liest daneben | rot, `-wal` weg | grün |
| App: fremden Crawl öffnen und pollen | grün | grün |

- **Voraussetzung:** zwei Prozesse auf derselben laufenden Datei. Ein Prozess allein ist sicher, weil SQLite Verbindungen im selben
  Prozess selbst verfolgt.
- **Geplante Aufgaben:** laufen als `openspider --run-task` (derselbe CLI-Crawl), `run_task.rs` listet außerdem selbst. Nicht
  eigens getestet.
- **Aufbewahrungsrunde der App:** nutzt denselben `Manager`. Nicht eigens getestet.
- **Versionen:** Store-Test auf `v1.0.0-beta.2` ausgeführt, dieselben Tests rot. beta.1 und `rel/beta3`: gleicher Code gelesen, nicht
  ausgeführt. Der Fehler besteht seit M1 (13.10.2026).
- **macOS:** gleiche POSIX-Sperren, daher vermutlich betroffen. Auf dem Mac nicht ausgeführt (Regel). **Windows:** vermutlich nicht
  betroffen, siehe unten.

## Fix

- **Kein `std::fs`-Öffnen einer Crawl-Datei unter Unix:** `open::read_header` liest den Kopf über das VFS von SQLite. SQLite kennt die
  Sperren je Inode und schiebt das Schließen auf, solange welche bestehen. Benutzt von `check_sqlite_header`, `probe_header`
  und dem Fingerprint (`probe_header` liefert den Kopf jetzt zurück, kein zweites Lesen).
- **Die Verwaltung liest nie mit Schreibrechten:** `files::MetadataConn`/`open_raw_metadata`. Mit Begleitdateien schreibgeschützt unter
  den normalen Sperren, sie lässt sie liegen. Ohne Begleitdateien `immutable`, damit die Verwaltung weiterhin nichts hinterlässt.
- **Schreibrechte nur noch gewollt:** `open_raw_write` für `put` (Umbenennen, Projekt) und den Checkpoint vor dem Löschen. Beide
  stehen hinter `guard`, das laufende Crawls abweist.
- `copy_snapshot` (Duplizieren, Export) und `read_listing` prüfen die Quelle nach dem Lesen, siehe nächster Abschnitt.

### Geprüft: Wettlauf mit `immutable`

Frage: Datei ohne Begleitdateien, der Besitzer startet danach.

- **Den Besitzer stören:** nicht möglich. `immutable` nimmt keine Sperren, legt nichts an und schreibt nichts. Auch das Schließen im selben
  Prozess läuft über das VFS und hebt keine Sperren auf.
- **Falsche Daten:** ohne Gegenmaßnahme möglich, solange der Besitzer während des Lesens zurückführt. Solange er nur ins WAL schreibt, sieht
  der Leser den alten, in sich stimmigen Stand; führt er während des Lesens zurück, ändert sich die Hauptdatei unter dem Leser. Mein erster Stand
  (Zwischenbericht) hatte diese Lücke. **Jetzt:** `MetadataConn::still_valid` prüft **nach** dem Lesen, dass weiter keine Begleitdateien liegen und
  Größe, Änderungszeit und Kopf der Hauptdatei gleich sind. Sonst wird neu gelesen, zuletzt unter Sperren. `copy_snapshot` veröffentlicht
  in dem Fall die Kopie nicht und kopiert unter Sperren neu.
- **Tests (Prüfnaht `read_listing_checked`):**
  - `a_writer_starting_during_the_read_is_seen_and_not_disturbed`: Besitzer startet zwischen Lesen und Prüfung und bleibt offen. Ergebnis ist
    sein Stand, sein WAL bleibt, er schreibt weiter, `integrity_check` ok.
  - `a_writer_that_started_and_finished_during_the_read_is_seen`: Schreiber ist schon wieder fertig. Ergebnis ist sein Stand, keine Begleitdateien.
  - `a_quiet_file_is_read_without_companions_and_stays_valid`.
- **Restrisiko:** Ein Schreiber, der in einer Lesezeit von wenigen Millisekunden startet, zurückführt und endet, ohne dass sich Größe, Kopf oder
  `mtime` (Raster bis 10 ms unter Linux) ändern. Dann bliebe nur eine Auflistung veraltet. Die Datei wird davon nicht berührt, und der
  Index speichert solche Ergebnisse wegen seines eigenen Vergleichs nur bei gleichem Fingerprint.

### Geprüft: Windows

- **Kopf-Lesepfad:** unter Windows bleibt es beim Weg über `std::fs` wie bisher (`read_header_plain`). Sperren gehören dort dem Handle
  (`LockFileEx`), ein weiteres Handle hebt sie nicht auf. Der Fingerprint liest den Kopf jetzt nur einmal statt zweimal, die Werte sind gleich.
- **Nachweis hier:** `read_header_plain` wird auch unter Linux im Test übersetzt und mit dem VFS-Weg verglichen
  (`header_reads_agree_for_short_empty_foreign_and_real_files`).
- **`immutable` unter Windows:** nutzt dieselbe `win32-longpath`-VFS wie `open_sqlite`, der Pfad steht prozentkodiert in der URI und wird
  zu derselben Zeichenkette dekodiert. Schlägt das Öffnen fehl, fällt `open_raw_metadata` auf die schreibgeschützte Verbindung zurück.
- **Nicht ausgeführt:** Auf dem Server gibt es kein Windows-Ziel. Die neuen Tests laufen erst in der Windows-CI. **Bitte dort vor der
  Veröffentlichung prüfen**, vor allem `wal_live_owner` und `paths_registry` (keine Begleitdateien).

## Testergebnisse (Server, Worktree `m3-listfix`)

- `cargo fmt --all --check`: ok. `cargo clippy --workspace --all-targets -- -D warnings`: ok.
- `cargo test --workspace --exclude openspider-desktop -j 6 --locked --no-fail-fast` (über `ws-slot.sh`): **4 378 bestanden, 0 fehlgeschlagen,
  49 ignoriert**, 604 Testprogramme, Exit 0 (keine neuen Ignores).
- `cargo test -p os-store -p os-engine -p os-cli -p os-mcp -p os-app --no-fail-fast`: **1 656 bestanden, 0 fehlgeschlagen, 16 ignoriert**,
  261 Testprogramme.
- **20× unter Last, 20/20 grün:** auf CPU 0–3 gepinnt, dort vier `yes`-Prozesse (mit `timeout`, danach beendet, 0 Reste). Je Lauf
  `wal_live_owner` (7 Tests einschließlich Rolle), `wal_live_owner_app` (5, zwei davon Prozessrollen ohne eigene Prüfung im Elternlauf),
  `listing_live_owner` (7 Tests, nacheinander; die zwei übernommenen Tests sind per Vergleich des Quelltexts unverändert). Dauer je Lauf:
  rund 1,2 s, 0,7 s und 45,5 s.
- **Gegenprobe auf dem alten Quelltext** (die fünf `os-store`-Dateien von `main`, Tests vom Fix): `wal_live_owner` 4 grün, 3 rot, wie oben; die
  CLI- und App-Tests siehe Tabelle „Betroffenheit“.
- Neu in `os-store` (Einheitentests): `registry::tests` 3/3, `open::header_tests` 2/2 (auch Pfade mit Leerzeichen, `#`, `%`, `?`, `&`, `=`, Umlaut).
- Nicht ausgeführt: Playwright, Vitest (kein Frontend berührt), macOS und Windows (siehe oben).
- Der erste Fix-Stand war auf demselben Weg grün, wurde danach aber um die Prüfung nach dem Lesen ergänzt. Die Zahlen oben gelten für den Endstand.
- Beobachtung am Rand, nicht geändert: `cargo test -p os-sched` allein übersetzt nicht (`Uuid::new_v4` fehlt, das Merkmal `v4` kommt nur über andere
  Crates). Im Workspace-Lauf unschädlich.

## Nebenwirkungen und offene Punkte

- **Begleitdateien bleiben länger liegen:** Liest die Verwaltung gerade unter Sperren, wenn der Besitzer schließt, kann dieser nicht aufräumen.
  `-wal`/`-shm` bleiben dann liegen, bis die nächste Schreibverbindung kommt. Die Engine führt das WAL vorher zurück; Daten gehen nicht verloren.
- **Übrige `std::fs`-Zugriffe auf Crawl-Dateien:** nur noch `std::fs::copy` für die Sicherungskopie vor einer Migration (`open_inner`). Eine ältere
  Datei läuft zu diesem Zeitpunkt nicht, deshalb unverändert. Der Rest des Workspace wurde per `git grep` durchsucht, kein weiterer Fund.
- **Schon beschädigte Dateien** repariert der Fix nicht; dafür bleibt die Reparatur-Kopie der Verwaltung.
- `m3/resume-tests` enthält dieselben zwei Tests in `acceptance_resume.rs`; beim Merge ist die Doppelung harmlos (andere Datei).
- Spec: CRAWL-125 und OUT-228 decken das Verhalten ab, keine Spec-Frage.

## Fremde Dateien (K2-92)

| Datei | Änderung | Grund |
|---|---|---|
| `crates/os-app/tests/wal_live_owner_app.rs` | neu | Betroffenheit App |
| `crates/os-cli/tests/listing_live_owner.rs` | neu | die zwei Tests unverändert, plus je Aufrufweg ein Test |

Produktivcode nur in `crates/os-store/src/{open,files,registry}.rs` und `registry/{index,management}.rs`.
