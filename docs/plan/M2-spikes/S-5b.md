# S-5b – Native Smoke-Nachprüfung nach Integration von K04

Stand: 19.10.2026, Review-3-Fixrunde, Branch `m2/s02`.
**Paket-DoD offen; E2E-B macOS nicht freigegeben, E2E-C-Rückfall nicht entschieden.**
Maßgeblich sind die Nachmessungen dieser Runde. Die vorherigen Befunde und
Prüfzeiten bleiben über Commit `0f21f7b` nachvollziehbar.

## Gerüst und Reproduktion

`apps/desktop/e2e-b/run.mjs` verwendet `webdriverio@7.40.0` als W3C-Client gegen
die echte App: macOS mit eingebettetem Server (`e2e-webdriver`), Windows mit
`tauri-driver` 2.1.0 und passendem WebView2-Treiber. App-Home und Memory-Secret-Store
sind isoliert; der Fixture-Crawl greift nur auf Loopback zu. Die Probe ersetzt
keinen App-Command. Fehlgeschlagene Checks ergeben weiterhin Exit 1.

```sh
npm ci
cargo build --locked -p openspider-desktop --features e2e-webdriver
swiftc apps/desktop/spikes/macos.swift -o "$TMPDIR/s02-macos"
# Feature-Binary vor weiteren Cargo-Builds sichern:
cp "${CARGO_TARGET_DIR:-target}/debug/openspider-desktop" "$TMPDIR/s02-desktop"
S02_GUI=1 S02_MAC_HELPER="$TMPDIR/s02-macos" \
  node apps/desktop/e2e-b/run.mjs "$TMPDIR/s02-desktop" smoke
```

Die Referenz-A-Abnahme verlangt eine entsperrte GUI-Sitzung; S3 braucht zusätzlich
PostEvent-/Bedienungshilfen-Berechtigung. Der Runner protokolliert Plattform,
UTC-Start, Revision, Runner-Hash, Laufzeit, native Aktivierung, App-Antworten und
Ergebnisse pro Check. Er prüft Hauptfenster, aktiven Desktop, separates
Einstellungsfenster, `app.requestQuit` bei laufendem Fixture-Crawl und danach den
sichtbaren Schließen-Dialog als **getrennte** Checks.
Die Startprüfung begrenzt einzelne WebDriver-Abfragen auf 5 s, damit ein
unbeantworteter Aufruf nicht das gesamte unveränderte 30-s-Bereitschaftsfenster
aufbraucht. Anzahl der Versuche und vorübergehende Fehler stehen unter `startup`
im Report. Nach erfolgreichem Start gilt wieder das normale 30-s-Scriptlimit. Eine erfolgreiche
`Confirm`-Antwort allein besteht nicht den Dialog-Check.

## Folgerunde Review 3 (19.10.2026)

Maßgeblicher Basisstand: `origin/main` **3f6fbbe**, integriert durch Merge
**5658122**. K04 (`477803c`) ist enthalten: `crawl.start` und `app.requestQuit`
sind echte Handler. Die in der Vorrunde dokumentierte `Unsupported`-Antwort beschreibt
nur den alten Stand; sie ist **keine aktuelle K04-Blockade**.

Der Merge war konfliktfrei, einschließlich npm-Manifest/Lockfile. Der vorhandene
WebdriverIO-Nachtrag blieb erhalten. `npm ci` lief ohne Lockfile-Änderung durch;
kein im Main-Lock vorhandenes Paket-/Versionspaar wurde entfernt. Keine neuen
manuellen Lizenzfelder, kein Upgrade und keine Änderungen fremder Test-Erwartungen.

### Befund 2: ausführbare Branch-Workflows

Beide eigenen Workflows haben zusätzlich `on.push.branches: [m2/s02]`, ausdrücklich
als temporär kommentiert. **Vor dem Merge entfernen**, sofern der Planer nicht
explizit den Verbleib für Nachläufe entscheidet. Nightly, Dispatch und der
reusable Workflow bleiben erhalten. `actionlint` prüft beide Dateien grün.

Der Auftrag verbietet Push und PR ausdrücklich. Deshalb wurden weder ein Push
noch neue CI-Läufe ausgelöst. Lauf-IDs, Windows-Laufzeiten und Artefakte für diesen
Fixstand sind **nicht vorhanden**. Erwartete Nachweise nach separat autorisiertem
Push: `native-smoke (windows-latest)`, `native-smoke (macos-15)` und
`native-s3 (windows-latest)`; Artefakte `s02-smoke-windows-latest` und
`s02-s3-windows-latest`, jeweils mit `report.json`. Alte grüne CI-Läufe belegen
den neuen Fixstand nicht. Das Windows-Gate bleibt offen.

### Befund 5: verbindlicher Schnittstellenbedarf an N01/Planer

1. **Lizenzmetadaten robust behandeln:**
   `apps/desktop/scripts/check-licenses.mjs` muss geklammerte SPDX-Ausdrücke
   korrekt parsen (u. a. `(MIT OR CC0-1.0)` für `type-fest@0.20.2`) bzw. eine
   begründete, versionsgebundene Override-Liste für fehlende Metadaten wie
   `css-value@0.0.1` erhalten. N01 prüft positive/negative AND-/OR-/Klammerfälle
   und die reproduzierbare Regeneration des Lockfiles. Anschließend sollen
   `npm install`/`npm ci` und `check:licenses` ohne manuelle Lizenzkorrekturen
   bestehen. Die aus der Vorrunde geerbten Felder bleiben hier unverändert;
   weitere Lockfile-Handarbeit ist ausdrücklich keine Lösung.
2. **WebdriverIO-Version entscheiden:** Planer/N01 entscheidet über 9.x/10.x
   erst nach Lizenzprüfung des vollständigen neuen Graphen, insbesondere
   `@promptbook/utils`, sowie erneuter nativer Transportprüfung auf macOS und
   Windows. `npm audit --json` bestätigt weiterhin **7 Befunde, davon 6 hoch
   und 1 moderat** (`devtools`, `extract-zip`, `puppeteer-core`, `tar-fs`, `uuid`,
   `webdriverio`, `ws`). Die Begrenzung auf Testcode/Loopback ist keine
   Sicherheitsfreigabe. Hier bleibt die beschlossene Version 7.40.0 unverändert.

Die bestehende hashgebundene Neutralitätsausnahme bleibt unverändert. Diese
Folgerunde ergänzt außerhalb `owns` keinen Code; Änderungen fremder Dateien
stammen ausschließlich aus dem ausdrücklich verlangten Main-Merge.


## Lokaler Nachlauf / Befunde 1 und 3

Referenz A: Mac mini M1 (`Macmini9,1`), 16 GiB, macOS 26.3 (25D125), arm64;
Node 22.20.0. LaunchServices mit `S02_GUI=1`, isolierter Home und unveränderter
Kopie des erfolgreichen Feature-Builds. Laufzeit: **39.905 s**, UTC-Start `2026-10-19T12:07:27.997Z`.
Der Nachlauf in `apps/desktop/spikes/evidence/smoke-gui.json` enthält die tatsächlichen Antworten:

| Check | Ergebnis |
|---|---|
| Hauptfenster + echtes IPC | grün |
| Aktiver nativer Desktop | rot: Frontend `hidden`, Fokus `false`; Sitzung gesperrt |
| Separates Einstellungsfenster | grün |
| Fixture-Crawl + `app.requestQuit` | grün: Crawl-Datei angelegt, HTTP-Anfragen und `running`-Ticks; Antwort `{"kind":"confirm","reasons":["crawl"]}` |
| Schließen bei laufendem Crawl | rot: Fenster bleibt erhalten, aber kein Rückfragedialog innerhalb von 30 s |

Damit ist der veraltete K04-Blocker beseitigt und die Kernentscheidung belegt.
Der konkrete neue Integrationsbedarf ist **die Anzeige/Weiterleitung der
Schließen-Rückfrage**: `apps/desktop/src-tauri/src/window.rs::request_quit`
beendet nur bei `Quit`; den `Confirm`-Zweig leitet es nicht an einen Dialog
weiter. `apps/desktop/src/app/windows.ts::quitApp` schließt lediglich das Fenster.
Dieser Bedarf geht an Planer/N01 bzw. das zuständige UI-/Hüllenpaket; S02 verändert
weder eingefrorene Hüllenlogik noch fremde UI-Tests. Die Probe verwendet den
vorhandenen Hüllen-Namensraum `plugin:os-app|requestQuit` (nicht Tauris eigenes
`app`-Plugin).

Native Aktivierung: `active=false`, `postEventAccess=false`,
`screenLocked=true`, `onConsole=false`. Die erneute S3-Diagnose bestätigt dieselbe
Sitzungsgrenze. Die vorhandene Berechtigung wurde nicht verändert. S3 und Smoke liefen als Funktionsdiagnosen während der übrigen Prüfungen;
die Laufzeiten sind keine Performance-Abnahme. Es liegt keine Referenz-A-Abnahme
in einer entsperrten GUI-Sitzung vor.

**Entscheidung „E2E-B macOS ja/nein“: Nein zur Freigabe dieses Stands.** Der
Transport, die beiden Fenster und die Kern-Rückfrageentscheidung funktionieren;
der vollständige Smoke besteht noch nicht. **Kein E2E-C-Rückfall beschlossen:**
Ein reproduzierbarer Lauf aus einer entsperrten GUI-Sitzung fehlt. Das Scheitern
der gesperrten Agent-Sitzung oder eines CI-Runners reicht dafür nicht aus.

Die neue Ratenaufnahme und die getrennte `native-occlusion-source`-Prüfung
stehen in [S-6](S-6.md); native Kürzel/Windows in [S-3](S-3.md).

## Fremdergänzungen und Test-Erwartungen

In dieser Folgerunde keine neuen Fremdergänzungen nach K2-61 und keine
Erwartungsänderungen nach K2-60/K2-63. Eigene Änderungen liegen ausschließlich
in S02-`owns`. Der gesonderte Main-Merge übernimmt fremde Paketänderungen
unverändert. Aus der Vorrunde bleiben der genehmigte `webdriverio@7.40.0`-
Manifest-/Lockfile-Nachtrag und der exakt hashgebundene Neutralitäts-Allow-Eintrag
bestehen. Clean-Room: eigene Spec und Projekt-/Bibliothekscode, keine Ressourcen
der Referenz.


## Prüfumgebung

Rust/Cargo 1.99.1, Node 22.20.0; `OPENSPIDER_SECRET_STORE=memory`.
Für `cargo test --workspace` steht ein temporärer `python3`-Link auf das bereits
installierte Homebrew-Python 3.10.13 / SQLite 3.43.2 vor dem System-Python im PATH.
Das folgt der in der Vorrunde belegten lokalen SQLite-VFS-Abweichung. Die sechs
unveränderten `read_api_export`-Tests bestehen in **4,23 s**, einschließlich beider
Python-Leser. Es wurden keine Test-Erwartungen, Ignores oder Goldens verändert.

Es werden keine Spec-IDs als vollständig abgenommen markiert
(`M2-S02.spec_ids=[]`). UI-002/UI-113/OUT-222/OUT-232/UI-143 sind Gegenstand der
Spikes; die offenen nativen Gates bleiben ausdrücklich offen.


## Abschlussprüfungen dieser Runde

| Prüfung | Ergebnis | Laufzeit |
|---|---|---:|
| `npm ci` | grün | 18.00 s |
| `npm run -w apps/desktop typecheck` | grün | 3.71 s |
| `npm run -w apps/desktop test` (162 Tests) | grün | 16.77 s |
| `npm run -w apps/desktop check:static` (inkl. Lizenzen, 28 Tests) | grün | 5.69 s |
| `cargo fmt --all --check` | grün | 3.34 s |
| `cargo deny check licenses bans sources` | grün | 2.87 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün | 105.51 s |
| `cargo test --workspace` | grün | 996.82 s |
| `cargo build --locked -p openspider-desktop --features e2e-webdriver` | grün | 250.80 s |
| Node-Fixture-Tests für S6 | grün, 4 Tests | < 1 s |
| Spike-Typecheck, Node-Syntax, Swift-Kompilierung, beide Workflows/actionlint, Diff-Whitespace | grün | kurze Einzelchecks |
| Neutralitätsprüfung | grün; vier geerbte verwaiste Ausnahmen als Warnung | < 1 s |

`cargo test --workspace`: **2819 bestanden, 0 fehlgeschlagen, 68 bestehende
Ignores**, einschließlich Desktop und Doc-Tests. Target-Verzeichnis nach den
Prüfungen: **24 GiB**. Die vorhandenen npm-Engine-/Deprecation-Hinweise und der
Audit-Befund sind dokumentiert; keine Lockerung der Prüfungen. `check:static`
wurde nach den Runner-Änderungen nochmals grün ausgeführt. Lockfiles unverändert
gegenüber dem Merge-Commit; `origin/main` ist Vorfahr des Branch-Kopfs.

Maschinenlesbare Zusammenfassung einschließlich C11-Ruheprüfung:
`apps/desktop/spikes/evidence/review3-validation.json`. Neue CI-Läufe fehlen
wegen des ausdrücklichen Push-Verbots; es wird keine grüne CI für diesen
Fixstand behauptet.


## Abschluss je Befund aus Review 3

1. **Basis-/Nachweisfehler behoben:** Main inklusive K04 gemergt, `npm ci` und
   Feature-Build grün, Smoke und S6 erneut ausgeführt. `crawl.start` und
   `app.requestQuit` funktionieren. Offen bleiben der tatsächlich fehlende
   Schließen-Dialog sowie die in S6 belegten Sichtbarkeits-/Restore-Probleme;
   keine veraltete K04-Stub-Begründung mehr.
2. **Trigger behoben, Windows-Nachweis offen:** beide Branch-Trigger ergänzt
   und mit actionlint geprüft. Kein Push laut Auftrag, daher keine neuen CI-IDs
   oder Windows-Artefakte und keine Behauptung einer bestandenen Plattform-DoD.
3. **Erneut geprüft, extern blockiert:** S3 und Smoke auf Referenz A ausgeführt,
   aber Sitzung weiterhin gesperrt und PostEvent-Zugriff fehlt. Die geforderte
   entsperrte GUI-Abnahme muss der Orchestrator nach C11 nachholen.
4. **Runner korrigiert, Abnahme weiterhin rot:** fünf echte ≥30-s-Aufnahmen,
   getrennte Ratenprüfungen und `native-occlusion-source`, vier grüne Fixture-
   Tests. Tatsächlich 1,764 Hz minimiert, Restore-Berechtigung fehlt; keine
   bestandene Raten-Teilabnahme behauptet. Daten und N01-Bedarfe stehen in S6.
5. **Empfehlung umgesetzt:** SPDX-/Override-Bedarf und Planer-Entscheidung zum
   WebdriverIO-Upgrade mit Lizenzprüfung dokumentiert. Audit bestätigt 7 Befunde
   (6 hoch); keine weitere manuelle Änderung am Lockfile.

**S02 bleibt nicht freigegeben.** Alle beauftragten lokalen Pflichtchecks sind
grün; die nativen Gates werden durch diese Checks nicht ersetzt. Kein Push,
kein PR und keine fremden Test-/Hüllenänderungen in dieser Fixrunde.
