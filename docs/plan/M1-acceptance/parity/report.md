# Paritätslauf: bestanden

Sites mit Status present: 5
(gefordert: 4)

| Site | Export | Mapping | Zellen | Übereinstimmung | Schwelle | Ergebnis |
|---|---|---|---:|---:|---:|---|
| out-basic-site | internal_all.csv | internal_all | 894 | 100.000 % | 99.0 % | ok |
| out-basic-site | external_all.csv | external_all | 5 | 100.000 % | 97.0 % | ok |
| out-basic-site | response_codes_all.csv | response_codes_all | 152 | 100.000 % | 99.0 % | ok |
| out-basic-site | links_all.csv | links_all | 100 | 100.000 % | 97.0 % | ok |
| out-basic-site | all_inlinks.csv | all_inlinks | 1067 | 100.000 % | 97.0 % | ok |
| out-basic-site | redirect_chains.csv | redirect_chains | 0 | 100.000 % | 97.0 % | ok |
| out-basic-site | crawl_overview.csv | crawl_overview | 124 | 100.000 % | 100.0 % | ok |
| parity-onpage | internal_all.csv | internal_all | 4761 | 100.000 % | 99.0 % | ok |
| parity-onpage | external_all.csv | external_all | 44 | 100.000 % | 97.0 % | ok |
| parity-onpage | response_codes_all.csv | response_codes_all | 911 | 100.000 % | 99.0 % | ok |
| parity-onpage | url_all.csv | url_all | 945 | 100.000 % | 97.0 % | ok |
| parity-onpage | page_titles_all.csv | page_titles_all | 756 | 100.000 % | 99.0 % | ok |
| parity-onpage | meta_description_all.csv | meta_description_all | 756 | 100.000 % | 99.0 % | ok |
| parity-onpage | h1_all.csv | h1_all | 588 | 100.000 % | 99.0 % | ok |
| parity-onpage | h2_all.csv | h2_all | 588 | 100.000 % | 99.0 % | ok |
| parity-onpage | content_all.csv | content_all | 336 | 100.000 % | 97.0 % | ok |
| parity-onpage | images_all.csv | images_all | 57 | 100.000 % | 97.0 % | ok |
| parity-onpage | all_inlinks.csv | all_inlinks | 3322 | 100.000 % | 97.0 % | ok |
| parity-onpage | crawl_overview.csv | crawl_overview | 118 | 100.000 % | 100.0 % | ok |
| parity-response | internal_all.csv | internal_all | 2158 | 100.000 % | 99.0 % | ok |
| parity-response | external_all.csv | external_all | 24 | 100.000 % | 97.0 % | ok |
| parity-response | response_codes_all.csv | response_codes_all | 407 | 100.000 % | 99.0 % | ok |
| parity-response | all_inlinks.csv | all_inlinks | 1064 | 100.000 % | 97.0 % | ok |
| parity-response | all_error_(4xx_5xx_no_response)_inlinks.csv | all_error_inlinks | 209 | 100.000 % | 97.0 % | ok |
| parity-response | redirect_chains.csv | redirect_chains | 281 | 100.000 % | 97.0 % | ok |
| parity-response | redirects_to_error.csv | redirects_to_errors | - | - | - | übersprungen (außerhalb des M1-Umfangs: Report Redirects to Errors (OUT-084) ist P1 und kein M1-Umfang (M1.md 7.2); OpenSpider hat in M1 nur Redirects:All Redirects und Redirects:Redirect Chains) |
| parity-response | crawl_overview.csv | crawl_overview | 121 | 100.000 % | 100.0 % | ok |
| parity-synthetic | internal_all.csv | internal_all | 20652 | 100.000 % | 99.0 % | ok |
| parity-synthetic | links_all.csv | links_all | 3410 | 100.000 % | 97.0 % | ok |
| parity-synthetic | all_inlinks.csv | all_inlinks | 50105 | 100.000 % | 97.0 % | ok |
| parity-synthetic | crawl_overview.csv | crawl_overview | 125 | 100.000 % | 100.0 % | ok |
| parity-technical | internal_all.csv | internal_all | 6065 | 100.000 % | 99.0 % | ok |
| parity-technical | canonicals_all.csv | canonicals_all | 1272 | 100.000 % | 99.0 % | ok |
| parity-technical | directives_all.csv | directives_all | 855 | 100.000 % | 99.0 % | ok |
| parity-technical | hreflang_all.csv | hreflang_all | 1696 | 100.000 % | 97.0 % | ok |
| parity-technical | links_all.csv | links_all | 950 | 100.000 % | 97.0 % | ok |
| parity-technical | sitemaps_all.csv | sitemaps_all | 0 | 100.000 % | 97.0 % | ok |
| parity-technical | all_inlinks.csv | all_inlinks | 4849 | 100.000 % | 97.0 % | ok |
| parity-technical | crawl_overview.csv | crawl_overview | 121 | 100.000 % | 100.0 % | ok |

## Begründete Paritätsabweichungen

| Kennung | Spec | Kennzeichen | Zellen/Zeilen | Sites | Begründung |
|---|---|---|---:|---|---|
| DEV-003 | ONPAGE-011 | OS-Festlegung | 2326 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Nicht-HTML-URLs sowie Weiterleitungen, Fehlerseiten (4xx/5xx) und URLs ohne Antwort haben leere Element-, Laengen- und Pixelspalten, auch wenn ihr Koerper <title>/<h1> enthaelt (ONPAGE-011, A-5, K-60, Befund N-01), und eine Seite ohne Element leere Wertspalten (ONPAGE-008); die Referenz schreibt 0. |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 85 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Nicht-HTML-URLs sowie Weiterleitungen, Fehlerseiten (4xx/5xx) und URLs ohne Antwort: Word Count und Text Ratio leer (ONPAGE-013, K-60, Befund N-01); die Referenz schreibt 0. |
| DEV-005 | ONPAGE-065 | OS-Festlegung | 9 | parity-onpage, parity-response | ONPAGE-065 (K-67): OpenSpider meldet die tatsaechliche Ursache, Connection Refused bei abgelehntem Verbindungsaufbau und Connection Timeout bei ausbleibender Antwort; die Referenz meldet fuer den geschlossenen Port localhost:1 Connection Timeout. |
| DEV-006 | OUT-031 | Paritaetsabweichung | 7 | parity-onpage, parity-response, parity-technical | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz schreibt 0 fuer robots.txt-blockierte Ziele. |
| DEV-006b | OUT-031 | Paritaetsabweichung | 2 | parity-response | wie DEV-006 (Fehler-Inlinks auf nicht abgerufene Ziele). |
| DEV-007 | OUT-031 | Paritaetsabweichung | 12 | out-basic-site | Rel = alle rel-Werte des Elements in Originalreihenfolge (OUT-031); die Referenz laesst rel="stylesheet" leer. |
| DEV-008 | OUT-031 | OS-Festlegung | 5 | parity-technical | Follow=False auch bei Meta/X-Robots nofollow der Quellseite (OUT-031); die Referenz bewertet nur das Link-Attribut. |
| DEV-009 | OUT-031 | Paritaetsabweichung | 37 | out-basic-site, parity-response, parity-synthetic, parity-technical | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Headern und Weiterleitungen. |
| DEV-010 | TECH-130 | OS-Festlegung | 4 | parity-technical | K-07: Selbstlinks zaehlen im Links-Tab nicht (TECH-130); die Referenz zaehlt sie. Betrifft die Seite mit Selbstlink. |
| DEV-011 | TECH-130 | OS-Festlegung | 8 | out-basic-site | K-07 (wie DEV-010), Seiten mit Selbstlink in out-basic-site. |
| DEV-012 | TECH-051 | OS-Festlegung | 3 | parity-technical | Population des Directives-Tabs: alle internen URLs mit HTTP-Antwort, jeder Status (TECH-051); die Referenz fuehrt Weiterleitungen und Fehlerseiten nicht. |
| DEV-013 | TECH-051 | OS-Festlegung | 5 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 8 | out-basic-site, parity-onpage, parity-response, parity-technical | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fuehrt Antworten ohne Content-Type (z. B. Weiterleitungen) unter Other. Der Filter Other ist P1 (M1.md 7.2). |
| DEV-016 | ONPAGE-166 | OS-Festlegung | 1 | parity-onpage | PDF-Inhalte (Word Count, Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2); in M1 nur Titel und Keywords aus den PDF-Eigenschaften. |
| DEV-017 | OUT-081 | OS-Festlegung | 8 | parity-response | OUT-081: die Hop-Spalten n beschreiben die Spruenge (Redirect Type n, Redirect URL n, Status Code n, Status n); die Referenz haengt als letzten Hop das Endziel an. |
| DEV-018 | ONPAGE-171 | OS-Festlegung | 3 | parity-onpage | IMG Inlinks (OpenSpider-Erweiterung) zaehlt auch Hyperlinks auf Bilder (ONPAGE-170 Nr. 2); die Referenz zaehlt nur eingebettete Bilder. |
| DEV-023 | TECH-064 | OS-Festlegung | 2 | parity-response, parity-technical | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im Response-Codes-Filter Redirection (HTTP Refresh). |
| DEV-024 | ONPAGE-166 | OS-Festlegung | 1 | parity-onpage | PDF-Inhalte (Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2), siehe DEV-016. |
| DEV-025 | TECH-001 | OS-Festlegung | 2 | parity-synthetic | Indexability Status nennt genau den ersten zutreffenden Grund in der Pruefreihenfolge (TECH-001); die Referenz nennt bei noindex und fremdem Canonical beide, kommagetrennt. |
| DEV-026 | TECH-064 | OS-Festlegung | 1 | parity-technical | Die Spalte Meta Refresh im Directives-Tab zeigt auch den Wert des Refresh-Headers (TECH-064: Refresh umfasst Meta und Header); die Referenz laesst sie bei reinem Header-Refresh leer. |
| DEV-027 | TECH-071 | OS-Festlegung | 1 | parity-technical | Population des Hreflang-Tabs: auch gecrawlte hreflang-Ziele (TECH-071); die Referenz fuehrt das umgeleitete Ziel (301) nicht. |
| DEV-028 | ONPAGE-170 | OS-Festlegung | 1 | parity-onpage | Grundmenge des Images-Tabs: auch per Bild-Referenz entdeckte Bild-URLs mit Fehlerantwort (Images-Intro, ONPAGE-170); die Referenz fuehrt das nicht abrufbare Bild (404) nicht. |
| DEV-029 | ONPAGE-170 | OS-Festlegung | 1 | parity-onpage | wie DEV-028: Zaehler des Abschnitts Images enthaelt das per Referenz entdeckte, nicht abrufbare Bild. |
| DEV-030 | OUT-081 | OS-Festlegung | 10 | parity-response | Bei einer Weiterleitungsschleife ist Final Address leer und Final Status = Redirect Loop (OUT-081); die Referenz nennt die letzte Station der Schleife samt ihrem Status. Das gilt auch fuer den Selbst-Refresh (Weiterleitung auf sich selbst, K-57): die Referenz nennt die Seite selbst mit 200/OK. |
| DEV-031 | OUT-081 | OS-Festlegung | 1 | parity-response | Meta Refresh und Refresh-Header gelten nur mit Verzoegerung > 0 als temporaer (OUT-081); die Referenz wertet den Meta Refresh ohne Verzoegerung als temporaer. |
| DEV-034 | TECH-071 | OS-Festlegung | 1 | parity-technical | wie DEV-027: Die Population des Hreflang-Tabs enthaelt auch gecrawlte hreflang-Ziele (TECH-071), hier das umgeleitete Ziel /hl/es-old/ (301); die Referenz zaehlt nur interne Seiten mit 2xx. |
| DEV-035 | TECH-015 | OS-Festlegung | 2 | parity-onpage | K-71: Die Grundgesamtheit der Referenz heisst 'Internal HTML or PDF pages with 2xx response' und enthaelt jede PDF; die Canonicals-Grundmenge von OpenSpider fuehrt Nicht-HTML-Ressourcen nur mit mindestens einer HTTP-Canonical (TECH-011) und nimmt eine PDF ohne Link-Header nicht in Missing auf (TECH-015). PDFs als Seiten sind P1 (ONPAGE-166). parity-onpage hat eine PDF ohne Canonical (/ctype/t-pdf.pdf): Referenz 85/81, OpenSpider 84/80. |
| DEV-036 | TECH-133 | OS-Festlegung | 1 | parity-technical | K-07/K-71 (wie DEV-010): Selbstlinks zaehlen im Links-Tab nicht, eine Seite nur mit Selbstlink hat 0 Outlinks (TECH-130, TECH-133); die Referenz zaehlt sie mit. Betrifft /links/self-only.html: Referenz 1, OpenSpider 2. |

## Referenzeinschränkungen (Free)

| Kennung | Spec | Kennzeichen | Zellen/Zeilen | Sites | Begründung |
|---|---|---|---:|---|---|
| REF-FREE-01 | TECH-210 | Annahme | 110 | parity-technical | Referenz-Free-Einschraenkung (Spider-Konfiguration gesperrt): verlinkte XML-Sitemaps und robots.txt-Sitemaps werden nicht abgerufen; die Referenz listet im Tab Sitemaps dennoch alle internen URLs, OpenSpider ohne verarbeitete Sitemap keine (TECH-211: Tab leer). |
| REF-FREE-02 | TECH-210 | Annahme | 5 | out-basic-site, parity-onpage, parity-response, parity-synthetic, parity-technical | Referenz-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt Sitemaps, OpenSpider 0 (TECH-211). |

