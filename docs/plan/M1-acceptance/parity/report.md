# Paritätslauf: nicht bestanden

Sites mit Status present: 5
(gefordert: 4)

| Site | Export | Mapping | Zellen | Übereinstimmung | Schwelle | Ergebnis |
|---|---|---|---:|---:|---:|---|
| out-basic-site | internal_all.csv | internal_all | 899 | 98.443 % | 99.0 % | FEHLER |
| out-basic-site | external_all.csv | external_all | 5 | 100.000 % | 97.0 % | ok |
| out-basic-site | response_codes_all.csv | response_codes_all | 152 | 100.000 % | 99.0 % | ok |
| out-basic-site | links_all.csv | links_all | 100 | 100.000 % | 97.0 % | ok |
| out-basic-site | all_inlinks.csv | all_inlinks | 1067 | 98.313 % | 97.0 % | ok |
| out-basic-site | redirect_chains.csv | redirect_chains | 0 | 100.000 % | 97.0 % | ok |
| out-basic-site | crawl_overview.csv | crawl_overview | 112 | 100.000 % | 100.0 % | ok |
| parity-onpage | internal_all.csv | internal_all | 4761 | 99.286 % | 99.0 % | ok |
| parity-onpage | external_all.csv | external_all | 44 | 97.727 % | 97.0 % | ok |
| parity-onpage | response_codes_all.csv | response_codes_all | 911 | 100.000 % | 99.0 % | ok |
| parity-onpage | url_all.csv | url_all | 945 | 97.778 % | 97.0 % | ok |
| parity-onpage | page_titles_all.csv | page_titles_all | 756 | 77.778 % | 99.0 % | FEHLER |
| parity-onpage | meta_description_all.csv | meta_description_all | 756 | 77.778 % | 99.0 % | FEHLER |
| parity-onpage | h1_all.csv | h1_all | 588 | 100.000 % | 99.0 % | ok |
| parity-onpage | h2_all.csv | h2_all | 588 | 100.000 % | 99.0 % | ok |
| parity-onpage | content_all.csv | content_all | 336 | 97.321 % | 97.0 % | ok |
| parity-onpage | images_all.csv | images_all | 57 | 100.000 % | 97.0 % | ok |
| parity-onpage | all_inlinks.csv | all_inlinks | 3322 | 99.880 % | 97.0 % | ok |
| parity-onpage | crawl_overview.csv | crawl_overview | 103 | 100.000 % | 100.0 % | ok |
| parity-response | internal_all.csv | internal_all | 2159 | 95.692 % | 99.0 % | FEHLER |
| parity-response | external_all.csv | external_all | 24 | 95.833 % | 97.0 % | FEHLER |
| parity-response | response_codes_all.csv | response_codes_all | 407 | 99.509 % | 99.0 % | ok |
| parity-response | all_inlinks.csv | all_inlinks | 1052 | 99.240 % | 97.0 % | FEHLER |
| parity-response | all_error_(4xx_5xx_no_response)_inlinks.csv | all_error_inlinks | 212 | 97.642 % | 97.0 % | ok |
| parity-response | redirect_chains.csv | redirect_chains | 236 | 100.000 % | 97.0 % | FEHLER |
| parity-response | redirects_to_error.csv | redirects_to_errors | - | - | - | übersprungen (außerhalb des M1-Umfangs: Report Redirects to Errors (OUT-084) ist P1 und kein M1-Umfang (M1.md 7.2); OpenSpider hat in M1 nur Redirects:All Redirects und Redirects:Redirect Chains) |
| parity-response | crawl_overview.csv | crawl_overview | 100 | 95.000 % | 100.0 % | FEHLER |
| parity-synthetic | internal_all.csv | internal_all | 20722 | 97.548 % | 99.0 % | FEHLER |
| parity-synthetic | links_all.csv | links_all | 3410 | 100.000 % | 97.0 % | ok |
| parity-synthetic | all_inlinks.csv | all_inlinks | 50105 | 98.487 % | 97.0 % | ok |
| parity-synthetic | crawl_overview.csv | crawl_overview | 113 | 100.000 % | 100.0 % | ok |
| parity-technical | internal_all.csv | internal_all | 6070 | 99.572 % | 99.0 % | FEHLER |
| parity-technical | canonicals_all.csv | canonicals_all | 1272 | 100.000 % | 99.0 % | ok |
| parity-technical | directives_all.csv | directives_all | 855 | 100.000 % | 99.0 % | ok |
| parity-technical | hreflang_all.csv | hreflang_all | 1696 | 100.000 % | 97.0 % | ok |
| parity-technical | links_all.csv | links_all | 950 | 100.000 % | 97.0 % | ok |
| parity-technical | sitemaps_all.csv | sitemaps_all | 0 | 100.000 % | 97.0 % | ok |
| parity-technical | all_inlinks.csv | all_inlinks | 4837 | 98.925 % | 97.0 % | FEHLER |
| parity-technical | crawl_overview.csv | crawl_overview | 112 | 93.750 % | 100.0 % | FEHLER |

## Fehler

- out-basic-site/internal_all.csv: Zellübereinstimmung 98.443 % liegt unter 99.000 %
- parity-onpage/page_titles_all.csv: Zellübereinstimmung 77.778 % liegt unter 99.000 %
- parity-onpage/meta_description_all.csv: Zellübereinstimmung 77.778 % liegt unter 99.000 %
- parity-response/internal_all.csv: Zellübereinstimmung 95.692 % liegt unter 99.000 %
- parity-response/external_all.csv: Zellübereinstimmung 95.833 % liegt unter 97.000 %
- parity-response/all_inlinks.csv: URL-Menge nicht identisch: 1 fehlen in OpenSpider, 0 zusätzlich
- parity-response/redirect_chains.csv: URL-Menge nicht identisch: 1 fehlen in OpenSpider, 0 zusätzlich
- parity-response/crawl_overview.csv: Zellübereinstimmung 95.000 % liegt unter 100.000 %
- parity-synthetic/internal_all.csv: Zellübereinstimmung 97.548 % liegt unter 99.000 %
- parity-technical/internal_all.csv: URL-Menge nicht identisch: 0 fehlen in OpenSpider, 1 zusätzlich
- parity-technical/all_inlinks.csv: URL-Menge nicht identisch: 1 fehlen in OpenSpider, 1 zusätzlich
- parity-technical/crawl_overview.csv: Zellübereinstimmung 93.750 % liegt unter 100.000 %

## Begründete Paritätsabweichungen

| Kennung | Spec | Kennzeichen | Zellen/Zeilen | Sites | Begründung |
|---|---|---|---:|---|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 10 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Referenz listet sie nicht. Offene Planerentscheidung: Festlegung beibehalten oder in_tables=0 fuer interne robots.txt (siehe Abnahmebericht, Befund B-01). |
| DEV-002 | CRAWL-098 | OS-Festlegung | 48 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Folge von DEV-001: Die robots.txt-Zeile zaehlt in allen Zaehlern, deren Grundgesamtheit alle internen URLs mit Antwort ist, um genau 1 hoeher als in der Referenz. |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 2230 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); die Referenz schreibt 0. |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 61 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |
| DEV-005 | ONPAGE-065 | Paritaetsabweichung | 9 | parity-onpage, parity-response | OS-Festlegung ONPAGE-065 unterscheidet Connection Refused (abgelehnte Verbindung) von Connection Timeout; die Referenz meldet fuer den geschlossenen Port localhost:1 Connection Timeout. |
| DEV-006 | OUT-031 | Paritaetsabweichung | 7 | parity-onpage, parity-response, parity-technical | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz schreibt 0 fuer robots.txt-blockierte Ziele. |
| DEV-006b | OUT-031 | Paritaetsabweichung | 2 | parity-response | wie DEV-006 (Fehler-Inlinks auf nicht abgerufene Ziele). |
| DEV-007 | OUT-031 | Paritaetsabweichung | 12 | out-basic-site | Rel = alle rel-Werte des Elements in Originalreihenfolge (OUT-031); die Referenz laesst rel="stylesheet" leer. |
| DEV-008 | OUT-031 | OS-Festlegung | 5 | parity-technical | Follow=False auch bei Meta/X-Robots nofollow der Quellseite (OUT-031); die Referenz bewertet nur das Link-Attribut. |
| DEV-009 | OUT-031 | Paritaetsabweichung | 34 | out-basic-site, parity-response, parity-synthetic, parity-technical | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Headern und Weiterleitungen. |
| DEV-010 | TECH-130 | OS-Festlegung | 4 | parity-technical | K-07: Selbstlinks zaehlen im Links-Tab nicht (TECH-130); die Referenz zaehlt sie. Betrifft die Seite mit Selbstlink. |
| DEV-011 | TECH-130 | OS-Festlegung | 8 | out-basic-site | K-07 (wie DEV-010), Seiten mit Selbstlink in out-basic-site. |
| DEV-012 | TECH-051 | OS-Festlegung | 3 | parity-technical | Population des Directives-Tabs: alle internen URLs mit HTTP-Antwort, jeder Status (TECH-051); die Referenz fuehrt Weiterleitungen und Fehlerseiten nicht. |
| DEV-013 | TECH-051 | OS-Festlegung | 5 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 6 | parity-onpage, parity-response, parity-technical | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fuehrt Antworten ohne Content-Type (z. B. Weiterleitungen) unter Other. Der Filter Other ist P1 (M1.md 7.2). |
| DEV-015 | OUT-080 | Paritaetsabweichung | 13 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Die Crawl-Depth-Zeilen der Referenz zaehlen nur interne HTML-Seiten mit 2xx (Zeile 0 = Start-URL); OpenSpider zaehlt alle internen URLs mit bekannter Tiefe (OUT-025 Site Structure, Summe = interne URLs). |
| DEV-016 | ONPAGE-166 | OS-Festlegung | 1 | parity-onpage | PDF-Inhalte (Word Count, Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2); in M1 nur Titel und Keywords aus den PDF-Eigenschaften. |
| DEV-017 | OUT-081 | OS-Festlegung | 8 | parity-response | OUT-081: die Hop-Spalten n beschreiben die Spruenge (Redirect Type n, Redirect URL n, Status Code n, Status n); die Referenz haengt als letzten Hop das Endziel an. |
| DEV-018 | ONPAGE-171 | OS-Festlegung | 3 | parity-onpage | IMG Inlinks (OpenSpider-Erweiterung) zaehlt auch Hyperlinks auf Bilder (ONPAGE-170 Nr. 2); die Referenz zaehlt nur eingebettete Bilder. |
| DEV-019 | ONPAGE-005 | Annahme | 2 | parity-onpage | Annahme A-4 (Duplikate exakt, Gross-/Kleinschreibung sensitiv) ist durch die Referenz widerlegt: Titel/Meta Description die sich nur in der Schreibweise unterscheiden zaehlen dort als Duplikat (Fixture dup-case.html). Spec-Korrektur offen (Abnahmebericht). |
| DEV-020 | ONPAGE-072 | Annahme | 1 | parity-onpage | Annahme A-23 (Underscore nur im Pfad) ist durch die Referenz widerlegt: sort_by in der Query zaehlt dort mit. Spec-Korrektur offen (Abnahmebericht). |
| DEV-021 | TECH-011 | OS-Festlegung | 6 | parity-onpage, parity-response, parity-technical | Die Tabs Canonicals, Links und Hreflang fuehren interne HTML-Seiten mit Status 200 (TECH-011/-071/-130); die Referenz zaehlt alle 2xx (Fixture /s201, Status 201). |
| DEV-022 | CRAWL-089 | OS-Festlegung | 5 | parity-onpage, parity-response, parity-technical | Der Wirkungsbereich von issues.ignore_non_indexable ist in CRAWL-089 abschliessend auf die Element-Tabs und Low Content Pages begrenzt; die Referenz schliesst nicht indexierbare URLs auch in Canonicals/Missing und Links/Pages Without Internal Outlinks aus. |
| DEV-023 | TECH-064 | OS-Festlegung | 2 | parity-response, parity-technical | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im Response-Codes-Filter Redirection (HTTP Refresh). |
| DEV-024 | ONPAGE-166 | OS-Festlegung | 1 | parity-onpage | PDF-Inhalte (Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2), siehe DEV-016. |
| DEV-025 | TECH-001 | OS-Festlegung | 2 | parity-synthetic | Indexability Status nennt genau den ersten zutreffenden Grund in der Pruefreihenfolge (TECH-001); die Referenz nennt bei noindex und fremdem Canonical beide, kommagetrennt. |
| DEV-026 | TECH-064 | OS-Festlegung | 1 | parity-technical | Die Spalte Meta Refresh im Directives-Tab zeigt auch den Wert des Refresh-Headers (TECH-064: Refresh umfasst Meta und Header); die Referenz laesst sie bei reinem Header-Refresh leer. |
| DEV-027 | TECH-071 | OS-Festlegung | 1 | parity-technical | Population des Hreflang-Tabs: auch gecrawlte hreflang-Ziele (TECH-071); die Referenz fuehrt das umgeleitete Ziel (301) nicht. |
| DEV-028 | ONPAGE-170 | OS-Festlegung | 1 | parity-onpage | Grundmenge des Images-Tabs: auch per Bild-Referenz entdeckte Bild-URLs mit Fehlerantwort (Images-Intro, ONPAGE-170); die Referenz fuehrt das nicht abrufbare Bild (404) nicht. |
| DEV-029 | ONPAGE-170 | OS-Festlegung | 1 | parity-onpage | wie DEV-028: Zaehler des Abschnitts Images enthaelt das per Referenz entdeckte, nicht abrufbare Bild. |
| DEV-030 | OUT-081 | OS-Festlegung | 5 | parity-response | Bei einer Weiterleitungsschleife ist Final Address leer und Final Status = Redirect Loop (OUT-081); die Referenz nennt die letzte Station der Schleife samt ihrem Status. |
| DEV-031 | OUT-081 | OS-Festlegung | 1 | parity-response | Meta Refresh und Refresh-Header gelten nur mit Verzoegerung > 0 als temporaer (OUT-081); die Referenz wertet den Meta Refresh ohne Verzoegerung als temporaer. |
| DEV-032 | ONPAGE-015 | OS-Festlegung | 39 | parity-response | % of Total teilt durch alle gecrawlten internen HTML-Seiten mit Status 200 (ONPAGE-015); die Referenz teilt durch alle HTML-Seiten mit 2xx (Fixture /s201, Status 201: 14 statt 13 Seiten). |

## Referenzeinschränkungen SF Free

| Kennung | Spec | Kennzeichen | Zellen/Zeilen | Sites | Begründung |
|---|---|---|---:|---|---|
| SF-FREE-01 | TECH-210 | Annahme | 110 | parity-technical | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): verlinkte XML-Sitemaps und robots.txt-Sitemaps werden nicht abgerufen; die Referenz listet im Tab Sitemaps dennoch alle internen URLs, OpenSpider ohne verarbeitete Sitemap keine (TECH-211: Tab leer). |
| SF-FREE-02 | TECH-210 | Annahme | 5 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt Sitemaps, OpenSpider 0 (TECH-211). |

# Paritätsbericht `internal_all` (Site `out-basic-site`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 98.443 % liegt unter 99.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 18 |
| Zeilen OpenSpider | 19 |
| zugeordnet | 18 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 899 |
| Zellen gleich | 885 |
| Übereinstimmung | 98.443 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Content Type | Content | 18 | 0 |
| Status Code | Status Code | 18 | 0 |
| Status | Status | 18 | 0 |
| Indexability | Indexability | 18 | 0 |
| Indexability Status | Indexability Status | 18 | 0 |
| Title 1 | Title 1 | 18 | 1 |
| Title 1 Length | Title 1 Length | 13 | 1 |
| Title 1 Pixel Width | Title 1 Pixel Width | 13 | 2 |
| Meta Description 1 | Meta Description 1 | 18 | 0 |
| Meta Description 1 Length | Meta Description 1 Length | 12 | 0 |
| Meta Description 1 Pixel Width | Meta Description 1 Pixel Width | 12 | 0 |
| Meta Keywords 1 | Meta Keywords 1 | 18 | 0 |
| Meta Keywords 1 Length | Meta Keywords 1 Length | 0 | 0 |
| H1-1 | H1-1 | 18 | 1 |
| H1-1 Length | H1-1 Length | 13 | 1 |
| H2-1 | H2-1 | 18 | 0 |
| H2-1 Length | H2-1 Length | 0 | 0 |
| Meta Robots 1 | Meta Robots 1 | 18 | 0 |
| X-Robots-Tag 1 | X-Robots-Tag 1 | 18 | 0 |
| Meta Refresh 1 | Meta Refresh 1 | 18 | 0 |
| Canonical Link Element 1 | Canonical Link Element 1 | 18 | 0 |
| rel="next" 1 | rel="next" 1 | 18 | 0 |
| rel="prev" 1 | rel="prev" 1 | 18 | 0 |
| HTTP rel="next" 1 | HTTP rel="next" 1 | 18 | 0 |
| HTTP rel="prev" 1 | HTTP rel="prev" 1 | 18 | 0 |
| Size (Bytes) | Size (bytes) | 18 | 0 |
| Transferred (Bytes) | Transferred (bytes) | 18 | 0 |
| Total Transferred (Bytes) | Total Transferred (bytes) | 18 | 0 |
| Word Count | Word Count | 13 | 1 |
| Text Ratio | Text Ratio | 13 | 1 |
| Crawl Depth | Crawl Depth | 18 | 0 |
| Folder Depth | Folder Depth | 18 | 0 |
| Inlinks | Inlinks | 18 | 0 |
| Unique Inlinks | Unique Inlinks | 18 | 0 |
| % of Total | % of Total | 18 | 0 |
| Outlinks | Outlinks | 18 | 0 |
| Unique Outlinks | Unique Outlinks | 18 | 0 |
| External Outlinks | External Outlinks | 18 | 0 |
| Unique External Outlinks | Unique External Outlinks | 18 | 0 |
| Hash | Hash | 18 | 6 |
| Last Modified | Last-Modified | 18 | 0 |
| Redirect URL | Redirect URL | 18 | 0 |
| Redirect Type | Redirect Type | 18 | 0 |
| URL Encoded Address | URL Encoded Address | 18 | 0 |
| H1-2 | H1-2 | 18 | 0 |
| H1-2 Length | H1-2 Length | 18 | 0 |
| H2-2 | H2-2 | 18 | 0 |
| H2-2 Length | H2-2 Length | 18 | 0 |
| Meta Description 2 | Meta Description 2 | 18 | 0 |
| Meta Description 2 Length | Meta Description 2 Length | 18 | 0 |
| Meta Description 2 Pixel Width | Meta Description 2 Pixel Width | 18 | 0 |
| Title 2 | Title 2 | 18 | 0 |
| Title 2 Length | Title 2 Length | 18 | 0 |
| Title 2 Pixel Width | Title 2 Pixel Width | 18 | 0 |

### Beispiele `Title 1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html |  | Not Found |

### Beispiele `Title 1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html | 0 | 9 |

### Beispiele `Title 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html | 0 | 93 |
| {BASE}/faq.html | 142 | 148 |

### Beispiele `H1-1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html |  | 404 |

### Beispiele `H1-1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html | 0 | 3 |

### Beispiele `Word Count`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html | 0 | 1 |

### Beispiele `Text Ratio`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/missing.html | 0.000 | 3.3 |

### Beispiele `Hash`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/assets/logo.png |  | c3fb39f9e97bac1b390c81585a3f7a01 |
| {BASE}/assets/site.css |  | b7bcf29995b0a078ad76116ffe73a4f4 |
| {BASE}/missing.html |  | 5d310c8f1ba4589b33ca2e3331b4cf9b |
| {BASE}/assets/site.js |  | 4ad8fac01d7f0aaccc36c4ae8ae9a78d |
| {BASE}/old-page |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/assets/widget.png |  | 5c2715fb082021ed5adbb7caa10348dc |

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 1 | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Refer... |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 68 | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); ... |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 5 | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |

## Ignorierte Spalten (25)

- `amphtml Link Element (AMP erst mit M2 (M1.md 7.2))`
- `CO2 (mg) (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Carbon Rating (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Sentence Count (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Average Words Per Sentence (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Flesch Reading Ease Score (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Readability (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Link Score (nicht in M1 (P1/P2, M1.md 7.2) (TECH-145))`
- `Unique JS Inlinks (nur mit JavaScript-Rendering (M2))`
- `Unique JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Unique External JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Closest Near Duplicate Match (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `No. Near Duplicates (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `Spelling Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Grammar Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Response Time (nicht deterministisch)`
- `Cookies (Cookie-Zusammenfassung erst mit M2 (OUT-112, M1.md 7.3))`
- `Language (nicht in M1 (P1/P2, M1.md 7.2): Sprache (ONPAGE-153))`
- `HTTP Version (Transportdetail, nicht Teil der Parität (HTTP-Versionen erst M2, CRAWL-025))`
- `Mobile Alternate Link (Mobile erst mit M2 (M1.md 7.2))`
- `Closest Semantically Similar Address (KI-Funktion, M4)`
- `Semantic Similarity Score (KI-Funktion, M4)`
- `No. Semantically Similar (KI-Funktion, M4)`
- `Semantic Relevance Score (KI-Funktion, M4)`
- `Crawl Timestamp (nicht deterministisch)`

## OpenSpider-Spalten ohne Mapping (nicht verglichen) (2)

- `Meta Keywords 2`
- `Meta Keywords 2 Length`

## Gemappte Spalten fehlen in der Referenz (7)

- `Canonical Link Element 2`
- `Canonical Link Element 3`
- `Meta Refresh 2`
- `Meta Robots 2`
- `Meta Robots 3`
- `X-Robots-Tag 2`
- `X-Robots-Tag 3`

# Paritätsbericht `page_titles_all` (Site `parity-onpage`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 77.778 % liegt unter 99.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 84 |
| Zeilen OpenSpider | 84 |
| zugeordnet | 84 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 756 |
| Zellen gleich | 588 |
| Übereinstimmung | 77.778 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Occurrences | Occurrences | 84 | 0 |
| Title 1 | Title 1 | 84 | 0 |
| Title 1 Length | Title 1 Length | 84 | 0 |
| Title 1 Pixel Width | Title 1 Pixel Width | 84 | 84 |
| Title 2 | Title 2 | 84 | 0 |
| Title 2 Length | Title 2 Length | 84 | 0 |
| Title 2 Pixel Width | Title 2 Pixel Width | 84 | 84 |
| Indexability | Indexability | 84 | 0 |
| Indexability Status | Indexability Status | 84 | 0 |

### Beispiele `Title 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 332 |  |
| {BASE}/urls/ | 346 |  |
| {BASE}/titles/ | 343 |  |
| {BASE}/meta/ | 347 |  |
| {BASE}/h2/ | 335 |  |
| {BASE}/h1/ | 386 |  |
| {BASE}/images/ | 354 |  |
| {BASE}/ctype/ | 316 |  |
| {BASE}/content/ | 340 |  |
| {BASE}/h2/dup-b.html | 338 |  |

### Beispiele `Title 2 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 0 |  |
| {BASE}/urls/ | 0 |  |
| {BASE}/titles/ | 0 |  |
| {BASE}/meta/ | 0 |  |
| {BASE}/h2/ | 0 |  |
| {BASE}/h1/ | 0 |  |
| {BASE}/images/ | 0 |  |
| {BASE}/ctype/ | 0 |  |
| {BASE}/content/ | 0 |  |
| {BASE}/h2/dup-b.html | 0 |  |

## Gemappte Spalten fehlen in OpenSpider (2)

- `Title 1 Pixel Width`
- `Title 2 Pixel Width`

# Paritätsbericht `meta_description_all` (Site `parity-onpage`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 77.778 % liegt unter 99.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 84 |
| Zeilen OpenSpider | 84 |
| zugeordnet | 84 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 756 |
| Zellen gleich | 588 |
| Übereinstimmung | 77.778 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Occurrences | Occurrences | 84 | 0 |
| Meta Description 1 | Meta Description 1 | 84 | 0 |
| Meta Description 1 Length | Meta Description 1 Length | 84 | 0 |
| Meta Description 1 Pixel Width | Meta Description 1 Pixel Width | 84 | 84 |
| Meta Description 2 | Meta Description 2 | 84 | 0 |
| Meta Description 2 Length | Meta Description 2 Length | 84 | 0 |
| Meta Description 2 Pixel Width | Meta Description 2 Pixel Width | 84 | 84 |
| Indexability | Indexability | 84 | 0 |
| Indexability Status | Indexability Status | 84 | 0 |

### Beispiele `Meta Description 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 560 |  |
| {BASE}/urls/ | 595 |  |
| {BASE}/titles/ | 567 |  |
| {BASE}/meta/ | 604 |  |
| {BASE}/h2/ | 588 |  |
| {BASE}/h1/ | 610 |  |
| {BASE}/images/ | 625 |  |
| {BASE}/ctype/ | 594 |  |
| {BASE}/content/ | 551 |  |
| {BASE}/h2/dup-b.html | 570 |  |

### Beispiele `Meta Description 2 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 0 |  |
| {BASE}/urls/ | 0 |  |
| {BASE}/titles/ | 0 |  |
| {BASE}/meta/ | 0 |  |
| {BASE}/h2/ | 0 |  |
| {BASE}/h1/ | 0 |  |
| {BASE}/images/ | 0 |  |
| {BASE}/ctype/ | 0 |  |
| {BASE}/content/ | 0 |  |
| {BASE}/h2/dup-b.html | 0 |  |

## Gemappte Spalten fehlen in OpenSpider (2)

- `Meta Description 1 Pixel Width`
- `Meta Description 2 Pixel Width`

# Paritätsbericht `internal_all` (Site `parity-response`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 95.692 % liegt unter 99.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 46 |
| Zeilen OpenSpider | 47 |
| zugeordnet | 46 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 2159 |
| Zellen gleich | 2066 |
| Übereinstimmung | 95.692 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Content Type | Content | 46 | 0 |
| Status Code | Status Code | 46 | 0 |
| Status | Status | 46 | 0 |
| Indexability | Indexability | 46 | 0 |
| Indexability Status | Indexability Status | 46 | 0 |
| Title 1 | Title 1 | 46 | 8 |
| Title 1 Length | Title 1 Length | 22 | 8 |
| Title 1 Pixel Width | Title 1 Pixel Width | 22 | 9 |
| Meta Description 1 | Meta Description 1 | 46 | 0 |
| Meta Description 1 Length | Meta Description 1 Length | 6 | 0 |
| Meta Description 1 Pixel Width | Meta Description 1 Pixel Width | 6 | 0 |
| Meta Keywords 1 | Meta Keywords 1 | 46 | 0 |
| Meta Keywords 1 Length | Meta Keywords 1 Length | 0 | 0 |
| H1-1 | H1-1 | 46 | 8 |
| H1-1 Length | H1-1 Length | 22 | 8 |
| H2-1 | H2-1 | 46 | 0 |
| H2-1 Length | H2-1 Length | 6 | 0 |
| Meta Robots 1 | Meta Robots 1 | 46 | 0 |
| X-Robots-Tag 1 | X-Robots-Tag 1 | 46 | 0 |
| Meta Refresh 1 | Meta Refresh 1 | 46 | 0 |
| Canonical Link Element 1 | Canonical Link Element 1 | 46 | 0 |
| rel="next" 1 | rel="next" 1 | 46 | 0 |
| rel="prev" 1 | rel="prev" 1 | 46 | 0 |
| HTTP rel="next" 1 | HTTP rel="next" 1 | 46 | 0 |
| HTTP rel="prev" 1 | HTTP rel="prev" 1 | 46 | 0 |
| Size (Bytes) | Size (bytes) | 46 | 0 |
| Transferred (Bytes) | Transferred (bytes) | 46 | 0 |
| Total Transferred (Bytes) | Total Transferred (bytes) | 46 | 0 |
| Word Count | Word Count | 22 | 13 |
| Text Ratio | Text Ratio | 22 | 8 |
| Crawl Depth | Crawl Depth | 46 | 0 |
| Folder Depth | Folder Depth | 46 | 0 |
| Inlinks | Inlinks | 46 | 0 |
| Unique Inlinks | Unique Inlinks | 46 | 0 |
| % of Total | % of Total | 7 | 0 |
| Outlinks | Outlinks | 46 | 0 |
| Unique Outlinks | Unique Outlinks | 46 | 0 |
| External Outlinks | External Outlinks | 46 | 0 |
| Unique External Outlinks | Unique External Outlinks | 46 | 0 |
| Hash | Hash | 46 | 29 |
| Last Modified | Last-Modified | 46 | 0 |
| Redirect URL | Redirect URL | 46 | 1 |
| Redirect Type | Redirect Type | 46 | 1 |
| URL Encoded Address | URL Encoded Address | 46 | 0 |
| H1-2 | H1-2 | 46 | 0 |
| H1-2 Length | H1-2 Length | 46 | 0 |
| H2-2 | H2-2 | 46 | 0 |
| H2-2 Length | H2-2 Length | 46 | 0 |
| Meta Description 2 | Meta Description 2 | 46 | 0 |
| Meta Description 2 Length | Meta Description 2 Length | 46 | 0 |
| Meta Description 2 Pixel Width | Meta Description 2 Pixel Width | 46 | 0 |
| Title 2 | Title 2 | 46 | 0 |
| Title 2 Length | Title 2 Length | 46 | 0 |
| Title 2 Pixel Width | Title 2 Pixel Width | 46 | 0 |

### Beispiele `Title 1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 |  | Status 999 |
| {BASE}/e410 |  | Error 410 |
| {BASE}/e429 |  | Error 429 |
| {BASE}/e400 |  | Error 400 |
| {BASE}/e500 |  | Error 500 |
| {BASE}/e403 |  | Error 403 |
| {BASE}/e503 |  | Error 503 |
| {BASE}/e404 |  | Error 404 |

### Beispiele `Title 1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 | 0 | 10 |
| {BASE}/e410 | 0 | 9 |
| {BASE}/e429 | 0 | 9 |
| {BASE}/e400 | 0 | 9 |
| {BASE}/e500 | 0 | 9 |
| {BASE}/e403 | 0 | 9 |
| {BASE}/e503 | 0 | 9 |
| {BASE}/e404 | 0 | 9 |

### Beispiele `Title 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 | 0 | 96 |
| {BASE}/e410 | 0 | 83 |
| {BASE}/e429 | 0 | 83 |
| {BASE}/e400 | 0 | 83 |
| {BASE}/e500 | 0 | 83 |
| {BASE}/e403 | 0 | 83 |
| {BASE}/e503 | 0 | 83 |
| {BASE}/e404 | 0 | 83 |
| {BASE}/meta-refresh-self.html | 246 | 238 |

### Beispiele `H1-1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 |  | Status 999 |
| {BASE}/e410 |  | Error 410 |
| {BASE}/e429 |  | Error 429 |
| {BASE}/e400 |  | Error 400 |
| {BASE}/e500 |  | Error 500 |
| {BASE}/e403 |  | Error 403 |
| {BASE}/e503 |  | Error 503 |
| {BASE}/e404 |  | Error 404 |

### Beispiele `H1-1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 | 0 | 10 |
| {BASE}/e410 | 0 | 9 |
| {BASE}/e429 | 0 | 9 |
| {BASE}/e400 | 0 | 9 |
| {BASE}/e500 | 0 | 9 |
| {BASE}/e403 | 0 | 9 |
| {BASE}/e503 | 0 | 9 |
| {BASE}/e404 | 0 | 9 |

### Beispiele `Word Count`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 234 | 191 |
| {BASE}/s999 | 0 | 2 |
| {BASE}/e410 | 0 | 2 |
| {BASE}/pages/p2.html | 117 | 114 |
| {BASE}/e429 | 0 | 2 |
| {BASE}/pages/p3.html | 117 | 114 |
| {BASE}/pages/p4.html | 117 | 114 |
| {BASE}/e400 | 0 | 2 |
| {BASE}/pages/p1.html | 117 | 114 |
| {BASE}/e500 | 0 | 2 |

### Beispiele `Text Ratio`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s999 | 0.000 | 7.6 |
| {BASE}/e410 | 0.000 | 6.9 |
| {BASE}/e429 | 0.000 | 6.9 |
| {BASE}/e400 | 0.000 | 6.9 |
| {BASE}/e500 | 0.000 | 6.9 |
| {BASE}/e403 | 0.000 | 6.9 |
| {BASE}/e503 | 0.000 | 6.9 |
| {BASE}/e404 | 0.000 | 6.9 |

### Beispiele `Hash`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/r-to-404 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/s999 |  | 8779067d1b59a97031c50831f0ac1d91 |
| {BASE}/e410 |  | 228f384e89bfae67d4115330ef975323 |
| {BASE}/r307 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/r304 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/r308 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/r-to-dead |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/loop1 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/chain1 |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/r301-noloc |  | d41d8cd98f00b204e9800998ecf8427e |

### Beispiele `Redirect URL`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/meta-refresh-self.html | {BASE}/meta-refresh-self.html |  |

### Beispiele `Redirect Type`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/meta-refresh-self.html | Meta Refresh Redirect |  |

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 1 | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Refer... |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 262 | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); ... |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 24 | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |
| DEV-032 | ONPAGE-015 | OS-Festlegung | 39 | % of Total teilt durch alle gecrawlten internen HTML-Seiten mit Status 200 (ONPAGE-015); die Referenz teilt durch all... |

## Ignorierte Spalten (25)

- `amphtml Link Element (AMP erst mit M2 (M1.md 7.2))`
- `CO2 (mg) (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Carbon Rating (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Sentence Count (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Average Words Per Sentence (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Flesch Reading Ease Score (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Readability (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Link Score (nicht in M1 (P1/P2, M1.md 7.2) (TECH-145))`
- `Unique JS Inlinks (nur mit JavaScript-Rendering (M2))`
- `Unique JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Unique External JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Closest Near Duplicate Match (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `No. Near Duplicates (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `Spelling Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Grammar Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Response Time (nicht deterministisch)`
- `Cookies (Cookie-Zusammenfassung erst mit M2 (OUT-112, M1.md 7.3))`
- `Language (nicht in M1 (P1/P2, M1.md 7.2): Sprache (ONPAGE-153))`
- `HTTP Version (Transportdetail, nicht Teil der Parität (HTTP-Versionen erst M2, CRAWL-025))`
- `Mobile Alternate Link (Mobile erst mit M2 (M1.md 7.2))`
- `Closest Semantically Similar Address (KI-Funktion, M4)`
- `Semantic Similarity Score (KI-Funktion, M4)`
- `No. Semantically Similar (KI-Funktion, M4)`
- `Semantic Relevance Score (KI-Funktion, M4)`
- `Crawl Timestamp (nicht deterministisch)`

## OpenSpider-Spalten ohne Mapping (nicht verglichen) (2)

- `Meta Keywords 2`
- `Meta Keywords 2 Length`

## Gemappte Spalten fehlen in der Referenz (7)

- `Canonical Link Element 2`
- `Canonical Link Element 3`
- `Meta Refresh 2`
- `Meta Robots 2`
- `Meta Robots 3`
- `X-Robots-Tag 2`
- `X-Robots-Tag 3`

# Paritätsbericht `external_all` (Site `parity-response`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 95.833 % liegt unter 97.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 5 |
| Zeilen OpenSpider | 5 |
| zugeordnet | 5 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 24 |
| Zellen gleich | 23 |
| Übereinstimmung | 95.833 % |
| Schwelle | 97.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Content Type | Content | 5 | 0 |
| Status Code | Status Code | 5 | 0 |
| Status | Status | 4 | 0 |
| Crawl Depth | Crawl Depth | 5 | 0 |
| Inlinks | Inlinks | 5 | 1 |

### Beispiele `Inlinks`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| http://localhost:1/ | 1 | 2 |

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-005 | ONPAGE-065 | Paritaetsabweichung | 1 | OS-Festlegung ONPAGE-065 unterscheidet Connection Refused (abgelehnte Verbindung) von Connection Timeout; die Referen... |

## Gemappte Spalten fehlen in der Referenz (2)

- `Redirect Type`
- `Redirect URL`

# Paritätsbericht `all_inlinks` (Site `parity-response`)

Ergebnis: **nicht bestanden** – URL-Menge nicht identisch: 1 fehlen in OpenSpider, 0 zusätzlich

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 91 |
| Zeilen OpenSpider | 90 |
| zugeordnet | 90 |
| fehlen in OpenSpider | 1 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 1052 |
| Zellen gleich | 1044 |
| Übereinstimmung | 99.240 % |
| Schwelle | 97.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Size (Bytes) | Size (Bytes) | 90 | 0 |
| Alt Text | Alt Text | 90 | 0 |
| Anchor | Anchor | 90 | 0 |
| Status Code | Status Code | 85 | 0 |
| Status | Status | 88 | 0 |
| Follow | Follow | 90 | 0 |
| Target | Target | 90 | 0 |
| Rel | Rel | 90 | 0 |
| Path Type | Path Type | 90 | 0 |
| Link Path | Link Path | 90 | 4 |
| Link Position | Link Position | 90 | 4 |
| Link Origin | Link Origin | 69 | 0 |

### Beispiele `Link Path`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| Meta Refresh \| {BASE}/mixed-chain.html \| {BASE}/r301 \| #1 | //head/meta[2] |  |
| Meta Refresh \| {BASE}/r-meta-to-404.html \| {BASE}/e404 \| #1 | //head/meta[2] |  |
| Meta Refresh \| {BASE}/meta-refresh.html \| {BASE}/ok.html \| #1 | //head/meta[2] |  |
| Meta Refresh \| {BASE}/meta-refresh-5.html \| {BASE}/ok.html \| #1 | //head/meta[2] |  |

### Beispiele `Link Position`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| Meta Refresh \| {BASE}/mixed-chain.html \| {BASE}/r301 \| #1 | Head |  |
| Meta Refresh \| {BASE}/r-meta-to-404.html \| {BASE}/e404 \| #1 | Head |  |
| Meta Refresh \| {BASE}/meta-refresh.html \| {BASE}/ok.html \| #1 | Head |  |
| Meta Refresh \| {BASE}/meta-refresh-5.html \| {BASE}/ok.html \| #1 | Head |  |

## Fehlen in OpenSpider (1)

- `Meta Refresh | {BASE}/meta-refresh-self.html | {BASE}/meta-refresh-self.html | #1`

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-005 | ONPAGE-065 | Paritaetsabweichung | 2 | OS-Festlegung ONPAGE-065 unterscheidet Connection Refused (abgelehnte Verbindung) von Connection Timeout; die Referen... |
| DEV-006 | OUT-031 | Paritaetsabweichung | 5 | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz s... |
| DEV-009 | OUT-031 | Paritaetsabweichung | 21 | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Header... |

## Ignorierte Spalten (1)

- `Crawlability (OpenSpider fuehrt die Spalte 'Crawlability' nicht (OUT-023 liefert 15 Spalten ohne sie); 'Follow' deckt nofollow ab)`

# Paritätsbericht `redirect_chains` (Site `parity-response`)

Ergebnis: **nicht bestanden** – URL-Menge nicht identisch: 1 fehlen in OpenSpider, 0 zusätzlich

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 8 |
| Zeilen OpenSpider | 5 |
| zugeordnet | 5 |
| fehlen in OpenSpider | 1 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 236 |
| Zellen gleich | 236 |
| Übereinstimmung | 100.000 % |
| Schwelle | 97.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Chain Type | Chain Type | 5 | 0 |
| Number of Redirects | Number of Redirects | 5 | 0 |
| Loop | Redirect Loop | 5 | 0 |
| Temp Redirect in Chain | Temp Redirect in Chain | 4 | 0 |
| Final Address | Final Address | 4 | 0 |
| Final Indexability | Final Indexability | 4 | 0 |
| Final Indexability Status | Final Indexability Status | 4 | 0 |
| Final Content | Final Content Type | 5 | 0 |
| Final Status Code | Final Status Code | 4 | 0 |
| Final Status | Final Status | 4 | 0 |
| Status Code 1 | Status Code 1 | 5 | 0 |
| Status 1 | Status 1 | 5 | 0 |
| Redirect Type 1 | Redirect Type 1 | 5 | 0 |
| Redirect URL 1 | Redirect URL 1 | 5 | 0 |
| Status Code 2 | Status Code 2 | 5 | 0 |
| Status 2 | Status 2 | 5 | 0 |
| Redirect Type 2 | Redirect Type 2 | 5 | 0 |
| Redirect URL 2 | Redirect URL 2 | 5 | 0 |
| Status Code 3 | Status Code 3 | 2 | 0 |
| Status 3 | Status 3 | 2 | 0 |
| Redirect Type 3 | Redirect Type 3 | 5 | 0 |
| Redirect URL 3 | Redirect URL 3 | 5 | 0 |
| Status Code 4 | Status Code 4 | 5 | 0 |
| Status 4 | Status 4 | 5 | 0 |
| Redirect Type 4 | Redirect Type 4 | 5 | 0 |
| Redirect URL 4 | Redirect URL 4 | 5 | 0 |
| Status Code 5 | Status Code 5 | 5 | 0 |
| Status 5 | Status 5 | 5 | 0 |
| Redirect Type 5 | Redirect Type 5 | 5 | 0 |
| Redirect URL 5 | Redirect URL 5 | 5 | 0 |
| Status Code 6 | Status Code 6 | 4 | 0 |
| Status 6 | Status 6 | 4 | 0 |
| Redirect Type 6 | Redirect Type 6 | 5 | 0 |
| Redirect URL 6 | Redirect URL 6 | 5 | 0 |
| Status Code 7 | Status Code 7 | 5 | 0 |
| Status 7 | Status 7 | 5 | 0 |
| Redirect Type 7 | Redirect Type 7 | 5 | 0 |
| Redirect URL 7 | Redirect URL 7 | 5 | 0 |
| Status Code 8 | Status Code 8 | 5 | 0 |
| Status 8 | Status 8 | 5 | 0 |
| Redirect Type 8 | Redirect Type 8 | 5 | 0 |
| Redirect URL 8 | Redirect URL 8 | 5 | 0 |
| Status Code 9 | Status Code 9 | 5 | 0 |
| Status 9 | Status 9 | 5 | 0 |
| Redirect Type 9 | Redirect Type 9 | 5 | 0 |
| Redirect URL 9 | Redirect URL 9 | 5 | 0 |
| Status Code 10 | Status Code 10 | 5 | 0 |
| Status 10 | Status 10 | 5 | 0 |
| Redirect Type 10 | Redirect Type 10 | 5 | 0 |
| Redirect URL 10 | Redirect URL 10 | 5 | 0 |

## Fehlen in OpenSpider (1)

- `{BASE}/meta-refresh-self.html`

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-017 | OUT-081 | OS-Festlegung | 8 | OUT-081: die Hop-Spalten n beschreiben die Spruenge (Redirect Type n, Redirect URL n, Status Code n, Status n); die R... |
| DEV-030 | OUT-081 | OS-Festlegung | 5 | Bei einer Weiterleitungsschleife ist Final Address leer und Final Status = Redirect Loop (OUT-081); die Referenz nenn... |
| DEV-031 | OUT-081 | OS-Festlegung | 1 | Meta Refresh und Refresh-Header gelten nur mit Verzoegerung > 0 als temporaer (OUT-081); die Referenz wertet den Meta... |

## Ignorierte Spalten (15)

- `Source (OS-Festlegung (OUT-081): eine Zeile je Start-URL statt je verlinkender Quelle; OpenSpider nennt 'Sample Source')`
- `Alt Text (OS-Festlegung (OUT-081): eine Zeile je Start-URL statt je verlinkender Quelle)`
- `Anchor Text (OS-Festlegung (OUT-081): eine Zeile je Start-URL statt je verlinkender Quelle)`
- `Link Path (OS-Festlegung (OUT-081): eine Zeile je Start-URL statt je verlinkender Quelle)`
- `Link Position (OS-Festlegung (OUT-081): eine Zeile je Start-URL statt je verlinkender Quelle)`
- `Content 1 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 2 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 3 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 4 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 5 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 6 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 7 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 8 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 9 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`
- `Content 10 (Content-Type je Sprung steht in OpenSpider nicht im Report (OUT-081))`

## OpenSpider-Spalten ohne Mapping (nicht verglichen) (2)

- `Inlinks`
- `Sample Source`

## Gemappte Spalten fehlen in OpenSpider (20)

- `Status Code 6`
- `Status 6`
- `Redirect Type 6`
- `Redirect URL 6`
- `Status Code 7`
- `Status 7`
- `Redirect Type 7`
- `Redirect URL 7`
- `Status Code 8`
- `Status 8`
- `Redirect Type 8`
- `Redirect URL 8`
- `Status Code 9`
- `Status 9`
- `Redirect Type 9`
- `Redirect URL 9`
- `Status Code 10`
- `Status 10`
- `Redirect Type 10`
- `Redirect URL 10`

# Paritätsbericht `crawl_overview` (Site `parity-response`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 95.000 % liegt unter 100.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 468 |
| Zeilen OpenSpider | 209 |
| zugeordnet | 127 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 100 |
| Zellen gleich | 95 |
| Übereinstimmung | 95.000 % |
| Schwelle | 100.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| URLs | URLs | 100 | 5 |

### Beispiele `URLs`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| response codes \| redirection (meta refresh) | 5 | 4 |
| response codes \| internal redirection (meta refresh) | 5 | 4 |
| response codes \| internal redirect chain | 9 | 7 |
| response codes \| internal redirect loop | 3 | 2 |
| content \| low content pages | 8 | 9 |

## Zeilen der Referenz außerhalb von M1 (nicht verglichen) (341)

- `summary | urls displayed`
- `summary | total internal urls`
- `summary | total external urls`
- `summary | total internal indexable urls`
- `summary | total internal non-indexable urls`
- `internal | media`
- `internal | fonts`
- `internal | xml`
- `internal | plugins`
- `internal | other`
- `external | media`
- `external | fonts`
- `external | xml`
- `external | plugins`
- `security | mixed content`
- `security | form url insecure`
- `security | form on http url`
- `security | unsafe cross-origin links`
- `security | protocol-relative resource links`
- `security | missing hsts header`
- `security | missing content-security-policy header`
- `security | missing x-content-type-options header`
- `security | missing x-frame-options header`
- `security | missing secure referrer-policy header`
- `security | bad content type`
- `response codes | blocked resource`
- `response codes | redirection (javascript)`
- `response codes | redirection (http refresh)`
- `response codes | internal blocked resource`
- `response codes | internal redirection (javascript)`
- `response codes | internal redirection (http refresh)`
- `response codes | external blocked resource`
- `response codes | external redirection (javascript)`
- `response codes | external redirection (http refresh)`
- `url | repetitive path`
- `url | contains space`
- `url | internal search`
- `url | broken bookmark`
- `url | ga tracking parameters`
- `page titles | over 561 pixels`
- `page titles | below 200 pixels`
- `page titles | same as h1`
- `page titles | outside <head>`
- `meta description | over 985 pixels`
- `meta description | below 400 pixels`
- `meta description | outside <head>`
- `meta keywords | all`
- `meta keywords | missing`
- `meta keywords | duplicate`
- `meta keywords | multiple`
- ... und 291 weitere (siehe report.json)

## Zeilen nur in OpenSpider (kein Fehler) (82)

- `crawl data – external | all`
- `crawl data – external | css`
- `crawl data – external | fonts`
- `crawl data – external | html`
- `crawl data – external | images`
- `crawl data – external | javascript`
- `crawl data – external | media`
- `crawl data – external | other`
- `crawl data – external | pdf`
- `crawl data – external | plugins`
- `crawl data – external | unknown`
- `crawl data – external | xml`
- `crawl data – internal | all`
- `crawl data – internal | css`
- `crawl data – internal | fonts`
- `crawl data – internal | html`
- `crawl data – internal | images`
- `crawl data – internal | javascript`
- `crawl data – internal | media`
- `crawl data – internal | other`
- `crawl data – internal | pdf`
- `crawl data – internal | plugins`
- `crawl data – internal | unknown`
- `crawl data – internal | xml`
- `crawl depth | 10`
- `external | flash`
- `issues | issues (high)`
- `issues | issues (low)`
- `issues | issues (medium)`
- `issues | opportunities (high)`
- `issues | opportunities (low)`
- `issues | opportunities (medium)`
- `issues | warnings (high)`
- `issues | warnings (low)`
- `issues | warnings (medium)`
- `response codes | external other`
- `response codes | external redirect chain`
- `response codes | external redirect loop`
- `response codes | internal other`
- `response codes | other`
- `response codes | redirect chain`
- `response codes | redirect loop`
- `response codes – external | all errors`
- `response codes – external | blocked by robots.txt`
- `response codes – external | blocked resource`
- `response codes – external | client error (4xx)`
- `response codes – external | no response`
- `response codes – external | redirect chain`
- `response codes – external | redirect loop`
- `response codes – external | redirection (3xx)`
- ... und 32 weitere (siehe report.json)

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-002 | CRAWL-098 | OS-Festlegung | 10 | Folge von DEV-001: Die robots.txt-Zeile zaehlt in allen Zaehlern, deren Grundgesamtheit alle internen URLs mit Antwor... |
| DEV-013 | TECH-051 | OS-Festlegung | 1 | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 3 | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fu... |
| DEV-015 | OUT-080 | Paritaetsabweichung | 5 | Die Crawl-Depth-Zeilen der Referenz zaehlen nur interne HTML-Seiten mit 2xx (Zeile 0 = Start-URL); OpenSpider zaehlt ... |
| DEV-021 | TECH-011 | OS-Festlegung | 4 | Die Tabs Canonicals, Links und Hreflang fuehren interne HTML-Seiten mit Status 200 (TECH-011/-071/-130); die Referenz... |
| DEV-022 | CRAWL-089 | OS-Festlegung | 2 | Der Wirkungsbereich von issues.ignore_non_indexable ist in CRAWL-089 abschliessend auf die Element-Tabs und Low Conte... |
| DEV-023 | TECH-064 | OS-Festlegung | 1 | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im ... |

## Referenzeinschränkungen SF Free

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| SF-FREE-02 | TECH-210 | Annahme | 1 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt... |

## Verworfene Referenzspalten (2)

- `Total URLs`
- `Total URLs Description`

## Ignorierte Spalten (1)

- `% of Total (abgeleitet aus URLs und Grundgesamtheit des Abschnitts; verglichen werden die Zaehler (URLs, A3). Die Grundgesamtheit unterscheidet sich ueber die robots.txt-Zeile (K-40) und die Crawl-Depth-Population)`

# Paritätsbericht `internal_all` (Site `parity-synthetic`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 97.548 % liegt unter 99.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 400 |
| Zeilen OpenSpider | 401 |
| zugeordnet | 400 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 20722 |
| Zellen gleich | 20214 |
| Übereinstimmung | 97.548 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Content Type | Content | 400 | 0 |
| Status Code | Status Code | 400 | 0 |
| Status | Status | 400 | 0 |
| Indexability | Indexability | 400 | 0 |
| Indexability Status | Indexability Status | 399 | 0 |
| Title 1 | Title 1 | 400 | 14 |
| Title 1 Length | Title 1 Length | 393 | 14 |
| Title 1 Pixel Width | Title 1 Pixel Width | 393 | 14 |
| Meta Description 1 | Meta Description 1 | 400 | 0 |
| Meta Description 1 Length | Meta Description 1 Length | 379 | 0 |
| Meta Description 1 Pixel Width | Meta Description 1 Pixel Width | 379 | 10 |
| Meta Keywords 1 | Meta Keywords 1 | 400 | 0 |
| Meta Keywords 1 Length | Meta Keywords 1 Length | 0 | 0 |
| H1-1 | H1-1 | 400 | 14 |
| H1-1 Length | H1-1 Length | 393 | 14 |
| H2-1 | H2-1 | 400 | 0 |
| H2-1 Length | H2-1 Length | 0 | 0 |
| Meta Robots 1 | Meta Robots 1 | 400 | 0 |
| X-Robots-Tag 1 | X-Robots-Tag 1 | 400 | 0 |
| Meta Refresh 1 | Meta Refresh 1 | 400 | 0 |
| Canonical Link Element 1 | Canonical Link Element 1 | 400 | 0 |
| rel="next" 1 | rel="next" 1 | 400 | 0 |
| rel="prev" 1 | rel="prev" 1 | 400 | 0 |
| HTTP rel="next" 1 | HTTP rel="next" 1 | 400 | 0 |
| HTTP rel="prev" 1 | HTTP rel="prev" 1 | 400 | 0 |
| Size (Bytes) | Size (bytes) | 400 | 0 |
| Transferred (Bytes) | Transferred (bytes) | 400 | 0 |
| Total Transferred (Bytes) | Total Transferred (bytes) | 400 | 0 |
| Word Count | Word Count | 393 | 393 |
| Text Ratio | Text Ratio | 393 | 14 |
| Crawl Depth | Crawl Depth | 400 | 0 |
| Folder Depth | Folder Depth | 400 | 0 |
| Inlinks | Inlinks | 400 | 0 |
| Unique Inlinks | Unique Inlinks | 400 | 0 |
| % of Total | % of Total | 400 | 0 |
| Outlinks | Outlinks | 400 | 0 |
| Unique Outlinks | Unique Outlinks | 400 | 0 |
| External Outlinks | External Outlinks | 400 | 0 |
| Unique External Outlinks | Unique External Outlinks | 400 | 0 |
| Hash | Hash | 400 | 21 |
| Last Modified | Last-Modified | 400 | 0 |
| Redirect URL | Redirect URL | 400 | 0 |
| Redirect Type | Redirect Type | 400 | 0 |
| URL Encoded Address | URL Encoded Address | 400 | 0 |
| H1-2 | H1-2 | 400 | 0 |
| H1-2 Length | H1-2 Length | 400 | 0 |
| H2-2 | H2-2 | 400 | 0 |
| H2-2 Length | H2-2 Length | 400 | 0 |
| Meta Description 2 | Meta Description 2 | 400 | 0 |
| Meta Description 2 Length | Meta Description 2 Length | 400 | 0 |
| Meta Description 2 Pixel Width | Meta Description 2 Pixel Width | 400 | 0 |
| Title 2 | Title 2 | 400 | 0 |
| Title 2 Length | Title 2 Length | 400 | 0 |
| Title 2 Pixel Width | Title 2 Pixel Width | 400 | 0 |

### Beispiele `Title 1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html |  | Nicht gefunden |
| {BASE}/s99/p199.html |  | Serverfehler |
| {BASE}/s40/p240.html |  | Serverfehler |
| {BASE}/s58/p358.html |  | Nicht gefunden |
| {BASE}/s29/p229.html |  | Nicht gefunden |
| {BASE}/s17/p217.html |  | Nicht gefunden |
| {BASE}/s82/p182.html |  | Serverfehler |
| {BASE}/s33/p233.html |  | Nicht gefunden |
| {BASE}/s76/p176.html |  | Serverfehler |
| {BASE}/s43/p243.html |  | Nicht gefunden |

### Beispiele `Title 1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html | 0 | 14 |
| {BASE}/s99/p199.html | 0 | 12 |
| {BASE}/s40/p240.html | 0 | 12 |
| {BASE}/s58/p358.html | 0 | 14 |
| {BASE}/s29/p229.html | 0 | 14 |
| {BASE}/s17/p217.html | 0 | 14 |
| {BASE}/s82/p182.html | 0 | 12 |
| {BASE}/s33/p233.html | 0 | 14 |
| {BASE}/s76/p176.html | 0 | 12 |
| {BASE}/s43/p243.html | 0 | 14 |

### Beispiele `Title 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html | 0 | 135 |
| {BASE}/s99/p199.html | 0 | 109 |
| {BASE}/s40/p240.html | 0 | 109 |
| {BASE}/s58/p358.html | 0 | 135 |
| {BASE}/s29/p229.html | 0 | 135 |
| {BASE}/s17/p217.html | 0 | 135 |
| {BASE}/s82/p182.html | 0 | 109 |
| {BASE}/s33/p233.html | 0 | 135 |
| {BASE}/s76/p176.html | 0 | 109 |
| {BASE}/s43/p243.html | 0 | 135 |

### Beispiele `Meta Description 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 151 | 156 |
| {BASE}/s2/p2.html | 151 | 156 |
| {BASE}/s3/p3.html | 151 | 156 |
| {BASE}/s1/p1.html | 151 | 156 |
| {BASE}/s7/p7.html | 151 | 156 |
| {BASE}/s5/p5.html | 151 | 156 |
| {BASE}/s9/p9.html | 151 | 156 |
| {BASE}/s8/p8.html | 151 | 156 |
| {BASE}/s6/p6.html | 151 | 156 |
| {BASE}/s4/p4.html | 151 | 156 |

### Beispiele `H1-1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html |  | 404 |
| {BASE}/s99/p199.html |  | Serverfehler |
| {BASE}/s40/p240.html |  | Serverfehler |
| {BASE}/s58/p358.html |  | 404 |
| {BASE}/s29/p229.html |  | 404 |
| {BASE}/s17/p217.html |  | 404 |
| {BASE}/s82/p182.html |  | Serverfehler |
| {BASE}/s33/p233.html |  | 404 |
| {BASE}/s76/p176.html |  | Serverfehler |
| {BASE}/s43/p243.html |  | 404 |

### Beispiele `H1-1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html | 0 | 3 |
| {BASE}/s99/p199.html | 0 | 12 |
| {BASE}/s40/p240.html | 0 | 12 |
| {BASE}/s58/p358.html | 0 | 3 |
| {BASE}/s29/p229.html | 0 | 3 |
| {BASE}/s17/p217.html | 0 | 3 |
| {BASE}/s82/p182.html | 0 | 12 |
| {BASE}/s33/p233.html | 0 | 3 |
| {BASE}/s76/p176.html | 0 | 12 |
| {BASE}/s43/p243.html | 0 | 3 |

### Beispiele `Word Count`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 122 | 112 |
| {BASE}/s2/p2.html | 122 | 112 |
| {BASE}/s3/p3.html | 122 | 112 |
| {BASE}/s1/p1.html | 122 | 112 |
| {BASE}/s12/p12.html | 122 | 112 |
| {BASE}/s7/p7.html | 122 | 112 |
| {BASE}/s5/p5.html | 122 | 112 |
| {BASE}/s9/p9.html | 122 | 112 |
| {BASE}/s11/p11.html | 122 | 112 |
| {BASE}/s8/p8.html | 122 | 112 |

### Beispiele `Text Ratio`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html | 0.000 | 3.2 |
| {BASE}/s99/p199.html | 0.000 | 11.8 |
| {BASE}/s40/p240.html | 0.000 | 11.8 |
| {BASE}/s58/p358.html | 0.000 | 3.2 |
| {BASE}/s29/p229.html | 0.000 | 3.2 |
| {BASE}/s17/p217.html | 0.000 | 3.2 |
| {BASE}/s82/p182.html | 0.000 | 11.8 |
| {BASE}/s33/p233.html | 0.000 | 3.2 |
| {BASE}/s76/p176.html | 0.000 | 11.8 |
| {BASE}/s43/p243.html | 0.000 | 3.2 |

### Beispiele `Hash`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/s67/p167.html |  | 86d3018671a21e0f2916f267c65f5042 |
| {BASE}/s99/p199.html |  | 887858d5c1d5ac4f062f93689f7980c2 |
| {BASE}/s40/p240.html |  | 887858d5c1d5ac4f062f93689f7980c2 |
| {BASE}/s60/p260.html |  | efe2b16de9a0bcf3093950ca3afd4e3e |
| {BASE}/s58/p358.html |  | 86d3018671a21e0f2916f267c65f5042 |
| {BASE}/s29/p229.html |  | 86d3018671a21e0f2916f267c65f5042 |
| {BASE}/s17/p217.html |  | 86d3018671a21e0f2916f267c65f5042 |
| {BASE}/s19/p219.html |  | efe2b16de9a0bcf3093950ca3afd4e3e |
| {BASE}/s73/p173.html |  | efe2b16de9a0bcf3093950ca3afd4e3e |
| {BASE}/s82/p182.html |  | 887858d5c1d5ac4f062f93689f7980c2 |

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 1 | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Refer... |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 870 | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); ... |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 7 | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |
| DEV-025 | TECH-001 | OS-Festlegung | 1 | Indexability Status nennt genau den ersten zutreffenden Grund in der Pruefreihenfolge (TECH-001); die Referenz nennt ... |

## Ignorierte Spalten (25)

- `amphtml Link Element (AMP erst mit M2 (M1.md 7.2))`
- `CO2 (mg) (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Carbon Rating (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Sentence Count (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Average Words Per Sentence (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Flesch Reading Ease Score (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Readability (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Link Score (nicht in M1 (P1/P2, M1.md 7.2) (TECH-145))`
- `Unique JS Inlinks (nur mit JavaScript-Rendering (M2))`
- `Unique JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Unique External JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Closest Near Duplicate Match (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `No. Near Duplicates (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `Spelling Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Grammar Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Response Time (nicht deterministisch)`
- `Cookies (Cookie-Zusammenfassung erst mit M2 (OUT-112, M1.md 7.3))`
- `Language (nicht in M1 (P1/P2, M1.md 7.2): Sprache (ONPAGE-153))`
- `HTTP Version (Transportdetail, nicht Teil der Parität (HTTP-Versionen erst M2, CRAWL-025))`
- `Mobile Alternate Link (Mobile erst mit M2 (M1.md 7.2))`
- `Closest Semantically Similar Address (KI-Funktion, M4)`
- `Semantic Similarity Score (KI-Funktion, M4)`
- `No. Semantically Similar (KI-Funktion, M4)`
- `Semantic Relevance Score (KI-Funktion, M4)`
- `Crawl Timestamp (nicht deterministisch)`

## OpenSpider-Spalten ohne Mapping (nicht verglichen) (2)

- `Meta Keywords 2`
- `Meta Keywords 2 Length`

## Gemappte Spalten fehlen in der Referenz (7)

- `Canonical Link Element 2`
- `Canonical Link Element 3`
- `Meta Refresh 2`
- `Meta Robots 2`
- `Meta Robots 3`
- `X-Robots-Tag 2`
- `X-Robots-Tag 3`

# Paritätsbericht `internal_all` (Site `parity-technical`)

Ergebnis: **nicht bestanden** – URL-Menge nicht identisch: 0 fehlen in OpenSpider, 1 zusätzlich

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 111 |
| Zeilen OpenSpider | 113 |
| zugeordnet | 111 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 1 |
| Zellen verglichen | 6070 |
| Zellen gleich | 6044 |
| Übereinstimmung | 99.572 % |
| Schwelle | 99.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Content Type | Content | 111 | 0 |
| Status Code | Status Code | 111 | 0 |
| Status | Status | 111 | 0 |
| Indexability | Indexability | 111 | 0 |
| Indexability Status | Indexability Status | 111 | 0 |
| Title 1 | Title 1 | 111 | 1 |
| Title 1 Length | Title 1 Length | 107 | 1 |
| Title 1 Pixel Width | Title 1 Pixel Width | 107 | 1 |
| Meta Description 1 | Meta Description 1 | 111 | 0 |
| Meta Description 1 Length | Meta Description 1 Length | 106 | 0 |
| Meta Description 1 Pixel Width | Meta Description 1 Pixel Width | 106 | 0 |
| Meta Keywords 1 | Meta Keywords 1 | 111 | 0 |
| Meta Keywords 1 Length | Meta Keywords 1 Length | 0 | 0 |
| H1-1 | H1-1 | 111 | 1 |
| H1-1 Length | H1-1 Length | 107 | 1 |
| H2-1 | H2-1 | 111 | 0 |
| H2-1 Length | H2-1 Length | 106 | 0 |
| Meta Robots 1 | Meta Robots 1 | 111 | 0 |
| Meta Robots 2 | Meta Robots 2 | 111 | 0 |
| X-Robots-Tag 1 | X-Robots-Tag 1 | 111 | 0 |
| Meta Refresh 1 | Meta Refresh 1 | 111 | 0 |
| Canonical Link Element 1 | Canonical Link Element 1 | 111 | 0 |
| Canonical Link Element 2 | Canonical Link Element 2 | 111 | 0 |
| rel="next" 1 | rel="next" 1 | 111 | 0 |
| rel="prev" 1 | rel="prev" 1 | 111 | 0 |
| HTTP rel="next" 1 | HTTP rel="next" 1 | 111 | 0 |
| HTTP rel="prev" 1 | HTTP rel="prev" 1 | 111 | 0 |
| Size (Bytes) | Size (bytes) | 111 | 0 |
| Transferred (Bytes) | Transferred (bytes) | 111 | 0 |
| Total Transferred (Bytes) | Total Transferred (bytes) | 111 | 0 |
| Word Count | Word Count | 107 | 14 |
| Text Ratio | Text Ratio | 107 | 1 |
| Crawl Depth | Crawl Depth | 111 | 0 |
| Folder Depth | Folder Depth | 111 | 0 |
| Inlinks | Inlinks | 111 | 0 |
| Unique Inlinks | Unique Inlinks | 111 | 0 |
| % of Total | % of Total | 111 | 0 |
| Outlinks | Outlinks | 111 | 0 |
| Unique Outlinks | Unique Outlinks | 111 | 0 |
| External Outlinks | External Outlinks | 111 | 0 |
| Unique External Outlinks | Unique External Outlinks | 111 | 0 |
| Hash | Hash | 111 | 4 |
| Last Modified | Last-Modified | 111 | 0 |
| Redirect URL | Redirect URL | 111 | 1 |
| Redirect Type | Redirect Type | 111 | 1 |
| URL Encoded Address | URL Encoded Address | 111 | 0 |
| H1-2 | H1-2 | 111 | 0 |
| H1-2 Length | H1-2 Length | 111 | 0 |
| H2-2 | H2-2 | 111 | 0 |
| H2-2 Length | H2-2 Length | 111 | 0 |
| Meta Description 2 | Meta Description 2 | 111 | 0 |
| Meta Description 2 Length | Meta Description 2 Length | 111 | 0 |
| Meta Description 2 Pixel Width | Meta Description 2 Pixel Width | 111 | 0 |
| Title 2 | Title 2 | 111 | 0 |
| Title 2 Length | Title 2 Length | 111 | 0 |
| Title 2 Pixel Width | Title 2 Pixel Width | 111 | 0 |

### Beispiele `Title 1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html |  | Canonical target gone |

### Beispiele `Title 1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html | 0 | 21 |

### Beispiele `Title 1 Pixel Width`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html | 0 | 196 |

### Beispiele `H1-1`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html |  | Canonical target gone |

### Beispiele `H1-1 Length`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html | 0 | 21 |

### Beispiele `Word Count`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/ | 93 | 85 |
| {BASE}/dir/ | 94 | 81 |
| {BASE}/tree/ | 86 | 80 |
| {BASE}/links/ | 110 | 96 |
| {BASE}/map/ | 93 | 85 |
| {BASE}/canon/ | 94 | 81 |
| {BASE}/hl/ | 82 | 76 |
| {BASE}/tree/c6/ | 108 | 102 |
| {BASE}/tree/c1/ | 108 | 102 |
| {BASE}/tree/c4/ | 108 | 102 |

### Beispiele `Text Ratio`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/canon/gone-404.html | 0.000 | 13.6 |

### Beispiele `Hash`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/hl/es-old/ |  | d41d8cd98f00b204e9800998ecf8427e |
| {BASE}/img/small.png |  | ceafb7558b76063e445e773b12aaeaca |
| {BASE}/canon/gone-404.html |  | 3ac01b92326fab8a6ae50922bda1b2f0 |
| {BASE}/canon/moved-301 |  | d41d8cd98f00b204e9800998ecf8427e |

### Beispiele `Redirect URL`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/dir/refresh-bare.html | {BASE}/dir/refresh-bare.html |  |

### Beispiele `Redirect Type`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| {BASE}/dir/refresh-bare.html | Meta Refresh Redirect |  |

## Zusätzlich in OpenSpider (1)

- `{BASE}/hl/it/`

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 1 | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Refer... |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 142 | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); ... |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 4 | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |

## Ignorierte Spalten (25)

- `amphtml Link Element (AMP erst mit M2 (M1.md 7.2))`
- `CO2 (mg) (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Carbon Rating (nicht in M1 (P1/P2, M1.md 7.2): Nachhaltigkeitsspalten)`
- `Sentence Count (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Average Words Per Sentence (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Flesch Reading Ease Score (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Readability (nicht in M1 (P1/P2, M1.md 7.2) (ONPAGE-149))`
- `Link Score (nicht in M1 (P1/P2, M1.md 7.2) (TECH-145))`
- `Unique JS Inlinks (nur mit JavaScript-Rendering (M2))`
- `Unique JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Unique External JS Outlinks (nur mit JavaScript-Rendering (M2))`
- `Closest Near Duplicate Match (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `No. Near Duplicates (nicht in M1 (P1/P2, M1.md 7.2): Near Duplicates (ONPAGE-144 ff.))`
- `Spelling Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Grammar Errors (nicht in M1 (P1/P2, M1.md 7.2): Rechtschreibung/Grammatik)`
- `Response Time (nicht deterministisch)`
- `Cookies (Cookie-Zusammenfassung erst mit M2 (OUT-112, M1.md 7.3))`
- `Language (nicht in M1 (P1/P2, M1.md 7.2): Sprache (ONPAGE-153))`
- `HTTP Version (Transportdetail, nicht Teil der Parität (HTTP-Versionen erst M2, CRAWL-025))`
- `Mobile Alternate Link (Mobile erst mit M2 (M1.md 7.2))`
- `Closest Semantically Similar Address (KI-Funktion, M4)`
- `Semantic Similarity Score (KI-Funktion, M4)`
- `No. Semantically Similar (KI-Funktion, M4)`
- `Semantic Relevance Score (KI-Funktion, M4)`
- `Crawl Timestamp (nicht deterministisch)`

## OpenSpider-Spalten ohne Mapping (nicht verglichen) (2)

- `Meta Keywords 2`
- `Meta Keywords 2 Length`

## Gemappte Spalten fehlen in der Referenz (5)

- `Canonical Link Element 3`
- `Meta Refresh 2`
- `Meta Robots 3`
- `X-Robots-Tag 2`
- `X-Robots-Tag 3`

# Paritätsbericht `all_inlinks` (Site `parity-technical`)

Ergebnis: **nicht bestanden** – URL-Menge nicht identisch: 1 fehlen in OpenSpider, 1 zusätzlich

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 405 |
| Zeilen OpenSpider | 405 |
| zugeordnet | 404 |
| fehlen in OpenSpider | 1 |
| zusätzlich in OpenSpider | 1 |
| Zellen verglichen | 4837 |
| Zellen gleich | 4785 |
| Übereinstimmung | 98.925 % |
| Schwelle | 97.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| Size (Bytes) | Size (Bytes) | 404 | 0 |
| Alt Text | Alt Text | 404 | 0 |
| Anchor | Anchor | 404 | 0 |
| Status Code | Status Code | 403 | 0 |
| Status | Status | 404 | 0 |
| Follow | Follow | 399 | 0 |
| Target | Target | 404 | 0 |
| Rel | Rel | 404 | 0 |
| Path Type | Path Type | 404 | 0 |
| Link Path | Link Path | 404 | 26 |
| Link Position | Link Position | 404 | 26 |
| Link Origin | Link Origin | 399 | 0 |

### Beispiele `Link Path`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| Hreflang \| {BASE}/hl/de/ \| {BASE}/hl/de/ \| #1 | //head/link[1] |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/de/ \| #1 | //head/link[2] |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/es-old/ \| #1 | //head/link[4] |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/fr/ \| #1 | //head/link[3] |  |
| Hreflang \| {BASE}/hl/de/ \| {BASE}/hl/en/ \| #1 | //head/link[2] |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #1 | //head/link[1] |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #2 | //head/link[2] |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #3 | //head/link[3] |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #4 | //head/link[4] |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #5 | //head/link[5] |  |

### Beispiele `Link Position`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| Hreflang \| {BASE}/hl/de/ \| {BASE}/hl/de/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/de/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/es-old/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/en/ \| {BASE}/hl/fr/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/de/ \| {BASE}/hl/en/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #1 | Head |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #2 | Head |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #3 | Head |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #4 | Head |  |
| Hreflang \| {BASE}/hl/bad-codes.html \| {BASE}/hl/en/ \| #5 | Head |  |

## Fehlen in OpenSpider (1)

- `Meta Refresh | {BASE}/dir/refresh-bare.html | {BASE}/dir/refresh-bare.html | #1`

## Zusätzlich in OpenSpider (1)

- `Hreflang | {BASE}/hl/en/ | {BASE}/hl/it/ | #1`

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-006 | OUT-031 | Paritaetsabweichung | 1 | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz s... |
| DEV-008 | OUT-031 | OS-Festlegung | 5 | Follow=False auch bei Meta/X-Robots nofollow der Quellseite (OUT-031); die Referenz bewertet nur das Link-Attribut. |
| DEV-009 | OUT-031 | Paritaetsabweichung | 5 | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Header... |

## Ignorierte Spalten (1)

- `Crawlability (OpenSpider fuehrt die Spalte 'Crawlability' nicht (OUT-023 liefert 15 Spalten ohne sie); 'Follow' deckt nofollow ab)`

# Paritätsbericht `crawl_overview` (Site `parity-technical`)

Ergebnis: **nicht bestanden** – Zellübereinstimmung 93.750 % liegt unter 100.000 %

| Kennzahl | Wert |
|---|---:|
| Zeilen Referenz | 474 |
| Zeilen OpenSpider | 209 |
| zugeordnet | 127 |
| fehlen in OpenSpider | 0 |
| zusätzlich in OpenSpider | 0 |
| Zellen verglichen | 112 |
| Zellen gleich | 105 |
| Übereinstimmung | 93.750 % |
| Schwelle | 100.000 % |

## Spalten

| Referenz | OpenSpider | verglichen | Fehler |
|---|---|---:|---:|
| URLs | URLs | 112 | 7 |

### Beispiele `URLs`

| Schlüssel | Referenz | OpenSpider |
|---|---|---|
| summary \| total urls encountered | 116 | 118 |
| internal \| all | 111 | 113 |
| security \| all | 111 | 113 |
| security \| http urls | 111 | 113 |
| response codes \| redirection (meta refresh) | 2 | 1 |
| response codes \| internal redirection (meta refresh) | 2 | 1 |
| response codes \| internal redirect loop | 1 | 0 |

## Zeilen der Referenz außerhalb von M1 (nicht verglichen) (347)

- `summary | urls displayed`
- `summary | total internal urls`
- `summary | total external urls`
- `summary | total internal indexable urls`
- `summary | total internal non-indexable urls`
- `internal | media`
- `internal | fonts`
- `internal | xml`
- `internal | plugins`
- `internal | other`
- `external | media`
- `external | fonts`
- `external | xml`
- `external | plugins`
- `security | mixed content`
- `security | form url insecure`
- `security | form on http url`
- `security | unsafe cross-origin links`
- `security | protocol-relative resource links`
- `security | missing hsts header`
- `security | missing content-security-policy header`
- `security | missing x-content-type-options header`
- `security | missing x-frame-options header`
- `security | missing secure referrer-policy header`
- `security | bad content type`
- `response codes | blocked resource`
- `response codes | redirection (javascript)`
- `response codes | redirection (http refresh)`
- `response codes | internal blocked resource`
- `response codes | internal redirection (javascript)`
- `response codes | internal redirection (http refresh)`
- `response codes | external blocked resource`
- `response codes | external redirection (javascript)`
- `response codes | external redirection (http refresh)`
- `url | repetitive path`
- `url | contains space`
- `url | internal search`
- `url | broken bookmark`
- `url | ga tracking parameters`
- `page titles | over 561 pixels`
- `page titles | below 200 pixels`
- `page titles | same as h1`
- `page titles | outside <head>`
- `meta description | over 985 pixels`
- `meta description | below 400 pixels`
- `meta description | outside <head>`
- `meta keywords | all`
- `meta keywords | missing`
- `meta keywords | duplicate`
- `meta keywords | multiple`
- ... und 297 weitere (siehe report.json)

## Zeilen nur in OpenSpider (kein Fehler) (82)

- `crawl data – external | all`
- `crawl data – external | css`
- `crawl data – external | fonts`
- `crawl data – external | html`
- `crawl data – external | images`
- `crawl data – external | javascript`
- `crawl data – external | media`
- `crawl data – external | other`
- `crawl data – external | pdf`
- `crawl data – external | plugins`
- `crawl data – external | unknown`
- `crawl data – external | xml`
- `crawl data – internal | all`
- `crawl data – internal | css`
- `crawl data – internal | fonts`
- `crawl data – internal | html`
- `crawl data – internal | images`
- `crawl data – internal | javascript`
- `crawl data – internal | media`
- `crawl data – internal | other`
- `crawl data – internal | pdf`
- `crawl data – internal | plugins`
- `crawl data – internal | unknown`
- `crawl data – internal | xml`
- `crawl depth | 10`
- `external | flash`
- `issues | issues (high)`
- `issues | issues (low)`
- `issues | issues (medium)`
- `issues | opportunities (high)`
- `issues | opportunities (low)`
- `issues | opportunities (medium)`
- `issues | warnings (high)`
- `issues | warnings (low)`
- `issues | warnings (medium)`
- `response codes | external other`
- `response codes | external redirect chain`
- `response codes | external redirect loop`
- `response codes | internal other`
- `response codes | other`
- `response codes | redirect chain`
- `response codes | redirect loop`
- `response codes – external | all errors`
- `response codes – external | blocked by robots.txt`
- `response codes – external | blocked resource`
- `response codes – external | client error (4xx)`
- `response codes – external | no response`
- `response codes – external | redirect chain`
- `response codes – external | redirect loop`
- `response codes – external | redirection (3xx)`
- ... und 32 weitere (siehe report.json)

## Angewandte Abweichungen

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-002 | CRAWL-098 | OS-Festlegung | 6 | Folge von DEV-001: Die robots.txt-Zeile zaehlt in allen Zaehlern, deren Grundgesamtheit alle internen URLs mit Antwor... |
| DEV-013 | TECH-051 | OS-Festlegung | 1 | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 1 | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fu... |
| DEV-015 | OUT-080 | Paritaetsabweichung | 2 | Die Crawl-Depth-Zeilen der Referenz zaehlen nur interne HTML-Seiten mit 2xx (Zeile 0 = Start-URL); OpenSpider zaehlt ... |
| DEV-021 | TECH-011 | OS-Festlegung | 1 | Die Tabs Canonicals, Links und Hreflang fuehren interne HTML-Seiten mit Status 200 (TECH-011/-071/-130); die Referenz... |
| DEV-022 | CRAWL-089 | OS-Festlegung | 2 | Der Wirkungsbereich von issues.ignore_non_indexable ist in CRAWL-089 abschliessend auf die Element-Tabs und Low Conte... |
| DEV-023 | TECH-064 | OS-Festlegung | 1 | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im ... |

## Referenzeinschränkungen SF Free

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| SF-FREE-02 | TECH-210 | Annahme | 1 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt... |

## Verworfene Referenzspalten (2)

- `Total URLs`
- `Total URLs Description`

## Ignorierte Spalten (1)

- `% of Total (abgeleitet aus URLs und Grundgesamtheit des Abschnitts; verglichen werden die Zaehler (URLs, A3). Die Grundgesamtheit unterscheidet sich ueber die robots.txt-Zeile (K-40) und die Crawl-Depth-Population)`

