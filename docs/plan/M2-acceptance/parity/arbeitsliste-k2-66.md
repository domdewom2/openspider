# K2-66: Auswertung der Arbeitsliste für Q02

Quelle: `docs/plan/M2-reports/parity-deviations-q02.md` (Stand Runde 5).
Die Quelldatei liegt außerhalb von owns und bleibt unverändert. Diese Datei enthält die Bearbeitungsstände;
Einzelwerte und alle betroffenen Schlüssel stehen in `befunde.csv`, angewandte Ausnahmen in `abweichungen.csv`.

| Punkt der Arbeitsliste | Status Q02 | Entscheidung / Beleg / nächster Besitzer |
| --- | --- | --- |
| 1 Response Codes: Blocked Resource / Redirection (JavaScript), je intern/extern/gesamt | Abweichung + Nachbesserung | Ohne JavaScript kein Referenzbeleg (M2.md 6.1), `REF-FREE-16/17` nur auf vorhandene Zellen. Die fehlende Zeile `response codes \| external redirection (javascript)` bleibt Mapping-Befund: OpenSpider liefert einen getrennten Abschnitt `response codes – external`. Keine pauschale Ausnahme für fehlende Zeilen. |
| 1 JavaScript: 15 Zeilen | Abweichung | `REF-FREE-03`, Rendering im Referenzprofil aus; keine Verifikation dieser IDs. Im aktuellen Produkt sind die Zeilen vorhanden, aber Zähler leer statt Null. |
| 1 Mobile: sechs Zeilen | Abweichung | `REF-FREE-06`, keine aktivierte Referenzanalyse; `All` ist eine andere Population, kein Funktionsbeleg. |
| 1 Accessibility: All | Abweichung | `REF-FREE-07`, M2.md 6.1 / K2-14; Fixture-Belege in `ohne-referenz.csv`. |
| 1 Custom JavaScript: All | Abweichung | `REF-FREE-10`, gleiche Referenzbeschränkung. |
| 1 Link Metrics: All | Abweichung | `REF-FREE-13`, kein konfigurierter Referenzdienst; nicht mit der Spalte Link Score verwechseln. |
| 2 Internal Other | Abweichung + Nachbesserung | Fehlender Content-Type ist laut ONPAGE-024 Unknown (DEV-014). Die Referenz klassifiziert solche Antworten als Other. Neue Fixture `/fixture.woff2`, `/fixture.mp4`, `/fixture.xml` separat über Export/Population prüfen; keine pauschale Mengenentschuldigung für Internal. |
| 2 Security: vier fehlende Header | Nachbesserung | ONPAGE-039–042, `os-analyze/src/tabs/security.rs`: out-basic-site 16 → 18, auch im aktuellen Integrationsstand. HTTP-Antwortpopulation und Behandlung von Weiterleitungen prüfen; Gegenprobe Security-Site CSP 17 → 13. |
| 2 Security: Bad Content Type | Nachbesserung | ONPAGE-043: out-basic-site 0 → 1; Security-Site 2 → 3. Nicht als Free-Einschränkung akzeptiert. |
| 2 Broken Bookmark | Referenzbesonderheit | ONPAGE-079 / OUT-080: Ohne Fragment-Konfiguration schreibt die Spec einen leeren Zähler vor; Filterexport der neuen Onpage-Site hat beidseitig null Zeilen. Keine Bestätigung des positiven Falls. Die Konfigurationsvoraussetzung ist kein zu füllender Nullzaehler. |
| 2 Near Duplicates | Referenzbesonderheit + Nachbesserung | Die zwei Exporte sind wegen ausgeschalteter Referenzanalyse unavailable. Overview 0 → leer ist bei needs_config laut OUT-080 korrekt. Keine Verifikation, keine erfundene Lizenzursache für die lizenziert erzeugten neuen Dateien. |
| 2 Spelling / Grammar | Abweichung | ONPAGE-155/156, `REF-FREE-14/15`; M2.md 6.1 schließt diese Referenzprüfung aus. |
| 3 Link Score | Nachbesserung | TECH-145 / ONPAGE-017, `os-analyze/src/analysis/steps/links.rs`: systematische Wertabweichungen auch auf den neuen Sites. Nachbesserungsauftrag, keine Kalibrierung in Q02. |
| 3 Inlinks / Selbstlinks | Abweichung + Nachbesserung | Bestehende DEV-010/011 wirken bei den dort genannten Adressen weiter. Keine Erweiterung auf andere Populationen ohne adressgenauen Beleg. Nicht entschuldigte Zellen bleiben in der Befundliste. |
| 4.1 Rendering-Zeilen entscheiden | Erledigt | Referenzumfang aus 6.1 angewandt, fehlende Abschnittszuordnung bleibt sichtbar. |
| 4.2 Security / Other klären | Erledigt als Übergabe | Spec-Entscheidung und Referenzklassifizierung getrennt; Fachfehler an `os-analyze`, keine Produktänderung. |
| 4.3 Leere Zähler füllen | Entschieden + Nachbesserung | OUT-080 verlangt bei needs_config/needs_rendering leer statt 0; Broken Bookmark/Near Duplicates sind deshalb Referenzbesonderheiten. Missing Size Attributes hat laut ONPAGE-179 keine Konfigurationsvoraussetzung: leere Zähler bzw. leerer Filter bleiben Fachfehler; die requires-Angabe im Katalog ist eine überholte Planannahme. |
| 4.4 Link Score kalibrieren / Inlinks entscheiden | Nachbesserung | TECH-145-Messwerte und vorhandene Selbstlink-Ausnahmen ausgewiesen. |
| 4.5 Mapping scharf / Test auf scharf / alle zehn Sites | Erledigt im temporären Overlay + Nachbesserung außerhalb owns | Vollständiger echter Lauf mit beiden Linkspalten und M1+M2-Overview-Filtern. `scharf.py` reproduziert die temporären Mappings. Dauerhafte Änderung der beiden TOML-Dateien und von `real_repo_files.rs` an den Orchestrator; keine fremden Dateien geändert. |

Die neuen REF-FREE-Einträge 03–17 schränken ausschließlich den nicht referenzfähigen Overview-Umfang aus 6.1 ein.
Die historischen Einträge 01/02 gelten jetzt nur für die fünf alten Sites; die neuen Manifeste dokumentieren
lizenzierte Erzeugung, weshalb eine vermeintliche Free-Sperre dort keine Sitemap-Abweichung entschuldigt.
