# S-5b – WebDriver gegen die echte App: Schnittstellenbedarf

Stand: 2026-10-17. Paket M2-S02, Branch `m2/s02`, Ausgangsstand `e6149ee`.
Status: **blockiert, kein Machbarkeitsurteil**. Kein grüner Smoke und kein
Nachweis eines technischen Scheiterns auf macOS; deshalb noch keine
Fallback-Entscheidung für E2E-C.

## Plattform und Startprüfung

macOS 26.3 (25D125), Node v22.20.0, Cargo 1.99.1.
Der Worktree war vor Beginn sauber. Abhängigkeiten auf `origin/main`:

```sh
git log --oneline origin/main -i --grep='Merge M2-F03' --grep='Merge M2-F04'
# 05a314e m2: Merge M2-F04 (Welle 0b)
# 509b146 m2: Merge M2-F03 (Welle 0b)
```

## Befund und benötigte fremde Änderung

```sh
rg -n 'webdriverio' apps/desktop/package.json package-lock.json
```

Das Desktop-Manifest enthält kein `webdriverio`. Das Lockfile enthält nur
Vitests optionale Peer-Metadaten zu `@vitest/browser-webdriverio`, keine
installierte `webdriverio`-Abhängigkeit. Der Plan 3.1 weist deren Vorbau F04
zu. `npm ci` kann damit den verlangten WebdriverIO-Runner nicht bereitstellen.

Benötigt: `apps/desktop/package.json` und `package-lock.json` (außerhalb
von M2-S02 `owns`, Manifestbesitz nach C7). Vorgeschlagener Diff-Ausschnitt,
**nicht angewendet**, Version erst nach Kompatibilitäts-/Lizenzprüfung exakt
festlegen:

```diff
   "devDependencies": {
+    "webdriverio": "<exakte geprüfte Version>",
```

Das zuständige Manifestpaket muss das Lockfile mit aktualisieren und die
Lizenzprüfung ausführen. Keine Installation über `npx` mit implizitem Download,
kein separates npm-Projekt zur Umgehung des Manifestbesitzes.

Das Rust-Feature `openspider-desktop/e2e-webdriver` und die bedingte
Plugin-Registrierung sind bereits vorhanden. Deren Vorhandensein belegt noch
keinen funktionierenden WebDriver-Smoke.

## Entscheidungsvorschlag und Fortsetzung

Gemäß M2.md Abschnitt 2/C10 und den Paket-Prompt-Notes wird die Umsetzung bei
Bedarf an fremden Dateien abgebrochen. Erst nach dem Manifestnachtrag fortsetzen:

1. WebdriverIO-Gerüst innerhalb `apps/desktop/e2e-b/**` erstellen.
2. Echte App mit isoliertem `OPENSPIDER_HOME` und Memory-Secret-Store starten;
   macOS mit Feature `e2e-webdriver`, Windows über `tauri-driver`.
3. Hauptfenster, separates Einstellungsfenster und Schließen-Rückfrage bei
   laufendem Fixture-Crawl prüfen (Spec-Tags UI-113, OUT-222, OUT-232).
4. Nightly-/workflow_dispatch-Job `ui-e2e-b.yml` und Plattform-Workflow anlegen;
   Fehlerlogs und Plattformdaten als Artefakte behalten.
5. Nur bei nachgewiesenem macOS-Fehlschlag E2E-C als Rückfall dokumentieren.

S-3, S-4, S-5 und S-6 wurden wegen des vorgeschriebenen Paketabbruchs nicht
durchgeführt. Es gibt keine Windows-/Linux-Messung und keine CI-Lauf-ID;
Push, Merge und PR sind vom Auftrag ausgeschlossen. Kein Spec-Kriterium wird
mit diesem Befund als umgesetzt oder bestanden ausgewiesen.

## Validierung des Befundstands

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets -- -D warnings`:
  grün (58,43 s).
- `bash scripts/check-neutral.sh`: grün; vorhandene Warnungen zu vier
  verwaisten Ausnahmen in `scripts/check-neutral.allow` bleiben unangetastet.
- Kein Frontend-Quellcode geändert; keine neuen Funktionstests oder Spec-Tags,
  da keine Funktion implementiert wurde.
- `cargo test --workspace --exclude openspider-desktop`: **rot**, Exit 101.
  `crates/os-cli/tests/read_api_export.rs:457`, Test
  `a_python_reader_reads_a_finished_file_and_rejects_an_unknown_read_api_version`:
  Python meldet `q_manifest nicht lesbar … unable to open database file`;
  Exitcode 1 statt erwartet 0. Fünf andere Tests dieses Testprogramms bestanden.
  Der Workspace-Lauf bricht dort ab; spätere Testprogramme sind damit nicht
  vollständig geprüft. Keine Änderung an dieser fremden Testdatei oder deren
  Implementierung. Gesamte Befehlsfolge einschließlich Build etwa fünf Minuten;
  keine erneute Messung des unveränderten Ausgangsstands.
