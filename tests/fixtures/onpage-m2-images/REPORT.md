# M2-D03 – Images

Stand: 18.10.2026, Branch `m2/d03`, lokal auf macOS arm64, Rust 1.99.1. Der Worktree war beim Start sauber. Clean-Room: ausschließlich Projekt-Specs, eigene Fixtures und offene Dateiformat-Spezifikationen; kein Zugriff auf das Referenzprodukt oder dessen Exporte.

## Umgesetzte Spec-IDs

- **CRAWL-055, ONPAGE-170 [M2]:** img-/picture-srcset erzeugt bei aktivierter Option Image-Links für alle Kandidaten (inklusive w-/x-Deskriptoren und Komma in der URL). Instanzen bleiben auch ohne Option erhalten; ein Loopback-Crawl prüft URL-Mengen, gespeicherte Instanzen und tatsächlich empfangene Requests.
- **ONPAGE-172:** Auswahl nach einbindender Seite oder Bild-URL, eindeutige Vereinigung bei Mehrfachauswahl, Gesamtzahl, Paging und Sortierung. Paritätsspalten in Spec-Reihenfolge; `Source` und `Alt Text Length` nur mit `extensions=true`. Reale/angezeigte Maße nur bei Viewport-Rendering; SVG ohne reale Rastermaße. Optional liest der Antwort-Hook PNG/JPEG/GIF/WebP/AVIF-Dateiköpfe, auch für externe Ressourcen (`image.real_w`, `image.real_h` in `url_ext`). AVIF berücksichtigt die Eigenschaftszuordnung des primären Items. XPath, Linkposition, Pfadart und Herkunft bleiben je Instanz erhalten.
- **ONPAGE-178:** optionale Instanzen aus style-Attributen und style-Blöcken über den bestehenden CSS-Parser. Die Crawl-Analyse klassifiziert CSS-/dynamische Bilder global und schließt jede URL mit img-/srcset-Instanz auf einer beliebigen Seite aus. Atomare, wiederholbare Ersetzung; Änderungen stempeln den Änderungsfeed, Abbruch behält den vorherigen Stand.
- **ONPAGE-179:** fehlende oder leere width-/height-Attribute auf Instanzebene; CSS-Größen ersetzen keine HTML-Attribute.
- **ONPAGE-180:** Einsparung aus Größen-/Flächenverhältnis, Schwellwert inklusive Gleichheit, keine Einsparung für hochskalierte, unsichtbare, nicht im Viewport gerenderte Bilder oder SVG. Die Schätzung wird als ganzzahlige Bytezahl ausgegeben; die konfigurierte Schwelle 0 wird ebenfalls unterstützt.
- **ONPAGE-181, OUT-053:** acht Bildexporte, exakt zwölf Instanzspalten, CSV und XLSX, passende Instanzen statt sämtlicher Einbindungen einer problematischen Bild-URL. Strikte Alt-/Byte-Grenzen; numerische sowie X-Exportnamen; Spec-Schreibweise `Background Images` als Alias der bestehenden Kennung `Background Images Inlinks`. Ein SQL-Lauf streamt den Export ohne Detail-Paging-Grenze und ohne vollständige Ergebnisliste im Rust-Speicher. Fehlende Analyse-/Rendering-Voraussetzungen werden als konkreter Fehler gemeldet.
- Drei neue Image-Issues mit eigenen deutschen/englischen what/why/fix-Texten und Dokumentation.

## Tests

Die Paketprüfungen verwenden `images_m2.rs`, `tab_images_m2.rs`, den erweiterten M1-Test `tab_images.rs` und Inline-Tests in `catalog/bulk_images.rs`. Fixtures und selbst verfasster CSV-Erwartungswert liegen ausschließlich unter `tests/fixtures/onpage-m2-images/`.

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün (letzter Lauf 8,88 s).
- `cargo test --workspace --exclude openspider-desktop --locked`: **rot** nach 389,73 s, genau ein fehlgeschlagener Test im erreichten Lauf: `os-mcp::tools_read::issues_that_need_the_crawl_analysis_are_named_instead_of_counted`. Siehe Fremdbefund unten. Alle zuvor erreichten Tests, einschließlich der neuen D03-Analyse-/Exporttests und der CLI-Golden, sind grün.
- Separater Zusatzlauf: `cargo test -p os-model -p os-parity -p os-parse -p os-render -p os-sched -p os-store -p os-testsite -p os-url --locked`: grün, einschließlich Doc-Tests, 200,24 s. Kein weiterer Fehler.
- 14 neue aktive Fixture-Tests für D03: vier Parser-, fünf Analyse-/Detail-, vier Exporttests und ein srcset-Crawl-Test. Der vorhandene Rendering-Ignore bleibt bei M2-R02.
- `cargo deny check licenses bans sources`: grün; keine neuen Abhängigkeiten.
- `scripts/check-neutral.sh`: grün (vier bereits vorhandene Warnungen zu verwaisten CI-Ausnahmen).
- `python3 -I docs/plan/M2-check.py`: grün.

Rust-Prüfungen mit `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `CARGO_PROFILE_TEST_DEBUG=line-tables-only`. Ein früherer paralleler Neuaufbau unterbrach einen Testlauf durch eine fehlende Testbinärdatei; der oben angegebene abschließende Lauf erfolgte seriell. Die CLI-Golden wurde nach dem ersten inhaltlichen Befund ausschließlich um die zwei eigenen numerischen Namen ergänzt.

Keine Frontend-Dateien geändert; die bedingten npm-Prüfungen sind daher nicht erforderlich. Kein Push, Merge oder PR; dementsprechend keine neue CI-Lauf-ID. Plattformübergreifende CI-Abnahme verbleibt beim Integrator.

## Blockierender Fremdbefund – Abbruchregel

Datei: `crates/os-mcp/tests/tools_read.rs:1717`, Test `issues_that_need_the_crawl_analysis_are_named_instead_of_counted` (`AGENT-013`). Der bestehende M1-Test erwartet für **alle** Einträge in `not_evaluated` den Zustand `needs_crawl_analysis`. Mit den neu registrierten D03-Issues liefern die drei Image-Filter ohne Rendering korrekt `needs_rendering` (ONPAGE-007, ONPAGE-178/180 und eingefrorener F05-Vertrag für ONPAGE-179). Der produktive MCP-Code meldet die Zustände bereits korrekt.

Der Test liegt außerhalb von `owns`, ist kein F05-Stub-Vertrag und enthält keine eigene D03-Stub-Liste. Eine Korrektur seiner bestehenden Assertion wäre eine nicht-additive Fremdänderung und ist nach Auftrag nicht erlaubt. Er wurde unverändert gelassen. Das Paket ist fachlich umgesetzt, die geforderte vollständig grüne Workspace-Abgabe ist dadurch blockiert.

Benötigte, hier **nicht angewendete** eng begrenzte Testanpassung:

```diff
-    assert!(pending.iter().all(|p| p["state"] == "needs_crawl_analysis"));
+    assert!(pending.iter().all(|p| {
+        let own_image_filter = p["tab"] == "Images"
+            && matches!(p["filter"].as_str(),
+                Some("Background Images" | "Missing Size Attributes" | "Incorrectly Sized Images"));
+        p["state"] == if own_image_filter { "needs_rendering" } else { "needs_crawl_analysis" }
+    }));
```

Die Klärung/Anwendung gehört dem Integrator oder einem dafür autorisierten Nachbesserungspaket. Keine produktive Verhaltensänderung erforderlich.

## Abweichungen und offene Fragen

**ONPAGE-179 / eingefrorener F05-Vertrag:** Die Fach-Spec beschreibt Missing Size Attributes ohne Voraussetzung. `M2-filters.toml`, `tabs::M2_FILTERS` und Paketvorgabe 3.14 ordnen den Filter dagegen `post_crawl:Images` mit `RenderingOrConfig(images.parse_inline_css_backgrounds)` zu. Diese vorhandene Zustandssemantik bleibt erhalten; ohne Rendering/Inline-CSS liefert der Filter `NeedsRendering`. Die Attributprüfung selbst benötigt kein Rendering. Zu klären bleibt eine spätere koordinierte Freigabe als Live-Filter; dieses Paket verändert weder die eingefrorene Registry noch fremde Vertragstests dafür.

Die echte Browser-Erfassung (Viewport, externe Stylesheets, JS/Lazy Loading und Origin-Zusammenführung) liegt gemäß Paketaufteilung bei M2-R01/E05/R02. Hier werden vollständige persistierte Rendering-Fakten aus dem F05-Vertrag geprüft. Der vorhandene M1-Lazy-Load-Ignore verbleibt ausschließlich für M2-R02; der srcset-Teil ist aktiv getestet. D16 liefert später die benutzerdefinierten Positionsregeln; D03 ruft den vorhandenen Klassifikations-Erweiterungspunkt auf.

Numerische Namen stehen über den dynamischen Katalog bereit; die bestehenden X-Namen bleiben als stabile Einträge/CLI-Schreibweisen erhalten.

## Fremdergänzungen (K2-61)

Fehlender Erweiterungspunkt: Die im Pakettext angekündigte numerische Auflösung der Bildexportnamen war im F05-Katalog noch nicht angebunden. Ein zusätzlicher Aufruf registriert die zwei numerischen Bildexporte in der bereits vorhandenen dynamischen Liste. Neue Fachfunktion und ihre Tests liegen im eigenen `bulk_images.rs`; keine bestehende Signatur oder fremder Test wurde geändert.

Datei: `crates/os-export/src/catalog/mod.rs` – vollständiger fremder Diff:

```diff
 pub fn dynamic_entries(cfg: &CrawlConfig) -> Vec<DynamicEntry> {
     let mut entries = bulk_custom_search::dynamic_entries(cfg);
+    entries.extend(bulk_images::dynamic_entries(cfg));
     entries.extend(bulk_custom_extraction::dynamic_entries(cfg));
     entries
 }
```

Zusätzlich angepasst: `tests/golden/out-cli/list_exports.txt`. Ausschließlich additive Ausgabe der eigenen Registrierung:

```diff
 Issues:All Issues
+Images:Images With Alt Text Over 100 Characters Inlinks
+Images:Images Over 100 KB Inlinks
 Security:HTTP URLs Inlinks
```

Alle bisherigen Zeilen bleiben unverändert. Keine weiteren fremden Goldens oder Fixtures geändert.

## Stub-Ablösung (K2-60)

In `crates/os-engine/f05a-stubs.toml` wurden genau die fünf Einträge mit `owner = "M2-D03"` entfernt: `images::extract_with`, `images::response_facts`, `analysis::steps::images::run`, `tabs::images::detail_rows` und `catalog::bulk_images::entries`. Alle anderen Besitzer und Einträge bleiben unverändert. Weitere F05-Vertragstests mussten nicht angepasst werden.
