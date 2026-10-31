# Entscheidungen zu den Spec-Fragen der Q02-Triage (Orchestrator, 30.10.2026)

Grundlage: `triage.md` Abschnitt 8. Leitlinie: Wo die Referenz auf allen Sites konsistent eine Festlegung der Spec widerlegt und die Spec keinen fachlichen Grund für die Abweichung nennt, folgt die Spec der Referenz. Die Spec-Texte werden in `docs/spec` nachgezogen (K2-83 ff. in `docs/plan/M2.md`); bis dahin gilt diese Datei.

| Nr. | Entscheidung |
|---|---|
| SQ-01 | Angenommen: `verified` je Filter-ID, wenn (1) der Tab-All-Export auf allen Sites grün und spaltenvollständig ist und (2) der eigene Filterexport, falls vorhanden, grün ist. DEV erlaubt, REF-FREE nicht. Nenner ohne Change Detection (TECH-301…305): 155. Umsetzung in N06. |
| SQ-02 | Angenommen: `style`-Attribute werden ohne Rendering standardmäßig nicht nach `url()` durchsucht; neue Option, Standard aus. `<style>`-Blöcke unverändert. |
| SQ-03 | Angenommen: Directives nur 2xx; Pagination/Hreflang nur interne HTML-2xx (Ziele ohne 2xx nur in TECH-036); Validation = alle internen URLs; Security-Headerfilter = interne URLs mit 2xx (jeder Inhaltstyp). |
| SQ-04 | Angenommen: Annotationen außerhalb `<head>` bzw. mit ungültigem Attribut zählen nur für ihren eigenen Problemfilter. |
| SQ-05 | Angenommen: `unavailable_after` in der Vergangenheit → Non-Indexable „Unavailable After“; HTTP-Header-Selbst-Refresh → Redirected und Filter HTTP Refresh; Meta-Selbst-Refresh bleibt nach K-57. |
| SQ-06 | Angenommen: TECH-020 Unlinked listet das Ziel. |
| SQ-07 | Angenommen: A-49 aufgehoben; Media/Fonts/XML/Plugins als eigene Filter, „Flash“ heißt „Plugins“; Other ohne diese Klassen. |
| SQ-08 | Angenommen: Labels der Referenz, 3 Nachkommastellen für Text Ratio, Flesch, AWPS; A-41 in N08 kalibrieren. |
| SQ-09 | Angenommen mit Auflage: Wortgrenze an jeder `<a>`-Grenze, `header`/`aside` standardmäßig ausgeschlossen – nur wenn N08 nachweist, dass keine M1-Belege (K-68) brechen; sonst DEV. |
| SQ-10 | Angenommen: Link Score für M2 als DEV „TECH-145 OS-Festlegung“; Kalibrierung als M3-Paket. |
| SQ-11 | Angenommen: Sitemap `lastmod` als Datum ohne Uhrzeit, Crawl-Datum ohne Header, changefreq-Standard daily; Mapping normalisiert das Crawl-Datum. |
| SQ-12 | Angenommen: gespeicherte, nicht gecrawlte Ziele erscheinen nicht in den Tabellen. |
| SQ-13 | Angenommen wie vorgeschlagen: (a) Akzeptanzfall TECH-137 streichen, Schwelle bleibt; (b) Parameter `s` in ONPAGE-077 streichen; (c) kein Sniffing bei `application/octet-stream`; (d) `aside` → `Aside`; (e) OUT-083 als DEV; (f) Spalten HTTP rel/Size/CO2 im Mapping ignorieren (Grund TECH-011/031/281). |
| SQ-14 | Angenommen: CLI-Option für den Crawl-Path-Report in N11; bis dahin `skip` mit Grund. |
| SQ-15 | Angenommen: Referenzartefakt, OS-Wert = Filterdatei. |
| SQ-16 | Angenommen: `ref_settings` bereinigen (N06, nur der Text, nie `ref/`). |
