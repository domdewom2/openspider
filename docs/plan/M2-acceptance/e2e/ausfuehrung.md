# U32 ausführen

Basis 27f92ff1, Branch m2/u32. Alle Befehle ab Repository-Wurzel. Tokio bleibt 1.53.3; kein `cargo update`. Keine Messläufe. Linux-Screenshots sind ausschließlich Entwicklungsstand, keine macOS-/Windows-Abnahme. Layout, Text und Kürzel werden unter Linux mit Chromium und WebKit geprüft; die simulierte Plattform der Kürzel ersetzt keine native Menüprüfung.

## Linux

```sh
nice -n 10 timeout 3600s env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo build --locked -p os-app --features e2e-bridge --bin os-app-bridge -j 6
nice -n 10 timeout 3600s npx playwright test -c apps/desktop/e2e/keyboard-suite/playwright.config.ts --workers=4
nice -n 10 timeout 3600s npx playwright test -c apps/desktop/e2e/visual/playwright.config.ts --workers=4
nice -n 10 timeout 3600s npx tsc -p apps/desktop/e2e-b/tsconfig.u32.json
node --check apps/desktop/e2e-b/run.mjs
node --check apps/desktop/e2e-b/u32-cases.mjs
node --check apps/desktop/e2e-b/u32-dialog.mjs
node --check apps/desktop/e2e-c/menu.mjs
```

Zusätzliche Abschlussprüfungen (Desktop-Vollprüfungen sind auf der Paketbasis durch U32-004 blockiert):

```sh
export OPENSPIDER_CHROMIUM="$(node -p 'require("@playwright/test").chromium.executablePath()')"
nice -n 10 timeout 3600s cargo fmt --all --check
nice -n 10 timeout 3600s env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo clippy --workspace --all-targets --locked -j 6 -- -D warnings
nice -n 10 timeout 3600s env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only cargo test --workspace --locked -j 6
nice -n 10 timeout 3600s npm run -w apps/desktop typecheck
nice -n 10 timeout 3600s npm run -w apps/desktop test
nice -n 10 timeout 3600s npm run -w apps/desktop check:static
bash scripts/check-neutral.sh
```

C4-Core-Gegenprobe: dieselben Cargo-Befehle zusätzlich mit `--exclude openspider-desktop`. Native Modultypen und Runner-Syntax separat wie oben prüfen.

Einmalig neue Linux-Entwicklungsbilder: derselbe Visual-Befehl mit `--update-snapshots`, anschließend zwingend normal vergleichen. Bildänderungen begründen und prüfen. Ergebnisse/Traces unter `apps/desktop/dist/u32-{visual,keyboard}`. Vite läuft isoliert auf einem ephemer zugewiesenen Loopback-Port mit `--strictPort`; Suiten nacheinander ausführen, maximal vier Worker. Echte Vergleichs-Crawls verwenden temporäres Home und Loopback-Ports aus der bestehenden Bridge-Fixture. Screenshot-Uhr ist fixiert, Visual-Daten kommen aus bestehenden Vertragsfixtures und dem eingefrorenen 30-Tab-Katalog der aktuellen Bridge. Alle belegten Navigationsgruppen werden mitgeprüft.

## Verbindliche Plattform-Baselines (Orchestrator)

Playwright nutzt `baselines/{platform}/{projectName}/{testFilePath}/{arg}{ext}`: darwin/WebKit und win32/Chromium sind getrennt von linux. Baselines erst nach visueller Prüfung als Abnahme committen. Linux-Ordner nicht auf andere Plattformen kopieren.

macOS (unlocked GUI ist erst für native Tests erforderlich; zuerst Werkzeugprüfung, N12 vor Baselines abschließen):

```sh
cargo build --locked -p os-app --features e2e-bridge --bin os-app-bridge -j 6
npx playwright test -c apps/desktop/e2e/visual/playwright.config.ts --project=webkit-2 --workers=1 --update-snapshots
npx playwright test -c apps/desktop/e2e/visual/playwright.config.ts --project=webkit-2 --workers=1
npx playwright test -c apps/desktop/e2e/keyboard-suite/playwright.config.ts --project=webkit --workers=1
```

Windows (PowerShell):

```powershell
cargo build --locked -p os-app --features e2e-bridge --bin os-app-bridge -j 6
npx playwright test -c apps/desktop/e2e/visual/playwright.config.ts --project=chromium-1 --project=chromium-1.25 --project=chromium-1.5 --project=chromium-2 --workers=4 --update-snapshots
npx playwright test -c apps/desktop/e2e/visual/playwright.config.ts --project=chromium-1 --project=chromium-1.25 --project=chromium-1.5 --project=chromium-2 --workers=4
npx playwright test -c apps/desktop/e2e/keyboard-suite/playwright.config.ts --project=chromium --workers=4
```

Screenshots enthalten Hauptfenster, vollständige Leerzustände, Einstellungen, Primitives/Dialogzugang, SERP-Snippet, View-Source-Diff und Wortwolke; jeweils de/en, hell/dunkel, kompakt/komfortabel. Layout/Text läuft in drei Größen und vier Chromium-Skalierungen sowie WebKit/Retina. Kollisionsprüfung behandelt Menü-/Dialog-Overlays als eigene aktive Ebene; deren Überdeckung des inaktiven Hintergrunds ist beabsichtigt. Textprüfung misst sowohl Controls als auch Beschriftungsblöcke, ohne Pixel-Toleranz. Horizontales Kappen ist nur mit Ellipse, vollem Tooltip und vollem zugänglichen Namen zulässig.

## E2E-B: macOS und Windows

Vorher U32-004 lösen; `npm ci` nur bei Bedarf. Bestehender Workflow `.github/workflows/ui-e2e-b.yml` führt standardmäßig nur `smoke` aus. Er benötigt den unten vorgeschlagenen Modus `u32`; `s6` ist eine Messung und gehört nicht zu diesem Paketlauf.

macOS:

```sh
cargo build --locked -p openspider-desktop --features e2e-webdriver -j 6
swiftc apps/desktop/spikes/macos.swift -o "$TMPDIR/s02-macos"
export S02_MAC_HELPER="$TMPDIR/s02-macos"
export S02_GUI=1
export U32_EXPECT_DSF=2
node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop u32 apps/desktop/dist/u32-native
node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop s3 apps/desktop/dist/u32-native-keys
```

Windows:

```powershell
cargo build --locked -p openspider-desktop -j 6
cargo install tauri-driver --version 2.1.0 --locked
./apps/desktop/e2e-b/setup-windows.ps1
$env:U32_EXPECT_DSF='1'
node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop.exe u32 apps/desktop/dist/u32-native-100
node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop.exe s3 apps/desktop/dist/u32-native-keys
```

Windows zusätzlich mit **echter OS-Skalierung** 125/150/200 % wiederholen, U32_EXPECT_DSF auf 1.25/1.5/2 setzen und getrennte Ausgabeordner benutzen. Ein env-Wert simuliert keine Skalierung. macOS muss Faktor 2 tatsächlich liefern. Der Runner startet dafür einen Loopback-Fixture-Crawl und wählt die URLs-Tabelle; eine fehlende Canvas-Fläche schlägt fehl.

B-Fälle: echte Zwischenablage (Produkt-Schreibweg, OS-Leseweg, Wiederherstellung), separate Einstellungen mit Singleton und unabhängigem Schließen, native Öffnen-/Speichern-Dialoge (OS-Erkennung, positive Dateiauswahl und Abbrechen), Benachrichtigungsberechtigung und echte Plugin-Annahme, tatsächlicher OS-/DOM-Skalierungsfaktor und Canvas-Backing-Store; danach vorhandener Smoke mit Fixture-Crawl, requestQuit und sichtbarer Schließen-Rückfrage. `s3` sendet native Bearbeitungskürzel und prüft Tisch-/Textfeld-Abgrenzung. Accessibility-/Automation-Zugang auf macOS und Benachrichtigungsberechtigung vorher gewähren. Kein Test setzt dafür Mocks ein oder erklärt fehlende Berechtigungen zum Erfolg.

Zusätzlich protokollieren: positive Ordnerauswahl, Abbrechen ohne Seiteneffekt, Überschreibbestätigung, sichtbare OS-Benachrichtigung samt Titel/Text, Abbrechen der Beenden-Rückfrage und Weiterbedienbarkeit. Automatisches B-Ergebnis allein ist dafür kein Abnahmebeleg. Aktuelle native Tests wurden hier nur statisch geprüft, nicht ausgeführt.

## E2E-C: echtes macOS-Menü

```sh
swiftc apps/desktop/e2e-c/menu.swift -o "$TMPDIR/u32-menu"
export U32_MENU_HELPER="$TMPDIR/u32-menu"
# S02_MAC_HELPER/S02_GUI und Build wie oben.
U32_UPDATE_NATIVE=1 node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop menu apps/desktop/dist/u32-menu
node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop menu apps/desktop/dist/u32-menu-compare
```

Der Helper liest das native AX-Menü (keinen DOM-Ersatz); der Runner vergleicht Menü-Snapshots in `e2e-c/baselines/darwin/` und betätigt Hilfe → Tastaturkürzel über AXPress, prüft Dialog/Register und legt Dialogbilder sowie echte Screenshots jedes geöffneten nativen Hauptmenüs ab. Für diese Bilder muss zusätzlich die macOS-Bildschirmaufnahme erlaubt sein; die PNGs sind Sichtprüfungsartefakte, die automatischen nativen Baseline-Vergleiche verwenden den AX-Baum. de/en werden über Einstellungen umgeschaltet. Erst Baseline-Update prüfen und committen, dann Vergleichslauf. Entsperrte GUI und Accessibility-Zugang sind zwingend; fehlende Rechte schlagen fehl. Swift-Compiler und AX-Verhalten auf Linux nicht prüfbar.

## Workflow-Vorschlag, keine Änderung an .github

Im vorhandenen `ui-e2e-b.yml` mode-Eingabe auch unter workflow_dispatch anbieten und den nativen B-Schritt mit `MODE=u32` ausführen; s3 als weiteren Aufruf behalten. U32_EXPECT_DSF muss der tatsächlich bereitgestellten Anzeige entsprechen. Für E2E-C auf macOS ergänzen:

```yaml
- name: U32 native cases
  shell: bash
  env:
    U32_EXPECT_DSF: ${{ runner.os == 'macOS' && '2' || '1' }}
  run: |
    binary=target/debug/openspider-desktop
    if [ "$RUNNER_OS" = Windows ]; then binary="$binary.exe"; fi
    node apps/desktop/e2e-b/run.mjs "$binary" u32 apps/desktop/test-results/u32-native
    node apps/desktop/e2e-b/run.mjs "$binary" s3 apps/desktop/test-results/u32-keys
- name: U32 native macOS menu
  if: runner.os == 'macOS'
  shell: bash
  run: |
    swiftc apps/desktop/e2e-c/menu.swift -o "$RUNNER_TEMP/u32-menu"
    U32_MENU_HELPER="$RUNNER_TEMP/u32-menu" node apps/desktop/e2e-b/run.mjs target/debug/openspider-desktop menu apps/desktop/test-results/u32-menu
```

Für Screenshot-CI je Plattform die obigen Playwright-Befehle als eigene Schritte nach `npm ci`, Browserinstallation und Bridge-Build ergänzen; `--update-snapshots` ausschließlich beim bewusst gestarteten Baseline-Aufbau. Artefakte auch bei Fehlern hochladen: `apps/desktop/dist/u32-*`, `apps/desktop/test-results/u32-*`. CI-Lauf-ID, Commit, OS, Browser-/WebView-Version und native Skalierung im Abnahmeprotokoll und u32.md nachtragen. Kein Push/Merge durch das Paket.
