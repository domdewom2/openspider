# M3 Integrationsrunde e – M2-Stand für die Beta

## Stand und Umfang

Linux-Build-Server, 31.10.2026, Branch `m3/int-e`. Basis `origin/m3/int-d` =
`7a7b4bb3e7a6c2c8fb89173e78e9b796e7b787fd`, integrierter M2-Stand
`origin/m2/int-21` = `a0f78a5b04719c58e9c7523da180f191f19ec7c3`.
Merge-Commit `8e18d71d`: `m3: Merge m2/int-21 (M2-Stand für die Beta)`.
Integrationskorrektur `73983789`: explizite Header-Optionen für den hinzugekommenen
CLI-Crawl-Path-Export. Kein Push, keine Tags, keine Veröffentlichung.

## Konfliktlösung und Semantikentscheidungen

### Textkonflikt: `crates/os-export/src/reports/redirects.rs`

Der einzige Git-Inhaltskonflikt betraf `start_ids`, nicht die Writer-Signatur:
M3-B01 hatte die SQL-Zeile formatiert, M2 hatte ihre Auswahl geändert. Übernommen
wurde M2s Auswahl aller erfolgreichen Listen-URLs in Listenreihenfolge, auch ohne
Weiterleitung. Nur so bleibt OUT-085 erhalten: der Chains-Report enthält solche
Listenstarts; All Redirects filtert weiterhin auf mindestens einen Sprung und der
Errors-Report verwirft Ketten ohne Sprung. Die automatisch übernommenen M2-Filter
bleiben erhalten, ebenso M3s `write_report_with_options` und explizite Optionen.
Keine Änderung an Kettenberechnung, Sortierung oder Spalten.

### (a) B01-Header in den seit `40031ed9` geänderten M2-Exportpfaden

Geprüft wurden die vollständigen M2-Differenzen in `os-export/src`: `links_export`,
`catalog/bulk_images`, Reports `canonical_chains`, `non_indexable_canonicals`,
`redirects`, `serp_summary`, Hreflang-Tests und `sitemap_gen/entries`; zusätzlich der
neue CLI-Aufruf in `exec.rs`. Die Q02b-/N14-Fachänderungen bleiben erhalten.

- Bulk-/Report-Pipeline reicht `WriterOptions` über `export_*_with_options`,
  `catalog::export_table_with_options` und `reports::save_report_in_with_options`
  bis an CSV/XLSX/XLS weiter. Die geänderten Tabellen-/Kettenroutinen schreiben auf
  diesen Writer; N14s `export_link_entry_with_options` erhält `opts` unverändert.
- Dynamische Kettenheader werden vor dem Schreiben erzeugt; ihre Übersetzung
  findet weiterhin am Writer statt. Sitemap-XML hat keine Tabellenheader.
- Semantischer Konflikt im automatisch gemergten CLI: M2s neuer Crawl-Path-Aufruf
  verwendete noch `csv::with_header_labels`, das B01 entfernt hat. Der erste
  Clippy-Lauf fand E0425 in `exec.rs:819`. Korrektur: CLI ruft
  `reports::crawl_path::export_detail_with_options` mit `labeled_writer_options`
  auf. Der bisherige `export_detail` delegiert mit den bisherigen Standardoptionen.
  Keine Wiedereinführung eines Thread-Local-Scope, keine fachliche Datenänderung.
- Zusätzliche direkte CLI-Probe auf einer temporären Kopie der Schema-v5-Fixture:
  CSV und XLSX jeweils mit Default, `canonical` und `ui`/Deutsch; je zwei Datenzeilen.
  Alle sechs Fälle korrekt, `Address`/`Status Code` bzw. `Adresse`/`Statuscode`.
  Default und canonical sind als CSV byte-gleich; lokalisierte CSV-Datenbytes und
  XLSX-Datenzellen sind unverändert. Das Prüfskript liegt außerhalb des Repos.
- Bestehender `os-cli/tests/n11_crawl_path.rs` prüft weiterhin Auswahlreihenfolge
  und Byte-Gleichheit von CLI und direktem Surface-Export; B01s Writer-Scope-Tests
  und die Header-Matrix für CSV/XLSX/XLS bleiben unverändert.

Keine eigenen Test-, Fixture-, Golden- oder Baseline-Anpassungen; K2-63 wurde nicht
in Anspruch genommen. M2s mitgebrachte Tests/Baselines wurden unverändert integriert.

### (b) N18-Quittung und M3-U02 Update/Quit

`pause_or_stop` signalisiert frühzeitig, wartet danach aber unter dem gemeinsamen
`SessionService::mutation`-Lock auf `settle`. `settle` nimmt den Handle, wartet
`h.wait()` inklusive dauerhaftem Engine-Checkpoint ab, schreibt den finalen
Progress über die Schreibqueue und veröffentlicht erst dann `feed::settled`, Tick
und Lifecycle-Event. Bereits vom Poller gejointe Handles werden quittiert, ohne
nochmals eine Pause zu verlangen. Ein lokaler Handle hält den Feed bis zum Join auf
Running; ein terminaler Watch-Wert allein reicht nicht aus.

M3-U02s `execute` behält Gate und denselben Mutations-Lock, prüft laufende Sessions
und aktive Operationen, settelt verbliebene Handles, flusht die Schreibqueue und
checkpointet alle schreibbaren Sessions vor `backend.install`. Der Lock verhindert
Installation zwischen Zustandswechsel und Progress-Schreibabschluss. `quitting`
unterdrückt Auto-Analyse beim kontrollierten Beenden; `AfterCrawl` wartet auch auf
pausierte Sessions, `OnQuit` läuft nach `request_quit`. N18 ändert diese Sperren und
Reihenfolge nicht. Entscheidung: automatischen Merge von `control.rs`/`feed.rs`
beibehalten; keine zusätzliche Zustands- oder Quit-Logik nötig.

### (c) Workflows

M2s `ui-e2e.yml` enthält beide N17-Voraussetzungen: Bau von CLI/Testsite/Parity vor
E2E und native WebKit-Tastaturnavigation auf macOS. Die vier Browser/OS-Jobs bleiben
erhalten. `ci.yml` nutzt `macos-15` für ARM und `macos-15-intel` für Intel;
`spike-grid.yml` übernimmt ebenfalls M2s Runner-Korrektur. M3s `release*.yml`,
`smoke-installed.yml`, `package-manager-smoke.yml` und `publish-packages.yml` bleiben
neben den M2-Workflows erhalten. actionlint prüft das gesamte Workflow-Verzeichnis
mit der vorhandenen Repository-Konfiguration, ohne zusätzliche Ausnahmen.
Push-Filter für M2-Paketbranches wurden nicht umgedeutet; M3 nutzt die bestehenden
Main-/PR-/Dispatch- bzw. Release-Trigger. Native CI-Läufe dieses lokalen Standes
wurden wegen des ausdrücklich ausgeschlossenen Pushs nicht gestartet.

### (d) Abhängigkeiten und Desktop

`Cargo.lock` automatisch zusammengeführt und mit `cargo metadata --locked`,
`cargo tree --locked -d` sowie `cargo tree --locked -p openspider-desktop -i rust-ini`
geprüft. Gegenüber M3-int-d ausschließlich `rust-ini 0.21.3 → 0.21.1` und neu
`trim-in-place 0.1.8`; keine weiteren Paketversionsänderungen. Tokio bleibt
`1.53.3`. M3s `hmac` als `os-fetch`-Abhängigkeit sowie Bundle-/Updater-/Release-
Abhängigkeiten bleiben erhalten. Der Baum enthält 99 bereits vorhandene mehrfach
aufgelöste Paketnamen; durch die Integration entsteht kein neuer mehrfach
aufgelöster Paketname. Der Desktop-Pfad lautet
`openspider-desktop → tauri-plugin-deep-link 2.6.2 → rust-ini 0.21.1`.
Kein `cargo update`, keine Regenerierung des Lockfiles.

## Linux-Prüfung

Vor Workspace-Läufen `~/ws-slot.sh`; schwere Befehle mit `timeout 3600s nice -n 10`,
Cargo `-j 8 --locked`, ohne Desktop-Ausschluss. `CARGO_INCREMENTAL=0`, Debugprofile
`line-tables-only`, `OPENSPIDER_SECRET_STORE=memory` und Chromium
`/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`.
Playwright erst nach dem Bau von `os-testsite`, `os-cli`, `os-parity` (`--bins`) und
`os-app-bridge` (`--features e2e-bridge`). Visual-Tests werden mit ihrer eigenen
Baseline-/DPI-Konfiguration separat von der Hauptsuite ausgeführt; beide mit vier
Workern, Hauptsuite Chromium und WebKit einschließlich beta/a11y/update.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -j 8 --locked -- -D warnings` | grün, Desktop eingeschlossen; erster Lauf E0425, anschließend Header-Konflikt korrigiert |
| `cargo test --workspace --no-fail-fast -j 8 --locked` | 4.083 bestanden, 3 fehlgeschlagen, 49 ignoriert, 564 Testziele inkl. Doc-Tests; ca. 20 min (Summe gemeldeter Ziel-Laufzeiten 1.184,34 s), Exit 101; nur die zwei `compat_archive`-Testziele rot |
| Desktop Typecheck / Vitest | grün; 92 Dateien, 864 Tests |
| `check:static` / `check:licenses` | grün; statische Vertragstests 2 Dateien / 29 Tests; 476 npm-Pakete unter der Positivliste |
| Playwright-Hauptsuite, Chromium + WebKit, 4 Worker | 990 bestanden, 3 übersprungen, 1 Erstfehler, 28,4 min; Einzelwiederholung 1/1 grün in 2,9 s |
| Playwright Visual, 4 Worker | 152 bestanden, 5 übersprungen, 16 Screenshot-Abweichungen, 3,6 min; Baselines unverändert |
| Direkte Crawl-Path-Headerprobe | 6/6 Fälle grün, CSV/XLSX × Default/canonical/Deutsch |
| `os-parity run --all` | 218/218 verglichen und grün, 3 vorgesehene Auslassungen, 10 Sites, 0 Fehler, 0 Warnungen |
| `os-parity coverage --parity-report …` | grün, verified 129/155 = 83,226 % |
| Node Release/Migrations/Homebrew/Lizenzgenerator | 30/30 grün nach Bereitstellung von `zip` |
| Python winget | 3/3 grün |
| Smoke-Vertragstests | 5/5 grün |
| Linux Installed-Smoke `--dry-run` | grün: Text 0,49 s, JavaScript 9,42 s, isolierte JS-Probe, Sitemap und direkte `.ostask`; Text-Median 40,31 URLs/s (18 URLs, inkl. Prozessstart) |
| `node website/check-site.mjs` | grün, 18 Seiten |
| `python3 -I docs/plan/M2-check.py` | grün, 131 Pakete, 590 spec_ids; kein M3-Prüfskript vorhanden |
| actionlint 1.7.12 | gesamtes Workflow-Verzeichnis grün |
| `bash scripts/check-neutral.sh` | grün |
| `cargo deny check licenses bans sources` | alle drei Prüfungen grün |

Die drei Hauptsuite-Auslassungen betreffen die nur für Chromium vorgesehenen
Windows-DPI-Skalierungen unter WebKit. Der einmalige Chromium-Fehler war
`e2e/crawls/compare.pending.spec.ts:11`: innerhalb von 5 s kein `compare.open`
beobachtet. Derselbe Fall bestand im WebKit-Hauptlauf und einmal isoliert in
Chromium (`--workers=1`, eigener Ausgabeordner); keine Test-/Timeoutänderung.
Der verbleibende MCP-Test wartete wie vorgesehen zehn echte Minuten auf den
Session-Ablauf und bestand. Der erste Hauptlauf bleibt als Exit 1 ausgewiesen.

Die fünf Visual-Auslassungen sind der vorhandene `test.fixme` für die vollständigen
es/fr/it-Kataloge nach K2-79, einmal je Browser-/DPI-Projekt. Die 16 Screenshotfälle
scheitern jeweils schon am ersten Bild (`shell`); sie sind keine Lastflakes.

`zip` war zunächst nicht installiert: 29/30 Node-Tests grün, fehlgeschlagen war der
Archivierungsvertrag. Installation mit sudo war nicht verfügbar. Statt einer
Projektänderung wurde das Distributionspaket mit `apt-get download zip` unter
`/tmp/m3-int-e-tools` entpackt und nur für den Wiederholungslauf dessen `usr/bin`
vorne in `PATH` gesetzt: danach 30/30 grün. Kein System-/Repository-Umbau.

## Offene Erwartungskonflikte und vorbereitete K2-63-Kandidaten

Die ausdrückliche Vorgabe „Goldens unverändert“ bleibt eingehalten. Im Chat wurde
für die folgenden konkreten Erwartungsänderungen eine Entscheidung angefragt;
bis zu einer Antwort werden sie nicht übernommen. Keine Abschwächung der Tests,
keine Toleranzänderung, keine Abschaltung der neuen M2/M3-Funktionalität.

1. **Archiv-CSV:** `tests/fixtures/migrations/schema-v{1,2,3,4,5}/internal_all.csv`
   stammt aus M3-int-b. M2 macht gemäß ONPAGE-019 vier Inhaltsspalten sichtbar:
   `Flesch Reading Ease Score`, `Readability`, `Closest Similarity Match`,
   `No. Near Duplicates`; alle acht zugehörigen Datenzellen je Datei sind leer.
   Außerdem wird `Meta Keywords 2 Length` für die HTML-200-Seite von leer zu `0`
   (M2s `COALESCE`, ONPAGE-110); alle übrigen bestehenden Zellen und die zwei Zeilen je Datei
   bleiben identisch. Kandidaten aus temporären Crawl-Kopien liegen ausschließlich
   unter `/tmp/m3-int-e-candidate-goldens/schema-v*.csv`; Zellvergleich unter
   `/tmp/m3-int-e-logs/golden-diff.log`. Der App-Archivtest reproduziert den Fehler
   auch einzeln (0/1), somit kein Lastflake. Dieselben Goldens betreffen zwei
   CLI-Archivtests. Weder Archivdatenbanken noch Testlogik wurden verändert.
2. **Linux-Visual-Baselines:** Unter
   `apps/desktop/e2e/visual/baselines/linux/{chromium-1,webkit-2}/screenshots.spec.ts/`
   benötigen genau 32 Bilder eine Erwartungsfortschreibung: je acht `shell-*`
   (`Nicht verfügbare Menüeinträge` 38 → 37, weil M3-U02 `help.updates` aktiviert)
   und acht `settings-*` (neue Updates-Rubrik), pro Browser. In Chromium ist die
   Shell-Differenz ausschließlich innerhalb x=888…894/y=625…632; 43 rohe Pixel,
   davon 13/14 oberhalb der Playwright-Vergleichsschwelle. Die Einstellungen
   unterscheiden sich durch genau den zusätzlichen Navigationseintrag. Das
   beabsichtigte Verhalten stammt aus M3-U02/OUT-250, nicht aus diesem Merge.
   Eine temporäre Kopie aller Baselines wurde mit unveränderter Testdatei und
   ausschließlich umgeleitetem Snapshot-Pfad geprüft: 16/16 Fälle grün, dabei alle
   sieben Szenen je Fall durchlaufen. Genau 32 Bilder ändern sich; die übrigen
   80 bleiben bytegleich. Kandidaten: `/tmp/m3-int-e-visual-candidates`; Liste:
   `/tmp/m3-int-e-logs/visual-candidate-changes.txt`. Keine dieser Änderungen ist
   in die Repository-Baselines übernommen worden.

Die Kandidaten sind reine Erwartungsänderungen für bereits integriertes
spec-gemäßes Verhalten; eine Übernahme wäre nach K2-63 zu dokumentieren und danach
mit den unveränderten Vertragstests und der regulären Visual-Konfiguration erneut
zu prüfen. Mit eingefrorenen Erwartungen kann die geforderte vollständig grüne
Linux-Prüfung nicht erreicht werden. Der Stand ist deshalb noch nicht als grüner
Beta-Build freigegeben.

Die native macOS-/Windows-Matrix, Installationsprüfung und Veröffentlichung liegen
beim Orchestrator; sie wurden in diesem lokalen Paket nicht ausgelöst. Der
Linux-Smoke ist ausdrücklich `dry_run=true` und ersetzt keine OS-Zeitplanung.

Laufprotokolle und Kandidaten liegen außerhalb des Repositorys unter
`/tmp/m3-int-e-logs`, `/tmp/m3-int-e-parity`, `/tmp/m3-int-e-smoke` und den oben
genannten Kandidatenpfaden. `target/` wurde nach Abschluss aller Prüfungen entfernt.


## Abschluss

Merge und Header-Integrationskorrektur sind committet, der Arbeitsstand enthält
keine eigenen Golden-/Baseline-Änderungen. Der vollständige Rust-Lauf wurde trotz
der Archivfehler bis einschließlich aller übrigen Ziele und Doc-Tests beendet;
keine weiteren Rust-Fehler und keine Rust-Lastflakes. Coverage wurde anschließend
mit diesem vollständigen Log erneut ausgeführt: weiterhin 129/155 verified, Exit 0.

**PAKET OFFEN:** Nächster Schritt ist die Entscheidung über die konkret vorbereitete
K2-63-Fortschreibung der fünf Archiv-CSVs und 32 Linux-Screenshot-Baselines. Bei
Freigabe genau diese Erwartungen übernehmen, Vertragstests und reguläre Visual-
Suite erneut grün prüfen und Bericht ergänzen; andernfalls bleiben die Befunde
offen. Kein Push. Dieser Stand erfüllt das grüne Beta-Gate noch nicht.
