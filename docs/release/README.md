# Release-Fundament M3

Die maßgebliche Version steht in `[workspace.package] version`. Plattformneutrale
Node-Skripte (ab Node 22), aus jedem Arbeitsverzeichnis aufrufbar:

```
node scripts/release/set-version.mjs 1.0.0-beta.1
node scripts/release/check-version.mjs
```

`set-version` aktualisiert auch die npm-Workspace-/Lockversionen und Tauri. Nach einer
Versionsänderung das Cargo-Lockfile für die Workspace-Pakete nachziehen, ohne den
Abhängigkeitsbaum per `cargo update` zu aktualisieren. Die Prüfung wird von M3-R04
in CI eingebunden. MSI-Abbildung folgt in M3-R02 (K3-07).

Bundle-ID: `org.openspider.desktop` (K3-02). Vorläufige Orchestrator-Entscheidungen
vom 30.10.2026: GitHub Releases (K3-03), Beta-Datei per Build-Variable (K3-04),
Azure Artifact Signing für Windows (K3-05; Umsetzung M3-R02).

Build-Variablen:

- `OPENSPIDER_UPDATER_PUBKEY`: öffentlicher minisign-Schlüssel; wird von build.rs
  gezielt als `plugins.updater.pubkey` übernommen. Ohne gesetzte Schlüssel-/Beta-Variable
  bleibt die eingehende Updater-Konfiguration (auch `--config`/`TAURI_CONFIG`) erhalten.
  Ein Release-Build mit leerem effektivem Schlüssel gibt eine Cargo-Warnung aus.
  Kein Produkt- oder Testschlüssel im Repo-Config.
- `OPENSPIDER_UPDATE_BETA=1`: `latest-beta.json`, bei `0` `latest.json`. Die Variable
  setzt ausschließlich den Endpunkt; ein eingehender Schlüssel bleibt erhalten.
  Bei allein gesetztem Schlüssel ergänzt der Build `latest.json`, sofern kein
  Endpunkt-Override vorliegt. Ohne Build-Variablen bleibt der Endpunkt-Platzhalter leer.
- `OPENSPIDER_UPDATE_REPO`: GitHub-Repository als `owner/repo`; der Standard steht
  einmalig in `apps/desktop/src-tauri/build_update.rs` und wird von `build.rs` verwendet.
  Endpunkte entstehen als `https://github.com/<repo>/releases/latest/download/<manifest>`.
  Diese Variable allein überschreibt keine eingehende Updater-Konfiguration.
- Der feste HTTP-User-Agent des Update-Kerns ist `OpenSpider-Updater`, ohne Kennung.

K3-04, Orchestrator-Entscheid vom 30.10.2026: Bis 1.0 werden Betas als **normale
GitHub-Releases**, ohne Pre-Release-Markierung, veröffentlicht. Jeder Release trägt
beide Manifeste `latest.json` und `latest-beta.json`. Beta-Builds verwenden
`releases/latest/download/latest-beta.json`. Die Veröffentlichung bleibt an das
Go des Nutzers gebunden.

F01 stellt ausschließlich Stubs bereit: keine Hintergrundprüfung, kein Download,
keine Installation; Commands melden `Unsupported`, CLI-Hooks Exit 2. Verifikation
schlägt geschlossen fehl. Die tatsächliche Plugin-Registrierung und die sichere
Deaktivierung bei fehlendem/ungültigem Schlüssel ergänzt M3-U02 (Besitz M3.md 4.2).
Bis dahin wird das Updater-Plugin nicht initialisiert; ein leerer Schlüssel löst
keine Requests und keinen Updater-Initialisierungsfehler aus.

Release-Builds müssen vor dem Desktop-Build die CLI nach
`apps/desktop/src-tauri/binaries/openspider-<target-triple>[.exe]` legen. Debug-Checks
ohne gestagte CLI entfernen nur für diesen Build den externalBin-Eintrag; Release
verlangt das echte Binary. M3-R01/R02 füllen die leeren Plattformdateien. Ressourcen
unter `resources/third-party` sind bis M3-L01 ein Platzhalter.

Mock: `os_testsite::mocks::update`, Routen unter `/update`: `valid/latest.json`,
`tampered/latest.json`, `older/latest.json`, `http/latest.json`, `artifact`,
`interrupted` (Download-Abbruch). Gültig heißt im Gerüst nur formatgültig;
M3-U01 ergänzt Testschlüssel und kryptographisch signierte Daten ausschließlich in
`tests/fixtures/update`. M3-D01 füllt das Migrationsarchiv.

Veröffentlichen, Signieren mit Produktzugängen und Push sind kein Teil von F01.
