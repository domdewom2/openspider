# Abnahmebericht M1 (M1-P29, Welle 5)

Stand: 2026-10-14 · Umsetzer: Sonnet 5.5 · Branch `m1/m1-p29` (CI-Lauf auf 185dcbd) · Basis `origin/main` 7a0e93a (Merge M1-N15) + `origin/parity/sf-reference` (4bed0f4)  
Kriterien A1–A9: `docs/plan/M1.md` Abschnitt 6. Belege: dieser Bericht, `docs/plan/M1-status.json`, `docs/plan/M1-acceptance/parity/` (Paritätsbericht), CI-Läufe (Links unten).

## 0 Kurzfassung

| Nr. | Kriterium | Ergebnis | Beleg |
|---|---|---|---|
| A1 | Build/Qualität | **erfüllt** | CI-Lauf [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445); main-Lauf [38544701689](https://github.com/domdewom2/openspider/actions/runs/38544701689) |
| A2 | Spec-Abdeckung | **erfüllt** (242/242, 0 open, 0 ungültige Ignores) | `docs/plan/M1-status.json`, Abschnitt A2 |
| A3 | Parität (SF Free) | **nicht erfüllt** – 26 von 38 Vergleichen erfüllen die Schwelle; 12 sind wegen der Befunde N-01 … N-10 offen | `docs/plan/M1-acceptance/parity/`, Abschnitt A3 |
| A4 | Performance | **teilweise** – Klein-Varianten grün; Großläufe (N14) liegen nicht vor | Abschnitt A4 |
| A5 | CLI | **erfüllt** | Abschnitt A5 |
| A6 | Crawl-Datei | **erfüllt** | Abschnitt A6 |
| A7 | MCP | **erfüllt** | Abschnitt A7 |
| A8 | Clean-Room | **erfüllt (mechanische Prüfung)**; Textstichprobe und Referenz-grep-Liste: Fable | Abschnitt A8 |
| A9 | Statusbericht | **erfüllt** (keine ID `open`) | `docs/plan/M1-status.json` |

**Fable-Gate M1 (14.10.2026): needs_work, 70/100.** Der Planer hat am 14.10.2026 die Befunde B-01 … B-05, N-03, N-07 … N-10 und die widerlegten Annahmen entschieden (K-54 … K-65 in `docs/plan/M1.md`; Grundsatz: Parität, außer die Spec enthält eine bewusste, begründete OS-Festlegung). Die Nachbesserungen laufen in Welle 6 (M1-P31, M1-N16, M1-N17); M1-N18 (Welle 6b) wiederholt danach den Paritätslauf und schaltet A3 in der CI scharf. A4 (M1-N14) läuft separat. Die Entscheidungen stehen unten jeweils unter dem Befund.

Offene Punkte für Orchestrator/Planer (Einzelheiten in den Abschnitten): 
- **A3:** zehn Nachbesserungsaufträge N-01 … N-10 (Implementierung) und fünf Entscheidungen B-01 … B-05 (Spec/Voreinstellung vs. Referenz); nach den Nachbesserungen `os-parity run --all` wiederholen und in `.github/workflows/ci.yml` bei Job `parity` das `continue-on-error` entfernen.
- **A4:** `docs/plan/M1-acceptance/loadtest/` und `benchmarks/` (M1-N14) fehlen auf `origin/main` (kein Merge von N14, kein Branch `m1/m1-n14` auf dem Remote): PERF-001/003/005 large, CRAWL-124-Profilvergleich, Fortsetzen mit 1 Mio. Queue und Benchmark-Ignores sind nicht belegt. P29 hat sie nicht neu gemessen (die Maschine lief unter Dauerlast, Load Average 100–360 im Lauf; N14 verlangt < 1–2).
- **Windows-Durchsatz (Befund an den Orchestrator):** In zwei von drei main-Läufen nach P30 liegt `crawler_urls_per_s` von windows-x64 in perf-001 small unter 50 % von macos-arm64 (49,3 % und 46,8 %; 57,5 % im dritten) – Tabelle in A4.
- **Aufräumen (E2):** `origin/parity/sf-reference` kann nach dem Merge von P29 gelöscht werden; die vier `os-testsite`-Prozesse der SF-Exporte (Ports 8902–8905, aus dem Scratchpad des Orchestrator-Laufs) belegten die Paritäts-Ports und wurden von P29 beendet.

## A1 Build/Qualität

CI-Lauf auf dem P29-Branch ([38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445), `workflow_dispatch`): Gesamtergebnis **success** – build (macos-arm64): success (8 min 00 s); build (macos-x64): success (12 min 49 s); build (windows-x64): success (17 min 29 s); cargo-deny (licenses, bans, sources): success (0 min 34 s); crawlfile create (macos-arm64): success (3 min 04 s); crawlfile create (macos-x64): success (8 min 36 s); crawlfile create (windows-x64): success (4 min 48 s); crawlfile load (macos-arm64): success (2 min 20 s); crawlfile load (macos-x64): success (5 min 53 s); crawlfile load (windows-x64): success (4 min 45 s); loadtest-small (macos-arm64): success (1 min 48 s); loadtest-small (macos-x64): success (5 min 18 s); loadtest-small (windows-x64): success (3 min 50 s); parity (macos-arm64): success (3 min 28 s); rustfmt: success (0 min 09 s); spec-coverage: success (10 min 28 s). Laufzeit `Rust tests` (`cargo test --workspace` inkl. Kompilieren, Job-Schritt): macos-arm64 6 min 25 s, macos-x64 9 min 05 s, windows-x64 14 min 06 s (< 15 min); `Clippy` mit `-D warnings` 25–51 s ohne Befund auf allen drei. Neue Jobs dieses Pakets im selben Lauf: `parity`, `crawlfile create`/`crawlfile load` (je 3 Plattformen), `spec-coverage` – alle grün.

Der CI-Lauf auf dem letzten Stand des Branches (Commit 887d779, [38556332688](https://github.com/domdewom2/openspider/actions/runs/38556332688)) ist ebenfalls vollständig grün (alle 16 Jobs, `spec-coverage` mit `--strict` ohne offene ID: implemented 237, verified 5, open 0 – identisch zu `docs/plan/M1-status.json`).

Zum Vergleich der letzte main-Lauf ([38544701689](https://github.com/domdewom2/openspider/actions/runs/38544701689), Merge M1-N15): alle Jobs grün. Laufzeit `Rust tests` (`cargo test --workspace`, inkl. Kompilieren, Job-Schritt): macos-arm64 5 min 54 s, macos-x64 10 min 17 s, windows-x64 14 min 19 s (< 15 min, Windows knapp). Clippy ohne Warnungen auf allen drei (`-D warnings`), `rustfmt` und `cargo-deny` grün. Lokal: `cargo fmt --all --check` und `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` ohne Befund.

## A2 Spec-Abdeckung

`os-parity coverage` mit eingelesenen Testergebnissen (`cargo test --workspace --exclude openspider-desktop --no-fail-fast --locked > out.txt 2>&1`, Stand dieses Branches): **242 IDs – implemented 237, verified 5, deferred 0, open 0**; Zusatz-IDs (`extra`, nicht gewertet): 6; ignorierte Tests ohne gültigen Grund: 0; Warnungen: 0.

Stichprobe der dünn belegten IDs (Nachtrag Welle-1-Sichtung):

| ID | Status | Tests mit Ergebnis ok |
|---|---|---|
| OUT-231 | implemented | 4: `paths_registry.rs::default_crawl_folder_is_the_one_defined_in_out_227`, `paths_registry.rs::fixture_crawl_in_openspider_home_is_listed_found_and_opened_from_the_crawl_folder`, `paths_registry.rs::changing_the_storage_location_leaves_existing_crawls_in_place`, `paths_registry.rs::default_crawl_folder_is_separate_from_the_override_and_never_inside_it` |
| ONPAGE-006 | implemented | 1: `issues_catalog.rs::end_to_end_samples` |

Mit Paritätsbeleg (`verified`, Regeln und Belegzuordnung in `tests/parity/evidence.toml`, nur IDs, deren Spec-Umfang die verglichenen Spalten vollständig abdecken und die ohne Referenzeinschränkung SF Free belegt sind): **5** – ONPAGE-012, ONPAGE-120, ONPAGE-130, TECH-010, TECH-011.

## A3 Parität (SF 24.3 Free)

Referenz: SF 24.3 Free (macOS arm64, englische Oberfläche), exportiert am 14.10.2026; **5 von 5 Sites mit Status `present`** (Anforderung ≥ 4), 39 Exporte laut Manifest, davon 38 verglichen und 1 übersprungen. Lauf: `os-parity run --all --openspider target/debug/openspider --out target/parity` (Bericht: `docs/plan/M1-acceptance/parity/report.md`, `report.json`).

Ergebnis: **26 von 38 Vergleichen erfüllen die Schwelle aus A3** (URL-Menge identisch nach dokumentierten Abweichungen, Zellübereinstimmung ≥ Schwelle). Die übrigen 12 Vergleiche verfehlen sie wegen der offenen Befunde N-01 … N-10 (Abschnitt „Nachbesserungsaufträge“); ihre Zellen sind absichtlich nicht als Abweichung entschuldigt.

| Site | Export | Zellen | Übereinstimmung | Schwelle | URL-Menge | Ergebnis |
|---|---|---:|---:|---:|---|---|
| out-basic-site | internal_all | 899 | 98.443 % | 99 % | identisch | **offen** |
| out-basic-site | external_all | 5 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | response_codes_all | 152 | 100.000 % | 99 % | identisch | ok |
| out-basic-site | links_all | 100 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | all_inlinks | 1067 | 98.313 % | 97 % | identisch | ok |
| out-basic-site | redirect_chains | 0 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | crawl_overview | 112 | 100.000 % | 100 % | identisch | ok |
| parity-onpage | internal_all | 4761 | 99.286 % | 99 % | identisch | ok |
| parity-onpage | external_all | 44 | 97.727 % | 97 % | identisch | ok |
| parity-onpage | response_codes_all | 911 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | url_all | 945 | 97.778 % | 97 % | identisch | ok |
| parity-onpage | page_titles_all | 756 | 77.778 % | 99 % | identisch | **offen** |
| parity-onpage | meta_description_all | 756 | 77.778 % | 99 % | identisch | **offen** |
| parity-onpage | h1_all | 588 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | h2_all | 588 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | content_all | 336 | 97.321 % | 97 % | identisch | ok |
| parity-onpage | images_all | 57 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | all_inlinks | 3322 | 99.880 % | 97 % | identisch | ok |
| parity-onpage | crawl_overview | 103 | 100.000 % | 100 % | identisch | ok |
| parity-response | internal_all | 2159 | 95.692 % | 99 % | identisch | **offen** |
| parity-response | external_all | 24 | 95.833 % | 97 % | identisch | **offen** |
| parity-response | response_codes_all | 407 | 99.509 % | 99 % | identisch | ok |
| parity-response | all_inlinks | 1052 | 99.240 % | 97 % | abweichend | **offen** |
| parity-response | all_error_inlinks | 212 | 97.642 % | 97 % | identisch | ok |
| parity-response | redirect_chains | 236 | 100.000 % | 97 % | abweichend | **offen** |
| parity-response | redirects_to_errors | – | – | – | – | übersprungen: außerhalb des M1-Umfangs: Report Redirects to Errors (OUT-084) ist P1 und kein M1-Umfang (M1.md 7.2); OpenSpider hat in M1 nur Redirects:All Redirects und Redirects:Redirect Chains |
| parity-response | crawl_overview | 100 | 95.000 % | 100 % | identisch | **offen** |
| parity-synthetic | internal_all | 20722 | 97.548 % | 99 % | identisch | **offen** |
| parity-synthetic | links_all | 3410 | 100.000 % | 97 % | identisch | ok |
| parity-synthetic | all_inlinks | 50105 | 98.487 % | 97 % | identisch | ok |
| parity-synthetic | crawl_overview | 113 | 100.000 % | 100 % | identisch | ok |
| parity-technical | internal_all | 6070 | 99.572 % | 99 % | abweichend | **offen** |
| parity-technical | canonicals_all | 1272 | 100.000 % | 99 % | identisch | ok |
| parity-technical | directives_all | 855 | 100.000 % | 99 % | identisch | ok |
| parity-technical | hreflang_all | 1696 | 100.000 % | 97 % | identisch | ok |
| parity-technical | links_all | 950 | 100.000 % | 97 % | identisch | ok |
| parity-technical | sitemaps_all | 0 | 100.000 % | 97 % | identisch | ok |
| parity-technical | all_inlinks | 4837 | 98.925 % | 97 % | abweichend | **offen** |
| parity-technical | crawl_overview | 112 | 93.750 % | 100 % | identisch | **offen** |

Hinweise zur Aussagekraft:

- Die Zellübereinstimmung bezieht sich auf die verglichenen Zellen **nach** den dokumentierten Abweichungen (2652 entschuldigte Zellen/Zeilen insgesamt, Tabellen unten); offene Befunde sind nicht entschuldigt.
- `redirect_chains` bei `out-basic-site` vergleicht 0 Zellen: beide Seiten haben keine Kette mit mindestens zwei Sprüngen.
- `Redirects to Error(s)` (OUT-084, P1) gehört nicht zum M1-Umfang und ist im Manifest mit `skip` übersprungen; `All Error Inlinks` wird über `Response Codes:Internal & External:All Error Inlinks` verglichen.
- Der CI-Job `parity` (macos-14, Lauf [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445)) liefert für alle 38 Vergleiche dieselben Zellzahlen und Treffer wie der lokale Lauf; solange Befunde offen sind, endet `os-parity run --all` mit Exit-Code 1, der Schritt steht deshalb auf `continue-on-error` (Bericht als Artefakt `parity-report` und in der Job-Zusammenfassung).
- Die Schwellen gelten je Export und Site (nicht gepoolt); die Schwelle für `crawl_overview` ist 100 % (Zähler der M1-Filter identisch), die `% of Total`-Spalte ist abgeleitet und wird nicht verglichen.

### Begründete Paritätsabweichungen (`tests/parity/deviations.toml`, DEV-nnn)

Jede steht mit Spec-ID und Kennzeichen in der Datei; „Anzahl“ = entschuldigte Zellen bzw. Zeilen im Lauf.

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-001 | CRAWL-098 | OS-Festlegung | 10 | K-40: Die robots.txt eines internen Hosts erscheint als eigene URL-Zeile in den Tabellen (200, text/plain). Die Referenz listet sie nicht. Offene Planerentscheidung: Festlegung beibehalten oder in_tables=0 fuer interne robots.txt (siehe Abnahmebericht, Befund B-01). |
| DEV-002 | CRAWL-098 | OS-Festlegung | 48 | Folge von DEV-001: Die robots.txt-Zeile zaehlt in allen Zaehlern, deren Grundgesamtheit alle internen URLs mit Antwort ist, um genau 1 hoeher als in der Referenz. |
| DEV-003 | ONPAGE-011 | OS-Festlegung | 2230 | Nicht-HTML-URLs haben leere Element-Spalten (ONPAGE-011) und eine Seite ohne Element leere Wertspalten (ONPAGE-008); die Referenz schreibt 0. |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 61 | Nicht-HTML-URLs: Word Count und Text Ratio leer (ONPAGE-013); die Referenz schreibt 0. |
| DEV-005 | ONPAGE-065 | Paritaetsabweichung | 9 | OS-Festlegung ONPAGE-065 unterscheidet Connection Refused (abgelehnte Verbindung) von Connection Timeout; die Referenz meldet fuer den geschlossenen Port localhost:1 Connection Timeout. |
| DEV-006 | OUT-031 | Paritaetsabweichung | 7 | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz schreibt 0 fuer robots.txt-blockierte Ziele. |
| DEV-006b | OUT-031 | Paritaetsabweichung | 2 | wie DEV-006 (Fehler-Inlinks auf nicht abgerufene Ziele). |
| DEV-007 | OUT-031 | Paritaetsabweichung | 12 | Rel = alle rel-Werte des Elements in Originalreihenfolge (OUT-031); die Referenz laesst rel="stylesheet" leer. |
| DEV-008 | OUT-031 | OS-Festlegung | 5 | Follow=False auch bei Meta/X-Robots nofollow der Quellseite (OUT-031); die Referenz bewertet nur das Link-Attribut. |
| DEV-009 | OUT-031 | Paritaetsabweichung | 34 | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Headern und Weiterleitungen. |
| DEV-010 | TECH-130 | OS-Festlegung | 4 | K-07: Selbstlinks zaehlen im Links-Tab nicht (TECH-130); die Referenz zaehlt sie. Betrifft die Seite mit Selbstlink. |
| DEV-011 | TECH-130 | OS-Festlegung | 8 | K-07 (wie DEV-010), Seiten mit Selbstlink in out-basic-site. |
| DEV-012 | TECH-051 | OS-Festlegung | 3 | Population des Directives-Tabs: alle internen URLs mit HTTP-Antwort, jeder Status (TECH-051); die Referenz fuehrt Weiterleitungen und Fehlerseiten nicht. |
| DEV-013 | TECH-051 | OS-Festlegung | 5 | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 6 | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fuehrt Antworten ohne Content-Type (z. B. Weiterleitungen) unter Other. Der Filter Other ist P1 (M1.md 7.2). |
| DEV-015 | OUT-080 | Paritaetsabweichung | 13 | Die Crawl-Depth-Zeilen der Referenz zaehlen nur interne HTML-Seiten mit 2xx (Zeile 0 = Start-URL); OpenSpider zaehlt alle internen URLs mit bekannter Tiefe (OUT-025 Site Structure, Summe = interne URLs). |
| DEV-016 | ONPAGE-166 | OS-Festlegung | 1 | PDF-Inhalte (Word Count, Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2); in M1 nur Titel und Keywords aus den PDF-Eigenschaften. |
| DEV-017 | OUT-081 | OS-Festlegung | 8 | OUT-081: die Hop-Spalten n beschreiben die Spruenge (Redirect Type n, Redirect URL n, Status Code n, Status n); die Referenz haengt als letzten Hop das Endziel an. |
| DEV-018 | ONPAGE-171 | OS-Festlegung | 3 | IMG Inlinks (OpenSpider-Erweiterung) zaehlt auch Hyperlinks auf Bilder (ONPAGE-170 Nr. 2); die Referenz zaehlt nur eingebettete Bilder. |
| DEV-019 | ONPAGE-005 | Annahme | 2 | Annahme A-4 (Duplikate exakt, Gross-/Kleinschreibung sensitiv) ist durch die Referenz widerlegt: Titel/Meta Description die sich nur in der Schreibweise unterscheiden zaehlen dort als Duplikat (Fixture dup-case.html). Spec-Korrektur offen (Abnahmebericht). |
| DEV-020 | ONPAGE-072 | Annahme | 1 | Annahme A-23 (Underscore nur im Pfad) ist durch die Referenz widerlegt: sort_by in der Query zaehlt dort mit. Spec-Korrektur offen (Abnahmebericht). |
| DEV-021 | TECH-011 | OS-Festlegung | 6 | Die Tabs Canonicals, Links und Hreflang fuehren interne HTML-Seiten mit Status 200 (TECH-011/-071/-130); die Referenz zaehlt alle 2xx (Fixture /s201, Status 201). |
| DEV-022 | CRAWL-089 | OS-Festlegung | 5 | Der Wirkungsbereich von issues.ignore_non_indexable ist in CRAWL-089 abschliessend auf die Element-Tabs und Low Content Pages begrenzt; die Referenz schliesst nicht indexierbare URLs auch in Canonicals/Missing und Links/Pages Without Internal Outlinks aus. |
| DEV-023 | TECH-064 | OS-Festlegung | 2 | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im Response-Codes-Filter Redirection (HTTP Refresh). |
| DEV-024 | ONPAGE-166 | OS-Festlegung | 1 | PDF-Inhalte (Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2), siehe DEV-016. |
| DEV-025 | TECH-001 | OS-Festlegung | 2 | Indexability Status nennt genau den ersten zutreffenden Grund in der Pruefreihenfolge (TECH-001); die Referenz nennt bei noindex und fremdem Canonical beide, kommagetrennt. |
| DEV-026 | TECH-064 | OS-Festlegung | 1 | Die Spalte Meta Refresh im Directives-Tab zeigt auch den Wert des Refresh-Headers (TECH-064: Refresh umfasst Meta und Header); die Referenz laesst sie bei reinem Header-Refresh leer. |
| DEV-027 | TECH-071 | OS-Festlegung | 1 | Population des Hreflang-Tabs: auch gecrawlte hreflang-Ziele (TECH-071); die Referenz fuehrt das umgeleitete Ziel (301) nicht. |
| DEV-028 | ONPAGE-170 | OS-Festlegung | 1 | Grundmenge des Images-Tabs: auch per Bild-Referenz entdeckte Bild-URLs mit Fehlerantwort (Images-Intro, ONPAGE-170); die Referenz fuehrt das nicht abrufbare Bild (404) nicht. |
| DEV-029 | ONPAGE-170 | OS-Festlegung | 1 | wie DEV-028: Zaehler des Abschnitts Images enthaelt das per Referenz entdeckte, nicht abrufbare Bild. |
| DEV-030 | OUT-081 | OS-Festlegung | 5 | Bei einer Weiterleitungsschleife ist Final Address leer und Final Status = Redirect Loop (OUT-081); die Referenz nennt die letzte Station der Schleife samt ihrem Status. |
| DEV-031 | OUT-081 | OS-Festlegung | 1 | Meta Refresh und Refresh-Header gelten nur mit Verzoegerung > 0 als temporaer (OUT-081); die Referenz wertet den Meta Refresh ohne Verzoegerung als temporaer. |
| DEV-032 | ONPAGE-015 | OS-Festlegung | 39 | % of Total teilt durch alle gecrawlten internen HTML-Seiten mit Status 200 (ONPAGE-015); die Referenz teilt durch alle HTML-Seiten mit 2xx (Fixture /s201, Status 201: 14 statt 13 Seiten). |

### Referenzeinschränkungen SF Free (`SF-FREE-nn`, K-53) – zählen nicht als Paritätsabweichung

Die Free-Version sperrt die Spider-Konfiguration: Hreflang und Pagination wurden nur gespeichert, verlinkte XML-Sitemaps nicht gecrawlt, robots.txt-Sitemaps nicht abgerufen. `tests/parity/profiles/parity.osconfig` bildet das nach (`spider.hreflang.crawl = false`, `sitemaps.crawl_linked = false`).

| Kennung | Spec | Anzahl | Begründung |
|---|---|---:|---|
| SF-FREE-01 | TECH-210 | 110 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): verlinkte XML-Sitemaps und robots.txt-Sitemaps werden nicht abgerufen; die Referenz listet im Tab Sitemaps dennoch alle internen URLs, OpenSpider ohne verarbeitete Sitemap keine (TECH-211: Tab leer). |
| SF-FREE-02 | TECH-210 | 5 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt Sitemaps, OpenSpider 0 (TECH-211). |

Betroffene Filter (Sitemaps-Tab und -Filter wie Orphan URLs / URLs not in Sitemap / Non-Indexable URLs in Sitemap, Hreflang-Filter Non-200 Hreflang URLs / Missing Return Links) erhalten **keinen** Status `verified`; ihr Beleg bleibt der spec-abgeleitete Fixture-Test (A2).

### Weitere Profilangleichungen an die tatsächliche Referenzkonfiguration

Beide Änderungen folgen dem beobachteten Verhalten der Referenz (nicht der OpenSpider-Voreinstellung); beide sind Entscheidungen für den Planer (Befunde B-02, B-03):

- `robots.unreachable_policy = "allow_all"`: Die Referenz crawlt URLs hinter nicht erreichbarer robots.txt (geschlossener Port `localhost:1`) trotzdem und meldet `Connection Timeout`; die OpenSpider-Voreinstellung „Host sperren“ ergibt `Blocked by Robots.txt (robots.txt unreachable)` (CRAWL-100, Q30: beantwortet).
- `advanced.respect_self_ref_meta_refresh = false`: Die Referenz wertet Selbst-Refresh-Seiten nicht als nicht indexierbar (TECH-068; OpenSpider-Voreinstellung `true`).

## Befunde und Nachbesserungsaufträge aus dem Paritätslauf

Klassifizierung nach `docs/plan/M1-packages.json` (P29): *Implementierungsfehler* → Nachbesserungsauftrag (kein Fix außerhalb des Besitzes von P29, keine Ausnahme in `deviations.toml`); *Spec-Abweichung/Annahme/OS-Festlegung* → Eintrag in `deviations.toml` (siehe oben). Alle Beispiele sind mit `os-parity run --site <site> --openspider target/debug/openspider` reproduzierbar; „Zellen“ = abweichende Zellen im Lauf.

### Nachbesserungsaufträge (Implementierungsfehler bzw. offene Abweichungen)

**N-01 · Fehlerseiten (4xx/5xx) liefern Inhaltswerte** (Priorität hoch) – Spec ONPAGE-009 (A-5: Inhalt von Fehlerseiten wird nicht ausgewertet), ONPAGE-011 (Internal-Werte = Werte der Element-Tabs), ONPAGE-013. Crate: `os-parse`/`os-engine` (Extraktion nur bei 2xx) oder `os-analyze` `tabs/internal.rs` (Spalten nur bei 2xx); Ursachenort nicht untersucht.
Minimales Beispiel: Fixture `parity-response`, `/e410` (Status 410, Body mit `<title>Error 410</title>` und `<h1>Error 410</h1>`): `Internal:All` liefert `Title 1` = „Error 410“, `Title 1 Length` 9, `Title 1 Pixel Width` 83, `H1-1` = „Error 410“, `H1-1 Length` 9, `Word Count` 2, `Text Ratio` 6.9; die Referenz lässt Titel/H1 leer und schreibt 0.
Umfang: 24 Seiten in 4 Sites (`out-basic-site` `/missing.html`; `parity-response` `/e400 … /e503`, `/s999`; `parity-synthetic` 14 Seiten; `parity-technical` `/canon/gone-404.html`), je 24 Zellen in den Spalten Title 1, Title 1 Length, H1-1, H1-1 Length, Text Ratio, dazu Pixel Width und Word Count. Das ist der häufigste Grund, warum `internal_all` bei `out-basic-site`, `parity-response`, `parity-synthetic` und `parity-technical` unter 99 % bleibt.

**N-02 · Page-Titles- und Meta-Description-Export ohne Pixel-Width-Spalten** – Spec ONPAGE-090 und ONPAGE-100 (Spaltenlisten enthalten `Title 1/2 Pixel Width` bzw. `Meta Description 1/2 Pixel Width`), UI-038. Datei: `crates/os-analyze/src/tabs/page_titles.rs` (Z. 30–50) und `meta_description.rs`: die vier Pixel-Spalten sind mit `default_visible = false` definiert, der Standardexport lässt sie weg (Internal:All zeigt sie).
Minimales Beispiel: `openspider --crawl http://127.0.0.1:8902/ --export-tabs "Page Titles:All"` (Fixture `parity-onpage`) → die Kopfzeile hat keine `Title 1 Pixel Width`; die Referenz hat sie (Wert z. B. 332 für `/`). 336 Zellen (84 Zeilen × 4 Spalten); `page_titles_all` und `meta_description_all` stehen deshalb bei 77,8 %.

**N-03 · Word Count weicht bei Seiten mit Links ab** (Priorität hoch) – Spec ONPAGE-141 (Tokenisierung an Unicode-Leerraum, A-35), ONPAGE-013. Beobachtung: Die Referenz zählt auf jeder geprüften Seite genau so viele Wörter mehr, wie die Seite Link-Elemente (`<a>`) im Inhaltsbereich hat – nicht so viele, wie Wörter in den Links stehen. `parity-synthetic` `/s1/p1.html` (H1 „Seite 1“ = 2 Wörter, Absatz 90 Wörter, 10 × `<li><a>Seite n</a></li>` = 20 Wörter): OpenSpider 112 (= 90 + 2 + 20, nach Spec korrekt), Referenz 122; `parity-onpage` `/` (8 Links mit zusammen 11 Wörtern): 79 gegen 87; `parity-technical` `/dir/` (13 Links in `<main>`, 2 in `<nav>`): 81 gegen 94 (Differenz 13); `parity-response` `/`: 191 gegen 234 (43 Links).
Umfang: 430 Zellen (`parity-synthetic` 393 von 393 HTML-Seiten), `Word Count` in `internal_all` und `content_all`, dazu Crawl Overview Content › Low Content Pages (parity-response 9 statt 8). Entscheidung: Spec/Annahme A-35 ändern oder die Abweichung als OS-Festlegung bestätigen (dann Eintrag in `deviations.toml`).
**→ Entschieden 14.10.2026 (K-59): Parität, Annahme A-35b (+1 je `<a href>` im Inhaltsbereich), ONPAGE-141 ergänzt (M1-P31).**

**N-04 · Pixelbreiten außerhalb der Toleranz** (UI-038: ≤ 3 %) – 39 Zellen in `internal_all` (`Title 1 Pixel Width` 28, `Meta Description 1 Pixel Width` 11): `parity-onpage` `/titles/pixel-i70.html` (70 × „i“, 20 px): Referenz 350, OpenSpider 311 (−11,1 %); `/titles/pixel-W60.html` (60 × „W“): 1080 gegen 1133 (+4,9 %); `/meta/dup-case.html` (Meta Description): 735 gegen 802; `out-basic-site` `/faq.html` (Titel): 142 gegen 148 (+4,2 %); `parity-response` `/meta-refresh-self.html`: 246 gegen 238; `parity-synthetic` Meta Description der 10 Seiten `/s*/p*.html`: 151 gegen 156 (+3,3 %). Auffällig: Die Referenzbreite der beiden Wiederholungsfixtures ist ein Vielfaches ganzer Pixel je Zeichen (i = 5 px, W = 18 px; OpenSpider 4,44 und 18,88 px). Prüfauftrag: Schriftprofil und Rundung von UI-038 (⚖ Annahmen) gegen die Golden-Werte der Referenz nachziehen.

**N-05 · Folder Depth bei doppeltem Schrägstrich** – Spec ONPAGE-014 / A-8: `/urls/a//b.html` → Referenz 2, OpenSpider 3 (das leere Segment zählt mit). 1 Zelle (`parity-onpage`). Minimal: leere Pfadsegmente bei der Zählung überspringen.

**N-06 · Hash für Nicht-HTML und leere Antworten** – Spec ONPAGE-018 („MD5 … der HTML-Seite“). OpenSpider schreibt `Hash` für jede abgerufene URL (Bilder, CSS, JS, Text, SVG, JSON, 301-Antworten mit leerem Körper: `d41d8cd98f00b204e9800998ecf8427e`); die Referenz nur für HTML-Seiten. Beispiel: `out-basic-site` `/assets/logo.png`, `/old-page` (301). 111 Zellen in `internal_all` und 21 in `url_all`. (Die Referenz schreibt MD5-Werte mit führender 0 um die Null verkürzt – 31 statt 32 Zeichen; das ist reine Darstellung und im Mapping per `hex_trim_zeros` neutralisiert, kein Befund.)

**N-07 · Selbstreferenzierende und leere Meta-Refresh-Seiten** (Profil `respect_self_ref_meta_refresh = false` wie die Referenz) – Spec TECH-068, ONPAGE-018, OUT-031, OUT-081. Seiten `parity-response` `/meta-refresh-self.html` (`content="30"`), `/http-refresh-nourl.html`, `parity-technical` `/dir/refresh-bare.html` (`content="60"`): OpenSpider lässt `Redirect URL` und `Redirect Type` leer, legt keine Link-Zeile `Meta Refresh` (Quelle = Ziel) in `Links:All Inlinks` an und führt die Seite nicht in `Redirects:Redirect Chains`; die Referenz schreibt `Redirect URL` = eigene Adresse, `Redirect Type` = `Meta Refresh Redirect`, die Inlinks-Zeile und eine Redirect-Chains-Zeile (Loop). Folgen im Crawl Overview: Response Codes › Redirection (Meta Refresh) 4 statt 5 (response) bzw. 1 statt 2 (technical), Internal Redirect Chain 7 statt 9, Redirect Loop 2 statt 3 bzw. 0 statt 1. Zellen: Redirect URL/Type 2 + 2, Inlinks 2 Zeilen, Redirect Chains 1 Zeile, Crawl Overview 5 + 2 Zähler.
**→ Entschieden 14.10.2026 (K-57): Parität (M1-P31).**

**N-08 · Nur über Hreflang entdeckte URLs bei `spider.hreflang.crawl = false`** – Spec TECH-071 / CRAWL-047-Konfiguration. `parity-technical` `/hl/it/` (Status 404, nur per `<link rel="alternate" hreflang>` verlinkt): OpenSpider listet die URL in `Internal:All` als Zeile ohne Status und führt die Hreflang-Link-Zeile (`Hreflang | /hl/en/ | /hl/it/`) in `Links:All Inlinks`; die Referenz kennt weder die Zeile noch die Link-Zeile. Folge: Crawl Overview Total URLs Encountered 118 statt 116, Internal All/Security All/HTTP URLs 113 statt 111 (neben DEV-002). 1 Zeile `internal_all`, 1 Inlinks-Zeile, 7 Zähler.
**→ Entschieden 14.10.2026 (K-62): Parität (M1-P31).**

**N-09 · Link Position und Link Path für Links im `<head>`** – Spec OUT-031 (kennt die Position `Head` nicht). Die Referenz schreibt für `<link>`/`<meta>` im Kopf `Link Position` = `Head` und `Link Path` = `//head/link[n]` bzw. `//head/meta[n]`; OpenSpider schreibt für Canonical, Hreflang und Meta Refresh beide Spalten leer und für CSS/JavaScript im Kopf `Link Position` = `Content`. Beispiel: `out-basic-site` `Canonical | / | /`, `CSS | / | /assets/site.css`. 426 Zellen `Link Position`, 414 `Link Path` (alle Sites). Aufgabe: Spec um `Head` ergänzen und `os-parse` Link-Position/-Pfad für Kopfelemente setzen. (Body-Links stimmen: `Navigation`/`Content`/`Footer` und der Pfad als Elementfolge sind in allen Sites gleich.)
**→ Entschieden 14.10.2026 (K-63): Parität, `Head` und Kopfpfad (M1-P31, `os-model`-Nachtrag 3.13).**

**N-10 · Externe `Inlinks` zählen Redirect- und Bild-Verweise** – Spec ONPAGE-025/-026 („Anzahl Link-Instanzen von internen Seiten“, Linktypen nicht festgelegt). `parity-response` `http://localhost:1/`: Referenz 1 (ein Hyperlink), OpenSpider 2 (Hyperlink + HTTP-Redirect von `/r-to-dead`); `parity-onpage` `{EXT}/img/size-102401.png`: Referenz 0, OpenSpider 1 (Bild-Link). 2 Zellen. Aufgabe: Spec präzisieren (nur Hyperlinks) und Zählung anpassen.
**→ Entschieden 14.10.2026 (K-61): nur Hyperlinks, ONPAGE-025 präzisiert (M1-N16).**

### Entscheidungsbedarf (Spec/Annahme vs. Referenz)

**B-01 · robots.txt als Tabellenzeile (K-40, CRAWL-098)** – Die Referenz listet die robots.txt des internen Hosts in keinem Tab; OpenSpider führt sie als URL-Zeile (200, `text/plain`, Klasse Unknown) in Internal, Response Codes, URL, Directives. Das verschiebt alle Gesamtzähler im Crawl Overview um 1 und erzeugt je Export eine Zusatzzeile. Als `DEV-001`/`DEV-002` dokumentiert (OS-Festlegung); der Paritätslauf bestätigt die K-40-Annahme **nicht**. Entscheidung: `in_tables = 0` für die robots.txt interner Hosts (Referenzverhalten) oder Festlegung beibehalten.
**→ Entschieden 14.10.2026 (K-54): Parität, keine Tabellen-Sichtbarkeit; DEV-001/-002 entfallen (M1-P31).**

**B-02 · Verhalten bei nicht erreichbarer robots.txt (CRAWL-100, Q30 beantwortet)** – Die Referenz crawlt URLs eines Hosts, dessen robots.txt nicht erreichbar ist (`http://localhost:1/`, Verbindung abgelehnt), trotzdem und meldet `Connection Timeout`/`No Response`; das entspricht der OpenSpider-Policy „Alles erlauben“ (`robots.unreachable_policy = allow_all`), nicht der Voreinstellung „Host sperren“. Das Paritätsprofil nutzt `allow_all`. Entscheidung: Voreinstellung ändern oder bewusst abweichen (CRAWL-100 nennt das Risiko selbst).
**→ Entschieden 14.10.2026 (K-55): bewusste OS-Festlegung bleibt (RFC 9309, Abschnitt 2.3.1.4); das Paritätsprofil nutzt die dafür vorgesehene Option `allow_all`, kein DEV-Eintrag.**

**B-03 · „Respect Self Referencing Meta Refresh“ (TECH-068)** – Die Referenz (Free-Standard) wertet Selbst-Refresh-Seiten nicht als nicht indexierbar; OpenSpider-Voreinstellung `true`. Das Paritätsprofil setzt `false`. Entscheidung: Voreinstellung für Parität anpassen oder beibehalten.
**→ Entschieden 14.10.2026 (K-57): Standard `false` wie die Referenz; Selbst-Refresh wird als Weiterleitung auf sich selbst geführt (N-07; M1-P31).**

**B-04 · Statusklassen und Populationen** – Die Tabs Canonicals/Links/Hreflang und der Nenner von `% of Total` verwenden Status 200 (TECH-011/-071/-130, ONPAGE-015); die Referenz zählt alle 2xx (Fixture `/s201`): `DEV-021`, `DEV-032`. Entscheidung: auf 2xx erweitern oder Festlegung bestätigen.
**→ Entschieden 14.10.2026 (K-56): 2xx; DEV-021/-032 entfallen (Canonicals/Links: M1-N16; Hreflang und `% of Total`: M1-P31).**

**B-05 · Wirkungsbereich von `issues.ignore_non_indexable` (CRAWL-089)** – Die Referenz schließt nicht indexierbare URLs auch in Canonicals › Missing und Links › Pages Without Internal Outlinks aus (parity-response: 9 gegen 13 bzw. 3 gegen 7; parity-technical: 86 gegen 94 bzw. 1 gegen 2); CRAWL-089 begrenzt den Wirkungsbereich abschließend auf Element-Tabs und Low Content Pages: `DEV-022`.
**→ Entschieden 14.10.2026 (K-56): Wirkungsbereich um TECH-015 und TECH-133 erweitert; DEV-022 entfällt (M1-N16).**

### Spec-Annahmen, die die Referenz widerlegt oder bestätigt

**→ Entschieden 14.10.2026:** A-4, A-23 und Crawl Depth folgen der Referenz (K-58, M1-P31; DEV-019/-020/-015 entfallen). Die Lücken TECH-130 (`Inlinks`) und TECH-051/TECH-071 (Indexability-Spalten) gehen nach M2 (`docs/plan/M2-backlog.md`).

- **widerlegt, A-4** (Duplikate exakt, Groß-/Kleinschreibung sensitiv, ONPAGE-005): Titel/Meta Description, die sich nur in der Schreibweise unterscheiden, zählt die Referenz als Duplikat (`parity-onpage` `dup-case.html`; Crawl Overview Page Titles › Duplicate 10 statt 9, Meta Description › Duplicate 10 statt 9) – `DEV-019`.
- **widerlegt, A-23** (Underscores nur im Pfad, ONPAGE-072): `?sort_by=x` zählt in der Referenz mit (URL › Underscores 2 statt 1) – `DEV-020`.
- **widerlegt, Crawl Depth Zeile 0** (OUT-025/OUT-080: „die Referenz beginnt bei 1“): Der Crawl Overview der Referenz beginnt bei Tiefe **0**; die Population dort sind interne HTML-Seiten mit 2xx, OpenSpider zählt alle internen URLs – `DEV-015`.
- **widerlegt, TECH-130** („`Inlinks` steht nicht in der Referenz-Spaltenliste des Links-Tabs“): Der Export der Referenz enthält `Inlinks` im Tab Links; OpenSpider blendet die Spalte im Export aus (Mapping ignoriert sie, Befund für die Spec).
- **Spec-Lücke, TECH-051/TECH-071:** Die Referenz exportiert `Indexability` und `Indexability Status` auch im Directives- und im Hreflang-Tab; die Spaltenlisten der Spec nennen sie nicht (Mapping ignoriert sie mit Grund).
- **bestätigt, K-02:** `Transferred (Bytes)` und `Total Transferred (Bytes)` stimmen in allen fünf Sites zellgleich überein (kein Header-Anteil in `Transferred`); `Size (Bytes)` ebenso – ONPAGE-012 ist `verified`.
- **bestätigt, K-44/K-45 (soweit in den Paritäts-Sites enthalten):** Response-Codes-Status/Indexability der Fixtures stimmen bis auf N-07 und `DEV-005` überein; `dir/none.html` = `Non-Indexable`/`noindex`, `canon/none.html` = `Indexable` wie in der Referenz (N12). `ext-noindex.html` gehört nicht zu den fünf Paritäts-Sites und ist daher nicht gegen die Referenz prüfbar. Externe Redirect-Ketten/-Schleifen sind leer (K-44), die Referenz hat in den Sites keine.
- **bestätigt, K-50:** Der Dateiname `links_all_inlinks.csv` wird im Runner über `OsExport::file_candidates` gefunden; der Name der Referenzdatei (`all_inlinks.csv`) steht im Manifest (`sf_file`).
- **bestätigt, N09 (Hreflang-Mehrfachspalten):** Die Spaltenfolge `HTML hreflang n`, `HTML hreflang n URL` (`ColumnSql::MultiPaired`) deckt sich mit der Referenz; `hreflang_all` stimmt zellgleich überein (1696 von 1696 Zellen). Dass os-export die URL-Spalten als Text statt `ColumnKind::Url` behandelt, beeinträchtigt die Parität nicht – kein Meldebedarf.
- **Referenzformat (K-53):** Die Referenz schreibt nummerierte Spalten nur bei vorhandenen Daten, Dezimalkomma (`91,670`) und im Crawl Overview ein Blockformat mit `%`-Zeichen und Dezimalkomma; sie schreibt `Last Modified`, `Content Type`, `Size (Bytes)` und `Transferred (Bytes)` mit anderen Namen als die Spec-Spalten (Mappings).
- **Nicht im M1-Umfang:** Der Export `Redirects to Error(s)` (OUT-084, P1) und der Bulk-Export `All Error Inlinks` unter dem Pfad „Response Codes:All Error Inlinks“ (OUT-067) – letzterer wird über `Response Codes:Internal & External:All Error Inlinks` verglichen (97,6 %).

## A4 Performance

**Klein-Varianten (10 000 URLs), grün in jeder CI:** Job `loadtest-small` auf allen drei Plattformen (PERF-001/003/005/009 und die Server-Messungen). Berichte des P29-Laufs: `docs/plan/M1-acceptance/ci-loadtest-small/loadtest-small-<plattform>.json`.

| Plattform | PERF-001 small: Spitzen-RSS | URLs/s | PERF-003 small: Spitzen-RSS | URLs/s | Ergebnis |
|---|---:|---:|---:|---:|---|
| macos-arm64 | 114,1 MiB | 3.320,9 | 109,4 MiB | 2.358,4 | grün |
| macos-x64 | 103,7 MiB | 1.199,8 | 104,1 MiB | 1.267,9 | grün |
| windows-x64 | 102,8 MiB | 2.022,9 | 99,1 MiB | 1.840,2 | grün |

**PERF-003 (K-52):** „100 ms je Batch“ gilt wörtlich (`SlowSink` wartet 100 ms je Schreibaufruf der Writer-Stufe, auch bei Gruppen-Commit); Kriterium bleibt RSS < 500 MB und reguläres Crawl-Ende. In der Klein-Variante liegt die Spitze bei ≈ 105–115 MB und der Crawl endet regulär. Das Drosseln selbst belegt `crates/os-engine/src/writer_thread.rs` › `full_channel_blocks_the_sender_until_the_writer_catches_up` (Tag PERF-003). Batchzahl, Kanalfüllstand und Wartezeit aus dem diag-Block berichtet M1-N14 nur zur Einordnung (nicht vorhanden, s. u.).

**Großläufe (A4, `docs/plan/M1-acceptance/loadtest/` und `benchmarks/`, M1-N14): nicht belegt.** Auf `origin/main` (7a0e93a) existiert `docs/plan/M1-acceptance/` nicht; N14 wurde nicht gemergt und hat keinen Remote-Branch. P29 verweist laut Auftrag auf die N14-Berichte und misst nicht selbst: eine gültige Großmessung braucht die ruhige Maschine, die N14 fordert (Load < 1–2, kein paralleler Build; bei P29 lag der Load Average zwischen 100 und 360, u. a. durch OBS, Bildschirmfreigabe und die Claude-Sitzungen). Offen bleiben deshalb: PERF-001 large (1 Mio. URLs × 30 Links, Spitzen-RSS < 1,5 GB, RSS(1 Mio.)/RSS(100 k) < 2), PERF-003 large (RSS < 500 MB, reguläres Ende), PERF-005 large (10 gesäte Abbrüche, `integrity_check = ok`, wiederholte Requests ≤ Threads + Kanalkapazität + größter Batch, K-49), CRAWL-124-Profilvergleich bei 1 Mio., Fortsetzen mit 1 Mio. Queue und die Benchmark-Ignores. Aufgabe an den Orchestrator: N14 ausführen und mergen, danach A4 hier mit Verweis ergänzen.

**Windows-Quote (Fable-Gate 4b):** `crawler_urls_per_s` in perf-001 small, windows-x64 gegen macos-arm64 (Job `loadtest-small`, drei main-Läufe nach dem Merge von P30, dazu der P29-Lauf und – zum Vergleich – der Lauf vor P30):

| Lauf | macos-arm64 | macos-x64 | windows-x64 | Quote windows/arm64 |
|---|---:|---:|---:|---:|
| [38507699280](https://github.com/domdewom2/openspider/actions/runs/38507699280) (main, vor P30) | 2.058,4 | 199,0 | 80,9 | 3,9 % |
| [38527465880](https://github.com/domdewom2/openspider/actions/runs/38527465880) (main, Merge N13) | 3.460,2 | 1.317,2 | 1.706,3 | 49,3 % |
| [38536936966](https://github.com/domdewom2/openspider/actions/runs/38536936966) (main, Plan 4c) | 3.416,6 | 1.440,6 | 1.965,3 | 57,5 % |
| [38544701689](https://github.com/domdewom2/openspider/actions/runs/38544701689) (main, Merge N15) | 4.193,1 | 2.082,0 | 1.962,1 | 46,8 % |
| [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445) (Branch m1/m1-p29) | 3.320,9 | 1.199,8 | 2.022,9 | 60,9 % |

**Befund an den Orchestrator (kein eigener Fix):** In zwei von drei main-Läufen (38527465880: 49,3 %; 38544701689: 46,8 %) liegt die Quote unter 50 %, im dritten (38536936966) bei 57,5 %; der P29-Branch-Lauf liegt bei 60,9 %. Gegenüber macos-x64 erreicht Windows 94–169 %: die Schwankung kommt wesentlich von den macos-arm64-Läufen (3,3–4,2 k URLs/s), nicht von Windows (1,7–2,0 k URLs/s). Der vor P30 gemessene Wert (80,9 URLs/s) ist durch P30 erledigt.

## A5 CLI

- **Exit-Codes:** je Code ein grüner Integrationstest (`crates/os-cli/tests`): 0 `cli_basics::the_binary_runs_without_a_gui_session_and_returns_an_exit_code`; 1 `cli_load::exit_code_1_when_a_running_crawl_is_resumed_within_the_heartbeat_window`; 2 `cli_basics::invalid_names_are_rejected_before_any_request_in_under_a_second` (und die Aufruffehler-Tests in `cli_config.rs`); 3 `cli_basics::exit_code_3_when_the_crawl_cannot_be_written`; 4 `cli_basics::exit_code_4_when_an_export_fails_but_the_crawl_succeeded`; 130 `cli_load::the_first_sigint_pauses_and_saves_with_exit_code_130_and_resume_completes_the_crawl` (Spec-Tag OUT-204).
- **`out-basic-site`-E2E-Exporte byte-identisch (nach Normalisierung) auf allen drei Plattformen:** Die Golden-Vergleiche (`tests/golden/out-basic-site`, `out-cli`) laufen im Job `build` von `cargo test --workspace` auf macos-arm64, macos-x64 und windows-x64 und sind dort grün (main-Lauf 38544701689 und P29-Lauf).
- **Namensvalidierung ohne Netzzugriff:** `cli_basics::invalid_names_are_rejected_before_any_request_in_under_a_second` und `exit_code_3_when_the_crawl_cannot_be_written` (`site.total_requests() == 0`).

## A6 Crawl-Datei

CI-Job-Paar `crawlfile-create` / `crawlfile-crossplatform` (neu in P29, `.github/workflows/ci.yml`): Jede Plattform (macos-arm64, macos-x64, windows-x64) crawlt dieselbe Fixture-Site (`parity-technical`, 116 URLs, Profil `parity.osconfig`), speichert die Crawl-Datei (`--save-crawl` → `crawl.ospider`) und exportiert `Internal:All` (`internal_all.csv`) sowie `Links:All Inlinks` (`links_all_inlinks.csv`). Danach lädt jede Plattform die Crawl-Dateien der **beiden anderen** (`--load-crawl`), exportiert erneut und vergleicht mit `cmp` byte-gleich mit den Exporten der Ursprungsplattform. Ergebnis im P29-Lauf [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445): alle sechs Richtungen grün – macos-arm64 → macos-x64, macos-arm64 → windows-x64, macos-x64 → macos-arm64, macos-x64 → windows-x64, windows-x64 → macos-arm64, windows-x64 → macos-x64 (Ausgabe „Crawl-Datei von <Ursprung> auf <Ziel>: Exporte byte-gleich“). Lokal (macOS arm64) ebenso: Der Export aus der geladenen Kopie ist byte-gleich zum Export direkt nach dem Crawl.

- **Determinismus:** `crates/os-engine/tests/lifecycle_determinism.rs` › `two_crawls_with_a_fixed_clock_a_seed_and_one_thread_are_byte_identical` (zwei Crawls mit fester Uhr, festem Seed und einem Thread liefern nach dem abschließenden Checkpoint byte-identische `.ospider`-Dateien; läuft in der CI auf allen drei Plattformen).
- **SIGINT → Exit 130 → `--load-crawl <id> --resume` vervollständigt:** `crates/os-cli/tests/cli_load.rs` › `the_first_sigint_pauses_and_saves_with_exit_code_130_and_resume_completes_the_crawl` (unter Windows über `GenerateConsoleCtrlEvent`, `cfg(windows)` in derselben Datei) sowie `resume_continues_a_crawl_that_stopped_at_the_url_limit`.

## A7 MCP

Alle Tests laufen in der CI auf drei Plattformen (`crates/os-mcp/tests`):
- **SDK-Client, initialize, tools/list, tools/call der fünf Werkzeuge:** `server_protocol::standard_client_can_run_the_five_basic_requests`, `server_process::rmcp_client_talks_to_the_binary_over_stdio`; Standard nur `read`: `server_protocol::tools_list_without_switches_and_with_read_only_has_only_read_tools`.
- **`crawl_overview` = Golden:** `tools_read::crawl_overview_equals_the_golden_of_the_overview_report` (und `…_has_exactly_the_numbers_of_the_crawl_overview_report`).
- **`query_urls`-Seitenvereinigung = Tab-Export ohne Dubletten:** `tools_read::internal_all_paged_with_limit_5_equals_the_tab_export_without_duplicates`, `server_protocol::paging_default_is_50_and_pages_union_to_the_full_list_without_duplicates`.
- **Injection-Fixture:** `tools_injection::injected_instructions_appear_only_inside_the_marked_data_block`, `tools_injection::no_tool_fetches_urls_that_are_only_mentioned_in_page_text` (kein Abruf von `/geheim`).
- **stdout nur Protokolldaten:** `server_process::stdout_carries_only_json_rpc_and_logs_go_to_stderr`.

## A8 Clean-Room

- **`sf/`-Dateien nur in `tests/golden/parity/*/sf/`:** `git ls-files | grep -E "(^|/)sf/"` liefert 39 Dateien, alle `tests/golden/parity/<site>/sf/*.csv`; `tests/parity/fixtures_check.rs` (`cargo test -p os-testsite --test parity_fixtures`) prüft je Site: `sf/` enthält genau die `sf_file`-Dateien der Manifest-Einträge mit `status = present` (vorhanden, nicht leer, `.csv`, keine Unterordner), `pending`/`unavailable` haben keine Datei.
- **Referenzdateien unverändert:** `git diff origin/parity/sf-reference -- tests/golden/parity/*/sf` ist leer (Merge `bea719f`, kein Umbenennen, kein Neukodieren; Zeilenenden normalisiert `.gitattributes`).
- **Runner verwirft Beschreibungsspalten:** `crates/os-parity` › `csvio::tests::description_columns_are_dropped_on_read`, `diff_engine::reference_description_texts_never_reach_the_report`, `reference_format::the_block_format_becomes_a_table_without_descriptions` (Spalten `Total URLs Description` im Crawl Overview werden gar nicht erst gelesen); jedes Mapping führt `drop_sf_columns = ["Description", "How To Fix"]`. In den 39 Referenzexporten kommen keine solchen Spalten vor.
- **Referenz-Issue-Texte im Repo:** Die Referenzexporte enthalten keine Issue-Beschreibungen. Die grep-Liste der Fable-Stichprobe hat der Planer am 14.10.2026 nachgetragen (Fable-Gate M1):
  - Befehl: `grep -rniE 'Screaming ?Frog|SEO Spider|Learn More' core crates docs/spec` (Stand main 218a67c).
  - `core/` (Issue-Katalog `core/issues/catalog.toml`, Texte `core/issues/texts/{de,en}.toml`): **0 Treffer**.
  - Treffer außerhalb von `core/` sind keine Referenz-Issue-Texte. Es sind Produktnamen in Quellenangaben und Kopfzeilen der Spec (`docs/spec/README.md`, `PARITY.md`, `01-crawl.md` Quellenliste, `02`/`04`/`05` Kopf) und im Modulkommentar `crates/os-parity/src/diff.rs`. Dazu kommt der Eintrag `learn more` in der Standardliste nicht beschreibender Ankertexte (TECH-136, `crates/os-model/src/config.rs` und das daraus erzeugte `docs/spec/schemas/osconfig.schema.json`); das ist eine eigene Festlegung, kein Issue-Text.
  - Fables eigene Ersatzprüfung (10 Katalogtexte de/en gelesen) war ohne Befund. Die Stichprobenliste unten bleibt die Grundlage für das nächste Gate.

Stichprobe für die Textprüfung (10 Katalogtexte, fester Seed 20261014; Quellen `core/issues/texts/{de,en}.toml`, Schlüssel `issues.<id>.what|why|fix`):

- `issues.canonicals-canonicalised`
- `issues.canonicals-multiple-conflicting`
- `issues.content-exact-duplicates`
- `issues.directives-nofollow`
- `issues.h1-duplicate`
- `issues.h1-multiple`
- `issues.h2-duplicate`
- `issues.hreflang-non-200-hreflang-urls`
- `issues.response-codes-external-no-response`
- `issues.response-codes-external-server-error-5xx`

## A9 Statusbericht

`docs/plan/M1-status.json` (erzeugt mit `os-parity coverage --packages docs/plan/M1-packages.json --test-output out.txt --parity-report … --evidence tests/parity/evidence.toml --write docs/plan/M1-status.json --strict`): 242 IDs – implemented 237, verified 5, deferred 0, **open 0**; Warnungen 0; ignorierte Tests ohne gültigen Grund 0.

Statusregeln 5.4: `implemented` (≥ 1 getaggter Test `ok`), `verified` (zusätzlich Paritätsbeleg aus `tests/parity/evidence.toml`), `deferred` (alle getaggten Tests ignoriert mit `M2:`/`M3:`/`Klärung K-xx`), `open`. Die fünf `verified`-IDs stehen in A2; alle Belege nennen Site, Export, Zellzahl, Übereinstimmung und die angewandten Abweichungen. Sitemaps- und Hreflang-Filter mit Referenzeinschränkung SF Free bleiben `implemented` (K-53).

