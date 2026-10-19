# S-5b – WebdriverIO-Gerüst und native Gegenversuche

Stand: 19.10.2026, Fix-Runde M2-S02, Branch `m2/s02`, Basis `ef32c7f`.
**Teilweise umgesetzt, Paket-DoD weiterhin offen.** Kein Push und kein PR.
Der frühere Manifestblocker ist behoben; die neue Evidenz trennt technische
Teilerfolge, Umgebungsgrenzen und fehlende Fachlaufzeit.

## Gerüst und konkrete Ausführung

`apps/desktop/e2e-b/run.mjs` verwendet das exakt gepinnte MIT-Paket
`webdriverio@7.40.0` als W3C-Client. Es startet die echte App mit isoliertem
`OPENSPIDER_HOME`, Memory-Secret-Store und eigenem Vite-Server. macOS verwendet
`e2e-webdriver`, Windows `tauri-driver` 2.1.0 plus passenden WebView2-Treiber.
Alle Testrequests bleiben auf Loopback. `spikes/probe.ts` beobachtet vorhandene
App-Schnittstellen; es ersetzt keine Commands durch Mocks. Fehler liefern Exit 1.

```sh
npm ci
cargo build --locked -p openspider-desktop --features e2e-webdriver
swiftc apps/desktop/spikes/macos.swift -o "$TMPDIR/s02-macos"
S02_GUI=1 S02_MAC_HELPER="$TMPDIR/s02-macos" \
  node apps/desktop/e2e-b/run.mjs "${CARGO_TARGET_DIR:-target}/debug/openspider-desktop" smoke
```

Bei parallelen Cargo-Builds kann ein Build ohne Feature dieselbe Debug-Binärdatei
ersetzen. Die abschließende Messung verwendete deshalb eine temporäre Kopie des
zuvor erfolgreich gebauten Feature-Binaries. Zwei zwischenzeitliche Starts eines
überschriebenen Binaries liefen ohne WebDriver-Server ins Timeout; sie zählen
nicht als Machbarkeitsbeleg. Der Runner/Workflow dokumentiert diese Voraussetzung.

`ui-e2e-b.yml`: **nightly (02:23 UTC) + workflow_dispatch**, außerdem reusable
für `spike-platform.yml`; kein Push-Job. Matrix macOS/Windows, mit Logs,
Screenshot, JSON-Ergebnissen und Laufzeiten als Artefakte. `spike-platform.yml`
ruft S3 auf beiden Plattformen und S6 auf macOS auf. Beide Dateien bestehen
`actionlint` 1.7.7. Keine dieser neuen Workflows wurde remote ausgeführt.

## Fortsetzung der fünf Schritte aus dem Erstbericht

| Schritt | Ergebnis und Test |
|---|---|
| 1. WebdriverIO-Gerüst | Implementiert und lokal gegen echten eingebetteten Server ausgeführt. Session, execute/sync-Promise-Polling, Fensterwechsel, Screenshot und Session-Ende funktionieren. Native Cmd-Tasten für S3 kommen über den Swift-Helfer, nicht über synthetische Plugin-KeyEvents. |
| 2. Isolierte echte App | Direkter Child-Start und Start als `.app` über LaunchServices durchgeführt, jeweils eigener Home. Weder App-Daten noch Schlüsselbund des Nutzers werden verwendet. |
| 3. Haupt-/Einstellungsfenster, Rückfrage | Hauptfenster und getrenntes Einstellungsfenster **grün** mit echtem IPC und nativer Sichtbarkeitsabfrage. Aktiver Desktop **rot**: Frontend hidden/kein Fokus. Rückfrage **offen/rot**: `crawl.start` antwortet Unsupported (K04-Stub), daher kein laufender Fixture-Crawl. |
| 4. Nightly/Plattform-Workflow | Angelegt und statisch validiert; Windows-Ausführung steht aus. Kein vorhandener Playwright-Chromium-Lauf wird als WebView2-Beleg umgedeutet. |
| 5. E2E-C-Rückfall | **Nicht entschieden.** Der Versuch über LaunchServices läuft ebenfalls in einer gesperrten GUI-Sitzung. Das ist kein reproduzierbares Scheitern in einer verfügbaren, aktiven GUI-Sitzung. |

Der abschließende GUI-Smoke dauerte **3,630 s**. Rohdaten:
`apps/desktop/spikes/evidence/smoke-gui.json`; S3 und S6 liegen daneben.
Die Vorversuche und endgültigen Bedingungen sind in [S-3](S-3.md) und
[S-6](S-6.md) beschrieben.

## Untersuchung hidden/kein Fokus

- Native Shell: Fenster sichtbar, nicht minimiert.
- Frontend: `visibilityState=hidden`, `document.hasFocus()=false`.
- `show()`/`setFocus()` ändern diesen Zustand nicht.
- `NSRunningApplication.activate(.activateAllWindows)` ebenfalls nicht;
  `activationPolicy=0` (regular), App nicht verborgen, aber nicht aktiv.
- Die Session-Diagnose meldet `screenLocked=true`, `postEventAccess=false`.
  Direkter Agent-Start und LaunchServices-Start zeigen denselben Zustand.

Damit ist eine Umgebungsursache konkret nachgewiesen, aber nicht bewiesen,
wie sich eine entsperrte Sitzung verhält. Kein Entsperren, kein Ändern von
Systemberechtigungen, kein erzwungenes `visibilityState` und keine Fake-Fokuswerte.
Eine optionale Rückfrage zum Entsperren blieb während der übrigen Arbeit ohne
Antwort. Native Messungen sind deshalb nicht freigegeben. Für S3 ist zusätzlich
native Ereigniszustellung erforderlich. Für S6 fehlt darüber hinaus die
K04-Laufzeit und die native Occlusion-Quelle; siehe den Hüllenbedarf in S-6.

## Befund 4: Python-Leser, unveränderter Test

Der erste vollständige Workspace-Lauf reproduziert exakt den alten Fehler:
`crates/os-cli/tests/read_api_export.rs:457`,
`a_python_reader_reads_a_finished_file_and_rejects_an_unknown_read_api_version`,
`q_manifest … unable to open database file`, Exit 101 nach 328,98 s.

| Umgebung | Unveränderter Test |
|---|---|
| Apple Python 3.9.6, SQLite 3.51.0, macOS 26.3 | rot |
| Homebrew Python 3.10.13, SQLite 3.43.2, gleicher Worktree | alle 6 `read_api_export`-Tests grün, Testzeit 6,49 s; mit Build 52,78 s |
| [linux-core 39135555277 / Job 117699227077](https://github.com/domdewom2/openspider/actions/runs/39135555277/job/117699227077), Ubuntu | gleicher Ausgangscommit `ef32c7f`; alle 6 Tests grün in 4,18 s am 18.10., 15:32:27 UTC |

Der im Review genannte Main-Lauf `8c12c7d` ist eine zusätzliche Angabe aus dem
Review. Unabhängig nachgelesen wurde hier der stärkere Vergleich **am gleichen
S02-Ausgangscommit** (CI-Logs inklusive Testname und Checkout-SHA).

Reproduktion ohne Änderung an Quellcode oder Test-Erwartungen:

```sh
# Ursprüngliche Umgebung:
cargo test -p os-cli --test read_api_export
# Kontrolllauf: PATH auf einen Ordner mit python3 -> vorhandenes Homebrew-Python setzen.
# Der Test selbst startet weiterhin python3 -I; PYTHONPATH spielt keine Rolle.
PATH="<temporärer Python-bin-Ordner>:$PATH" cargo test -p os-cli --test read_api_export
PATH="<temporärer Python-bin-Ordner>:$PATH" cargo test --workspace
```

**Einordnung:** lokal/umgebungsabhängig, nicht S02-Regression. Der genaue
Unterschied im SQLite-VFS ist damit nicht abschließend erklärt; kein
unbelegter Pfadfehler wird behauptet. Der abschließende volle Workspace-Lauf
ist mit dem funktionierenden Python grün. Fremde Tests wurden nicht geändert.

## Fremdergänzungen (K2-61)

1. `apps/desktop/package.json`: ausschließlich `devDependencies.webdriverio`
   exakt `7.40.0`; ausdrücklich durch die Planer-Entscheidung im Review freigegeben.
2. `package-lock.json`: dazugehöriger Abhängigkeitsgraph. Keine bereits im
   Basis-Lock vorhandene Paketversion wurde verändert. Lizenz-Metadaten:
   `css-value@0.0.1` ergänzt MIT aus dessen mitgeliefertem `Readme.md`;
   bei `type-fest@0.20.2` wird die ausdrücklich angebotene MIT-Alternative
   aus `(MIT OR CC0-1.0)` gewählt. Die unveränderte Lizenzprüfung kann den
   geklammerten Ausdruck nicht korrekt parsen; die Lizenzwahl ändert keine
   Rechte und umgeht keine verbotene Lizenz. `npm ci` erhält das Lock unverändert.
3. `scripts/check-neutral.allow`: **eine additive, exakt hashgebundene Zeile**
   für den npm-Integritätshash von `devtools-protocol@0.0.1260888`. Zufällige
   Base64-Zeichen trafen die Kürzel-Regel. Keine allgemeine Paket-/Dateiausnahme,
   keine Änderung des Prüfers, keine Bereinigung fremder Ausnahmen.

Diff-Kern:

```diff
+ "webdriverio": "7.40.0"
+ css-value@0.0.1: license MIT (mitgelieferter Lizenztext)
+ type-fest@0.20.2: license MIT (gewählte Alternative)
+ check-neutral.allow: package-lock.json + exakter Integritätshash + Begründung
```

Keine Rust-Manifeste, kein Hüllencode, keine K04-Implementierung verändert.
Keine Anwendung von K2-60/K2-63 nötig: keine fremde Test-Erwartung geändert.
Clean-Room: ausschließlich eigene Spec, Projektcode und Bibliotheksquellen.

Lizenzentscheidung: Die geprüften WebdriverIO-Versionen 10.1.0 und 9.10.1 zogen
u. a. `@promptbook/utils` (CC-BY-4.0) und neue nicht freigegebene Lizenzen ein.
7.40.0 erfüllt mit obiger belegter Metadatenkorrektur die bestehende Positivliste.
**Verbleibendes Risiko:** `npm audit` meldet im älteren, ausschließlich für Tests
verwendeten Graph 7 Befunde (1 moderat, 6 hoch; devtools/puppeteer-core,
extract-zip, tar-fs, uuid, ws und weitergereicht webdriverio). Der Runner benutzt
nur den festen Loopback-W3C-Endpunkt und startet weder DevTools/Puppeteer noch
Archivdownloads. Das ist keine allgemeine Sicherheitsfreigabe dieser alten
Abhängigkeiten; ein Upgrade benötigt eine gesonderte Lizenz-/Manifestentscheidung.

## Abschlussprüfung und Bericht nach M1 2.7 / M2 Regel 7

Lokale Plattform: macOS 26.3 (25D125), Apple arm64; Node 22.20.0,
Cargo/Rust-Toolchain 1.99.1. **Cloud-Plattform: keine.** Neue CI-Lauf-ID: keine
(kein Push/PR beauftragt). Vorhandene CI-Evidenz: S5 `39135561316`,
Python/Linux `39135555277`; Details in den jeweiligen Abschnitten und
`apps/desktop/spikes/evidence/ci.json`.

| Eigene Prüfung | Ergebnis | Laufzeit |
|---|---|---:|
| `cargo fmt --all --check` | grün | 2,38 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün, einschließlich Desktop | 148,91 s |
| `cargo test --workspace` (Homebrew-Python im PATH) | grün, einschließlich Desktop; 2272 bestanden, 40 bestehende Ignores | 379,59 s |
| `npm ci` | grün | 6,38 s |
| `npm run -w apps/desktop typecheck` | grün | 1,37 s |
| `npm run -w apps/desktop test` | grün, 132 Tests / 12 Dateien | 4,47 s |
| `cargo deny check licenses bans sources` | grün | 2,00 s |
| `npm run -w apps/desktop check:static` | grün, einschließlich npm-Lizenzen und 28 statischer Tests | 1,78 s |
| Feature-Build `e2e-webdriver` | grün | 144,74 s; letzter Build 8,03 s |
| Spike-Typecheck, Node-Syntax, Swift-Kompilierung, beide Workflows/actionlint, Diff-Whitespace | grün | jeweils kurzer Einzelcheck, keine gesonderte Zeitmessung |
| Neutralitätsprüfung | grün; vier bereits bestehende verwaiste Ausnahmen bleiben als Warnungen | <1 s |

Target-Verzeichnis nach den Prüfungen: **12 GiB** (`du -sh`). Eigene
Spike-Laufzeiten: S3 14,636/3,022 s, S5b 3,630 s, S6 2,835 s. Die ersten beiden
nativen Grundfunktionen des Smokes bestehen; der vollständige native Smoke
bleibt rot. Es werden keine Spec-IDs als vollständig implementiert abgehakt
(`M2-S02.spec_ids=[]`); UI-002/UI-113/OUT-222/OUT-232/UI-143 werden geprüft.
Vorhandene Ignored-Tests bleiben unverändert. Bestehende Node-Engine-Hinweise
(jsdom verlangt eine neuere Node-22-Patchversion) sind keine neuen
Produktwarnungen; die geforderten Frontend-Prüfungen liefen dennoch erfolgreich.

## Bericht je Review-Befund

1. **Teilweise behoben:** alle fünf Protokolle, Spike-/E2E-Gerüst und beide
   Workflows liegen vor. S4 entschieden, S5 anhand echter Bridge-Läufe bestanden.
   S3 braucht entsperrtes macOS und Windows; S6 braucht K04 plus Hüllen-Telemetrie.
2. **Teilweise behoben:** Manifestnachtrag, Lizenzprüfung, WebDriver-Verbindung,
   Haupt-/Einstellungsfenster und beide Startarten ausgeführt. Voller Smoke noch
   nicht grün; Gründe und Belege oben. Kein voreiliger E2E-C-Rückfall.
3. **Noch offen:** echter Crawl-Aufruf durchgeführt, aber Unsupported; eine
   gültige 30-s-Messung mit laufendem Crawl ist auf dieser Basis unmöglich.
   Konkreter Runner und Hüllenbedarf stehen in S6; keine simulierte Ersatzmessung.
4. **Behoben:** Fehler reproduziert, Python/SQLite-Gegenprobe und gleicher
   Commit unter Linux verglichen; unveränderte Tests und voller Workspace grün.
5. **Behoben:** Plattform, vorhandene/neue CI-Lauf-IDs, Laufzeiten, Target-Größe,
   Fremdergänzungen, Risiken und zurückgestellte Kriterien sind dokumentiert.

Die Fix-Runde ist damit prüfbar, aber **kein vollständig bestandener S02-Abschluss**.
Die verbleibenden Gates brauchen externe Ausführungsbedingungen bzw. fremde
Fachimplementierung; weder Paketbesitz noch Testerwartungen wurden dafür aufgeweicht.
