# Paritäts-Golden: Referenz-Exporte erzeugen

Diese Anleitung ist für den **Nutzer**. Agents bedienen das Referenzprodukt nie und legen nichts unter `ref/` ab
(Clean-Room, `docs/plan/M1.md` Abschnitt 2 Regel 1 und Abschnitt 5). Die Roh-Exporte aus der Referenz sind
Messdaten: unverändert ablegen, nicht bearbeiten, nicht umbenennen (außer im Manifest nachgetragen).

Die fünf Test-Sites werden von `os-testsite` ausgeliefert. Der Vergleich (`os-parity run`, Paket M1-P29) crawlt
dieselben Sites mit OpenSpider und gleicht beide Seiten ab; Adressen werden dabei zu `{BASE}`
(`http://127.0.0.1:<port>`) und `{EXT}` (`http://localhost:<port>`) normalisiert.

## Die Sites

| Site | Port | Start-URL für den Crawl | Inhalt |
|---|---:|---|---|
| `out-basic-site` | 8900 | `http://127.0.0.1:8900/` | Querschnitt: 12 HTML-Seiten, 2 Bilder, CSS, JS, ein 404, ein 301, ein externer Link |
| `parity-onpage` | 8902 | `http://127.0.0.1:8902/` | Titel, Meta Description, H1, H2, Content, Bilder, URL-Varianten, Inhaltstypen (intern/extern) |
| `parity-technical` | 8903 | `http://127.0.0.1:8903/` | Canonicals, Directives, Hreflang, Links (Tiefe bis 6), Sitemaps (Index + gz, Orphans), Linkbaum |
| `parity-response` | 8904 | `http://127.0.0.1:8904/` | Statuscodes, Weiterleitungen (auch auf Fehlerziele), Ketten, Schleife, robots-gesperrt, externe Ziele |
| `parity-synthetic` | 8905 | `http://127.0.0.1:8905/` | 400 synthetische Seiten (Struktur, Tiefen, Duplikat-Titel, noindex, Canonical) |

Wichtig:

- Als Start-URL **immer** `http://127.0.0.1:<port>/` verwenden (nicht `localhost`): `127.0.0.1` ist der interne Host,
  `localhost:<port>` gilt in der Referenz wie in OpenSpider als externer Host. Der Port ist fest (siehe Tabelle);
  externe Links stehen mit diesem Port im HTML.
- Eine Site hat höchstens 500 URLs (Grenze der Free-Version). Nicht auf andere Hosts oder Ports ausweichen.
- Eine Site nie ändern, während ein Crawl läuft. Wurden die Fixtures verändert (`python3 -B tests/parity/gen/generate.py`),
  müssen alle Exporte dieser Site neu erzeugt werden.

## Ablauf je Site (Schritt für Schritt)

1. **Site starten** (eigenes Terminalfenster, offen lassen):
   ```sh
   cargo run -p os-testsite --release -- parity-onpage 8902
   ```
   (Name und Port aus der Tabelle einsetzen.) Die Ausgabe nennt `http://127.0.0.1:8902`. Kurz prüfen, z. B. im
   Browser oder mit `curl -i http://127.0.0.1:8902/` (erwartet: `200`). Beenden mit `Ctrl+C`, erst wenn alle Exporte fertig sind.
   Für `parity-synthetic` genügt derselbe Aufruf (`... -- parity-synthetic 8905`).
2. **Referenzprodukt öffnen** (Free-Version), Modus *Spider*, Konfiguration auf Standard (Datei → Konfiguration → Standardkonfiguration,
   falls zuvor etwas geändert wurde). Folgende Optionen sollen gelten – was die Free-Version sperrt, bleibt aus und wird in
   Schritt 6 vermerkt:
   - *robots.txt beachten* (Standard), kein JavaScript-Rendering, kein Wechsel des User-Agents.
   - Canonicals speichern und crawlen (Standard).
   - **Hreflang speichern und crawlen** (Konfiguration → Spider → Crawl; Standard: aus) – nötig für `parity-technical`.
   - **XML-Sitemaps crawlen** („Crawl Linked XML Sitemaps“ mit „Auto Discover XML Sitemaps via robots.txt“; Standard: aus) –
     nötig für `parity-technical` und die Sitemaps-Spalten.
3. **Crawl starten**: Start-URL aus der Tabelle eintragen, *Start* klicken, bis der Fortschritt 100 % zeigt.
   Danach, falls im Menü verfügbar, *Crawl Analysis → Start* ausführen und warten (liefert Canonicals-,
   Sitemaps- und Orphan-Filter).
4. **Exporte erzeugen** laut Exportliste der Site (unten). Jeder Export als **CSV**, Dateiname wie von der Referenz vorgeschlagen.
   - Tab-Exporte: Tab wählen, Filter *All*, *Export*.
   - Bulk-Exporte: Menü *Bulk Export*; Reports: Menü *Reports*.
5. **Dateien ablegen**: nach `tests/golden/parity/<site>/ref/` kopieren (Ordner anlegen; unverändert, nicht öffnen und neu speichern).
   Weicht ein Dateiname von `ref_file` im Manifest ab, trägst du den **tatsächlichen** Namen im Manifest ein.
6. **Manifest aktualisieren** (`tests/golden/parity/<site>/manifest.toml`):
   - `ref_version` (neutraler Kennzeichner `"referenz-2026-10"`) und `exported_at` (Datum, `"JJJJ-MM-TT"`) eintragen;
   - je Export `status = "present"` setzen, wenn die Datei abgelegt ist; `status = "unavailable"`, wenn der Export in der
     Free-Version nicht möglich ist (Menüpunkt gesperrt) – dann in `ref_settings` einen kurzen Hinweis ergänzen;
   - sonst bleibt `status = "pending"`.
7. **Einchecken**: nur `tests/golden/parity/<site>/` (also `ref/` und `manifest.toml`). Der Abnahmetest verlangt mindestens
   4 von 5 Sites mit Status `present`.

## Exportlisten

Die Zeile nennt Menüpfad und erwarteten Dateinamen; maßgeblich ist immer die `[[export]]`-Liste im Manifest der Site.
Die Spalten jedes Exports bildet die Mapping-Datei `tests/parity/mappings/<mapping>.toml` ab; die Spalten *Description*
und *How To Fix* werden beim Einlesen verworfen.

| Export | Menüpfad | Datei (`ref_file`) | Mapping |
|---|---|---|---|
| Internal:All | Tab *Internal*, Filter *All*, Export | `internal_all.csv` | `internal_all` |
| External:All | Tab *External*, Filter *All*, Export | `external_all.csv` | `external_all` |
| Response Codes:All | Tab *Response Codes*, Filter *All*, Export | `response_codes_all.csv` | `response_codes_all` |
| URL:All | Tab *URL*, Filter *All*, Export | `url_all.csv` | `url_all` |
| Page Titles:All | Tab *Page Titles*, Filter *All*, Export | `page_titles_all.csv` | `page_titles_all` |
| Meta Description:All | Tab *Meta Description*, Filter *All*, Export | `meta_description_all.csv` | `meta_description_all` |
| H1:All / H2:All | Tab *H1* bzw. *H2*, Filter *All*, Export | `h1_all.csv` / `h2_all.csv` | `h1_all` / `h2_all` |
| Content:All | Tab *Content*, Filter *All*, Export | `content_all.csv` | `content_all` |
| Images:All | Tab *Images*, Filter *All*, Export | `images_all.csv` | `images_all` |
| Canonicals:All | Tab *Canonicals*, Filter *All*, Export | `canonicals_all.csv` | `canonicals_all` |
| Directives:All | Tab *Directives*, Filter *All*, Export | `directives_all.csv` | `directives_all` |
| Hreflang:All | Tab *Hreflang*, Filter *All*, Export | `hreflang_all.csv` | `hreflang_all` |
| Links:All | Tab *Links*, Filter *All*, Export | `links_all.csv` | `links_all` |
| Sitemaps:All | Tab *Sitemaps*, Filter *All*, Export | `sitemaps_all.csv` | `sitemaps_all` |
| Links:All Inlinks | *Bulk Export → Links → All Inlinks* | `all_inlinks.csv` | `all_inlinks` |
| Response Codes:All Error Inlinks | *Bulk Export → Response Codes → All Error Inlinks* | `all_error_inlinks.csv` | `all_error_inlinks` |
| Crawl Overview | *Reports → Crawl Overview* | `crawl_overview.csv` | `crawl_overview` |
| Redirect Chains | *Reports → Redirects → Redirect Chains* | `redirect_chains.csv` | `redirect_chains` |
| Redirects to Errors | *Reports → Redirects → Redirects to Error(s)* | `redirects_to_errors.csv` | `redirects_to_errors` |

Welche Exporte zu welcher Site gehören:

| Site | Exporte |
|---|---|
| `out-basic-site` | Internal, External, Response Codes, Links (je *All*), All Inlinks, Redirect Chains, Crawl Overview |
| `parity-onpage` | Internal, External, Response Codes, URL, Page Titles, Meta Description, H1, H2, Content, Images (je *All*), All Inlinks, Crawl Overview |
| `parity-technical` | Internal, Canonicals, Directives, Hreflang, Links, Sitemaps (je *All*), All Inlinks, Crawl Overview |
| `parity-response` | Internal, External, Response Codes (je *All*), All Inlinks, All Error Inlinks, Redirect Chains, Redirects to Errors, Crawl Overview |
| `parity-synthetic` | Internal, Links (je *All*), All Inlinks, Crawl Overview |

## Hinweise

- Die Filter-Zähler (*Crawl Overview*) decken alle Filter der Tabs Internal bis Sitemaps ab; einzelne Filter-Exporte sind
  nicht nötig. Wer einzelne Filter prüfen will, kann sie zusätzlich exportieren und im Manifest als weiteren `[[export]]`
  (mit `os = "tab:<Tab>:<Filter>"` und passendem Mapping) eintragen.
- `http://localhost:1/` in den Sites ist absichtlich ein geschlossener Port (Verbindung abgelehnt). Das ist das erwartete
  Verhalten, kein Fehler der Anleitung.
- Die Sites enthalten bewusst fehlerhafte Seiten (404, 5xx, Schleifen, doppelte Titel …). Sie nicht „reparieren“.
- Nach dem Ablegen der Exporte läuft der Abgleich nur noch lokal: `os-parity run --all` (siehe `docs/plan/M1.md` Abschnitt 5.4).
- Die Absicht jeder Fixture je Tab und Filter (welche Adresse Treffer sein soll, welche nicht) steht in
  `tests/parity/coverage/<site>.toml` und wird von `tests/parity/fixtures_check.rs` gegen die ausgelieferten Seiten geprüft.
  Die Fixtures entstehen aus `tests/parity/gen/` (`python3 -B tests/parity/gen/generate.py`).

## M2: fünf zusätzliche Sites (P01, Plan Abschnitt 6)

Die bisherigen Sites und ihre 39 CSV-Dateien bleiben unverändert. Für die neuen Sites gilt
Standardkonfiguration der Free-Version, macOS, Oberfläche **English (UK)**. Hreflang und
Pagination nur **speichern, nicht crawlen**; Always Follow bleibt aus (**Not Followed**),
kein JavaScript und kein TLS. Die Hinweise oben zu eingeschaltetem Hreflang-/Sitemap-Crawl
sind historische M1-Exportwünsche; für M2 gelten die tatsächlich verfügbaren Standards (K2-59).

| Site | Port | Fälle |
|---|---:|---|
| `parity-m2-onpage` | 8906 | Keywords, Pixelgrenzen ±1, Unicode, Headings, URL, Content, Bilder, PDF |
| `parity-m2-technical` | 8907 | Canonicals, Pagination, Directives, Hreflang, Validation, Refresh, Schleifenvorlauf |
| `parity-m2-links` | 8908 | Anker, Follow/Nofollow, Outlinks, Linkpositionen, Graph und Tiefe 8 |
| `parity-m2-security` | 8909 | HTTP-Formulare, externe Links, Header, protokollrelative Ressourcen, Content-Type |
| `parity-m2-list` | 8910 | List-Modus mit 60 Eingabezeilen, Duplikaten, ungültigen Zeilen und Ketten |

1. Je Site `cargo run -p os-testsite --release -- <site> <port>` starten. Immer
   `http://127.0.0.1:<port>/` verwenden; `localhost` bezeichnet externe Ziele derselben Site.
2. In der Referenz Standardkonfiguration laden, Spider-Modus wählen und crawlen. Für
   `parity-m2-list`: **List** wählen und `tests/fixtures/parity-m2-list/list.txt` hochladen.
   Zeilen weder sortieren noch bereinigen; auch Leerzeile und Duplikate gehören zur Eingabe.
3. Nach Abschluss **Crawl Analysis → Start** ausführen und abwarten.
4. **Jeden** `[[export]]`-Eintrag des Site-Manifests abarbeiten. `tab:` bedeutet Tab → Filter →
   Export, `bulk:` bedeutet Bulk Export, `report:` bedeutet Reports. CSV-Dateien unverändert
   unter `tests/golden/parity/<site>/ref/<ref_file>` speichern (Ordner dabei anlegen).
   `bulk_` und `report_` im Referenzdateinamen verhindern Kollisionen zwischen gleichnamigen Exportarten.
   OpenSpider-Ausgaben werden zusätzlich nach Tab/Bulk/Report und Konfigurationsvariante in getrennten Ordnern erzeugt.
   Auch leere Filter als CSV mit Kopfzeile exportieren.
5. Besonderheiten:
   - **Crawl Path Report** gehört zu `parity-m2-links`: `/deep/8.html` auswählen, Rechtsklick →
     Export → Crawl Path Report → `crawl_path_deep8.csv`. `Target Address` bestimmt die Auswahl;
     alle Schritte bleiben im Vergleich erhalten. Die CLI liefert den Report, der Runner filtert
     den Zielblock. Ein fehlendes Ziel wird als Fehler gemeldet.
   - `sitemap:default`: Sitemaps → XML Sitemap, Standardoptionen → `sitemap_default.xml`.
   - Zweiter XML-Export: lastmod aus Server-Antwort, priority und changefreq nach Tiefe,
     Bilder einschließen → `sitemap_options.xml`. Die entsprechenden `export.config`-Werte
     stehen im Manifest; der Runner startet diese Variante separat mit `--set`.
   - Sitemaps → Images Sitemap → `sitemap_images.xml`.
   - Internal:All im Listenmodus in **Upload-Reihenfolge** exportieren; jede doppelte
     Listen-URL behält ihre eigene Zeile. Der Abgleich prüft die Reihenfolge zusätzlich zur Menge.
6. `ref_version`, `exported_at` und `ref_settings` im Manifest aktualisieren. Je Datei `present`
   setzen. Fehlt ein Menüpunkt, ist er gesperrt oder bleibt ein konfigurationsabhängiger Export
   leer, `unavailable` setzen und den konkreten Grund als Kommentar am Eintrag festhalten.
   Das betrifft insbesondere Near Duplicates. Keine Konfiguration zum Freischalten ändern.
   HTTPS-/HSTS-/Mixed-Content-Prüfungen bleiben spec-abgeleitet; sie werden hier durch HTTP
   nicht als nachgewiesen angesehen. Alle noch offenen Einträge bleiben `pending`.
7. Der Orchestrator übernimmt die Referenzdateien und Statusänderungen. Implementierende Agents
   erzeugen oder öffnen keine Referenzexporte. Die fünf neuen Manifeste werden hier nur vorbereitet.

Die Exportlisten enthalten die Tab-/Filter-/Bulk-/Report-Sammlung aus M2.md 6.3. Hreflang-Reports,
Canonical-/Directives-/Security-Bulk-Exporte sind einzeln aufgeführt. Gesperrte Funktionen werden
mit `unavailable` dokumentiert. Die Schätzwerte für URL- und Dateizahlen im Plan sind keine Mindestzahlen;
hohe Outlink-Zahlen entstehen durch mehrere Links auf dieselben Ziele (Spec: nicht unique).

Fixture-Quellen und Kontrollen:

```sh
python3 -B tests/parity/gen/m2_sites.py --check
cargo test -p os-testsite --test parity_fixtures
cargo test -p os-parity --test real_repo_files --test run_site
```

Nur zum Ändern der eigenen neuen Fixtures `m2_sites.py` ohne `--check` aufrufen. Das Skript
`m2_exports.py` legt fehlende M2-Mappings/Manifeste an und überschreibt keine bestehenden Dateien.
`cases.json` in jeder neuen Site beschreibt Pfad/Spec/Status und dient Q02 als Einstieg
für spec-abgeleitete Belege nach K2-14 (keine Referenz-Verifikation); `pixels.json` dokumentiert die
Breiten der eigenen kalibrierten Engine und `expected_jaccard.json` die exakten Shingle-Werte.
Die über 2 MiB großen Antworten entstehen über `body_size` im Fixture-Server, ohne große Repo-Dateien.
Die Tiefe-8-Kette ist nur über ihren ersten Knoten verlinkt.

Der Runner entdeckt zehn Sites. `thresholds.toml` verlangt mindestens vier bisherige Sites
mit mindestens einem vorhandenen Export (M1 A3). Sobald eine `parity-m2-*`-Site einen
`present`-Eintrag hat, verlangt `run --all` zusätzlich mindestens vier neue Sites (M2 B3(d)).
Solange alle M2-Exporte pending/unavailable sind, bleibt der M1-Regressionslauf ausführbar.
Q02 prüft den M2-Nachweis nach den Referenzexporten; ein grüner Fixture-Test ersetzt ihn nicht.
Es gelten identische URL-Mengen, ≥99 % für die genannten Tabs, ≥97 % für Bilder,
Hreflang/Links/Bulk/Reports/Listen/XML und identische Overview-Zähler aus M1 **und M2**.

Der XML-Leser ignoriert Attribute wie `xsi:schemaLocation`; verglichen werden Sitemap-Elemente.
Die Spalte `Images` speichert sortierte Bilddatensätze als JSON-Strings, getrennt durch Zeilenumbrüche.
Sie wird im Sitemap-Vergleich mit der 97-%-Schwelle für Attribute bewertet; die URL-Menge bleibt exakt.
Der Fixture-Test führt `m2_sites.py --check` einschließlich `generated_m2.txt` automatisch aus.
Der Generator-Test versucht ohne Shell `python3`, bei fehlendem Interpreter `python`, danach `py -3`.
`real_repo_files.rs` prüft Tab-/Filter-Aufträge exakt über das Overview-Mapping, das die
Default-Platzhalter bereits auflöst. Der Fixture-Test prüft Tab-Kennung und Label-Präfix.
Die zwei vollständigen Bulk-Katalogtests liegen unter „Schnittstellenbedarf M2-N01“ in
`docs/plan/M2-reports/p01.md`: Erst N01 darf die fehlende `os-export`-Testabhängigkeit nach C7
ergänzen und die Tests aufnehmen. Jeder angeforderte Bulk-Name muss auflösbar sein; zusätzliche
Katalogeinträge sind erlaubt. Q02/Orchestrator prüfen nach D04/D06/D11, ob neue Exporte je
Filter weitere Manifestaufträge benötigen. Die zehn Links-Exporte setzen das D08-Register voraus;
N01 übernimmt und aktiviert auch diesen Vertrag nach dessen Integration.
