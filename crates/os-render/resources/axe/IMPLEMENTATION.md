# M2-D21 – Umsetzung und Integrationsbericht

Umgesetzt: TECH-250, TECH-251, TECH-252, TECH-253, TECH-254, TECH-255,
TECH-256, TECH-257, TECH-259, OUT-062, OUT-109; zusätzlich UI-064 [P1]
und der Detail-Export nach OUT-021 / K2-72.

## Verhalten

- Unverändertes axe-core 4.11.0, alle 93 spezifizierten Regeln explizit aktiviert,
  nicht verfügbare Regeln einmalig protokolliert. In 4.11.0 sind alle 93 vorhanden. Ein Audit im bestehenden CDP-Seitenlauf.
- Alle Ergebnisarten; je Regel eine kanonische Ergebniszeile. axe kann dieselbe
  Regel für verschiedene Knoten in mehreren Ergebnisarten melden. Priorität der
  Zusammenfassung: Violation, Incomplete, Passed, Inapplicable. Nur Violation
  speist Zähler und Issues. Passed/Inapplicable speichern weder Impact noch Knoten.
- Knoten: Selektor, HTML bis 500 Unicode-Zeichen, eigene Beschreibung und
  `axe_message` als JSON-Felder in der vorhandenen `summary`-Spalte. Keine Migration.
- Population: interne, sichtbare HTML-200-Seiten mit Audit-Ergebnis und aktivem
  JavaScript-Rendering/Accessibility. 16 Spalten, 93 Regelfilter, feste Gruppen.
- Details unterstützen Ergebnis-/Regelauswahl, Mehrfachauswahl, Sortierung,
  Paging und stabile Knoten-IDs. Die UI übergibt `rule` (Kurztext); alternativ
  wird `ruleId` unterstützt. Leere Parameter bedeuten All; die UI setzt ihren
  Standard Violation ausdrücklich.
- Bulk: eine Zeile pro Knoten, einschließlich separatem Incomplete-Export;
  Summary: Seitenzahlen und Prozente, nach Priorität und Seitenzahl sortiert.
- Detail-Export: CSV `_rules.csv` und `_locations.csv`, XLSX `Rules`/`Locations`.
  Das Operationsergebnis enthält zusätzlich `files`; `path` bleibt die erste Datei,
  `bytes` enthält die Gesamtsumme. Alle anderen Detailarten behalten ihren Pfad.

## Fremdergänzungen (K2-61)

Die exakten Diffs der vorhandenen Dateien stehen in [foreign-additions.patch](foreign-additions.patch).

| Datei | Additive Ergänzung | Spec |
|---|---|---|
| `apps/desktop/src-tauri/tauri.conf.json` | zusätzliche Resource-Map: unverändertes Bundle, Lizenz und Notices separat im App-Paket | TECH-250 |
| `NOTICE` | Attribution und Verweis auf vollständige MPL-Lizenz | TECH-250 |
| `crates/os-render/src/api.rs` | `RenderOpts.accessibility = false`; `RenderedPage.accessibility = None` | TECH-250 |
| `crates/os-render/src/page.rs` | bedingter Audit-Aufruf und zusätzliches Ergebnisfeld | TECH-250 |
| `crates/os-engine/src/render_glue.rs` | Konfigurationswert an Renderoptionen weitergeben | TECH-250 |
| `crates/os-engine/src/worker.rs` | zusätzliche A11yWrite-Zeilen im bestehenden Batch | TECH-250 |
| `crates/os-analyze/src/details/mod.rs` | zusätzliche Adapter für beide A11y-Detailarten | TECH-259 |
| `crates/os-analyze/src/tabs/pagespeed.rs` | neue Funktion für bestehende PSI-Status-/Fehlerspalten | TECH-251 |
| `crates/os-app/src/commands/view.rs` | zusätzliche Auflösung echter Knoten-IDs nur für A11yNodes | TECH-259 |
| `apps/desktop/src/features/detail/a11y/i18n/de.ftl` | neue Filter-/Regeltext-Schlüssel | TECH-251, TECH-259 |
| `apps/desktop/src/features/detail/a11y/i18n/en.ftl` | dieselben neuen Schlüssel, eigene englische Texte | TECH-251, TECH-259 |

Zusätzliche generierte Goldens: `docs/issues/accessibility-*.md` (93 neue Dateien),
ausschließlich aus den neuen D21-Katalogeinträgen und Texten. Kein vorhandenes
Issue-Dokument wird geändert. Weitere additive Ausgabe-Goldens:
`tests/golden/out-cli/help_export_tabs.txt` und `tests/golden/out-cli/list_exports.txt`
(je genau 93 neue Accessibility-Regelzeilen, TECH-251; Diff in `foreign-additions.patch`).
Die vollständigen Diffs der Issue-Dokumente stehen in
[issue-goldens.patch](issue-goldens.patch).

## Stub-Ablösung (K2-60)

`crates/os-app/tests/manifest_contract.rs`: ausschließlich `M2-D21` aus der Liste
noch erwarteter Stub-Besitzer entfernt. `detail.a11yNode` ist implementiert.

## Fremdänderung (K2-72)

`crates/os-app/src/commands/export.rs`: zusätzlicher Zweig ausschließlich für
Accessibility Details. Die Daten/Spalten und Writer-Steuerung liegen in D21-eigenen
Modulen; vorhandene Writer bieten bereits mehrere XLSX-Blätter. Exakter Diff in
`foreign-additions.patch`. Spec: TECH-259, OUT-021.

## Erwartungsänderungen (K2-63)

- `crates/os-analyze/tests/catalog_split.rs`: erwartete Issue-Filtermenge um die
  93 konkreten Accessibility-Regel-IDs erweitert (TECH-251/252–257, UI-064).
- `crates/os-analyze/tests/filters_registry.rs`: erwartete dynamische Filterliste
  um dieselben 93 IDs erweitert; erwartete Stub-Anzahl 5 → 4 (TECH-250/251).
- `crates/os-analyze/tests/framework_registry.rs`: Accessibility zur Erwartung
  „All benötigt aktivierte Konfiguration“ ergänzt (TECH-250).
- `crates/os-analyze/tests/issues_catalog.rs`: erwartetes M2-Register um die
  konkrete Expansion des axe-Platzhalters ergänzt (TECH-251, UI-064).

Die Assertions und die Prüfung fremder Kategorien bleiben erhalten. Exakte Diffs
stehen in `foreign-additions.patch`; die feste erwartete ID-Liste liegt in der
D21-Fixture `rule-ids.txt`.

## Platzhalter-Ablösung (K2-71)

Keine zusätzlichen Änderungen unter K2-71 erforderlich.

## Abweichungen und offene Fragen

Die Prioritäten folgen ausdrücklich den 93 Tabellenwerten TECH-252–257 und
UI-064 sowie dem Paketauftrag. Der ältere Anhang B in `03-tabs-technical.md`
fordert widersprüchlich eine Ableitung aus dem aktuellen axe-Impact. Diese wird
nicht angewendet. Das axe-Laufzeitfeld Impact wird unabhängig davon angezeigt.
TECH-258 (P2, Score-Filter) gehört nicht zum Paket; die Score-Spalte liest vorhandene
PSI-Werte, ohne selbst PSI oder Lighthouse aufzurufen.

## Prüfung

Gezielte Tests grün: Renderer mit echter Chromium-Ausführung (einschließlich
sauberer Seite und aller 93 aktivierten Regeln), Engine → Writer → Analyse,
Population/Regelzählung, Ergebnisarten, Unicode-Kürzung, Frame-/Shadow-Selektoren,
Detail-Paging und stabile IDs, Katalog/DE/EN-Texte, neun CSV-Goldens sowie
CSV-Zweiteilung/XLSX-Blätter. Desktop: `npm ci`, Typecheck und 72 Testdateien / 711 Tests grün.

`cargo fmt --all --check` und `cargo clippy --workspace --exclude openspider-desktop --all-targets -- -D warnings` sind grün.
`cargo test --workspace --exclude openspider-desktop` ist vollständig mit Exit 0 abgeschlossen: 3636 Tests bestanden, 53 gemäß Testsuite ignoriert, keine Fehler. Die tatsächlichen Chromium-Integrationstests wurden ausgeführt.

## Herkunft

Quelle: https://github.com/dequelabs/axe-core/releases/tag/v4.11.0
Distribution: https://registry.npmjs.org/axe-core/-/axe-core-4.11.0.tgz
SHA-256 `axe.min.js`: `e9e5863c33a874f09bc01acd9234b7e3c871479f5eef8802fa582544465e6d01`.
Desktop-Resource-Map nach https://v2.tauri.app/develop/resources/; alle drei
Quellpfade geprüft. Das Release-App-Bundle selbst wurde nicht gebaut (Desktop
ist gemäß Paketauftrag aus den Rust-Workspace-Prüfungen ausgeschlossen).

Clean-Room: ausschließlich Projektspec, Projektquellen und axe-core; kein Zugriff
auf das Referenzprodukt. Kein Push, Merge oder PR.
