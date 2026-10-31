# Q02b: angewandte Entscheide und K2-96

Grundlage sind SQ-01 … SQ-16, K2-93 … K2-95 und der explizite Orchestrator-Nachtrag K2-96 vom 30.10. Die dort freigegebenen Produkt- und Spec-Korrekturen sowie der Crawl-Path-Messfix sind umgesetzt. Ein Fix-Auftrag wird weiterhin nicht als DEV-Ausnahme oder bestandener Vergleich gezählt.

| Thema | Umsetzung / Beleg |
|---|---|
| SQ-09 | N08-Auflage durch neuen nativen M1-Nachlauf bestätigt: 38/38, null Word-Count-Zellfehler. Kein Wortzählungs-DEV. |
| TECH-133 / CRAWL-089 | DEV-046a/b: Technical 56/41, Liste 44/43. Nicht indexierbare Sackgassen einschließlich N09-Korrekturen; N10-Einzelbelege bleiben Grundlage. Keine Änderung der Indexierbarkeitsregel. |
| SQ-13a / TECH-137 | DEV-047a/b: exakt external-99.html, 99 Follow-Instanzen; Filtermenge und Overview 2/1. Schwelle bleibt 100. |
| ONPAGE-147 / K2-95 | DEV-048a/b: deutscher Treffer soft404-2.html und Zähler 2/3. Die mehrsprachige OS-Phrasenliste bleibt. |
| ONPAGE-149 / K2-95 | DEV-049: positions.html, Inhaltsbereich nur main, W=S=Syl=1; Very Easy statt Very Hard der Referenz, Zähler 7/6. Keine URL-Ausnahme in der Berechnung. |
| SQ-10 / SQ-13e / SQ-15 | Bestehende DEV-037, DEV-042 und REF-ART-01 bestätigt; nicht dupliziert. Link Score bleibt M3-Kalibrierung, Internal/Links bleiben nach SQ-10 ohne verified. |
| OUT-015 | DEV-040b repariert das wirkungslose bisherige Präfix `http:// |`: der Runner wendet Ausnahmen am Tupel ohne Anzeige-Ordinal an. Exakter Schlüssel `http://`; Reihenfolgeprüfung bleibt unverändert und wird grün. |
| TECH-141 | DEV-039-bulk: genau der eigene Start-Host samt Port, 2392 Verbindungen; 135 Quellpräfixe verlangen zusätzlich Ziel `{BASE}/`. Die 200 fremden localhost-Verbindungen bleiben verglichen. Kein Blanket für echte Loopback-Ziele und kein Links-All-DEV. |
| ONPAGE-024 | DEV-014-q02b-1 … 5: explizite Other-Zähler, abgesichert durch [other-populations.json](other-populations.json). Headerlose Weiterleitungen bzw. ungültiger Content-Type bleiben Unknown. Media/Fonts/XML/Plugins nicht ausgenommen. |
| OUT-081/084 | DEV-050a: vorgeschriebene Weiterleitung auf robots-gesperrtes Ziel; DEV-050b: Meta Refresh mit Verzögerung 0 ist nicht temporär. Exakte Adresse/Werte. DEV-005 und DEV-044-hop-M1 auf ausgelassene Endstatus-/Schleifenspalten ergänzt. |
| OUT-031 | DEV-051: Status Not Crawled ist vorgeschrieben. Die weiterhin abweichende Größe 0/leer ist **nicht** mitentschuldigt. |
| OUT-091 | REF-ART-02: [native Typ-Probe](hreflang-non200-type.json) zeigt Hyperlink statt Hreflang für `/ → pagination-404-target.html`; Referenz-Beifang, kein REF-FREE. Der echte Hreflang-Verweis bleibt vollständig verglichen. |
| TECH-283 / K2-93 | DEV-052: wrong.css wird als text/html ausgeliefert, Inhalt `/* fixture */`, ohne head. Missing Head ist in beiden Overviews 1; Head Not First nur in OS 1. Die Referenz behandelt den fehlenden Head auf m2-technical anders; keine auf allen Sites konsistente Widerlegung der ausdrücklich entschiedenen Missing-Head-Regel. |
| OUT-083 / K-57/K-69 | DEV-053: OUT-083 erweitert OUT-081. Die dort ausdrücklich verlangte Header-Selbst-Refresh-Schleifenzeile betrifft refresh-self.html; der historische Meta-Selbst-Refresh ist in der Referenz vorhanden. Keine Ausnahme für andere Startzeilen. |
| TECH-139 / DEV-008 | DEV-054: [eigene Filter-/Inlink-Belege](links-detail.json) isolieren sechs gemischte Ziele. Drei davon entstehen ausschließlich aus fünf nativ bereits durch DEV-008 belegten Meta-nofollow-Verbindungen; die anderen drei aus rel=nofollow/ugc/sponsored. Der Zähler 3/6 ist die direkte Folge der schon begründeten Follow-Semantik, kein neuer Alt-Referenz-DEV ohne Ursache. |
| ONPAGE-179 / DEV-028 | DEV-055: [eigener Filter-/Instanzexport](images-detail.json) zeigt zehn Bilder ohne Größenattribute. Der einzige Bildpopulationsunterschied ist schon nativ durch DEV-028 als img/missing.png (404) belegt. Ohne diese URL bleiben neun Instanzen; drei weitere Bild-URLs besitzen nur Hyperlinks, keine img-Instanz. Die Attributprüfung ist nach ONPAGE-179 vom Abrufstatus unabhängig. |
| TECH-136 / TECH-144 | DEV-056: der eigene Filter isoliert links/self-only.html, Anchor `This page`. Dieser exakte Treffer ist durch die ausdrücklich festgelegte Defaultliste vorgeschrieben; TECH-136 nimmt Selbst-Hyperlinks nicht aus, K-07 beschreibt die Linkzähler. Die Referenzursache (Liste oder Selbstlink) ist nicht belegt, daher kein REF-ART behauptet. Ausschließlich dieser Zähler 0/1 wird als OS-Festlegung begründet. |

## Messkorrekturen

- `all_inlinks`: zwei fehlende Typ-Aliasse HTML Rel Next/Prev → Rel Next/Prev. Beziehungsmenge bleibt voll verglichen.
- `images_missing_sizes`: Referenz-Length ist Alt-Text-Länge 7/15/9, nicht Dateigröße 74 Bytes. Der OS-Instanzvertrag ONPAGE-172/181/OUT-053 enthält keine Alt-Text-Längenspalte; Ignoriergrund explizit, Alt Text bleibt verglichen. XPath wie im bestehenden all_inlinks-Mapping als Elementfolge; absolute Positionsindizes sind eine ausdrückliche OS-Festlegung.
- `content_soft404`/`serp_summary`: zuvor fälschlich bitgenaue Pixelprüfung an die bereits festgelegte Metrik UI-038/K-64 (≤3 %) angeglichen. Identische Regel wie Internal/Page Titles; keine Änderung der Export-Schwellen 99/97/100 und keine neue Toleranzdefinition. Die 36 SERP-Pixelabweichungen liegen zwischen 0,302 % und 2,667 %, die zwei Soft-404-Titel bei 0,485 %. SERP bleibt wegen real404.html rot.
- `hreflang_inconsistent_returns`: Bezugspaar aus URL with Inconsistent Language Return Link und URL Returning with Inconsistent Language; URL Target wiederholt nur die Selbstannotation. Expected/Actual Language und beide Beziehungen werden voll verglichen. Die Probe mit der zunächst vermuteten Gegenrichtung bleibt rot erhalten; die richtige Zuordnung ergibt 100 %.
- `hreflang_noncanonical_returns`: die Referenz benennt das A/B-Paar in der Gegenrichtung; Zuordnung korrigiert, Canonical-Wert bleibt geprüft. Native Probe 100 %, identische Beziehungsmenge.

## K2-96: freigegebene Umsetzung

- OUT-098: nur interne HTML-Seiten mit 2xx im SERP Summary; 404 und Weiterleitung werden im Fixture-Test ausgeschlossen.
- OUT-087: indexierbare Unlinked-Ziele nur im eigenen Unlinked-Filter/Report. Nicht indexierbare unlinked-Ziele bleiben hier enthalten. K2-63: die frühere Test-Erwartung für `annotation` wurde entsprechend dem ausdrücklichen Spec-Entscheid von enthalten auf ausgeschlossen geändert; Auswahl, Export und übrige Assertions bleiben geprüft.
- OUT-093: Sprachcode des fehlenden Rücklinks leer. Der Code der vorhandenen Hinannotation bleibt im All-Hreflang-Report erhalten; keine Mapping-Regel „leer = beliebig“.
- UI-038/ONPAGE-096: separate Zeichenbreiten für `/` und `.`, 20 px Regular. 1.021 native Titelmessungen, 297 verbessert, keiner verschlechtert; keine Ziffern-, URL- oder Schwellenänderung. Andere Schriftprofile unverändert. Beleg `pixel-k296.json`. K2-63: zwei eigene Golden-Zellen (`onpage-titles/page_titles__all.csv`, `page_titles__below_30_characters.csv`) für den wörtlichen Titel `A <b>B</b>` von 106 auf 108 px angepasst (eine Slash-Glyphe +2 px); keine sonstige Zelle und keine Referenzdatei geändert.
- ONPAGE-011/TECH-022: Internal zeigt Canonicals fragmentfrei; Rohannotation bleibt als Filterbeleg gespeichert.
- ONPAGE-013: in den Body reparierte title-Elemente begrenzen den Text-Ratio-Text. Inhaltswortzahl und normale Inline-Grenzen bleiben unverändert. Die beiden realen fehlerhaften HTML-Fixtures sind Regressionstests.
- OUT-031/ONPAGE-012: bislang unbestimmter Größenwert eines bekannten ungecrawlten internen Ziels im Linkexport auf 0 präzisiert (Referenz konsistent, kein fachlicher Grund für Abweichung). Unbekannte externe Größen bleiben leer; keine Änderung an Status-/Follow-Regeln. K2-63: bestehender Fixture-Test erhält nur eine zusätzliche Assertion für die neu präzisierte Größe, keine entfernte Prüfung. K2-63: zusätzlich zwölf Größenwerte in den eigenen Goldens `out-links/links_all_inlinks.csv` und `links_all_outlinks.csv` für bekannte interne Not-Crawled-Ziele leer → 0; alle übrigen Zellen unverändert, Einzelbeleg `golden-k263.json`.
- OUT-023: Auswahl per `--export-crawl-path` zur Laufzeit an die gestartete Site gebunden. Referenz-Trenner werden entfernt; alle echten Kanten bleiben geprüft. Knotenblöcke mit lückenlosen Steps, Ziel-/Startvalidierung und Zyklusprüfung werden auf Kanten projiziert. Quelle, Ziel, Typ, Anchor, Quelltiefe und Zielstatus werden verglichen; kein Status-/Wert-Blanket. Negativtests für fehlende Knoten/Kanten, Reihenfolge, Ziel, gemischte Blöcke, Anchor, Tiefe und Status; Positivtests mit Redirect und zwei Zielblöcken.

INTEG-009/pending_w1_i01 bleibt ausdrücklich N13 zugeordnet; keine Änderung in dessen Besitz.
