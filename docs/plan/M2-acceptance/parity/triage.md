# Triage M2-Q02: Ursachengruppen, Nachbesserungspakete, Gate-Einschätzung

Stand 30.10.2026. Grundlage: Branch `build/m2-q02` (Q02-Ergebnisse unter `docs/plan/M2-acceptance/parity/`), Produktstand `origin/parity/ref-m2b` (= `m2/int-16` + Referenzexporte), Spec `docs/spec/*`. Gelesen wurden nur Repo-Dateien. Die Referenz-Exportdateien unter `tests/golden/parity/*/ref/` dienten als Messdaten. Es lief kein Build und kein Test.

Hilfsdateien in diesem Ordner:
- `classify.py` ordnet jede der 5.142 Zeilen aus `befunde.csv` genau einer Gruppe zu. Die Summe geht auf.
- `tabcheck.py` zeigt je Tab alle Blocker für `verified`.
- `data/` enthält Kopien der Q02-CSV- und JSON-Dateien.
- `src/` ist ein Archivabzug des Branches (Code, Spec, Mappings, Fixtures, Referenzdateien).

---

## 1 Kurzfassung

1. **0/160 verified hat vor allem messtechnische Gründe.** Die Auswertung (`belege.py`, Logik wie `coverage/evidence.rs`) zählt eine Filter-ID nur, wenn **alle** Exporte **aller** Sites mit demselben Mapping-Slug ihres Tabs fehlerfrei und spaltenvollständig sind. Diese Regel gilt für den ganzen Tab: Ein einzelner roter Filterexport sperrt alle IDs des Tabs. Dazu kommen zwei Messfehler:
   - Die Referenz lässt die nummerierten Folgespalten weg, wenn keine URL einen Wert hat. Beispiele sind `Title 2 Length` und `Canonical Link Element 2/3`. os-parity wertet das als „unvollständig“ oder vergleicht die Zelle mit einem leeren Wert (G01).
   - Mehrere Mappings passen nicht zu den Spaltennamen der neuen Referenzversion (G02).
   Allein G01 und G02 sperren die Tabs Meta Description, H1, H2, Meta Keywords, External, Internal und Security.
2. **Rund 1.400 der 2.070 „Implementierungsfehler“ sind in Wahrheit Mapping- oder Formatthemen.** Die größten Posten:
   - Folgespalten-Längen `…2 Length/Pixel Width`: rund 600 Befunde. Die Elementtabs schreiben bewusst 0. Die neue Referenz lässt die Spalte ganz weg, wenn keine Seite ein zweites Vorkommen hat.
   - Lesbarkeitsspalten fehlen im OS-Export: 296 Befunde.
   - Sitemap-Datumsformat: 152 Befunde. Das ist eine Spec-Festlegung, kein Codefehler.
3. **Viele echte Abweichungen sind Konflikte zwischen Spec und Referenz.** Die Spec hat hier eine Festlegung getroffen, die die Referenz widerlegt. Das betrifft:
   - Grundmengen: Directives, Pagination, Hreflang, Validation, Security-Header
   - Inline-CSS-Entdeckung
   - Elemente außerhalb von `<head>`
   - `unavailable_after`
   - Unlinked Canonical
   - Wortgrenzen
   - Lesbarkeits-Labels
   - Internal-Filter Media/Fonts/XML
   Diese Fälle muss der Orchestrator entscheiden: Entweder folgt die Spec der Referenz (dann Implementierung), oder die Spec bleibt (dann DEV-Eintrag). Beides entsperrt `verified`, denn DEV-Einträge sind zulässig, nur REF-FREE nicht.
4. **Echte Produktfehler** sind deutlich weniger. Die meisten sind klein.
   - Klein:
     - Missing Size Attributes hängt an einer falschen Voraussetzung.
     - Soft-404- und Content-Export haben falsche Spaltensätze.
     - Bad Content Type wertet eine fehlende Content-Type-Angabe als Treffer.
     - Ziele von nofollow-Links landen in der Tabelle.
     - Crawl-Tiefe über rel=next.
     - Listenmodus-Kettenreport.
     - Pixelbreite der Sigma-Formen.
     - Overview-Zeile.
   - Mittel: die Flesch-Werte.
   - Groß: Link Score. Das ist eine Kalibrierung der eigenen Formel an den Referenzdaten.
5. **Mit 6 Nachbesserungspaketen plus Wiederholungslauf sind 60 % erreichbar.** Voraussetzung sind die Spec-Entscheidungen. Link Score ist dafür nicht nötig (Rechnung in Abschnitt 6).

## 2 Methode und Zahlen

| Quelle | Inhalt |
|---|---|
| `befunde.csv` | 5.142 Einzelbefunde. Q02-Kategorien: 2.070 Implementierungsfehler, 935 Mapping-/Formatfehler, 2.137 Referenzbesonderheiten |
| `roh/lauf.json` | Vollständiger Bericht je Export, inklusive `missing_ref_columns`, `unmapped_ref_columns`, `missing_os_columns` und `applied_deviations` |
| `verified.json` + `belege.py` | Definition und Prüfung von `verified`: je Tab alle Mapping-Slugs, alle Sites, URL-Menge, Zellen, Spaltenvollständigkeit, kein REF-FREE |

Jeder Befund wurde regelbasiert einer Gruppe zugeordnet (`classify.py`). Die Ursachen sind belegt:
- an der Referenzdatei (Kopfzeile oder Wert),
- an der Fixture (`tests/fixtures/parity-m2-*`),
- an der Spec-Stelle,
- und, wo möglich, an der Codezeile.

„Befunde“ in der Tabelle sind Q02-Zeilen. Eine Gruppe mit wenigen Befunden kann trotzdem einen ganzen Tab für `verified` sperren.

## 3 Ursachengruppen (Übersicht)

Kategorie nach Prüfung gegen die Spec:
- **I** = Implementierungsfehler
- **M** = Mapping-/Formatfehler (Messung)
- **R** = Referenzbesonderheit mit Spec-Begründung (DEV nötig)
- **S** = Konflikt Spec↔Referenz oder Spec-Lücke (Orchestrator entscheidet)

| ID | Ursache (kurz) | Befunde | Kat. (Q02 → geprüft) | Spec-IDs | Stelle | Größe | Gate (B3g) |
|---|---|---:|---|---|---|---|---|
| G01 | Dynamische Folgespalten (`…2`, `…n`): Referenz lässt sie weg, wenn leer | 789 + ca. 600 aus „Impl“ | M/I → **M** | ONPAGE-008/011, TECH-011/031/051/071 | `crates/os-parity/src/diff.rs` (numbered columns, `missing_ref_columns`) | S | ja (MD, H1, H2, MK, PT, Internal, Directives, Canonicals, Hreflang, Security) |
| G02 | Spaltennamen, -sätze und Schlüssel der neuen Referenzversion (Security, Validation, Pagination, Canonicals, External, Filter-Exporte, Reports, Ketten-Sprungspalten) | 421 + 9 Exportfehler | M → **M** (+S klein) | ONPAGE-030, TECH-011/031/281, OUT-081/083/085/086 | `tests/parity/mappings/*.toml`, Manifeste | M | ja (Security, Validation, Pagination, External, Content) |
| G03 | Exportaufruf: alle Reports in **einem** CLI-Aufruf, ein unbekannter Name (`Crawl Path Report`) beendet ihn mit Exit 2; Bulk-/Report-Dateinamen nicht auflösbar | 16 Exporte | M → **M** (+S: Crawl Path per CLI) | OUT-023, OUT-080 | `crates/os-parity/src/run.rs:152-160`, `find_file`; `crates/os-export/src/catalog/bulk_links.rs` (Namen `… (Bulk)`), `reports/hreflang/mod.rs` (`file_stem`) | S | indirekt (m2-links crawl_overview nicht verglichen → B3c) |
| G04 | Auswertung `verified`: Tab-weites Alles-oder-nichts, Change Detection im Nenner, Vollständigkeitsprüfung nur im Skript, kein Site/Datei-Selektor; dazu P01/P03/P04/P06 | – | M → **M** + **S** (Definition) | B3g, TECH-301…305 | `crates/os-parity/src/coverage/evidence.rs`, `tests/parity/evidence.toml`, `crates/os-parity/tests/real_repo_files.rs:248`, `tests/golden/parity/parity-response/manifest.toml` | M | **ja, zentral** |
| G05 | Fehlende oder zu enge Ausnahmen: DEV-003/004 nicht für `internal_m2`/`list_internal`, DEV-014 nicht für neue Sites, leere Zähler bei needs_config (OUT-080), TECH-141, OUT-015, Sitemaps-All-Zähler, Overview-Inkonsistenz der Referenz | 1.989 | R → **R** | ONPAGE-011/013, ONPAGE-020/024, OUT-080, TECH-141, TECH-211, OUT-015 | `tests/parity/deviations.toml` | S | ja (Internal, Links; B3c) |
| G06 | Nachkommastellen: Referenz 3 Stellen (Text Ratio 4.058, Flesch 36.629, AWPS 0.991), Spec und OS 1 Stelle | 5 (+ Folge in G07/G08) | M → **S/M** | ONPAGE-013, ONPAGE-149 | `crates/os-analyze/src/tabs/content.rs:217-222`, `tabs/internal.rs` (Text Ratio) | S | ja (Internal, Content) |
| G07 | Content-Export: Spalten `Sentence Count`, `Average Words Per Sentence`, `Flesch Reading Ease Score`, `Readability` fehlen im Tab-Export; Soft-404-Filterexport ohne `Address` | 296 + 1 Exportfehler | I → **I** | ONPAGE-149, ONPAGE-142, ONPAGE-147 | `crates/os-analyze/src/tabs/content.rs` (`p1_column` mit `needs`, Filter-`default_columns: &["Matched Phrase"]`) | S | ja (Content) |
| G08 | Flesch-Werte und Labels weichen ab. Labels der Referenz: Very Easy … Fairly Hard/Hard/Very Hard (Spec A-54: Difficult); Satz- und Silbenzählung. Alte Sites: technical „difficult“ 74→10 und „very difficult“ 0→79, synthetic 0→268 | 26 (+ Mengen in Filterexporten) | I → **I + S** (A-41/A-54) | ONPAGE-149/150/151 | `crates/os-parse/src/readability.rs` | M | ja (Content); B3c |
| G09 | „Missing Size Attributes“ hängt fälschlich an `RenderingOrConfig(images.parse_inline_css_backgrounds)`: Filter leer, Overview-Zähler leer auf 9 Sites | 11 | I → **I** | ONPAGE-179 | `crates/os-analyze/src/tabs/images.rs:137` | S | ja (Images) |
| G10 | Bad Content Type zu weit: fehlender Content-Type zählt (`content_types.len() != 1`); Sniffing schlägt bei `application/octet-stream` an (`/submit`) | 8 | I → **I + S** | ONPAGE-043 | `crates/os-parse/src/security.rs:211-224` | S | ja (Security) |
| G11 | Nicht gecrawlte Linkziele: Ziel nur über nofollow-Link landet in Internal (`nofollow-only.html`); Ziel nur über rel=next landet in Internal (`pagination-unlinked.html`) und verkürzt die Crawl-Tiefe (`page-3` 2 statt 3) | 5 (+ Folgen) | I → **I** (nofollow, Tiefe) + **S** (Paginierung in Tabellen) | CRAWL-056, CRAWL-047, CRAWL-038 | `crates/os-engine/src/frontier.rs:532/566` (`in_tables: c.store`), Tiefenvergabe `frontier.rs:786`, `decide.rs` | M | ja (Internal) |
| G12 | Internal-Filter nach Inhaltstyp: Die Referenz führt eigene Filter Media/Fonts/XML/Plugins (Overview m2-onpage); `ui.parity_filters` legt sie unter Other; Antwort ohne Content-Type → Referenz „Other“ | 10 + Zeilenmengen | R → **S** (A-49 widerlegt) + R (DEV-014) | ONPAGE-020/023/024 | `crates/os-analyze/src/tabs/internal.rs:354`, `tabs/mod.rs:1165-1171` | S | ja (Internal) |
| G13 | Hintergrundbild aus `style`-Attribut wird als Bild-URL entdeckt (`/background.png`), die Referenz entdeckt es nicht | 16 | I → **S** (CRAWL-040 verlangt es) | CRAWL-039/040, ONPAGE-178 | `crates/os-parse/src/lib.rs:179` (`css::extract_inline`) | S | **ja (URL, Response Codes, Images, Internal = 35 IDs)** |
| G14 | Grundmengen technischer Tabs: Directives (Spec: jeder Status; Referenz: nur 2xx), Pagination/Hreflang (Spec: plus gecrawlte Ziele ohne 2xx; Referenz: nur HTML-2xx), Validation (Spec: HTML-200; Referenz: alle internen URLs), Security-Headerfilter (A-12: jede Antwort; Referenz: ohne 3xx/4xx) | 60 | I → **S** | TECH-051, TECH-031, TECH-071, TECH-281, ONPAGE-038…042 | `crates/os-analyze/src/tabs/{directives,pagination,hreflang,validation,security}.rs` (Population) | M | ja (Directives, Pagination, Hreflang, Validation; B3c Security) |
| G15 | Canonical/Hreflang außerhalb von `<head>` und Canonical mit ungültigem Attribut: Die Referenz ignoriert sie für Werte, Occurrences, Indexierbarkeit und Folgefilter, meldet sie aber im Filter „Outside <head>“ bzw. „Invalid Attribute“ | 70 | I → **S** (TECH-023/086 legen das Gegenteil fest) | TECH-021/023/086, TECH-012/014/019, TECH-082/084 | `crates/os-parse/src/canonical.rs`, `hreflang.rs`; `crates/os-analyze/src/analysis/steps/{canonicals,hreflang}.rs` | M | ja (Canonicals, Hreflang, Directives, Internal) |
| G16 | Indexierbarkeit: `unavailable_after` in der Vergangenheit → Referenz Non-Indexable „Unavailable After“; Selbst-Refresh per **HTTP-Header** → Referenz „Redirected“ und im Filter „Internal Redirection (HTTP Refresh)“ | 37 | I → **S** (TECH-063 Festlegung; Header-Fall offen) | TECH-001, TECH-063, ONPAGE-059, CRAWL-015 | `crates/os-model/src/indexability.rs`, `crates/os-analyze/src/tabs/response_codes.rs` | S | ja (Response Codes, alle Technik-Tabs: je 4 Zellen) |
| G17 | Weitere Technik-Filterlogik: Unlinked Canonical (Referenz listet das **Ziel**), Canonical mit Fragment (Referenz zeigt ohne `#fragment`), Hreflang-Rücklinkfilter (inconsistent/non-canonical/noindex/unlinked), Pagination Sequence Error (`pagination-404.html` fehlt), „<head> not first“ (`validation-head-missing`), Link-Typnamen in Bulk-Inlinks („HTML Rel Next“) | 60 | I → **I + S** (TECH-020) | TECH-018/020, TECH-075…079, TECH-041, TECH-283, OUT-031 | `crates/os-analyze/src/analysis/steps/{canonicals,hreflang,pagination}.rs`, `crates/os-parse/src/validation.rs`, `crates/os-export/src/catalog/bulk_canonicals.rs` | M–L | ja (Canonicals, Hreflang, Pagination, Validation) |
| G18 | Link Score weicht auf allen vier Sites ab (z. B. `/contact.html` 90 vs 96, `/services.html` 13 vs 32). Die Formel in TECH-145 ist eine OS-Festlegung, Kalibrierung gegen die Referenz offen (Anhang A Punkt 36) | 892 | I → **S + I** (Kalibrierung) | TECH-145, ONPAGE-017 | `crates/os-analyze/src/analysis/steps/link_score.rs`, `src/graph/**` | L | ja (Links, Internal) – Vorschlag: nach M3 |
| G19 | Schwellen und Heuristiken: High External Outlinks (Referenz listet 99 Links, Spec ≥ 100), Pages Without Internal Outlinks (m2-technical 56→41), High Crawl Depth (Folge G11), Internal Search `/?s=stone` (A-26), Soft 404 2→3 (A-39), Titles below 30 chars 30→32, alte technical TECH-136/139 | 10 | I → **S/I gemischt** | TECH-132/133/136/137/139, ONPAGE-077/094/147 | `crates/os-analyze/src/tabs/{links,url}.rs`, `crates/os-parse/src/content.rs` | S–M | ja (Links, URL) |
| G20 | Wortzählung: Die Referenz trennt benachbarte Inline-Anker (`anchors.html` 6 vs 4, `external-99.html` 99 vs 1, `graph/*` 4 vs 3) und schließt `<header>`/`<aside>` aus (`positions.html` 1 vs 3) | 152 | R/I → **S** | ONPAGE-140, ONPAGE-141 | `crates/os-parse/src/{text,content}.rs` | M | ja (Internal, Content) |
| G21 | Pixelbreite der Sigma-Formen (Σ/σ/ς): `unicode-4/5` 12 statt 11, `unicode-6` 10 statt 11 | 9 | I → **I** | ONPAGE-011, ONPAGE-090 | `crates/os-parse/src/pixel/**` | S | ja (Page Titles) |
| G22 | Sitemap-Generator: Die Referenz schreibt `lastmod` immer als Datum und setzt ohne `Last-Modified` das Crawl-Datum, daraus folgt `changefreq` daily. Die Spec legt ISO-Datum mit Uhrzeit und „kein lastmod“ fest | 152 | I → **S** (OUT-123/125 OS-Festlegung) | OUT-123, OUT-125 | `crates/os-export/src/sitemap_gen/entries.rs` | S | nein (B3b) |
| G23 | Ketten-Reports: Listenmodus liefert eine Zeile statt einer je Listen-URL (46 vs 1); Chain Type „Mixed“ vs „Redirect & Canonical“; Schleife `/tail` (Referenz: Final Address `loop-a`, 302); Zwischenglied `loop-a` als eigene Zeile | 98 | M → **I** (OUT-085) + **R** (OUT-081/083 Festlegungen) | OUT-081, OUT-083, OUT-085 | `crates/os-export/src/reports/{redirects,redirect_canonical_chains,canonical_chains}.rs` | M | nein (B3b) |
| G24 | Linkposition `<aside>`: Referenz „Aside“, OS „Sidebar“ | 1 | I → **S** (INTEG-081 legt Sidebar fest) | INTEG-080/081, OUT-031 | `crates/os-model/src/config/v2.rs` (Standardregeln), `crates/os-parse/src/link_position.rs` | S | nein |
| G25 | Overview-Zeile „External Redirection (JavaScript)“ fehlt im OS-Overview auf allen 9 Sites | 9 | M → **I** (prüfen) | ONPAGE-057, OUT-080 | `crates/os-export/src/reports/overview.rs` | S | nein (B3c) |

Summe der Befunde: 5.142. Quersummen in `classify.py`.

## 4 Gruppen im Einzelnen

### G01 Dynamische Folgespalten (Mapping)
- **Beleg 1:** `parity-m2-onpage/ref/page_titles_all.csv` hat nur 7 Spalten (`Address, Occurrences, Title 1, Title 1 Length, Title 1 Pixel Width, Indexability, Indexability Status`). Keine Seite der Site hat einen zweiten Titel. Im älteren `parity-onpage/ref/page_titles_all.csv` gibt es eine Seite mit zweitem Titel, dort stehen `Title 2*`-Spalten mit `0` für alle anderen Zeilen. OS schreibt in den Elementtabs bewusst `COALESCE(u.title_2_len, 0)` (`crates/os-analyze/src/tabs/page_titles.rs:36`). os-parity setzt eine fehlende nummerierte Referenzspalte als leer an (`diff.rs`, Kommentar „Nummerierte Spalten, die die Referenz nicht schreibt … Referenz = leer“). Ergebnis: Leer und `0` stehen sich in 50 Zeilen je Elementtab gegenüber (Title/MD/H1/H2/MK 2 Length/Pixel Width).
- **Beleg 2:** `missing_ref_columns = Canonical Link Element 2/3, Meta Refresh 2, Meta Robots 2/3, X-Robots-Tag 2/3` auf allen Sites bzw. `HTML hreflang 4…8`. Keine der beiden Seiten hat dort Werte, trotzdem gilt der Export als „Spaltenabdeckung unvollständig“.
- **Kategorie:** Mapping/Format. Die Spec verlangt keine feste Spaltenzahl für zweite und weitere Vorkommen. Der Referenzexport erzeugt sie nur bei Bedarf.
- **Fix (os-parity):**
  - Fehlt eine nummerierte Spalte in der Referenz, gilt der Erwartungswert „leer oder 0“ (neue Normalisierung `absent_numbered = "empty_or_zero"`).
  - Fehlt sie auf beiden Seiten, ist das keine Lücke.
  - Fehlt sie in OS, obwohl die Referenz Werte hat, bleibt das ein Fehler.
- **Erzeugungsbezug:** Die alten Sites (Free, ältere Version) schreiben die Folgespalten immer, die neue Version nur bei Bedarf. Die Regel muss beide Generationen tragen.

### G02 Spaltensätze und Schlüssel der neuen Referenzversion (Mapping)
- **Security:**
  - Die Referenz nennt `Content Type` statt `Content` und hat `HTTP Version`. Mapping `security_all.toml:7` erwartet `Content`. Das ergibt `missing_ref_columns=Content` und `unmapped=Content Type, HTTP Version` auf allen 12 Security-Exporten.
  - Bulk-Inlinks ohne `Type` (`bulk_security_form_url_insecure_inlinks.csv`).
- **Validation:** Die Referenz hat zusätzlich `Size (Bytes)`, `Transferred (Bytes)`, `Total Transferred (Bytes)`, `CO2 (mg)` und `Carbon Rating`. Diese Spalten fehlen in TECH-281. Lösung: ignorieren mit Begründung, oder die Spec ergänzt sie.
- **Pagination:** Die Referenz hat kein `Occurrences`, dafür `HTTP rel="next" 1`, `HTTP rel="prev" 1` und `HTTP Canonical`.
- **Canonicals:** Die Referenz hat `HTTP rel="next"/"prev" 1`, das OS-Export nicht (`missing_os_columns`). TECH-011 nennt sie nicht.
- **External (alte Sites):** `Redirect URL/Type` fehlen in der Referenz.
- **Filterexporte mit eigenem Spaltensatz:**
  - Lorem: `Address, Occurrences, Indexability…`
  - Readability: `Word Count … Readability`
  - Soft 404: `Title 1 … H2-1 Length, Word Count`
- **Reports:**
  - SERP Summary: Schlüssel `URL`, doppelte Spaltennamen `Character Length`/`Pixel Length`.
  - Images-Missing-Size-Bulk: `Source/Destination` statt `From`.
  - Hreflang-Reports ohne `Address`.
  - Non-Indexable Canonicals ohne `Canonical`.
- **Kettenberichte:** Die Referenz schreibt nach dem letzten Sprung einen weiteren Sprungblock für das Endziel (z. B. `Status Code 3 = 200`, `Status 3 = OK` in `canonical-chain-a`; in `parity-m2-list/ref/redirects_redirect_chains.csv` hat jede Zeile `Status Code 1 = 200`, auch ohne Weiterleitung). Dazu kommt ein Alias `Number of Redirects/Canonicals`. Lösung im Mapping: Sprung `n+1` wird gegen `Final Status Code`/`Final Status` geprüft bzw. ignoriert. Betrifft auch die alten Sites `out-basic-site`/`parity-response` (`redirect_chains`).
- **Kategorie:** Mapping. Teilweise kleine Spec-Lücken: HTTP-rel-Spalten in TECH-011/031, Validation-Größenspalten. Vorschlag: zunächst ignorieren mit Begründung. Ob die Spalten ergänzt werden, entscheidet der Orchestrator (SQ-13).

### G03 Exportaufruf und Dateinamen (Runner)
- **Beleg:** `crates/os-parity/src/run.rs:152-160` übergibt `--export-tabs`, `--bulk-export` und `--save-report` je als **eine** Liste an einen CLI-Aufruf. Für `parity-m2-links` steht im Manifest `report:Crawl Path Report`. Diesen Namen kennt das CLI nicht. Crawl Path ist bei OS ein Detail-Export (`crates/os-export/src/detail_export.rs`, OUT-023 „nicht im Reports-Menü“). Folge: Exit 2, `crawl_overview.csv` von m2-links wird nicht erzeugt.
- Bulk-Links heißen in OS `Links:<Name> (Bulk)` (`catalog/bulk_links.rs:30-38`), Hreflang-Reports haben `file_stem` „Hreflang … Report“. Die Dateisuche (`find_file`) findet beide nicht.
- **Fix:**
  - Reports einzeln aufrufen, damit ein Fehler isoliert bleibt.
  - Manifest-Feld `os_file` oder eine Namensregel für Bulk- und Report-Dateien.
  - Crawl Path: entweder CLI-Unterstützung (SQ-14) oder vorerst `skip` mit Begründung und Fixture-Beleg OUT-023.
- Hierher gehören auch Q02-E01 und der Rest von Q02-P05.

### G04 Auswertung `verified`, Manifest-Altlasten
- **Ist-Zustand:**
  - `belege.py` prüft je Tab **alle** Slugs und alle Sites. Eine einzige rote Filterdatei (z. B. `security_bad_content_type.csv` mit Zusatzzeile `/submit`) sperrt alle 14 Security-IDs.
  - Der Nenner 160 enthält **TECH-301…305 (Change Detection)**. Dafür gibt es keinen Export, nach M2.md 6.1 bzw. K2-14 ist das Thema nicht referenzfähig. Richtig sind 155.
  - Die Vollständigkeitsprüfung steckt nur im Skript, nicht in `coverage/evidence.rs` (Q02-P02).
  - `evidence.toml` hat keinen Selektor für Site und Datei.
- **Offene Q02-Punkte:**
  - P01: M2-Filter in `crawl_overview.toml` und Link Score/Inlinks in `links_all.toml` scharf schalten.
  - P03: `real_repo_files.rs:248` erwartet Pending, Ist ist Present.
  - P04: Skip OUT-084 in `parity-response/manifest.toml` entfernen.
  - P06: DEV-016/022, M2-Geltungsbereich.
- **Kategorie:** Messung. Die Granularität muss der Orchestrator entscheiden (SQ-01).

### G05 Ausnahmen mit Spec-Begründung, deren Geltungsbereich fehlt (DEV)
- **DEV-003/004** (OS lässt Längen und Pixelbreiten bei Nicht-HTML, Nicht-2xx und fehlendem Element leer, die Referenz schreibt 0): Die Einträge gelten nur für `internal_all`, nicht für `internal_m2` und `list_internal`. Davon betroffen: rund 1.700 Befunde, z. B. `H2-1 Length` 339, `MD 1 Pixel Width` 336, `Title 2 Length` m2-technical 66.
- **OUT-080** (needs_config/needs_rendering → leerer Zähler): Overview-Zähler Near Duplicates, Background Images, Incorrectly Sized und Broken Bookmark auf 9 Sites: Referenz 0, OS leer.
- **TECH-141:** Eigener Start-Host und Port sind ausgenommen. Die Referenz meldet jede Seite mit Links auf `127.0.0.1:8908`: 137 Zeilen, Overview z. B. synthetic 379→0.
- **OUT-015:** Ungültige Listenzeilen bleiben erhalten (2 Zeilen).
- **TECH-211:** Die Referenz zählt „Sitemaps: All“ auch ohne Sitemap, nämlich alle internen HTML- und XML-Seiten (m2-onpage 56).
- **TECH-130/K-07:** Inlinks 14→13.
- **Neu, Messartefakt der Referenz:** Im Overview von m2-security steht „Missing Content-Security-Policy Header = 17“, die Filterdatei der Referenz hat aber 13 Zeilen. Vier Seiten senden den Header (`scenario.toml`), 13 ist also richtig. Wahrscheinlich ist das eine Folge davon, dass Zähler und Filterexport im Headless-Lauf getrennt aus dem gespeicherten Crawl erzeugt wurden. Vorschlag: Eintrag als Referenzartefakt (z. B. `REF-ART-01`) mit diesem Beleg. Er darf nicht als REF-FREE gelten.
- **Kategorie:** Referenzbesonderheit. Die Spec begründet diese Fälle, sie sperren `verified` nicht. Die Einträge legt N06 an.

### G06 Nachkommastellen
- Die Referenz schreibt `Text Ratio`, `Flesch Reading Ease Score` und `Average Words Per Sentence` mit 3 Stellen. Die Spec (ONPAGE-013, ONPAGE-149) und OS (`Float { decimals: 1 }`) verwenden eine Stelle. `4.058` gegen `4.1` liegt außerhalb der Toleranz von 1 %.
- Zwei echte Abweichungen kommen dazu: `validation-body-before-html` 27.397 vs 26.0 und `validation-head-not-first` 30.588 vs 29.4. Text-Ratio-Grundlage bei kaputtem Markup gehört zu G17/N09.
- **Vorschlag:** Spec auf 3 Stellen umstellen, wie die Referenz (SQ-08). Alternativ Toleranz im Mapping.

### G07 Content-Export ohne Lesbarkeitsspalten, Soft-404-Spaltensatz
- `missing_os_columns = Sentence Count, Average Words Per Sentence, Flesch Reading Ease Score, Readability, Closest Similarity Match, No. Near Duplicates` in `content_all.csv` (m2-onpage).
- Die Spalten sind als `p1_column` mit `needs: Config("extraction.readability")` definiert (`tabs/content.rs:145-200`). Sie erscheinen im CLI-Tab-Export nicht. Wahrscheinliche Ursache: `default_visible`/`needs` werden bei der Export-Spaltenwahl als „ausgeblendet“ gewertet. Zu prüfen in der Export-Spaltenwahl (`crates/os-export/src/pipeline.rs` bzw. Views).
- Der Soft-404-Filter hat `default_columns: &["Matched Phrase"]` (`content.rs:289`). Der Filterexport enthält dadurch nur diese Spalte, `Address` fehlt (Exportfehler „Schlüsselspalte Address fehlt im OpenSpider-Export“).
- **Kategorie:** Implementierungsfehler, Größe S.

### G08 Flesch-Werte und Labels
- **Beleg:** Die Referenz liefert für `{BASE}/` (m2-onpage) W=110, S=111, AWPS 0.991, Flesch 36.629, Label „Hard“. Auf den alten Sites gibt es große Mengenabweichungen (technical „Readability Difficult“ 74→10, „Very Difficult“ 0→79; synthetic 0→268). OS bewertet viel strenger. Die Ursache liegt wahrscheinlich in der Satzzählung (A-41: Block = Satz) und der Silbenheuristik.
- Die Referenz verwendet die Labels `Very Easy, Easy, Fairly Easy, Normal, Fairly Hard, Hard, Very Hard`. Die Filter „Readability Difficult/Very Difficult“ entsprechen „Hard/Very Hard“ (m2-onpage: 5 bzw. 23 = Anzahl der Labels).
- **Kategorie:** I + S. Die Labels sind mit V-5 belegt, also nur noch eine Datenanpassung. Für W, S und Silben gibt es jetzt Referenzwerte je Seite. Die Kalibrierung braucht erst G07, damit die OS-Werte überhaupt im Export stehen.
- **Größe:** M.

### G09 Missing Size Attributes
- `crates/os-analyze/src/tabs/images.rs:137` verlangt `Requirement::RenderingOrConfig("images.parse_inline_css_backgrounds")`. ONPAGE-179 nennt als Konfiguration „keine“.
- Folgen: Der Filterexport ist leer (Referenz: `wide.png`, `picture.png`). Der Overview-Zähler ist leer statt 2/1/9/0 auf 9 Sites. `crates/os-analyze/src/analysis/steps/images.rs:14` prüfen, ob der Teilschritt ebenso ausgelassen wird.
- **Kategorie:** I, Größe S.

### G10 Bad Content Type
- `crates/os-parse/src/security.rs:211` zählt `content_types.len() != 1` als Treffer. Weiterleitungen ohne Content-Type werden dadurch zu Treffern: out-basic `/old-page`, response 22, m2-technical 4, m2-list 2.
- Das Sniffing meldet `/submit` (HTML-Body, `application/octet-stream`).
- Die Referenz meldet nur Endungs-Widersprüche (`wrong.js` text/plain, `wrong.css` text/html).
- **Kategorie:**
  - I: Ein fehlender Header ist nach Regel 1 nicht „syntaktisch ungültig“.
  - S: Sniffing bei generischem Typ und die Obsolet-Liste, die die Referenz nicht anwendet (SQ-13).
- **Größe:** S.

### G11 Nicht gecrawlte Linkziele in Tabellen und Tiefe
- **Beleg nofollow:**
  - `nofollow-only.html` ist nur über `<a rel="nofollow">` von `nofollow-source.html` verlinkt.
  - Das Profil setzt `follow_internal_nofollow = false`.
  - CRAWL-056: „URLs, die ausschließlich über nofollow-Links erreichbar sind, fehlen in der Crawl-Tabelle.“
  - OS führt die URL in Internal, weil der Frontier gespeicherte, aber nicht gefolgte Ziele mit `in_tables: c.store` aufnimmt (`frontier.rs:532/566`).
  - Folgen: Zusatzzeile im All-Anchor-Text-Bulk und Internal-URL-Menge m2-links 142→143.
- **Beleg Paginierung:**
  - `pagination-unlinked.html` ist nur über `<link rel="next">` erreichbar, Profil `pagination.crawl = false`.
  - Die Referenz führt die URL nicht.
  - CRAWL-038 sagt allgemein „erscheint in den Tabellen ohne Statuscode“, CRAWL-047 sagt „nicht angefragt“.
  - Das ist ein Spec-Konflikt (SQ-12).
- **Beleg Tiefe:**
  - `page-3.html` ist per `<a>` erst ab `page-2` (Tiefe 2) verlinkt, also Tiefe 3. Per rel=next von `pagination-multiple.html` (Tiefe 1) wäre es Tiefe 2.
  - OS vergibt 2. Nach CRAWL-047 werden rel=next/prev „weder gecrawlt noch zur Entdeckung genutzt“.
  - Das ist I. Folgefehler: TECH-132 High Crawl Depth 1→0.
- **Größe:** M (os-engine).

### G12 Internal-Filter nach Inhaltstyp
- **Beleg:** Das Overview m2-onpage der Referenz führt `Media 1, Fonts 1, XML 1, Plugins 1, Other 1`. Die Referenzdatei `internal_other.csv` enthält nur `text-links.pdf` (`application/octet-stream`).
- A-49 (ONPAGE-023: „die Referenz führt solche URLs unter Other“) ist damit widerlegt. OS legt sie im Paritätsmodus unter Other (`tabs/internal.rs:354`, `tabs/mod.rs:1165`).
- Antworten ohne Content-Type führt die neue Referenz unter „Other“ (m2-technical 4, Unknown 0). Das entspricht DEV-014, der Eintrag muss auf die neuen Sites ausgeweitet werden.
- **Erzeugungsbezug:** Die lizenzierte, neue Referenzversion unterscheidet sich hier von der alten Free-Referenz.
- **Kategorie:** S (A-49 aufheben, Paritätsmodus ändern) + R. Größe S.

### G13 Inline-CSS-Hintergrundbild
- **Beleg:**
  - `tests/fixtures/parity-m2-onpage/images.html`: `<div style="background-image:url(/background.png)">`.
  - Die Referenz führt `background.png` in keinem Export (Internal 59, Response Codes 59, URL 59, Images 3).
  - OS entdeckt das Bild über `css::extract_inline` (`crates/os-parse/src/lib.rs:179`).
  - CRAWL-040 verlangt ausdrücklich: „`<style>`-Blöcke und `style`-Attribute im HTML werden ebenfalls nach `url()` durchsucht.“
- **Kategorie:** S. Die Spec ist durch die Referenz widerlegt, zumindest für `style`-Attribute. Zu `<style>`-Blöcken gibt es keinen Beleg.
- Hebel: entsperrt die URL-Mengen von 4 Tabs (35 IDs).
- **Größe:** S.

### G14 Grundmengen technischer Tabs und der Security-Headerfilter
- **Directives:**
  - Referenz `directives_all` 62 = alle internen 67 URLs ohne `loop-a`, `loop-b`, `tail`, `redirect-canonical` (3xx) und `pagination-404-target` (404).
  - TECH-051 legt „jeder Status“ fest.
  - Auf dem alten parity-technical ebenso: Referenz 107, OS 110.
- **Pagination und Hreflang:** Referenz 60 = HTML-2xx-Seiten. OS nimmt `pagination-404-target.html` (404) zusätzlich auf. TECH-031/071 nehmen gecrawlte Ziele ohne 2xx ausdrücklich auf.
- **Validation:**
  - Referenz `validation_all` 67 = **alle** internen URLs (inklusive `large.css`, `head.png`, 3xx, 404).
  - TECH-281: „interne HTML-Seiten mit Status 200“.
  - Overview auf allen Sites (out-basic 18→12, response 46→13, synthetic 400→379).
- **Security-Headerfilter (ONPAGE-039…042):**
  - Die Referenz zählt nur Antworten ohne 3xx/4xx: out-basic 16 = 18 minus `/missing.html` (404) minus `/old-page` (301); response 15 von 46; m2-technical 62 von 67.
  - Nicht-HTML mit 200 zählt mit (`picture.png`, `wrong.js` in der Referenzdatei).
  - A-12 nennt „alle internen URLs mit HTTP-Antwort“.
- **Kategorie:** S, die Spec ist jeweils widerlegt. Nach der Entscheidung ist die Umsetzung je Tab klein: Populationsmakro. Größe M insgesamt.

### G15 Elemente außerhalb von `<head>` und ungültige Canonical-Annotation
- **Canonical außerhalb des Kopfs:** Für `canonical-outside.html` gibt die Referenz `Occurrences 0`, `Canonical Link Element 1` leer, `Indexable` aus und nennt die Seite trotzdem im Filter „Outside <head>“. OS: `Occurrences 1`, `Canonicalised`. TECH-023 legt fest: „Ein Canonical im Body wird trotzdem … angezeigt und zählt in Occurrences.“
- **Ungültiges Attribut:** `canonical-invalid-attribute.html` verhält sich in der Referenz genauso. TECH-021 sagt dazu nichts.
- **Hreflang außerhalb des Kopfs:** `lang-outside.html` hat in der Referenz `Occurrences 0`. Die Seite steht nicht in „Missing Self Reference“ und nicht in „Missing x-default“. TECH-086 legt fest: zählt für alle anderen Filter.
- **Folgen:**
  - canonicals contains/canonicalised/relative je 8→10
  - Bulk-Canonical-Inlinks
  - Indexierbarkeit in 8 Exporten
- **Kategorie:** S. Größe M.

### G16 Indexierbarkeit: `unavailable_after` und HTTP-Refresh auf sich selbst
- **Fall `directive-7.html`:** `unavailable_after: 01 Jan 2000`. Referenz: Non-Indexable „Unavailable After“. TECH-063: „ändert die Indexierbarkeit nicht (Festlegung)“.
- **Fall `refresh-self.html`:** `Refresh`-Header auf die eigene URL (K-69). Die Referenz meldet Non-Indexable „Redirected“ und führt die Seite in „Internal Redirection (HTTP Refresh)“, wo OS sie nicht hat. TECH-001/K-57 regelt nur den Selbst-Refresh per **Meta**, mit dem Ergebnis Indexable.
- **Kategorie:** S. Je 4 Zellen in allen Technik-Tabs, Response Codes, Internal. Größe S.

### G17 Weitere Technik-Filterlogik
- **Unlinked Canonical:** Die Referenz listet `canonical-chain-b.html` und `canonical-unlinked-target.html`, also das **Ziel**. Die Spec TECH-020 listet die Quelle, OS ebenfalls. Auf alt parity-technical: Referenz 0, OS 3. → S.
- **Fragment:** Die Referenz zeigt `canonical-target.html` ohne `#fragment` (6 Zellen). → S/I klein. Anzeige ohne Fragment, Filter TECH-018 bleibt.
- **Hreflang-Rücklinkfilter:**
  - inconsistent: Referenz 2, OS 5
  - non-canonical: Referenz `lang-noncanonical-return`, OS `lang-noncanonical`
  - noindex: `lang-noindex-return` fehlt in OS
  - unlinked: `lang-unlinked` ist in der Referenz per Hyperlink erreichbar (Tiefe 1), OS meldet die Seite trotzdem
  → I (mit G15 zusammen prüfen).
- **Pagination Sequence Error:** `pagination-404.html` (next auf 404-Ziel) fehlt in OS → I.
- **„<head> not first“:** `validation-head-missing.html` fehlt in OS → I.
- **Bulk-Inlinks:** Die Referenz nennt den Typ `HTML Rel Next/Prev`. Weitere Zusatzzeilen folgen aus G15/G16.
- **Größe:** M–L.

### G18 Link Score
- 892 Befunde auf out-basic, parity-technical, parity-synthetic und m2-links. Alle Werte weichen ab, auch in der Rangfolge (`/services.html` 13 vs 32).
- TECH-145 ist eine OS-eigene PageRank-Variante. Die Spec selbst sagt: „Welche Variante der Referenz entspricht, entscheidet der Benchmark (Anhang A Punkt 36).“ Mit vier Referenzgraphen bis 400 Knoten liegt der Benchmark jetzt vor.
- **Kategorie:** S + I (Kalibrierung). Größe L, Ergebnis ungewiss.
- **Vorschlag:** M3, siehe SQ-10.

### G19 Schwellen und Heuristiken
- **TECH-137:** `external-99.html` hat 99 externe Follow-Links (gezählt in der Fixture) und steht in der Referenz in „Pages With High External Outlinks“. Die Spec-Schwelle ist 100, ihr Akzeptanzfall sagt ausdrücklich „`/99-external.html` → nicht“. → S.
- **TECH-133:** m2-technical Referenz 56, OS 41. Ursache offen. Prüfen: zählt die Referenz Seiten ohne **folgbare** interne Hyperlinks auf HTML-Ziele? Die Bedingung in der Spec lautet `Outlinks = 0` → I/S.
- **ONPAGE-077:** `/?s=stone` ist in der Referenz nicht „Internal Search“, `/search?q=river` schon. A-26 ist eine OS-eigene Heuristik → S.
- **ONPAGE-147:** Soft 404 2→3 (eigene Phrasenliste A-39) → S/I.
- **ONPAGE-094:** m2-technical 30→32. Wahrscheinlich Validierungsseiten mit kaputtem Markup, also Titelextraktion → mit G17.
- Alt parity-technical TECH-136 0→1, TECH-139 3→6, TECH-077/075 0→1: alte Free-Referenz. Erst nach den neuen Regeln prüfen, dann DEV oder Fix.

### G20 Wortzählung und Inhaltsbereich
- **Belege aus `parity-m2-links`:**
  - `anchors.html` `<a>click here</a><a>read more</a><a></a><a>River guide</a>`: Referenz 6, OS 4.
  - `external-99.html`: 99 vs 1.
  - `graph/4.html`: 4 vs 3.
  - `uncrawlable.html`: 4 vs 3.
  Die Referenz trennt aneinandergrenzende Inline-Anker. OS verkettet die Texte ohne Leerraum.
- `positions.html` (je ein Link in nav, header, footer, aside, main): Referenz 1, OS 3. Die Referenz schließt `<header>` und `<aside>` aus, ONPAGE-140 schließt sie ein.
- Q02 hat das als Referenzbesonderheit verbucht. Eine ausdrückliche Festlegung zu Ankergrenzen findet sich in ONPAGE-141 nicht.
- **Kategorie:** S. Größe M.

### G21 Pixelbreite der Sigma-Formen
- `unicode-4`/`unicode-5`: 12 statt 11, `unicode-6`: 10 statt 11 (Σ, σ, ς; M2-S04-Fixture). Das sind nur 3 Seiten, sie sperren aber Page Titles (9 IDs).
- **Kategorie:** I (Breitentabelle), Größe S.

### G22 Sitemap `lastmod`/`changefreq`
- Referenz `sitemap_default.xml`: `{BASE}/` `<lastmod>2026-10-01</lastmod>` (OS `2026-10-01T10:00:00+00:00`).
- `lorem.html` (ohne `Last-Modified`): Referenz `2026-10-30` (Crawl-Datum) und `changefreq daily`, OS ohne lastmod und `monthly`.
- OUT-123/125 sind OS-Festlegungen → S.
- **Erzeugungsbezug:** Das Crawl-Datum macht die Datei zeitabhängig. Das Mapping muss es als „Crawl-Datum“ normalisieren.

### G23 Ketten-Reports
- **Listenmodus:** Die Referenz hat 46 Zeilen, eine je Listen-URL, auch mit 0 Weiterleitungen. OS hat eine Zeile. OUT-085 verlangt „jede Listen-URL ist Startpunkt einer Zeile“ → **I** (Q02 hatte Mapping).
- **Chain Type:** Referenz „Mixed“ gegen OS „Redirect & Canonical“. OUT-083 legt den OS-Wert ausdrücklich fest → R (DEV) oder S.
- **Schleife `/tail`:** Die Referenz zeigt `Final Address loop-a`, `302`, `Found` und markiert „Loop Detected“ im letzten Sprung. Die Spec sagt: Final Address leer, `Redirect Loop` → R (DEV-030 historisch).
- **Zeile für `loop-a`:** In OS gibt es keine eigene Zeile für das Zwischenglied (OUT-081 Festlegung) → R.
- **Größe:** M (Listenmodus I).

### G24 Linkposition `<aside>`
- Die Referenz schreibt `Aside`. INTEG-081 ordnet `aside` der Position `Sidebar` zu. Die Werteliste in OUT-031 kennt beide.
- **Kategorie:** S, Größe S.

### G25 Overview-Zeile „External Redirection (JavaScript)“
- Die Zeile fehlt im OS-Overview auf allen 9 vergleichbaren Sites. In der Registry existiert sie (`tabs/mod.rs:294`).
- Wahrscheinliche Ursache: Abschnittszuordnung oder Filter für needs_rendering im Overview-Writer (`crates/os-export/src/reports/overview.rs`).
- **Kategorie:** I (prüfen), Größe S.

## 5 Abweichungen gegenüber der Q02-Klassifizierung

| Q02 | geprüft | Grund |
|---|---|---|
| ca. 600 „Impl“ `…2 Length/Pixel Width` (ONPAGE-009 u. a.) | Mapping (G01) | Die Referenz lässt die Spalte weg, OS schreibt bewusst 0 |
| 296 „Impl“ Lesbarkeitswerte | I, aber Spaltenauswahl im Export (G07); Werte erst danach messbar | `missing_os_columns` |
| 152 „Impl“ Lastmod/Changefreq | S (G22) | OUT-123/125 OS-Festlegung |
| Security-Header-Zähler „Impl“ | S (G14) | A-12 durch Referenz widerlegt |
| Directives/Validation/Pagination/Hreflang-Populationen „Impl“ | S (G14) | TECH-051/281/031/071 festgelegt |
| Canonical/Hreflang außerhalb `<head>` „Impl“ | S (G15) | TECH-023/086 Festlegung |
| `unavailable_after` „Impl“ | S (G16) | TECH-063 Festlegung |
| `background.png` „Impl“ | S (G13) | CRAWL-040 verlangt die Entdeckung |
| Listenketten 90 Zeilen „Mapping“ | I (G23) | OUT-085 |
| ONPAGE-023 Other „Refe“ | S (G12) + R | A-49 widerlegt |
| Word Count „Refe“ | S (G20) | keine ausdrückliche Festlegung zu Ankergrenzen; `positions.html` widerlegt ONPAGE-140 |
| Missing Size Attributes Overview „Impl“ in os-export | I in os-analyze (G09) | falsche Voraussetzung im Filter |

## 6 Pakete

Freie Nummern: N01–N04 sind vergeben, N05 ist für die i18n-Texte reserviert. Die neuen Pakete heißen **M2-N06 … M2-N11**, dazu der Wiederholungslauf **M2-Q02b**. Alle laufen auf `server` (Linux-Build-Server, Codex, `gpt-6.1-sol` medium). Entwürfe stehen in `packages.json`.

| Paket | Titel | Gruppen | Welle | hängt an | Größe |
|---|---|---|---|---|---|
| **M2-N06** | Paritätsmessung: Mappings, Runner, verified-Auswertung, Ausnahmen | G01–G05, G06 (Toleranz bis Entscheid), Teile G23 (R) | 4q (zuerst) | M2-Q02 (Branch `build/m2-q02`) | L |
| M2-N07 | Onpage-Parität: Entdeckung, Bildfilter, Inhaltstyp-Filter, Pixelbreite, URL-Heuristik, Linkposition | G09, G12, G13, G21, G19 (ONPAGE-077), G24 | 4r | N06 + Entscheide SQ-02/07/13b/13d | M |
| M2-N08 | Inhaltsanalyse: Content-Export, Lesbarkeit, Wortzählung, Nachkommastellen | G06, G07, G08, G20, G19 (ONPAGE-147) | 4r | N06 + SQ-08/09 | M |
| M2-N09 | Technik-Tabs: Grundmengen, außerhalb `<head>`, Indexierbarkeit, Filterlogik | G14 (ohne Security), G15, G16, G17, G19 (ONPAGE-094) | 4r | N06 + SQ-03/04/05/06 | L |
| M2-N10 | Security, Crawl-Entdeckung, Links-Filter | G10, G14 (Security), G11, G19 (TECH-132/133/137) | 4r | N06 + SQ-03/12/13a/13c | M |
| M2-N11 | Exporte: Ketten-Reports, Sitemap-Generator, Overview, Crawl Path per CLI | G22, G23 (I), G25, G03 (CLI-Teil) | 4r | N06 + SQ-11/14 | M |
| M2-Q02b | Paritätslauf 2 und Gate-Beleg B3 | – | 4s | N06–N11 | M |
| (M3) | Link-Score-Kalibrierung | G18 | M3 | Q02b | L |

### Besitz (disjunkt in Welle 4r)
- **N06:**
  - `crates/os-parity/**`
  - `tests/parity/**`
  - `tests/golden/parity/*/manifest.toml` (nie `ref/`)
  - `docs/plan/M2-acceptance/parity/**`
  - Bericht
  N06 läuft vor den anderen. In 4r schreibt nur noch Q02b in diesen Bereich.
- **N07:**
  - `os-parse/src/{lib.rs (nur Inline-CSS-Aufruf),css.rs,pixel/**}`
  - `os-analyze/src/tabs/{images,internal,url}.rs`
  - `tabs/mod.rs` (nur `parity_filters`-Block)
  - `analysis/steps/images.rs`
  - `os-parse/src/link_position.rs`
  - `os-model/src/config/**` (nur neue Option zum Inline-Style und Standardregel `aside`) + Schema
- **N08:**
  - `os-parse/src/{readability,text,content}.rs`
  - `os-analyze/src/tabs/content.rs`
  - Export-Spaltenwahl nur, falls die Ursache dort liegt. Dann `crates/os-export/src/pipeline.rs`, abgestimmt mit N11 (N11 fasst `pipeline.rs` nicht an).
- **N09:**
  - `os-analyze/src/tabs/{directives,pagination,hreflang,canonicals,validation,response_codes}.rs`
  - `analysis/steps/{canonicals,hreflang,pagination}.rs`
  - `os-parse/src/{canonical,hreflang,validation,refresh}.rs`
  - `os-model/src/indexability.rs`
  - `os-engine/src/recompute/**` (nur Indexierbarkeit)
  - `os-export/src/catalog/bulk_canonicals.rs`
- **N10:**
  - `os-parse/src/security.rs`
  - `os-analyze/src/tabs/{security,links}.rs`
  - `os-engine/src/{frontier,decide}.rs`
- **N11:**
  - `os-export/src/reports/{redirects,redirect_canonical_chains,canonical_chains,overview}.rs`
  - `os-export/src/sitemap_gen/**`
  - `os-cli/src/{args.rs,exec.rs}` (nur Crawl-Path-Export)

**Konfliktrisiko:**
- `os-model/src/config/**` liegt allein bei N07 (Option und `aside`-Regel). Deshalb ist G24 bei N07.
- `os-engine/src/frontier.rs` liegt nicht bei N04. N04 besitzt `memory.rs`, `worker.rs` und `crawl.rs`.
- Vor dem Start prüfen, ob Q03 oder E07 `frontier.rs` ändern.

### Reihenfolge und Dauer (grob)
1. **Sofort:**
   - N06 auf Basis `build/m2-q02`, ca. 1–1,5 Tage inklusive zwei voller Paritätsläufe.
   - Parallel entscheidet der Orchestrator die Spec-Fragen (Abschnitt 8). Die DEV-Einträge für entschiedene „Spec bleibt“-Fälle gehen in N06 oder, falls später entschieden, in Q02b.
2. **Nach dem Merge von N06:** N07, N08, N09, N10 und N11 parallel. Erwartung: N07/N08/N10/N11 je ca. 0,5–1 Tag, N09 ca. 1,5 Tage.
3. **Danach:** Integrationsrunde und Q02b, ca. 0,5–1 Tag.

**Kritischer Pfad:** N06 (1–1,5 T) → N09 (1,5 T) → Integration + Q02b (0,5–1 T) = **ca. 3–4 Arbeitstage**. Voraussetzung: Die Spec-Entscheide liegen vor dem Start von 4r vor.

## 7 Gate-Einschätzung (B3, insbesondere B3g ≥ 60 %)

Rechnung mit der heutigen Definition (Tab-weise, alle Slugs und Sites). Kumulierter Stand nach jedem Paket, Nenner 160 bzw. 155 ohne Change Detection (`tabcheck.py`):

| Stand nach | grüne Tabs | verified | % (160 / 155) |
|---|---|---:|---:|
| N06 | Meta Description, H1, H2, Meta Keywords, External | 24 | 15,0 / 15,5 |
| + N07 | + Page Titles, Images, URL | 51 | 31,9 / 32,9 |
| + N10 | + Security | 64 | 40,0 / 41,3 |
| + N09 teilweise | + Response Codes, Validation, Directives | 92 | 57,5 / 59,4 |
| + N08 | + Content | **100** | **62,5 / 64,5** |
| + N09 vollständig | + Canonicals, Pagination, Hreflang | 137 | 85,6 / 88,4 |
| + Link Score + Internal | + Links, Internal | 155 | 96,9 / 100 |

### Vor dem Gate zwingend
- N06 (Messung).
- Die Spec-Entscheide SQ-01 bis SQ-05 und SQ-12.
- N07, N08, N10.
- Von N09 mindestens Directives, Validation, Response Codes (G14/G16) und die Teile von G15, die Directives betreffen (`Canonical Link Element 1` in `directives_all`).
- Damit sind ca. 62 % erreichbar (100 IDs). Mit Canonicals, Pagination und Hreflang aus N09 ca. 86 %.

### Weitere Gate-Kriterien
- **B3b** (Schwellen ≥ 99/97 %, `Internal:All` je Site identisch) braucht:
  - G11, G13
  - G23 (Listenketten)
  - G22 (Sitemap ≥ 97 %)
  - G20 (sonst Word Count in Internal unter 99 %, falls kein DEV)
- **B3c** (Overview-Zähler aller Filter identisch oder per DEV/REF-FREE begründet) braucht für **jede** Zeile in Abschnitt 3 einen Fix oder einen DEV-Eintrag. Mit den Paketen ist das erreichbar. Link Score (G18) und TECH-141 laufen über DEV.
- **B3a:** Die 38 M1-Vergleiche bleiben grün. N06 muss das nach jeder Mapping-Änderung belegen.

### Nach M2-backlog bzw. M3 verschiebbar
- G18 Link Score. Vorschlag: DEV „TECH-145 OS-Festlegung, Kalibrierung M3“ und Links/Internal ohne `verified`.
- Alt-Site-Restbefunde G19 (TECH-136/139 alte Referenz).
- G24 Aside (klein, liegt in N07, nicht gate-relevant).
- Crawl Path per CLI (G03, falls SQ-14 „später“).
- Chain-Type- und Schleifen-Festlegungen G23 (R): nur DEV nötig.

### Einschätzung
60 % sind mit N06–N10 erreichbar. Link Score ist dafür nicht nötig. Das gilt, wenn die Spec-Fragen zügig entschieden werden, egal in welche Richtung (DEV oder Fix).

**Größtes Risiko:** G08/G20 (Content) und G15/G17 (Technik) sind ursachenseitig nicht ganz geklärt. Fallen Content und alle drei Technik-Tabs aus, bleibt es bei ca. 59 % (155er-Nenner) bzw. 57,5 % (160er-Nenner). Dann entscheidet SQ-01.

**Mit Filter-Granularität (SQ-01-Vorschlag)** kommen die IDs hinzu, deren Tab-All-Export grün ist, während einzelne andere Filterexporte desselben Tabs rot bleiben. Das betrifft vor allem Security (13 IDs schon vor N10), dazu Tabs, deren Filterexporte nach den Teilfixes einzeln grün werden. Grob gerechnet bringt das +10 bis +15 IDs. Den Bedarf an N09 (Directives/Response Codes) ersetzt es nicht.

## 8 Spec-Fragen an den Orchestrator

| Nr. | Frage | Beleg | Vorschlag |
|---|---|---|---|
| **SQ-01** | B3g-Definition: Ist `verified` Tab-weise alles oder nichts, oder gilt es je Filter-ID? Gehört Change Detection (TECH-301…305) in den Nenner? | Ein roter Filterexport sperrt 14 Security-IDs. Change Detection ist nach K2-14 nicht referenzfähig. | Je Filter-ID verified, wenn (1) der Tab-All-Export auf allen Sites grün und spaltenvollständig ist und (2) der eigene Filterexport, falls vorhanden, grün ist. DEV erlaubt, REF-FREE nicht. Nenner 155. Umsetzung in `coverage/evidence.rs` (N06). |
| **SQ-02** | CRAWL-040: `style`-Attribute nach `url()` durchsuchen? | `background.png` fehlt in allen Referenzexporten (G13) | Der Standard folgt der Referenz: `style`-Attribute ohne Rendering nicht durchsuchen. Neue Option, Default aus. `<style>`-Blöcke vorerst unverändert (kein Beleg). CRAWL-039/040 anpassen. |
| **SQ-03** | Grundmengen: Directives (TECH-051), Pagination/Hreflang (TECH-031/071), Validation (TECH-281), Security-Headerfilter (A-12) | G14, auf 9 Sites konsistent | Alle vier an die Referenz anpassen: Directives nur 2xx; Pagination/Hreflang nur interne HTML-2xx (Ziele ohne 2xx nur im Filter TECH-036); Validation = alle internen URLs; Headerfilter = interne URLs mit 2xx (jeder Inhaltstyp) |
| SQ-04 | Canonical/Hreflang außerhalb `<head>`, Canonical mit ungültigem Attribut (TECH-021/023/086) | G15 | Der Referenz folgen: Solche Annotationen zählen nicht für Werte, Occurrences, Indexierbarkeit und Folgefilter, sondern nur für den eigenen Problemfilter |
| SQ-05 | `unavailable_after` in der Vergangenheit (TECH-063), Selbst-Refresh per HTTP-Header (TECH-001) | G16 | Der Referenz folgen: Non-Indexable „Unavailable After“ (Statuswert neu in TECH-001); Header-Selbst-Refresh = Redirected und im Filter HTTP Refresh. Meta-Selbst-Refresh bleibt nach K-57 |
| SQ-06 | TECH-020 Unlinked: Quelle oder Ziel listen? | Die Referenz listet das Ziel (G17) | Ziel listen, Export „Unlinked Canonical“ unverändert mit Quelle und Ziel |
| SQ-07 | A-49 / ONPAGE-023: Media/Fonts/XML/Plugins als eigene Filter? Heißt „Flash“ jetzt „Plugins“? | Overview der neuen Referenz (G12) | A-49 aufheben, Filter im Paritätsmodus sichtbar, Other ohne diese Klassen. Namen nach Overview der Referenz. DEV-014 auf alle Sites ausweiten |
| SQ-08 | Lesbarkeit: Labels (A-54/V-5), Nachkommastellen (ONPAGE-013/149), Satzregel A-41 | G06/G08 | Labels der Referenz übernehmen (Very Easy … Very Hard; Filter Difficult = Hard). 3 Nachkommastellen für Text Ratio, Flesch, AWPS. A-41 an den Referenzwerten kalibrieren (N08) |
| SQ-09 | Wortzählung: Grenze an Inline-Elementgrenzen (ONPAGE-141)? `<header>`/`<aside>` im Inhaltsbereich (ONPAGE-140)? | G20 (`anchors.html`, `positions.html`) | Wortgrenze an jeder `<a>`-Grenze. `header`/`aside` standardmäßig ausschließen, wie die Referenz. Nur nach Prüfung der Folgen für M1-Belege (K-68) |
| **SQ-10** | Link Score: gate-relevant für M2? | 892 Befunde, eigene Formel (G18) | Für M2 DEV „TECH-145 OS-Festlegung“ (Spalte in links_all/internal ignoriert); Kalibrierung als M3-Paket mit den 4 Referenzgraphen. Links/Internal-IDs bis dahin nicht verified |
| SQ-11 | Sitemap `lastmod` Datum ohne Uhrzeit, Crawl-Datum ohne Header, changefreq daily (OUT-123/125) | G22 | Der Referenz folgen. Das Mapping normalisiert das Crawl-Datum |
| SQ-12 | Gespeicherte, nicht gecrawlte Ziele (Paginierung) in den Tabellen? (CRAWL-038 vs CRAWL-047) | `pagination-unlinked.html` fehlt in der Referenz (G11) | Nicht in Tabellen führen (wie nofollow nach CRAWL-056). CRAWL-038 präzisieren |
| SQ-13 | Kleine Festlegungen | jeweils Referenzdatei | (a) TECH-137: Schwelle so, dass 99 externe Links zählen, oder Akzeptanzfall streichen (genauer Wert unbekannt). (b) ONPAGE-077: Parameter `s` streichen. (c) ONPAGE-043: kein Sniffing bei `application/octet-stream`, Obsolet-Liste für Parität leer. (d) INTEG-081: `aside` → `Aside`. (e) OUT-083 „Mixed“ statt „Redirect & Canonical“ oder DEV. (f) TECH-011/031 und TECH-281: Spalten HTTP rel/Size/CO2 aufnehmen oder ignorieren |
| SQ-14 | Crawl Path Report per CLI (OUT-023) | Die Referenzdatei wurde per Oberfläche erzeugt, OS hat keinen CLI-Weg (G03) | CLI-Option `--export-crawl-path <URL>` (N11) oder Export aus der Parität nehmen und per Fixture-Test OUT-023 belegen |
| SQ-15 | Overview-Zähler der Referenz widerspricht der eigenen Filterdatei (m2-security CSP 17 vs 13) | G05 | Als Referenzartefakt dokumentieren (nicht REF-FREE). OS-Wert = Filterdatei |
| SQ-16 | `ref_settings` in den neuen Manifesten beginnen mit „Free, …“ und sagen später „nicht Free“ | `tests/golden/parity/parity-m2-*/manifest.toml` | Text bereinigen, sonst widersprechen sich Belege (REF-FREE-01/02 gelten nur für alte Sites) |

## 9 Erzeugungsbedingte Effekte (Mess- und Mappingthemen, keine Produktfehler)

- **Lizenzierte, neue Version statt Free:**
  - Andere Spaltensätze (G02).
  - Folgespalten nur bei Bedarf (G01).
  - Eigene Inhaltstyp-Filter (G12).
  - Lesbarkeits-Labels der Form „Hard“ (G08).
  - Dezimalpunkt statt Komma (alte Referenzen: `5,167`).
  Die Mappings müssen beide Generationen tragen, Kennzeichen über `ref_version` im Manifest.
- **Headless aus gespeichertem Crawl, Exporte getrennt:**
  - Overview-Zähler und Filterdatei können sich widersprechen (CSP 17 vs 13, G05/SQ-15).
  - Kettenreports enthalten einen Sprungblock für das Endziel (G02).
- **Listenmodus:**
  - `internal_all` per Oberfläche mit `Original Url` (Q02-Schlüsselregel, noch auf `build/m2-q02`, mit N06 mergen).
  - Kettenreports headless mit einer Zeile je Listen-URL (G23 ist ein Produktfehler nach OUT-085).
- **Automatische Crawl-Analyse:** konsistent mit OS, kein Effekt gefunden.
- **Crawl-Datum in Sitemaps (G22)** macht `sitemap_*.xml` zeitabhängig. Normalisierung im Mapping nötig.
- **Per Oberfläche erzeugte Dateien** (`crawl_path_deep8`, `sitemap_options`): Für Crawl Path gibt es keinen CLI-Weg in OS (G03/SQ-14).

## 10 Hinweise für die Planübernahme

- `packages.json` folgt dem Format von `M2-packages.json` und hat zwei zusätzliche Felder: `groups` (Gruppen-IDs dieser Triage) und `acceptance` (Abnahmekriterium). Beim Einpflegen kann der Orchestrator `acceptance` in `prompt_notes` übernehmen.
- Neue Wellen:
  - **4q**: N06
  - **4r**: N07–N11, parallel
  - **4s**: Q02b
- M2-Q04 sollte zusätzlich an **M2-Q02b** hängen. Das Prüfskript (`M2-check.py`) muss die Wellen 4q/4r/4s kennen.
- Die Triage selbst (diese Datei) sollte unter `docs/plan/M2-acceptance/parity/triage.md` eingecheckt werden. Die `prompt_notes` verweisen darauf.
- Die Spec-Entscheide SQ-02…SQ-13 müssen vor dem Start von Welle 4r in `docs/spec` stehen (K2-Eintrag je Entscheidung). Bei „Spec bleibt“ legt N06 bzw. Q02b den DEV-Eintrag an. Das Fachpaket lässt den Punkt dann aus.
