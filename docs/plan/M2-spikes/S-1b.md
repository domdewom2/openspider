# M2-U02B – Eigenes Canvas-Grid (Fallback B)

Stand: 18.10.2026. Branch `m2/u02b`, Basis `ca88944` (S01 mit vorab integrierten F05-Schnittstellen). Der Worktree war beim Start sauber. Clean-Room: ausschließlich Projekt-Spec, Plan, eigener Code und eigene synthetische Fixtures; kein Zugriff auf das Referenzprodukt.

## Implementierung und Spec-Nachweise

Das Grid rendert ohne Glide hinter den **unveränderten** Schnittstellen `DataGridProps`, `GridController` und `ViewDataSource`. Keine Änderungen an UI-Feature-Paketen, Datenquelle, Auswahlmodell, npm-Manifesten oder Lockfiles. Die ungenutzte, gepinnte Glide-Abhängigkeit bleibt wegen des Manifestbesitzes bestehen.

- **UI-140:** Die logische Zeilenposition ist unabhängig vom nativen Scroll-Pixelwert. Der vollständige Datenbestand wird auf höchstens 1.000.000 CSS-Pixel abgebildet; gerundete Rückmeldungen des Scrollbalkens überschreiben keine gezielten Sprünge. Mausradbewegungen bleiben zeilenbezogen. Canvas und ARIA-Baum enthalten nur sichtbare Zeilen/Spalten, Row und angeheftete Spalten bleiben stehen. Die bestehende Datenquelle lädt 200er-Blöcke, zwei Bildschirmseiten Puffer und höchstens 20.000 Zeilen im LRU. Fehlende Daten erscheinen als Platzhalter, einschließlich `aria-busy`.
- **UI-007:** Typisierte, lokalisierte Zellen, Zahlen rechtsbündig, Statusfarbe mit Text, Zebra/Auswahl, Tooltips, Kürzung mit Ellipse ohne durchtrennte UTF-16-Paare. Spaltenbreiten per Ziehen und Doppelklick-Autobreite, Reihenfolge innerhalb einer Anheftungsgruppe, Kopfmenü-, Kontextmenü-, Sortier- und Aktivierungs-Callbacks. Persistenz bleibt beim Aufrufer über `onColumnsChange`. Textmessung für die Darstellung erfolgt über einen begrenzten DOM-Messcache; SEO-Pixelbreiten bleiben unverändert im Kern (bestehende statische UI-038-Prüfung).
- **UI-012:** Einzel-, Bereichs- und Modifikatorauswahl, Pfeile, Bild auf/ab, Pos1/Ende, horizontale Fokusnavigation, Kopfbedienung per Tastatur, Enter und Kontextmenütaste/Shift+F10. Mod+A und eine Shift-Auswahl über die gesamte Ansicht benötigen keine fünf Millionen IDs. Kopieren läuft ausschließlich über den bestehenden Kernvertrag einschließlich Größenwarnung/Exportgrenze. Ausstehende Auswahlvorgänge werden bei Unmount, Versionswechsel oder neuer Auswahl verworfen.
- **S-1 (5)/(6):** Echter `role="grid"`-Baum mit `aria-rowcount=5000001`, logischen Zeilen-/Spaltenindizes, Auswahl und `aria-activedescendant`; auch unter React 19 StrictMode. Keine Glide-Lifecycle-Abhängigkeit.
- **S-2:** Backing-Store = gerenderte Canvas-Größe × DPR, entsprechend lokaler CSS-Größe × App-CSS-Zoom × DPR. ResizeObserver, Zoom-Mutationen und DPR-Wechsel führen die Auflösung nach. Ein WebKit-Scrollereignis zwischen Zoomänderung und Layout-Commit darf den logischen Anker nicht verschieben. Native Scrollleisten bleiben außerhalb der Canvas-Hitfläche erreichbar.

UI-007/UI-012/UI-140 bleiben gemäß Paketdefinition **M2-S01 zugeordnet**; U02B liefert Implementierung und zusätzliche getaggte Nachweise. Die Behauptung einer vollständig abgenommenen nativen UI-140 wäre ohne Referenzmessung und Screenreader-Prüfung nicht gerechtfertigt.

## Messverfahren

Lokale Hardware: Mac mini M1 (`Macmini9,1`), 16 GiB, macOS 26.3 (25D125). Referenz A verlangt einen MacBook Air M1 mit 8 GiB; diese Maschine ersetzt dessen verbindliche Abnahme nicht. Während der Implementierung liefen auch Tests/Builds anderer Worktrees. Deren Prozesse wurden nicht verändert oder beendet.

Unveränderte Messfälle aus S01: 5.000.000 Zeilen × 65 Spalten, echte Rust-Bridge mit `stub-views`, Sprung zum Ende und zu Zeilen 400/2.500.000 (0-basiert), zehn Sekunden vertikal/horizontal, App-Zoom 80/100/125/150/200 % bei DPR 1/2/3. Zusätzliche Assertions prüfen StrictMode-ARIA, axe, alle S-2-Kriterien, exakte mittlere Sprünge, echte Scrollbalken-Ziehbewegungen, Mausrad, horizontale Virtualisierung und Spaltenbedienung. Chromium läuft ohne seine Headless-Option `--hide-scrollbars`, damit der Drag-Test tatsächlich den nativen Scrollbalken trifft.

Der Frame-Recorder bewahrt sämtliche rAF-Abstände und gegebenenfalls Long-Animation-Frames. `OS_GRID_STRICT_BUDGETS=1` schaltet die FPS-/50-ms-Budgets für einen ruhigen Referenzlauf als Assertions scharf; gewöhnliche CI-Läufe zeichnen diese Werte als Indizien auf. Funktionale Assertions einschließlich Zoom/ARIA sind immer aktiv. Native Ergebnisse werden niemals durch Playwright-Werte ersetzt.

## Lokale Messwerte

Implementierungsstand: `4cc458b` und `4766329` (entspricht dem finalen Produktcode). Vollständiger Browserlauf: **16 Tests bestanden, 1,2 min**. [Rohdaten einschließlich aller Frames und Zoom-Snapshots](../../../apps/desktop/e2e/grid/evidence/u02b-browser-macos.json).

Die Prüfung `scripts/bench/wait_quiet.sh 2 1` schlug fehl: `load1=6.91 blieb >= 2`. Der Browserblock lief unter `caffeinate` nach Ende der eigenen Builds/Rust-Tests. Er ist trotzdem **kein ruhiger Referenzlauf**. Eine frühe Funktionsmessung unter Build-Last ist [separat erhalten](../../../apps/desktop/e2e/grid/evidence/u02b-functional-macos.json); dort dauerte der WebKit-Sprung 154 ms, im abschließenden Produktlauf 96 ms.

| S-1-Kriterium, 5 Mio. × 65 | Chromium | WebKit | WKWebView nativ | WebView2 |
|---|---:|---:|---|---|
| (1) letzte Row | 5.000.000 | 5.000.000 | geladen, kein Paint-Nachweis | offen |
| (2) vertikal: fps / max. Frame | 60,00 / 16,8 ms | 66,07 / **58 ms** | nicht messbar | offen |
| (3) horizontal: fps / max. Frame | 60,00 / 16,8 ms | 63,06 / 45 ms | nicht messbar | offen |
| (4) Sprung ans Ende bis Inhalt + Paint | 55,2 ms | 96 ms | nicht messbar | offen |
| (5) ARIA unter StrictMode | 5.000.001 | 5.000.001 | 5.000.001 im DOM; Ansage offen | offen |
| (6) React-Warnungen | 0 | 0 | keine Aussage aus DOM allein | offen |
| DOM-Zeilen beim Scrollen / Cache-Maximum | 29 / 20.000 | 29 / 20.000 | – | offen |
| axe-Verstöße im Grid | 0 | 0 | – | offen |

**Kriterium (2) ist damit noch nicht vollständig nachgewiesen:** Die beobachtete WebKit-Spitze von 58 ms liegt über 50 ms. Der grüne Browser-Test bestätigt Durchführung und funktionale Verträge, nicht die verbindliche Performanceabnahme. Ein zusätzlich geprüfter rAF-Paint-Coalescer zeigte auf dieser belasteten Maschine keine Verbesserung (WebKit vertikal 57,57 fps / 111 ms, horizontal 55,84 fps / 150 ms) und wurde nicht übernommen; [Rohdaten des verworfenen Versuchs](../../../apps/desktop/e2e/grid/evidence/u02b-paint-experiment.json). Ohne ruhige Vergleichsmessung wird daraus keine kausale Performanceaussage abgeleitet.

| Mittlerer Sprung (0-basiert) | Chromium: erste Zeile / Inhalt + Paint | WebKit: erste Zeile / Inhalt + Paint |
|---|---:|---:|
| 400 | 400 / 52,2 ms | 400 / 78 ms |
| 2.500.000 | 2.500.000 / 64,3 ms | 2.500.000 / 59 ms |

S-2: Alle **30 Kombinationen** bestanden sämtliche Assertions für Backing-Store, Sollbreite, Anker, Auswahl, Kopfzeilengeometrie und Zell-Hit-Test. Die Tabelle zeigt je Engine **maximalen Backing-Store-Fehler in px / gemessene Address-Breite in px**. Sollbreite: 320 × Zoom; zulässige Abweichung jeweils ±1 px. Native Zoom-/Windows-Skalierungswerte bleiben offen.

| DPR | Zoom | Chromium: Fehler / Breite | WebKit: Fehler / Breite | Anker, Auswahl, Kopf und Hit korrekt |
|---|---|---:|---:|---|
| 1 | 80 % | 0.203 / 256.000 | 0.203 / 256.000 | ja |
| 1 | 100 % | 0.000 / 320.000 | 0.000 / 320.000 | ja |
| 1 | 125 % | 0.500 / 400.000 | 0.500 / 400.000 | ja |
| 1 | 150 % | 0.500 / 480.190 | 0.500 / 480.188 | ja |
| 1 | 200 % | 0.000 / 639.494 | 0.000 / 640.000 | ja |
| 2 | 80 % | 0.406 / 256.000 | 0.406 / 256.000 | ja |
| 2 | 100 % | 0.000 / 320.000 | 0.000 / 320.000 | ja |
| 2 | 125 % | 0.000 / 400.000 | 0.000 / 400.000 | ja |
| 2 | 150 % | 0.000 / 480.190 | 0.000 / 480.188 | ja |
| 2 | 200 % | 0.000 / 639.494 | 0.000 / 640.000 | ja |
| 3 | 80 % | 0.391 / 256.000 | 0.391 / 256.000 | ja |
| 3 | 100 % | 0.000 / 320.000 | 0.000 / 320.000 | ja |
| 3 | 125 % | 0.500 / 400.000 | 0.500 / 400.000 | ja |
| 3 | 150 % | 0.500 / 480.190 | 0.500 / 480.188 | ja |
| 3 | 200 % | 0.000 / 639.494 | 0.000 / 640.000 | ja |

Native Versuche: Debug-Binary und reguläres `.app`-Bundle erfolgreich gebaut und jeweils mit dem vorhandenen WebDriver-Runner gestartet. Beide melden trotz `show`/`setFocus` `visibilityState="hidden"`, `hasFocus=false`; der Runner bricht deshalb mit Exit 1 ab. Die View lädt die letzte Row und hat den korrekten ARIA-Zähler, doch FPS/Paint/Zoom und tatsächliche Screenreader-Ansagen können so nicht abgenommen werden. [Beide nativen Berichte](../../../apps/desktop/e2e/grid/evidence/u02b-native-macos.json).


## Reproduktion

Im Repository-Hauptverzeichnis:

```sh
npm ci
cargo build -p os-app --locked --features e2e-bridge,stub-views --bin os-app-bridge
npm run -w apps/desktop e2e -- --config e2e/grid/playwright.config.ts --project=chromium --project=webkit
```

Für die verbindliche Performanceprüfung auf freier Referenzhardware vorher `scripts/bench/wait_quiet.sh` ausführen und den Lauf mit `caffeinate` wach halten; `OS_GRID_STRICT_BUDGETS=1` setzen. Der vorhandene native Runner bleibt verwendbar:

```sh
npm run -w apps/desktop tauri -- build --debug --no-bundle --features stub-views,e2e-webdriver --config e2e/grid/native.conf.json
node apps/desktop/e2e/grid/native.mjs "${CARGO_TARGET_DIR:-target}/debug/openspider-desktop" target/loadtest/grid-u02b-native
```

Ohne abweichendes `CARGO_TARGET_DIR`: `target/debug/openspider-desktop`; Windows: `.exe`. Der Runner verwendet temporäres Home, Memory-Secrets und ausschließlich die eigene synthetische View. Windows-Skalierungen 100/125/150/200 % werden über den vorhandenen S01-CI-Aufbau gemessen. Für VoiceOver/NVDA: Grid fokussieren, Zeilenanzahl, Überschriften, Fokuszelle und Auswahl beim Navigieren prüfen; axe/DOM-Attribute ersetzen keine tatsächliche Ansageprüfung.

## Fremdergänzungen (K2-61)

**Keine.** Alle Änderungen liegen in `apps/desktop/src/grid/**`, `apps/desktop/e2e/grid/**` oder diesem Bericht. Daher keine fremden Diffs oder neu erzeugten fremden Goldens. K2-60 wurde ebenfalls nicht benötigt.

## Prüfungen, Abweichungen und offene Abnahme

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün (2 min 41 s).
- `cargo test --workspace --exclude openspider-desktop --locked`: grün, **2.307 bestanden, 40 bestehende Ignores, 0 Fehler**, 258 Testziele inklusive Doc-Tests. Gemeldete Testlaufzeiten summiert 451,84 s; Test-Build 3 min 31 s, zusätzlich Wartezeit auf den Bridge-Build. Der bestehende Clippy-Negativtest wartete zeitweise auf den nativen Build-Lock.
- `npm ci`: erfolgreich; Node 22.20.0 meldet die bereits bestehenden Engine-Hinweise einzelner jsdom-Abhängigkeiten. Keine npm-Audit-Funde.
- Frontend-Typecheck, **148 Vitest-Tests**, `check:static` und Produkt-Build: grün. Der abschließende Vitest-Lauf auf dem beibehaltenen Stand dauerte 3,49 s.
- Browser-Messsuite: **16 bestanden**; funktionale Gate-Assertions und alle Zoomrasterpunkte grün. Performancewerte und deren Einschränkungen stehen oben; keine verdeckte Freigabe von S-1 (2).
- `cargo deny check licenses bans sources`, npm-Lizenzprüfung (263 Pakete) und `scripts/check-neutral.sh`: grün (bestehende Hinweise auf verwaiste Ausnahmen in `scripts/check-neutral.allow`, nicht verändert).
- Native macOS-App und `.app`-Bundle erfolgreich gebaut (Bundle 65,31 MiB); native Messläufe wegen unsichtbaren Fensters nicht bestanden, siehe oben.
- Target-Verzeichnis zuletzt rund 17 GiB inklusive Workspace-Tests, Bridge, nativer App und Bundle; nicht versioniert.

**Abweichungen/offen:** Keine nicht-additiven oder additiven Fremdänderungen, keine geänderten F05-Stub-Verträge, keine neuen Abhängigkeiten, keine fachlichen Spec-Unklarheiten. Die vollständige Gate-Abnahme bleibt offen: ruhige Referenz-A-Messung einschließlich WebKit/WKWebView-50-ms-Budget, native Zoom/DPR-Prüfung, Windows/WebView2-Skalierung und VoiceOver/NVDA. Windows- und native technische Prüfungen sind im vorhandenen CI-Aufbau vorgesehen; die Ansageprüfung bleibt manuell. In diesem Auftrag gab es ausdrücklich keinen Push, Merge oder PR und daher keine CI-Lauf-ID. G0/Fable-Abnahme wird nicht vorweggenommen.

Implementierungs-Commits: `4cc458b` (Canvas-Grid und Fixture-Tests), `4766329` (Controller-Geometrie außerhalb des sichtbaren Ausschnitts). Messsuite, Rohdaten und dieser Bericht folgen im Abschlusscommit, dessen Kennung im Übergabebericht steht.

