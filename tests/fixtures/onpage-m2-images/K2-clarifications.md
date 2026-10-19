# K2-Klärungseintrag: ONPAGE-179 – Missing Size Attributes

| Feld | Eintrag |
|---|---|
| Status | Offen; vorgelegt mit dem M2-D03-Fix-Bericht vom 19.10.2026, Fable-Befund 5 |
| Adressat | Planer; koordinierte Umsetzung gegebenenfalls M2-N01 |
| Quelle | `docs/spec/02-tabs-onpage.md`, ONPAGE-179: keine Voraussetzung; fehlende/leere HTML-Attribute genügen |
| Abweichender Vertrag | `docs/plan/M2-filters.toml`, Filter `Images:Missing Size Attributes`: `kind = "post_crawl:Images"`, `requires = ["images.parse_inline_css_backgrounds"]`; Umsetzung als `RenderingOrConfig` im F05-Vertrag in `crates/os-analyze/src/tabs/images.rs` |
| Auswirkung | Ohne Rendering/Inline-CSS meldet der Filter `NeedsRendering`, obwohl die HTML-Instanzen bereits alle benötigten Attribute enthalten. |
| Entscheidungsbedarf | Soll Missing Size Attributes gemäß ONPAGE-179 als Live-Filter ohne Rendering-/Analyse-Voraussetzung freigegeben werden? Bitte K2-Nummer zentral vergeben und Entscheidung in die K2-Klärungsliste übernehmen. |
| Folgeschritte bei Freigabe | N01 gleicht Registry und Tabdefinition ab, aktualisiert die zugehörigen Registry-/Zustandstests und die Erwartung dieses Filters im MCP-Test `issues_that_need_the_crawl_analysis_are_named_instead_of_counted`. Export-Voraussetzungen mitprüfen. Background/Incorrectly Sized behalten ihre eigenen Voraussetzungen. |
| Zwischenstand D03 | Eingefrorener Vertrag bleibt erhalten; Attributprädikat einschließlich leerer Attribute ist implementiert und durch `missing_size_is_per_img_instance_including_empty_attributes` geprüft. |

Die Entscheidung über die zentrale Registry liegt beim Planer. Dieser Eintrag ist die Übergabe zur K2-Klärung; eine Live-Freigabe ist noch nicht beschlossen.
