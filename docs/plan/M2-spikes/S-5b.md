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

## Befund 2: Referenz A und Produktbedarf

Referenz A: Mac mini M1, 16 GiB, macOS 26.3 (25D125), arm64, Node 22.20.0.
Die Vorprüfung dieser Runde bestätigt weiterhin `screenLocked=true`,
`onConsole=false`, `postEventAccess=false`. Entsperren und Eingabeberechtigung
wurden beim Nutzer angefragt; sie lassen sich nicht durch einen Codefix ersetzen.
Die C11-Abnahme benötigt S3, Smoke und S6 in entsperrter GUI-Sitzung.
Die nativen Gates bleiben offen; S3 ist Merge-Bedingung für U04/U11.

Der bestehende Produktbedarf bleibt getrennt: Der lokale Smoke der Vorrunde
lieferte bei laufender Fixture tatsächlich
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
