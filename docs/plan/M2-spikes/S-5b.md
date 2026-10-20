# S-5b – CI ausgewertet, Startdetektor korrigiert, native Freigabe offen

Stand: 20.10.2026, M2-S02, Fixrunde zu Fable `reviews-r5/M2-S02.md`,
Branch `m2/s02`, Basis `6e31faa` (enthält Main `3f6fbbe` und K04).
**Paket-DoD offen; E2E-B macOS nicht freigegeben. Kein E2E-C-Rückfall beschlossen.**

## Reproduktion

`apps/desktop/e2e-b/run.mjs` nutzt `webdriverio@7.40.0` gegen die echte App:
macOS mit `e2e-webdriver`, Windows mit `tauri-driver` 2.1.0 und passendem
WebView2-Treiber. App-Home und Memory-Secret-Store sind isoliert; der laufende
Crawl verwendet ausschließlich eine Loopback-Fixture. Kein App-Command wird ersetzt.

```sh
npm ci
cargo build --locked -p openspider-desktop --features e2e-webdriver
swiftc apps/desktop/spikes/macos.swift -o "$TMPDIR/s02-macos"
cp "${CARGO_TARGET_DIR:-target}/debug/openspider-desktop" "$TMPDIR/s02-desktop"
sh scripts/bench/wait_quiet.sh
S02_GUI=1 S02_MAC_HELPER="$TMPDIR/s02-macos" caffeinate -di \
  node apps/desktop/e2e-b/run.mjs "$TMPDIR/s02-desktop" smoke
```

Die Kopie schützt den Feature-Build vor nachfolgenden Cargo-Builds. Der Runner
startet seinen eigenen Vite auf Port 1420 (strictPort). Reports enthalten
UTC-Start, Plattform, Revision, Runner-/Probe-/Readiness-Hash, Dauer,
App-Antworten und Checks. Fehler liefern Exit 1. `app.requestQuit` und der
sichtbare Schließen-Dialog sind getrennte Checks; `Confirm` allein genügt nicht.

## Befund 1: abgeschlossene CI-Läufe und Fehlerursache

Alle folgenden Ergebnisse gehören zu **6e31faaffe1214fd0e3dabceeb363b4f73c31834**;
sie belegen noch keinen CI-Nachlauf des in dieser Runde korrigierten Runners.

| Lauf | Job / Job-ID | Ergebnis | Artefakt / ID | Runner-Dauer |
|---|---|---|---|---:|
| [39376824496](https://github.com/domdewom2/openspider/actions/runs/39376824496) | `native-smoke (windows-latest)` / 118476726446 | failure | `s02-smoke-windows-latest` / 12243648073 | 30,148 s |
| 39376824496 | `native-smoke (macos-15)` / 118476726592 | failure | `s02-smoke-macos-15` / 12244121295 | 30,182 s |
| [39376824502](https://github.com/domdewom2/openspider/actions/runs/39376824502) | `menu / native-s3 (windows-latest)` / 118476727246 | failure | `s02-s3-windows-latest` / 12244425976 | 30,213 s |
| 39376824502 | `menu / native-s3 (macos-15)` / 118476727038 | failure | `s02-s3-macos-15` / 12243598042 | 30,259 s |
| 39376824502 | `visibility / native-s6 (macos-15)` / 118476727254 | failure | `s02-s6-macos-15` / 12243872067 | 30,262 s |
| [39376824390](https://github.com/domdewom2/openspider/actions/runs/39376824390) | `linux-core (ubuntu-latest)` / 118476726496 | success | `openspider-linux-x86_64` / 12244766172 | — |
| [39376824418](https://github.com/domdewom2/openspider/actions/runs/39376824418) | `e2e-a (windows-chromium)` / 118476726468 | success | `playwright-report-windows-chromium` / 12244500786 | — |
| 39376824418 | `e2e-a (linux-both)` / 118476726491 | success | `playwright-report-linux-both` / 12243803060 | — |
| 39376824418 | `e2e-a (macos-webkit)` / 118476726659 | success | `playwright-report-macos-webkit` / 12244557300 | — |

Bei allen fünf nativen Jobs sind Build, Lizenzprüfung, Spike-Typecheck und
Node-Tests grün; `Native check` ist rot. Der komplette Check-/Fehler-Auszug
jedes `report.json` lautet:

```json
{"checks":[],"errors":["Error: Vite ready: timed out"],"status":"failed"}
```

Insbesondere wurden `main window and real IPC`, `active native desktop`,
`separate settings window`, `app.requestQuit confirms a running fixture crawl`
und `closing during a running fixture crawl asks before quitting` **nicht
ausgeführt**. Deshalb gibt es aus Windows keine `Confirm`-Antwort und keinen
belegten Dialogfehler. Die Erwartung im Review ist noch unbestätigt; dieser
CI-Abbruch ist ein **S02-Runnerfehler**, kein nachgewiesener Produktfehler.

Alle fünf heruntergeladenen Artefakt-ZIPs stimmen mit GitHubs SHA256 überein.
Unveränderte Reports und Logs: `apps/desktop/spikes/evidence/review5-ci/`;
Metadaten aller vier Läufe: `review5-ci.json`. Die Logs zeigen tatsächlich
bereiten Vite, dessen ANSI-Farbcodes beide bisherigen Suchtexte unterbrechen.
`readiness.mjs` normalisiert diese Ausgabe und prüft die vollständige lokale
URL. Regression: alle fünf CI-Logs farbig/farblos, Chunk-Grenzen, falscher Port
und unvollständige Meldungen. **7 Node-Tests grün**, einschließlich der vier
unveränderten S6-Tests. Echter Vite-Start unter CI-Farbumgebung: alter Detektor
falsch, neuer richtig, HTTP 200. Der Workflow führt die Regression mit aus.
Ein korrigierter Windows-/macOS-CI-Nachlauf steht wegen des Push-Verbots aus.

## Befund 3: temporäre Push-Trigger entfernt

Nach Auswertung aller vier Läufe wurden `on.push.branches: [m2/s02]` samt
temporärem Kommentar aus `ui-e2e-b.yml` und `spike-platform.yml` entfernt,
in einem eigenen Commit. Nightly, `workflow_dispatch` und `workflow_call`
bleiben erhalten. Beide Workflows bestehen `actionlint`; die Regressionstests
bleiben Teil des nativen Workflows. Keine Planer-Ausnahme zum Behalten nötig.

## Befund 2: Referenz A und Produktbedarf

Referenz A: Mac mini M1, 16 GiB, macOS 26.3 (25D125), arm64, Node 22.20.0.
Die Vorprüfung dieser Runde bestätigt weiterhin `screenLocked=true`,
`onConsole=false`, `postEventAccess=false`. Entsperren und Eingabeberechtigung
wurden beim Nutzer angefragt; sie lassen sich nicht durch einen Codefix ersetzen.
Die C11-Abnahme benötigt S3, Smoke und S6 in entsperrter GUI-Sitzung.
Die nativen Gates bleiben offen; S3 ist Merge-Bedingung für U04/U11.

**Aktueller Nachlauf:** LaunchServices, frischer Feature-Build, Revision
`06ea8be`, erzwungene CI-Farbausgabe; UTC-Start
**2026-10-20T08:24:29.547Z**, Dauer **36,666 s**, Exit 1.
`smoke-gui.json` wurde durch diesen aktuellen Report ersetzt.

| Check (exakter Reportname) | Status / Beleg |
|---|---|
| `main window and real IPC` | passed |
| `active native desktop` | passed: Frontend sichtbar und fokussiert |
| `separate settings window` | passed |
| `app.requestQuit confirms a running fixture crawl` | passed: `{"kind":"confirm","reasons":["crawl"]}` |
| `closing during a running fixture crawl asks before quitting` | failed: kein Rückfragedialog innerhalb von 30 s |

Finale native Aktivierung: `active=true`, `postEventAccess=false`,
`screenLocked=false`, `onConsole=false`. Die Sitzung wurde während der ersten
S6-Aufnahme entsperrt; der Agent hat diese Änderung nicht veranlasst. S3 und
Smoke wurden deshalb erneut ausgeführt. Der finale S3-Lauf (4,899 s) erreicht
jetzt die native Tasteneingabe; beide Kürzel-Checks scheitern ausdrücklich an
`Native key injection unavailable (PostEvent access)`.

Die vorgeschaltete Ruheprüfung `wait_quiet.sh 2 1` scheiterte nach 60,08 s mit
Load **3,49 ≥ 2**; nach S6 lag Load1 bei 12,33. Alle Modi laufen unter
`caffeinate -di`, gelten unter diesen Bedingungen aber ausschließlich als
Diagnosen. Die ersten S3-/Smoke-Nachläufe (4,666/35,824 s) hatten noch
`screenLocked=true`, `active=false` und versteckte WebView. Ihre Ergebnisse
bleiben in `review5-validation.json`; die drei `*-gui.json` enthalten jeweils
den letzten Lauf. S6 hat jetzt zwei bestandene Ratenchecks (sichtbar/minimiert),
jedoch weiterhin Restore-/Occlusion-Fehler, siehe [S-6](S-6.md).

Die geforderte vollständige Referenz-A-Abnahme bleibt **offen**: PostEvent-
Berechtigung und Ruhebedingung fehlen. Keine Rückmeldung zur angefragten
Berechtigung lag bis zum letzten Nachlauf vor.

Der bestehende Produktbedarf wird durch den aktuellen Smoke erneut belegt:
Er lieferte bei laufender Fixture tatsächlich
`{"kind":"confirm","reasons":["crawl"]}`, aber keinen sichtbaren Rückfragedialog.
`apps/desktop/src-tauri/src/window.rs::request_quit` beendet bei `Quit`, stellt
bei `Confirm` jedoch keinen Dialog dar; `apps/desktop/src/app/windows.ts::quitApp`
schließt lediglich das Fenster. Planer/N01 bzw. das zuständige Hüllen-/UI-Paket
muss die Anzeige/Weiterleitung ergänzen. S02 macht daraus weder einen Skip noch
eine schwächere Assertion. Der Windows-Lauf ist erst nach dem Startfix auswertbar.
S6-Restore-/Occlusion-Bedarf steht in [S-6](S-6.md).

## Übernommene Entscheidungen und Besitz

Der SPDX-/Override-Bedarf aus Review 3 bleibt bei N01/Planer:
`apps/desktop/scripts/check-licenses.mjs` muss geklammerte SPDX-Ausdrücke korrekt
verarbeiten und fehlende Metadaten über begründete versionsgebundene Overrides
lösen. Keine manuelle Lockfile-Lizenzkorrektur. Ein WebdriverIO-Upgrade erfordert
Lizenzprüfung des gesamten Graphen und native Transportprüfung beider Plattformen.
Audit-Stand der Vorrunde: 7 Befunde (6 hoch, 1 moderat); keine Sicherheitsfreigabe.

Diese Runde ändert ausschließlich S02-`owns`. **Keine Fremdergänzungen K2-61,
keine Stub-/Test-Erwartungsänderungen K2-60/K2-63**, keine neuen Abhängigkeiten,
keine Änderungen an Manifesten/Lockfiles, keine Hüllen-/UI-Fachänderungen.
Clean-Room eingehalten. `M2-S02.spec_ids=[]`; die Spike-Bezüge
UI-002/UI-113/OUT-222/OUT-232/UI-143 werden nicht als vollständig abgenommen markiert.
Frühere Runden samt Prüfzeiten bleiben über `6e31faa` nachvollziehbar.

## Abschlussprüfungen R5

| Prüfung | Ergebnis | Laufzeit |
|---|---|---:|
| `npm ci` | grün, Lockfile unverändert; weiterhin 7 Audit-Befunde (6 hoch, 1 moderat) | 5,02 s |
| `npm run -w apps/desktop typecheck` | grün | 1,30 s |
| `npm run -w apps/desktop test` | grün, 162 Tests | 5,78 s |
| `npm run -w apps/desktop check:static` | grün, inkl. Lizenzen und 28 Tests; nach dem Fix erneut grün | 1,25 s (erster Lauf) |
| `cargo fmt --all --check` | grün | 1,71 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün | 125,87 s |
| `cargo test --workspace` | grün, 2.819 bestanden, 0 fehlgeschlagen, 68 bestehende Ignores, inklusive Desktop und Doc-Tests | 630,86 s (vollständiger erfolgreicher Nachlauf) |
| `cargo deny check licenses bans sources` | grün | 1,77 s |
| Desktop-Feature-Build `--locked --features e2e-webdriver` | grün | 41,93 s |
| Node-Fixture-Tests | grün, 7 Tests | < 1 s |
| Echter Vite-Start mit `CI=true`, `FORCE_COLOR=1`, ohne `NO_COLOR` | neuer Detektor grün, alter Detektor falsch, HTTP 200 | 0,22 s |
| Spike-Typecheck, Node-Syntax, Swift-Kompilierung, beide Workflows/actionlint, Diff-Whitespace | grün | kurze Einzelchecks |
| Neutralitätsprüfung | grün; vier geerbte verwaiste Allowlist-Einträge als Warnungen | < 1 s |

Prüfumgebung: Rust/Cargo 1.99.1, Node 22.20.0, Memory-Secret-Store.
Ein temporärer `python3`-Link nutzt das vorhandene Homebrew-Python 3.10.13 /
SQLite 3.43.2 wie in der Vorrunde; keine Änderungen an Python-Lesertests.

**Ressourcenfehler und Wiederholungen offengelegt:** Der erste Workspace-Lauf
brach nach 529,94 s beim unveränderten
`crawl_cmd_lifecycle::fixture_pause_copy_reopen_resume_matches_uninterrupted_crawl`
mit `crawl.resume: Store / disk I/O error` ab. Anschließend sank der freie
Speicher auf rund 300 MiB. Ein gezielter Paket-Nachlauf wurde vor Testausführung
abgebrochen. Nur das eigene Target wurde per `cargo clean` bereinigt (27 GiB).

Abweichung von C5 ausschließlich in der lokalen Prüfumgebung: Für Wiederholung
und Feature-Build wurden `CARGO_PROFILE_DEV_DEBUG=0` und
`CARGO_PROFILE_TEST_DEBUG=0` gesetzt; `CARGO_INCREMENTAL=0` blieb gesetzt.
Die erste Wiederholung scheiterte nach 298,89 s mit Linker-`errno=28` (kein
Speicherplatz). 282 bereits erzeugte eigene Mach-O-Programme wurden mit
`strip` verkleinert, lokal ad hoc neu signiert und mit `codesign --verify`
geprüft (3.920.385.104 Bytes eingespart). Der nächste Lauf scheiterte nach
245,37 s an einem aus dem fehlgeschlagenen `rust-objcopy` verbliebenen
2.560-Byte-Dateifragment für `cmd_api_framework` (`Permission denied`, nie
ausgeführt). Genau dieses erzeugte Artefakt und sein Cargo-Fingerprint wurden
entfernt. Der darauf folgende **vollständige** `cargo test --workspace` bestand
alle Tests, einschließlich Lifecycle und `cmd_api_framework`, ohne
Compilerwarnungen. Es wurden keine Tests gefiltert, deaktiviert oder angepasst.
Die ursprüngliche SQLite-Ursache ist damit nicht abschließend bewiesen;
der Fehler trat im vollständigen Nachlauf mit freiem Speicher nicht mehr auf.

Nach dem grünen Gesamtlauf wurden ausschließlich die 345 ausgeführten eigenen
Testprogramme entfernt (12.726.012.112 Bytes), um Platz für den anschließend
grünen Feature-Build zu schaffen. Keine fremden Worktrees, Targets oder
Repository-Dateien wurden bereinigt. Alle Versuche, Profile, Ergebnisse und
Laufzeiten stehen in `apps/desktop/spikes/evidence/review5-validation.json`.

## Abschluss je R5-Befund

1. **CI-Auswertung und S02-Startfehler behoben:** alle vier Läufe beendet
   ausgewertet, fünf Originalreports/-logs mit Artefakt-IDs und geprüften
   ZIP-Digests gesichert. ANSI-Startdetektor korrigiert; sieben Fixture-Tests,
   echter farbiger Vite-Start und lokale native App-Starts bestanden. Windows
   benötigt weiterhin einen Nachlauf des korrigierten Runners; der alte Lauf
   enthält keine App-Antwort und belegt keinen Windows-Dialogfehler.
2. **Teilweise nachgemessen, Freigabe weiterhin blockiert:** S3, Smoke und S6
   frisch ausgeführt; nach Entsperrung S3/Smoke nochmals ausgeführt und alle drei
   Evidenz-JSONs aktualisiert. Smoke 4/5 grün, sichtbare/minimierte S6-Raten grün;
   S3 scheitert nun konkret an fehlender PostEvent-Berechtigung. Ruhebedingung,
   vollständiger nativer Nachweis, Dialog-/Restore-/Occlusion-Bedarf und grünes
   Windows-S3 bleiben offen. S-3 und U04/U11 werden ausdrücklich nicht freigegeben.
3. **Behoben:** beide temporären Push-Trigger in eigenem Commit `06ea8be`
   entfernt; beide Workflows mit `actionlint` geprüft.

Alle beauftragten Cargo-/npm-Pflichtprüfungen sind abschließend grün. Der
Ressourcenverlauf und die roten nativen Checks sind vollständig offengelegt.
Target-Größe nach Testartefakt-Bereinigung und Feature-Build: **6,3 GiB**.
Nur eigene Pfade committet, keine Fremdergänzungen oder fremden
Test-Erwartungsänderungen. **Kein Push, kein PR.**
