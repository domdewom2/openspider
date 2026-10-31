# Release-Prozess (M3-R04)

Stand 30.10.2026: frühe Beta für SEO-Agenturen. Bundle-ID `org.openspider.desktop`, GitHub Releases als Update-Host, kein Chromium im Bundle. R03, D01 und offene M2-Parität sind ausstehende Abnahmen. Diese Orchestrierung ersetzt sie nicht.

## Version und Start

Die vorbereitete `[workspace.package] version` ist maßgeblich. Der Workflow setzt keine Version und ändert keine Abhängigkeiten. Vor dem Lauf im separaten Release-Vorbereitungsbranch:

```bash
node scripts/release/set-version.mjs 1.0.0-beta.0
cargo metadata --locked --format-version 1 > /dev/null
node scripts/release/check-version.mjs
```

`set-version.mjs` synchronisiert Rust, npm, Tauri und die lokalen Workspace-Pakete in Cargo.lock. Registry-Pakete bleiben unverändert; kein `cargo update`. Versionsänderungen separat prüfen und committen. `beta.0` ist für den technischen Trockenlauf reserviert; die Windows-MSI-Version ist `1.0.0` und liegt vor beta.1 (`1.0.1`), RC.1 (`1.0.500`) und stable (`1.0.999`). Der Produktversionsstring bleibt `1.0.0-beta.0`.

`CHANGELOG.md` muss eine nicht leere Sektion `## [<Version>]` enthalten. Die Notes werden ausschließlich daraus extrahiert; der Workflow ergänzt den tatsächlichen Signaturstatus und offene Abnahmen. Beta.0 hat bereits einen technischen Eintrag, beta.1 und spätere Versionen brauchen eigene freigegebene Inhalte.

`release.yml` startet bei Tags `v*` oder manuell über `workflow_dispatch`. `release_tag` muss exakt `v<Workspace-Version>` sein. Bei `dry_run=true` ist kein echter Tag erforderlich. Ein Entwurf verlangt einen vorhandenen Tag auf genau demselben Commit. Tags nur nach Nutzer-Go anlegen/pushen (D12).

## Jobs und Artefakte

1. `prepare`: Versionsgleichstand, Tag, Changelog und Release-Vertragstests.
2. `macos`/`windows`: lokale wiederverwendbare R01/R02-Workflows, Kanal aus der Version, Secrets geerbt. R02 verwendet Azure Artifact Signing mit OIDC im Environment `release`. Native macOS-Tests laufen vor dem Signierjob.
3. `linux-core`: Ubuntu 22.04 x86_64/glibc, bestehendes `scripts/ci/linux-core.sh release` und `golden`, höchstens vier Cargo-Jobs. Archiv extrahieren und CLI starten. Kein Linux-Desktop, keine deb/rpm, kein Linux-arm64-Versprechen.
4. `assemble`: Assets einsammeln, Kollisionsprüfung, Windows-CLI zippen, sichtbarer STUB/TODO(R03), Notes, Manifeste, verbindliches D01-Kompatibilitätsarchiv vor den Prüfsummen. Actions-Artefakte `release-assets` und `release-reports` sichern.
5. `draft`: nur bei Tag-Push oder `dry_run=false`; Environment `release`, `contents: write`. `gh release create --verify-tag --draft --prerelease=false --latest=false`. Bestehende Releases werden nie überschrieben, auch keine bestehenden Entwürfe. Bei Wiederholung mit vorhandenem Entwurf zuerst manuell entscheiden.

Assets: R01-DMGs (aarch64/x86_64/universal), Windows NSIS/MSI, Updater-Archive plus `.sig`, CLI-Archive `openspider_<Version>_darwin-{aarch64,x86_64,universal}.tar.gz`, `openspider_<Version>_windows-x86_64.zip` und `openspider_<Version>_linux-x86_64.tar.gz`. CLI-Archive enthalten `openspider` bzw. `openspider.exe`, LICENSE, NOTICE und die vollständige `THIRD_PARTY_NOTICES.txt` aus dem L01-Artefakt `third-party-notices`. Assemble ergänzt diese Datei in den eigenständigen macOS-CLI-Archiven aus R01; signierte Updater-Archive bleiben bytegleich. macOS-CLI stammt aus der signierten App. Signierte Updater-Bytes werden beim Sammeln nicht verändert. Prüfprotokolle bleiben in den Actions-Artefakten.

## Signaturen und Update-Endpunkte

Updater: Secrets `TAURI_SIGNING_PRIVATE_KEY`, optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; Variable `OPENSPIDER_UPDATER_PUBKEY`. Apple-Konfiguration siehe [macos-signing.md](macos-signing.md), Azure siehe [windows-signing.md](windows-signing.md). Werte nie protokollieren. Environment `release` und dessen Freigaben/Branch-Regeln vorab konfigurieren: `v*`-Tags und den Release-Branch (`release/1.0`, beim Trockenlauf auch `m3/r04-beta0`) als Deployment-Ziele zulassen. Apple- und Updater-Zugangsdaten für R01 müssen als Repository-Secrets verfügbar sein; R01 bindet derzeit kein Environment. R02 liest die Konfiguration im Environment `release`.

Ohne Apple-Konfiguration überspringt R01 seine Distributions-Builds grün. R02 baut ohne Azure-Konfiguration unsigniert und ohne Updater-Schlüssel ohne Updater-Signaturen. R04 meldet bei fehlenden signierten Zielpaketen ausdrücklich **unsigniert** und erzeugt keine unvollständigen Manifeste. Beide Manifeste entstehen ausschließlich bei vier vollständigen signierten Zielen: macOS arm64, macOS x64, Windows x64 NSIS und Windows x64 MSI. CLI-/Installer-Assets und Berichte bleiben verfügbar. Ungültige Signaturhüllen und mehrdeutige Installer sind Fehler. Der Generator prüft das Tauri/Minisign-Hüllenformat; kryptografische Prüfung und echte Installation bleiben Aufgabe des Updaters und der Installations-/Update-Abnahme.

Plattformschlüssel: `darwin-aarch64`, `darwin-x86_64`, `windows-x86_64-nsis`, `windows-x86_64-msi`. Universal-DMG/CLI sind zusätzliche Downloads; Updates wählen das native Architekturpaket. Kein erfundener Windows-ARM-Schlüssel: x64 unter Emulation gehört zu R03.

K3-04 (vorläufige Nutzerentscheidung): bis 1.0 sind Betas **normale GitHub Releases**, keine Pre-Releases. Jeder vollständige Release trägt identische `latest.json` und `latest-beta.json`; stable aktualisiert ebenfalls beide Endpunkte. Betas lesen `https://github.com/domdewom2/openspider/releases/latest/download/latest-beta.json`. Stable-Clients lehnen Beta-Versionen ab. Beim späteren manuellen Veröffentlichen muss der freigegebene Release ausdrücklich als **Latest** gesetzt werden, sonst funktioniert `releases/latest/download/…` nicht zuverlässig. Der Entwurf setzt Latest ausdrücklich nicht. Ein Entwurf ist für den öffentlichen Updater noch nicht erreichbar.

`SHA256SUMS` erfasst alle Dateien in `release-assets` außer sich selbst, einschließlich `.sig` und Manifesten. `updater-status.txt` bleibt ausschließlich in `release-reports` (auch in der Summary), außerhalb von Release-Assets und SHA256SUMS. Die Notes und Prüfberichte sind Release-Body bzw. Actions-Artefakte und keine separaten Release-Assets.

## Noch fehlende Paket-Schnittstellen

- **Abhängigkeit L01 (Gate):** `licenses.yml` fehlt auf diesem Branch. Der lokale `workflow_call` ist als kommentierter Job `licenses` vorbereitet. Nach Integration diesen Job aktivieren und `licenses` in `needs` von `linux-core` und `assemble` aufnehmen. Schnittstelle: Artefakt `third-party-notices` mit nicht leerer `THIRD_PARTY_NOTICES.txt` in der Wurzel, aus demselben Actions-Lauf. Beide Jobs laden es bereits verpflichtend; fehlt es, scheitert der Lauf vor der CLI-Packung. Keine Platzhalter-Lizenztexte und keine Archive ohne Drittlizenzen. Der vollständige Trockenlauf setzt daher die L01-Integration voraus.

- **STUB/TODO(R03):** `release-reports/R03.txt` sowie Release-Body und Summary zeigen den fehlenden installierten Plattform-Smoke. Nach Integration `smoke-installed.yml` mit Artefakten desselben Runs verdrahten; Schnittstelle gemeinsam mit R03 festlegen. Keine vorgetäuschte erfolgreiche Smoke-Abnahme.
- **D01 (integriert):** `scripts/release/archive-compat-fixture.mjs --version <Version> --out release-reports/compat --cli <release-cli>` wird verpflichtend mit der gebauten Linux-Release-CLI ausgeführt. Fehler blockieren den Lauf. Jede Version erzeugt `compat_<version>.tar.gz`, auch bei Trockenläufen; das Archiv wird vor SHA256SUMS gepackt und als Release-Asset sowie Actions-Bericht gesichert. Ab `1.0.0-beta.1` jede veröffentlichte Version prüfen und unverändert als `tests/fixtures/migrations/releases/<version>/` ins Repo übernehmen. Diese Grenze betrifft nur die Übernahme ins Repo; der Workflow committet nichts zurück.
- **TODO(M2):** Offene Parität und Lizenz-Abnahme L01 vor externer Freigabe prüfen. Keine neue Abhängigkeit in R04.

Veröffentlichung, Update-Verteilung, Website und Paketmanager-Einreichungen folgen erst nach ausdrücklichem Nutzer-Go über M3-A01/M3-REL. Die [Release-Checkliste](checkliste-release.md) hält die manuellen Schritte fest.
