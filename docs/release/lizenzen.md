# Drittlizenzen im Release (M3-L01)

Der Release erzeugt `apps/desktop/src-tauri/resources/third-party/THIRD_PARTY_NOTICES.txt`
vor Vite und Tauri. Git enthält dort ausschließlich einen Entwicklungsplatzhalter.
Die Datei enthält je Komponente Name, Version, SPDX-Ausdruck, vollständige Lizenztexte
und verfügbare Copyright-/NOTICE-Angaben. Gleiche Volltexte stehen einmal am Ende;
das Inventar verweist über eindeutige Text-IDs darauf. `NOTICE` wird zusätzlich unverändert in das
Ressourcenverzeichnis kopiert. Beide Texte stehen unter Hilfe → Über → Drittlizenzen;
die Suche filtert Absätze ohne Beachtung der Groß-/Kleinschreibung. Leere Suche zeigt
wieder den vollständigen Text. Die Projektlizenz bleibt daneben separat erreichbar.

## Erzeugung

Voraussetzungen: Rust laut `rust-toolchain.toml`, Node 22, `npm ci`, Internetzugang zu
crates.io und den im Quellenmanifest festgeschriebenen GitHub-Dateien. Das Werkzeug
`cargo-about 0.9.3` ist MIT OR Apache-2.0 lizenziert, dient ausschließlich Build/CI
und wird weder Cargo-Workspace-Abhängigkeit noch Bestandteil des Produkts.

Von der Repository-Wurzel unter Linux:

```sh
timeout 3600s nice -n 10 cargo install cargo-about --version 0.9.3 --features cli --locked -j 4
node --test scripts/release/licenses/generate.test.mjs
timeout 3600s nice -n 10 node scripts/release/licenses/generate.mjs
OPENSPIDER_REQUIRE_NOTICES=1 npm run -w apps/desktop build
```

Die Plattform-Workflows `release-macos.yml` und `release-windows.yml` installieren das
Werkzeug nur bei einem Cache-Miss und erzeugen die Datei nach `npm ci`. Fehler stoppen
den Build. `actions/cache` (SHA-gepinnt, v4.3.0) speichert ausschließlich das
Werkzeugverzeichnis `runner.temp/cargo-about/bin`; der exakte Schlüssel enthält
cargo-about-Version, CLI-Feature, Betriebssystem, Architektur und Cache-Revision.
Es gibt keine unscharfen Restore-Schlüssel und keinen Cache für Produkt-Builds.
Auch `licenses.yml` verwendet diesen Cache. Bei Versions-/Featureänderungen den
Schlüssel mitändern; `v1` bei Bedarf zur Neuerzeugung erhöhen.

Beide Release-Workflows setzen beim Tauri-Build `OPENSPIDER_REQUIRE_NOTICES=1`.
Nur mit diesem exakten Wert lehnt Vite einen Platzhalter oder fehlende Rust-/npm-
Abschnitte ab. Normales `npm run -w apps/desktop build`, `tauri build --debug`,
der Entwicklungsserver und Frontend-Tests funktionieren mit dem Platzhalter.
Tauri bündelt das gesamte Ressourcenverzeichnis; Vite bettet denselben Text für den
Über-Dialog ein. Die Anzeige braucht weder Netzwerk noch Dateisystemrechte; das lokale Textmodul wird
erst beim Öffnen von Über geladen.

`licenses.yml` ist über `workflow_dispatch` und `workflow_call` sowie bei PRs und
Push auf main erreichbar. Er prüft Generator-Negativfälle, die bisherige npm-Positivliste,
`cargo deny check licenses` und die vollständige Erzeugung. Das Actions-Artefakt
`third-party-notices` enthält Textdatei und NOTICE. Es veröffentlicht keinen Release.

## Umfang und Prüfregeln

- Rust: `cargo about generate --locked --workspace --fail`, ohne Dev-Abhängigkeiten,
  einschließlich Build-Abhängigkeiten. Targets und erlaubte Lizenzen werden direkt
  aus `deny.toml` übernommen: macOS arm64/x64 und Windows x64. Ein zweiter Graph erfasst ausschließlich
  `crates/os-cli/Cargo.toml` für Linux x64 (ohne Linux-Desktop-Abhängigkeiten). Das ist eine konservative
  Vereinigung aller Workspace-Produktionsabhängigkeiten einschließlich CLI. Änderungen
  an der Positivliste sind keine Aufgabe dieses Generators.
- npm: `npm ls --workspace apps/desktop --omit=dev --all --parseable` liefert den
  installierten Produktionsbaum einschließlich transitiver und Peer-Abhängigkeiten.
  Name/Version/Lizenz werden aus dem Paket gelesen, Version und Lizenz mit dem Lockfile
  abgeglichen. Die Positivliste ist dieselbe wie im bestehenden npm-Prüfer; dessen
  ausschließlich für Dev-Pakete geltende Ausnahmen werden hier nicht angewendet.
- SPDX: OR braucht eine erlaubte Alternative, AND alle Bestandteile, WITH eine explizit
  erlaubte Ausnahme. Unbekannte/fehlende Angaben, fehlerhafte Ausdrücke und leere Texte
  brechen die Erzeugung ab. Es gibt keine allgemeine Ausnahme für fehlende Texte.
- Lizenz-, Copyright- und NOTICE-Dateien aus den Paketarchiven bleiben erhalten.
  `rust-sources.json` und `npm-sources.json` ergänzen fehlende Archivdateien aus festen
  Upstream-Commits; SHA-256 und HTTPS sind verbindlich. Eine Versionsänderung braucht
  eine erneute Quellenprüfung. Ausfall oder geänderte Bytes stoppen den Build.
- Fünf ältere Rust-Pakete veröffentlichen auch upstream keine vollständige Lizenzdatei
  bzw. verweisen auf ein nicht mehr erreichbares Repository: genawaiter 0.99.1,
  genawaiter-macro 0.99.1, genawaiter-proc-macro 0.99.1, next-gen-proc_macros 0.1.1,
  selectors 0.38.0. `rust-canonical.json` dokumentiert diese versionsgebundenen Fälle.
  Hier werden die vollständigen kanonischen SPDX-Texte von cargo-about, die Autoren
  und das Repository aus Cargo.toml und ein ausdrücklicher Herkunftshinweis geliefert.
  Die Lizenzdeklaration muss exakt zum geprüften Eintrag passen. Neue Fälle scheitern.
- Bei npm fehlen Dateien in @fluent/bundle und @fluent/langneg (jeweils Lizenz aus dem
  Release-Commit), client-only (React-MIT-Lizenz aus 2022, Produkt-/Lizenzbezug laut
  veröffentlichtem Paketmanifest) und react-easy-swipe (nachträglich upstream ergänzte
  MIT-Datei mit Copyright 2016 Leandro Lemos). Gründe und Hashes stehen im Manifest.
- axe-core 4.11.0 wird bereits als JS-Ressource ausgeliefert: Lizenz und seine eigenen
  Dritthinweise werden zusätzlich vollständig aufgenommen. Liberation-Sans-Schriftdateien
  werden nicht ausgeliefert; der bestehende Projekt-NOTICE erklärt ihre Nutzung.
- K3-01: Chromium, Lighthouse und Node werden nicht gebündelt. Daher werden dafür keine
  Lizenztexte als mitgelieferte Komponenten behauptet. Bei künftig geänderter Bündelung
  muss dieses Inventar vor dem Release erweitert werden.

## Übergabe an Release-Orchestrierung

M3-R04 muss die erzeugten Hinweise auch dem separaten Linux-CLI-Archiv beilegen und den
Lizenzjob als Freigabebedingung verwenden. Dafür kann es `licenses.yml` per workflow_call
aufrufen und das Artefakt `third-party-notices` übernehmen. Die Schnittstelle ist:
keine Inputs, keine Secrets, keine Workflow-Outputs; `permissions: contents: read`.
Das Artefakt enthält im Wurzelverzeichnis `THIRD_PARTY_NOTICES.txt` und `NOTICE`.
Der nachgelagerte Archiv-/Release-Job muss per `needs` vom erfolgreichen Lizenzjob
abhängen und dieses Artefakt aus demselben Workflow-Lauf herunterladen.

```yaml
jobs:
  licenses:
    uses: ./.github/workflows/licenses.yml
    permissions:
      contents: read
  # Im bestehenden R04-Job: needs: [licenses, ...]
  # actions/download-artifact (SHA-gepinnt) mit name: third-party-notices
```

Die Desktop-Workflows erzeugen für ihre Installer bereits selbstständig vollständige Hinweise.

STUB/TODO(R03): Mit ausgeliefertem Installer prüfen, dass `third-party/NOTICE` und
`third-party/THIRD_PARTY_NOTICES.txt` vorhanden sind und Hilfe → Über offline funktioniert.
STUB/TODO(D01): Archiv-Kompatibilitätsprüfung folgt im zugehörigen Paket; sie ist keine
Voraussetzung der Lizenzerzeugung. M2-Paritätsnachweise bleiben Aufgabe des Integrators.

Generierte Texte nie committen. Nach lokaler Prüfung die Platzhalterdatei aus dem
Commit wiederherstellen; das generierte `third-party/NOTICE` ist gitignoriert.
