# M2-D22 – Custom JavaScript

Clean-Room-Umsetzung anhand von `docs/spec/04-custom-integrations.md`, ohne Zugriff auf das Referenzprodukt. Ausgangsbranch `m2/d22`, Arbeitsverzeichnis zu Beginn sauber.

## Umgesetzte Spec-IDs

- **INTEG-060:** transaktionale Snippet-Verwaltung (`SnippetManager`: Hinzufügen/Bearbeiten/Entfernen/Umsortieren, 100er-Grenze, eindeutige und automatische Namen), MIME-Filter inklusive kommagetrennter Typen, Parameter-/Großschreibungstoleranz und `*`; Ausführung ausschließlich intern/200/JavaScript/nicht-PDF. Zähler und sequenzielles Ausführungsprotokoll in `url_ext` (`custom_js.executed`, `custom_js.log`), auch null ausgeführte Snippets im Textmodus. Konfigurationswarnung zum Textmodus über den vorhandenen Vertrag.
- **INTEG-061:** asynchrone Extraktionen, Einzelwerte/Arrays/JSON-Objekte, fehlendes `return`, Exceptions und Timeout. CDP `Runtime.terminateExecution` beendet auch synchrone Endlosschleifen. `timeout_s = 0` erbt den Rendering-Timeout. Passive Seitenressourcen werden während der Extraktion gestoppt/blockiert; explizite Fetch-/XHR-Aufrufe bleiben gemäß INTEG-072 unter der vorhandenen Ressourcenrichtlinie erlaubt.
- **INTEG-062:** Aktionen vor Extraktionen, jeweils Tabellenreihenfolge; gemeinsame Wartezeit entspricht dem größten Aktions-Timeout. Änderungen stehen vor DOM-Erfassung, Link-Erkennung und Extraktion zur Verfügung. Aktionen liefern weder Spalten noch Filter. Timeout ist bei Aktionen Pflicht (1–120 s).
- **INTEG-063:** identische APIs `openspider` und `seoSpider`, auch über `window`; `data`, `error`, null/undefined/leere Arrays, Objektserialisierung.
- **INTEG-071:** dynamische, crawlweit stabile Wertspalten und Fehlerspalten im Custom-JavaScript- und Internal-Tab; pro Extraktion ein Filter mit ausschließlich eigenen Spalten. Auch leere und fehlgeschlagene Ausführungen bleiben filterbar. Persistenz und Wiederöffnen über den vorhandenen Writer.
- **INTEG-072:** Ausführung im isolierten Chromium-Kontext ohne Node-/Tauri-/Host-Dateibindings; Dateioperationen verweigert. `PendingImport` hält importierten Code außerhalb der ausführbaren Konfiguration, bietet eine Vorschau und verlangt explizites `confirm`. Warnung vor Fetch/XHR-Code über die bestehende Konfigurationsprüfung in App/CLI.

Die Verwaltung ist eine transportunabhängige Kern-API innerhalb der Paketpfade. Die Desktop-Dialoge mit Snippet-Editor gehören laut Paketplan zu M2-U13B. Die Tab-Spalten nutzen die bestehenden Ansichtsverträge.

## Tests

Fixture-Verzeichnis: `tests/fixtures/integ-custom-js/`. Chromium ist Pflicht, fehlender Browser führt zum Testfehler. Sämtliche Netztests verwenden Loopback.

- `os-render::snippets::tests`: Verwaltung/Import-Freigabe, MIME-/Modus-/Status-Gates, echte Browser-Extraktion inklusive Endlosschleife und nächster URL, Ressourcenblockierung/Fetch-Freigabe, 3-/8-Sekunden-Aktionen mit gemeinsamer 8-Sekunden-Wartezeit, DOM-/Hover-/Scroll-Auswirkungen, geerbter Timeout bei dauerhaft offenem Promise, getrennte Ressourcenblockierung paralleler Seiten auch bei gemeinsam verwendeten Renderoptionen. Tests tragen die jeweiligen `// spec: INTEG-…`-Referenzen.
- `os-analyze/tests/tab_custom_js.rs`: persistierte Mehrfachspalten, Filter/Counts/Fehler/leere Ergebnisse, Internal-Spalten, Wiederöffnen, echte Crawl-Link-Erkennung aus Aktionen, Scope/MIME-Gates im echten Crawl, null Ausführungen im Textmodus, Snippet-Zeitbudget auch für eingesetzte Renderer-Overrides mit virtueller Uhr.
- Schema-Golden über `UPDATE_GOLDEN=1 cargo test -p os-model --test config_schema --locked` erzeugt: fünf Tests grün.
- Neue Pakettests: sechs Laufzeittests (davon vier mit echtem Chromium) und fünf Crawl-/View-/Budgettests, alle grün.
- `npm run -w apps/desktop check:licenses`: 474 Pakete unter der Lizenz-Positivliste.
- `cargo deny check licenses bans sources`: grün.
- `scripts/check-neutral.sh`: grün.
- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo test --workspace --exclude openspider-desktop --locked`: **3596 bestanden, 63 vorhandene Ignores, 0 Fehler**, 473 Testprogramme einschließlich Doc-Tests. Keine neuen Ignores.
- Vollständiger letzter Durchlauf einschließlich Neubau: **1462.80 s** (24 min 23 s); gemeldete Laufzeiten der einzelnen Testprogramme zusammen **817.41 s**.
- Der nachträglich ergänzte Budgettest ist zusätzlich isoliert mit virtueller Uhr grün (0,04 s).

Rust-Prüfungen mit `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_STRIP=debuginfo`, `CARGO_PROFILE_TEST_STRIP=debuginfo`, `CARGO_PROFILE_DEV_OPT_LEVEL=1`, `CARGO_PROFILE_TEST_OPT_LEVEL=1`. Funktionsnamen für Backtrace-Prüfungen bleiben erhalten; Debug-Assertions und Overflow-Prüfungen entsprechen weiterhin dem Testprofil.

Die Builds mit normalen Debug-Informationen und anschließend mit M2/C5 (`line-tables-only`) überschritten den verfügbaren Plattenplatz. Ein weiterer Versuch mit entfernten Symboltabellen lief bis zum bestehenden Panik-Logging-Test, der Funktionsnamen im Backtrace erwartet. Daher abschließend leichter optimierter Neubau mit erhaltenen Funktionssymbolen. Ausschließlich der eigene Build-Cache `target-d22` wurde bereinigt. Keine Testlogik oder Erwartung wegen dieser Umgebungsprobleme geändert.

## Fremdergänzungen (K2-61)

Alle folgenden Stellen sind additiv; leere Standardwerte erhalten das Verhalten ohne Snippets. Vollständige Diffs stehen in [foreign-additions.patch](foreign-additions.patch).

| Datei | Ergänzung | Spec |
|---|---|---|
| `crates/os-render/src/api.rs` | `RenderOpts.snippets` mit leerem Default, `RenderedPage.snippets` mit leerem Default | INTEG-060–063 |
| `crates/os-render/src/page.rs` | zusätzlicher Lauf vor DOM-Erfassung, Rückgabefeld, zusätzliches Zeitbudget für konfigurierte Snippets | INTEG-061/062 |
| `crates/os-render/src/intercept.rs` | zusätzlicher, nur während Extraktion aktiver Ressourcen-Blocker | INTEG-061/072 |
| `crates/os-engine/src/render_glue.rs` | Initialisierung des neuen Optionsfelds aus Scope/Status/MIME/Konfiguration; zusätzlicher Snippet-Budgetaufruf auch für Renderer-Overrides (ohne Snippets identische Deadline) | INTEG-060/062 |
| `crates/os-engine/src/worker.rs` | zusätzliche Writer-Beiträge für Werte/Protokolle/Zähler; zusätzliche Ausführung für passende Nicht-HTML-Ressourcen, nur bei konfigurierten Snippets im JS-Modus | INTEG-060/062/071 |
| `crates/os-engine/src/config_check.rs` | zusätzlicher Snippet-Netzwerkhinweis | INTEG-072 |
| `crates/os-cli/src/plan.rs` | Ausgabe ausschließlich dieses zusätzlichen Netzwerkhinweises | INTEG-072 |
| `crates/os-analyze/src/tabs/mod.rs` | zusätzliche Spaltenquelle für Internal | INTEG-071 |

Keine durch additive Ausgabe veränderten fremden Fixtures/Goldens.

## Stub-Ablösung (K2-60)

`crates/os-engine/f05a-stubs.toml`: ausschließlich die beiden Einträge für `crates/os-analyze/src/tabs/custom_js.rs::tab` entfernt (D22 und dessen doppelte F02-Metadaten). Der Tab liefert echtes Verhalten. `crates/os-analyze/tests/filters_registry.rs::every_tab_is_a_real_definition_or_a_marked_stub`: erwartete Stub-Anzahl **5 → 4**, ausschließlich wegen der Ablösung des D22-Tabs (INTEG-071). Diffs ebenfalls in `foreign-additions.patch`.

## Platzhalter-Ablösung (K2-71)

`crates/os-model/src/config/v2.rs`, `JsSnippet::default`:

```diff
-            timeout_s: 10,
+            timeout_s: 0,
```

**INTEG-061/062:** Der bisherige Vorbau-Default begrenzte Extraktionen implizit auf 10 Sekunden statt auf den Rendering-Timeout und ließ Aktionen ohne explizites Timeout zu. Null bedeutet bei Extraktionen Vererbung; die bestehende Aktionsvalidierung lehnt Null bereits ab. Explizit konfigurierte positive Timeouts bleiben erhalten. Keine fremde Signatur und keine fremde Testlogik geändert.

## Erwartungsänderung (K2-63)

`docs/spec/schemas/osconfig.schema.json`, ausschließlich `$defs.JsSnippet.properties.timeout_s.default`: **10 → 0**. Generiertes Schema-Golden an den notwendigen Default aus INTEG-061/062 angepasst; sonst keine Änderung.

`crates/os-analyze/tests/filters_registry.rs::dynamic_column_sources_are_registered_per_tab_and_names_are_interned`: Erwartung um **Custom JavaScript** und eine weitere **Internal**-Spaltenquelle ergänzt (INTEG-071). Die Testlogik bleibt unverändert.

`crates/os-analyze/tests/framework_registry.rs::overview_labels_use_the_current_thresholds_and_states_are_complete`: ausschließlich für `Custom JavaScript:All` im Textmodus Erwartung **Ready → NeedsRendering** und Prozentwert **Some(true) → None** (INTEG-060/071; Rendering ist Voraussetzung). Die vorhandene Prüflogik bleibt bestehen; nur die erwarteten Werte unterscheiden diesen Tab.

## Abweichungen und offene Fragen

Keine zusätzlichen Abhängigkeiten und keine Frontend-Dateien geändert. Snippet-Bibliothek/„Aus Bibliothek hinzufügen“ bleiben außerhalb dieses P1-Pakets (INTEG-069 ist P2). Datei-Schreib-/Downloadfunktionen (INTEG-064/065, P2) sind nicht implementiert und werden sicher verweigert. Import-Freigabe und Verwaltung werden als Kern-API bereitgestellt; eine UI-Bibliothek wird nicht hinzugefügt.

Offene Spec-Frage zu INTEG-060/071: Für Snippet-Namen, die mit Systemfiltern oder Basisspalten kollidieren (etwa `All` oder `Address`), und Kollisionen erzeugter Fehlerspalten (etwa `x` und `x Error`) ist keine Reservierungs-/Umbenennungsregel definiert. Die bestehenden Register teilen diesen Namensraum. Es wird keine zusätzliche Namensregel erfunden; dafür ist eine Planerentscheidung erforderlich.

Keine CI-Lauf-ID: gemäß Auftrag kein Push, Merge oder PR. Lokal macOS arm64; weitere Plattformen sind hier nicht ausgeführt.
