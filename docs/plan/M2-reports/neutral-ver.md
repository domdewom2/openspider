# neutral-ver – Neutralität der Referenzangaben

Nutzerentscheidung vom 2026-10-29; Branch `m2/neutral-ver`, Basis `origin/m2/int-16`. Lokale Umsetzung, kein Push, Merge oder PR.

Geänderte Dateien (Verzeichnisgruppen beziehen sich nur auf die im Commit geänderten Dateien):

- `.github/workflows/parity.yml`, `scripts/check-neutral.{sh,allow}`.
- `docs/ARCHITECTURE.md`; `docs/spec/{01-crawl,02-tabs-onpage,03-tabs-technical,04-custom-integrations,05-ui,06-output-ops,10-agent-interface,PARITY,README,reviews-open-v1}.md`.
- `docs/plan/{M1,M2}.md`, `docs/plan/{M1,M2}-packages.json`, `docs/plan/M1-acceptance/checklist.md`, dieser Bericht.
- `crates/os-fetch/src/auth.rs`, `crates/os-parity/src/manifest.rs`.
- `tests/fixtures/crawl-robots/scenario.toml`, `tests/fixtures/crawl-robots/hosts/big.site.test/scenario.toml`.
- `tests/golden/parity/README.md` und alle zehn `tests/golden/parity/*/manifest.toml`.
- `tests/parity/{README.md,deviations.toml,profiles/parity.osconfig,gen/m2_exports.py}` und 28 Dateien unter `tests/parity/mappings/*.toml`.

Ersetzungsregeln: Produktversionen werden zu „Referenzstand (Oktober 2026)“, „frühere/jüngere Referenzversion“ oder entfallen; Free-Angaben bleiben als „Free-Version“. Release-Notes-Spannen und Versionslisten heißen „mehrere Hauptversionen“, einzelne Feature- und Bugfix-Zuordnungen bleiben ohne konkrete Nummer erhalten. In ARCHITECTURE entfallen der bisherige Referenzstand-Zusatz und die URL-Grenze wie beauftragt. Historische Anweisungen, Versionsnummern beizubehalten, sind an die neue Nutzerentscheidung angepasst.

`ref_version` bleibt als Feld erhalten und enthält überall den neutralen Kennzeichner `referenz-2026-10` (Manifeste, Generator, Rust-Fixture und Doku-Beispiel). Codeprüfung: `Manifest.ref_version` ist `Option<String>`; keine Auswertung einer bestimmten Produktversion. Der bestehende Parser-Test prüft nun ausdrücklich den neutralen Wert. Referenz-CSV-Messdaten sind unverändert.

Das zusätzliche Muster prüft beide Reihenfolgen von Referenzmarker und numerischer Version innerhalb derselben Zeile, unabhängig von Groß-/Kleinschreibung. Marker umfassen auch `ref_version` und `ref_settings`; Hauptversionsbereich ist zehn bis neunundzwanzig. Teilstrings werden im Skript zusammengesetzt. 105 neue, begründete Allowlist-Einträge decken die geprüften Fehlalarme ab: Kalenderdaten, Plattform-/Laufzeitversionen, Tausenderzahlen und Planabschnitte. Das Muster wird dafür nicht abgeschwächt.

Bewusst erhalten: Rust-, Node-, npm-, React-, Browser-, Betriebssystem- und Bibliotheksversionen (etwa axe-core), OpenSpider-Versionen, Google-Regeldateiversionen sowie Spec-Revisionen. Ebenfalls erhalten sind Datumsangaben, Mengen, Messwerte, Prozentwerte, IP-Adressen und Abschnittsnummern. Beispiele für Grenzfälle: Referenzhardware A mit macOS, Safari als Mindestplattform, Tausenderzahlen bei semantischer Suche und Suchvolumen sowie `3.14.*` als Planabschnitte; diese haben keinen Produktversionsbezug. Keine ungeklärte Produktversionsangabe zurückgelassen.

Validierung (lokal, macOS arm64):

- `bash scripts/check-neutral.sh`: grün, keine Fundstellen oder verwaisten Ausnahmen.
- `python3 -I docs/plan/M2-check.py`: grün.
- `cargo fmt --all --check`: grün.
- `cargo clippy -p os-parity --all-targets -- -D warnings`: grün.
- `cargo test -p os-parity`: grün, 168 Tests bestanden, keine ignoriert.
- 50 zusätzliche isolierte Skriptprüfungen in einem temporären Git-Repository: Marker, Groß-/Kleinschreibung, Reihenfolge, Versionsbereich, Zeilengrenzen, Allowlist und Referenz-CSV-Ausnahme grün; `git diff --check` grün.

Aufräumen: Paketbezogenes Cargo-Target `/Users/dominikwojcik/Projects/build.noindex/target-neutral-ver` nach den Gates gelöscht; im Worktree existiert kein `target`.
