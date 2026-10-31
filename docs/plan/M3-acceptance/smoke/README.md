# Installations-Smoke M3-R03 / OUT-263

Die Suite ruft ausschließlich die in der Installation gefundene CLI auf. Matrix:
`macos-15` (aarch64-DMG), `macos-15-intel` (x86_64-DMG), `windows-latest`
und `windows-11-arm` (jeweils dieselben **x64**-MSI/NSIS, auf ARM emuliert).
Bundle-ID: `org.openspider.desktop`; LaunchAgent-Präfix bleibt `de.openspider.task.`.
Kein Chromium im Bundle; die Runner stellen Chrome/Edge bereit. Keine Signier- oder
Updater-Secrets für die Smoke-Suite erforderlich. „unsigniert“ ist zulässig;
Installations-, Laufzeit- und Vergleichsfehler bleiben rot.

## Start durch den Orchestrator

Paketbranch `m3/r03` nicht pushen. Der Orchestrator übernimmt die Commits in seinen
Testzweig. Alle folgenden Befehle laufen auf dessen Repository. `TEST_REF` muss auf
den übernommenen Stand zeigen; die Workflows müssen für `workflow_dispatch` im
Repository registriert sein. Bei erstmaliger Registrierung nutzt der Orchestrator
wie vereinbart seinen **push-Trigger auf dem Testzweig**: dort `release.yml` für den
Test-Push aktivieren (kein Tag, kein Publish, `dry_run=true`); dessen `workflow_call`
startet die neue Smoke-Suite nach R01/R02. Diese Triggeranpassung gehört nur in den
Testzweig. Die Paketdatei selbst bietet `workflow_dispatch` und `workflow_call`.

In einer Bash-Sitzung (IDs aus `gh run list` übernehmen):

```bash
export GH_REPO=OWNER/REPOSITORY
export TEST_REF=ORCHESTRATOR_TEST_BRANCH
export RELEASE_TAG="v$(node --input-type=module -e "import {readVersion} from './scripts/release/check-version.mjs'; console.log(readVersion(process.cwd()))")"
# Variante A: zusammenhängender Release-Trockenlauf, einschließlich Smoke.
gh workflow run release.yml --repo "$GH_REPO" --ref "$TEST_REF" \
  -f release_tag="$RELEASE_TAG" -f dry_run=true
gh run list --repo "$GH_REPO" --branch "$TEST_REF" --workflow release.yml --limit 5
export RELEASE_RUN_ID=123456789
# Die IDs hier und unten sind Platzhalter aus der vorangehenden Liste.
gh run watch "$RELEASE_RUN_ID" --repo "$GH_REPO" --exit-status

# Variante B: getrennte R01/R02-Builds, beide im Beta-Kanal.
gh workflow run release-macos.yml --repo "$GH_REPO" --ref "$TEST_REF" \
  -f channel=beta -f arch=all -f run_tests=true
gh workflow run release-windows.yml --repo "$GH_REPO" --ref "$TEST_REF" \
  -f channel=beta -f offline_webview=false
gh run list --repo "$GH_REPO" --branch "$TEST_REF" --workflow release-macos.yml --limit 5
gh run list --repo "$GH_REPO" --branch "$TEST_REF" --workflow release-windows.yml --limit 5
export MACOS_RUN_ID=123456790 WINDOWS_RUN_ID=123456791
gh run watch "$MACOS_RUN_ID" --repo "$GH_REPO" --exit-status
gh run watch "$WINDOWS_RUN_ID" --repo "$GH_REPO" --exit-status
# Vor dem Smoke vorhandene Artefakte bestätigen.
gh api "repos/$GH_REPO/actions/runs/$MACOS_RUN_ID/artifacts" --jq '.artifacts[].name'
gh api "repos/$GH_REPO/actions/runs/$WINDOWS_RUN_ID/artifacts" --jq '.artifacts[].name'
gh workflow run smoke-installed.yml --repo "$GH_REPO" --ref "$TEST_REF" \
  -f macos_run_id="$MACOS_RUN_ID" -f windows_run_id="$WINDOWS_RUN_ID" \
  -f macos_arm_artifact=openspider-macos-aarch64 \
  -f macos_intel_artifact=openspider-macos-x86_64 \
  -f windows_artifact=openspider-windows-x86_64
# Für alle Artefakte aus einem Lauf alternativ: -f run_id="$RELEASE_RUN_ID"
gh run list --repo "$GH_REPO" --branch "$TEST_REF" --workflow smoke-installed.yml --limit 5
export SMOKE_RUN_ID=123456792
gh run watch "$SMOKE_RUN_ID" --repo "$GH_REPO" --exit-status
gh run download "$SMOKE_RUN_ID" --repo "$GH_REPO" --pattern 'installed-smoke-*' --dir smoke-evidence
```

Der lokale Checkout für die Versionsableitung muss dem Testzweig entsprechen.
Variante A leitet den Kanal aus der Version ab (Beta nur bei Vorabversion). Keine Tags oder
Releases erstellen. `OPENSPIDER_UPDATE_BETA` setzen R01/R02 aus `channel=beta`.
Windows-Signierung und Updater-Schlüssel folgen am 02.11.2026; Environment `release`
bleibt für die Build-Workflows zuständig. R01 überspringt bei fehlenden **Apple**-
Zugangsdaten den gesamten DMG-Build: ein grüner Build ohne DMG ist kein Smoke-Nachweis.
Der Download fehlender/abgelaufener Artefakte schlägt absichtlich fehl.

## Erwartete Ergebnisse

- Vier grüne Installationsjobs, sechs Installationen (zwei DMGs, zwei MSI, zwei NSIS).
  Windows prüft den PE-Maschinentyp der installierten CLI auf AMD64, auch auf ARM.
- Je Installation `out-basic-site` aus `os-testsite`, Crawl in Text- und JS-Modus,
  vollständige `Internal:All`-Exporte, XML-Sitemap mit Startseite und `about.html`.
  Zusätzliche isolierte JS-Probe: Titel wird erst durch JavaScript zu `R03 rendered`.
- Echte `.ostask` über `--run-task`: Windows-Aufgabenplanung bzw. LaunchAgent startet
  zur nächsten übernächsten Minute; Historie bestätigt Startabweichung ≤ 60 s,
  erfolgreichen Abschluss und Export. Registrierung erfolgt durch die Suite, nicht
  über den Produkt-Planer (OUT-166 bleibt anderen Paketen vorbehalten).
  Registrierungen werden im `finally` entfernt
  und Abwesenheit geprüft. Danach Installer-Deinstallation auch bei Smoke-Fehlern,
  jedoch nur nach erfolgreicher Installation (Exit 0/3010). Folgefehler beim
  Aufräumen verdecken den ursprünglichen Fehler nicht. Alle `--crawl`-Aufrufe
  haben 600 s Timeout.
- `compare` führt zuerst `python3 -m unittest discover -s tests/smoke -v` aus.
  Danach: beide normalisierten Exporte jeder Installation byte-gleich mit dem
  installierten Windows-x64-MSI-Build. Keine Sortierung/Entfernung fachlicher Spalten.
  Spec 06 §0: Basis-URL → `{BASE}`, Zeitspalten → `<TS>`, Antwortzeit → `<MS>`,
  Crawl-ID → `<ID>`, BOM/CSV-Zeilenenden vereinheitlicht. Hash bleibt erhalten.
- Vorhandener Fixture-Paritätsport **8900** auf den isolierten Runnern, damit die
  in HTML eingesetzte Basis-URL und damit Hash/Bytegrößen gleich bleiben. Lokaler
  Portkonflikt ist ein Fehler; keine fremden Prozesse beenden. Die Suite kopiert die
  Fixture temporär und ergänzt nur eine vom Hauptcrawl unerreichbare JS-Proberoute.
- Drei zusätzliche Text-Crawls je Installation: Median URLs/s, Einzelzeiten, CPU-
  Angaben, `PROCESSOR_ARCHITEW6432` und `RUNNER_ARCH` (wenn gesetzt), Browserpfad
  und Browser-Version in `result.json`; Job-Summary und Vergleichsjob zeigen
  ARM/x64-Quotienten
  pro Installer. K3-10: Richtwert ≥ 50 % nur auf vergleichbarer Hardware. Hosted
  Runner sind unterschiedlich; Unterschreitung meldet Hardware-Prüfbedarf und ist
  kein künstlicher CI-Fehler. Die kleine Fixture misst inklusive Prozessstart.

Artefakte `installed-smoke-<runner>` enthalten rohe CSVs, normalisierte CSVs,
Crawl-Logs, Sitemaps, Scheduler-Historie/-Definition und Messwerte. Lauf-IDs und
Quotienten nach den echten Läufen im Paketbericht ergänzen; keine Werte schätzen.

Bei Planerfehlern/Timeout liegen vor der Abmeldung erfasste Diagnosen im jeweiligen
Installationsordner der Evidence: `launchctl-print` samt stderr (aktive Domain
`gui/<uid>` bzw. `user/<uid>`), unter Windows `scheduled-task-info` mit
State/LastRunTime/LastTaskResult und `scheduled-task-definition.xml`. `task.stdout`
und `task.stderr` enthalten die Aufgabenausgabe; fehlt die Ausgabe, steht dort ein
Hinweis auf einen möglichen Nichtstart. Der macOS-Bootstrap versucht nach einem
Fehler in `gui/<uid>` die Domain `user/<uid>` und protokolliert dies in
`bootstrap-fallback.log`. Windows registriert explizit den aktuellen Benutzer mit
`LogonType Interactive`; die Runner-Sitzung muss dafür angemeldet sein.

## Linux-Trockenlauf

```bash
# Ressourcen-/Platzregeln des Paketauftrags gelten vor Cargo-Aufrufen.
timeout 3600s nice -n 10 env CARGO_INCREMENTAL=0 \
  cargo build -p os-cli -p os-testsite -j 4 --locked
# Installationsverzeichnis als Argument; lokal darf das gebaute CLI-Verzeichnis dienen.
# Chromium-Pfad aus der vorhandenen System-/CI-Installation ermitteln.
export OPENSPIDER_CHROMIUM=/path/from/system/or/ci/chrome
python3 -m unittest discover -s tests/smoke -v
timeout 3600s nice -n 10 python3 scripts/smoke/installed.py \
  --install-dir target/debug --testsite target/debug/os-testsite \
  --chromium "$OPENSPIDER_CHROMIUM" --output /tmp/r03-linux-smoke --dry-run
```

Der Ausgabeordner darf noch nicht existieren. Linux führt dieselbe `.ostask` direkt
aus und kennzeichnet `dry_run=true`: kein Beleg für OS-Zeitplanung oder Installation.
Der Vergleichsjob lehnt solche Trockenlauf-Ergebnisse ab.
