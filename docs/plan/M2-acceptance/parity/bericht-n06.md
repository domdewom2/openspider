# M2-N06 – Paritätsmessung nach Q02

Stand 30.10.2026, Linux-Build-Server, Branch `m2/n06`. Basis: `b4aebd7c1f9cb552ae74ff8f6c7431bd439c0fed` (`origin/m2/n06`, Q02 plus Orchestrator-Triage). Verbindlich: `triage.md` und `entscheidungen-sq.md`. Keine Produktdatei außerhalb von os-parity und keine Referenzdatei geändert; kein Push, keine Playwright-Läufe.

## Ergebnis und Abnahme

| Punkt | Stand | Beleg / Einschränkung |
| --- | --- | --- |
| (1) keine Exportfehler, m2-links Overview verglichen | **Nicht erfüllt** | 216 Vergleiche, vier begründete Skips, ein Produktfehler: `content_soft_404_pages.csv` enthält in OS nur Matched Phrase, keinen Address-Schlüssel (G07/N08). Alle Aufruf-/Dateinamenfehler behoben; m2-links Overview ist verglichen. |
| (2) vollständige Spalten | **Nicht erfüllt** | `missing_ref_columns = 0`, `unmapped_ref_columns = 0`. Noch 15 `missing_os_columns` in vier Content-Exporten, vollständig an G07/N08 übergeben. Keine fehlenden Produktspalten durch Ignore verdeckt. |
| (3) Rust-Coverage, Nenner 155, mindestens 24 IDs der fünf Tabs | **Erfüllt** | 36/155 = 23,226 %. Meta Description, H1, H2, Meta Keywords und External tragen zusammen 24 IDs bei; Security weitere zwölf. |
| (4) Security / Page Titles eingegrenzt | **Erfüllt für die Tab-/Filterbelege** | Security nur `security_bad_content_type.csv`, zusätzliche OS-Zeile `/submit`. Page Titles nur Title 1 Pixel Width bei unicode-4/5/6. Weitere Overview-Produktbefunde bleiben an N07–N11 offen. |
| (5) B3a | **Erfüllt im unveränderten M1-Prüfumfang** | 38/38 mit aktuellem Diff-Kern erneut grün; `b3a-n06.json`. Der volle M2-Lauf prüft zusätzlich M2-Zähler auf den alten Sites und bleibt dort teilweise rot. |
| (6) fmt / clippy / test | **Erfüllt** | 179 Tests, null Fehler, null Ignorierungen; einschließlich real_repo_files. Neutralitätsprüfung grün. |

Verified vorher/nachher: Q02 hatte historisch **0/160**. Der frische Vorher-Lauf ergibt mit derselben neuen SQ-01-Auswertung **2/155**; nach N06 **36/155**. Beide Belege liegen in `verified-n06-vorher.json` bzw. `verified.json`. Die gemeinsam genutzten IDs ONPAGE-009 und ONPAGE-001 verlangen auch die übrigen zugeordneten Tabs; sie werden nicht mehrfach oder vorzeitig gutgeschrieben. SQ-10 hält Links/Internal bis zur Kalibrierung in M3 aus verified heraus.

## Läufe und Herkunft

Alle schweren Aufrufe liefen mit `timeout 3600s nice -n 10`; Cargo mit `-j 8 --locked`, Tokio unverändert 1.53.3. Kein cargo update. Q03 lief parallel; diese Läufe sind Funktionsvergleiche, keine Performancemessung.

| Lauf | Dauer ungefähr | Ergebnisjournal |
| --- | --- | --- |
| voller Vorher-Lauf | 252 s | `roh/lauf-n06-vorher.json` |
| voller Kontrolllauf, Einzelaufrufe | 603 s | `roh/lauf-n06-kontrolle.json` |
| voller Abschlusslauf, Gruppen mit Einzel-Fallback | 295 s | `roh/lauf-n06-voll.json` |
| gezielter Technical-Lauf nach Aufteilung des Unlinked-Hreflang-Spaltensatzes | 6 s | `roh/lauf-n06-technical.json` |
| Redirects-to-Errors erneut vergleichen, ohne weiteren Crawl | unter 1 s | `roh/diff-n06-redirects-to-errors.json` |

`roh/lauf-n06.json` ist das nachvollziehbar zusammengesetzte Abschlussjournal: Abschlusslauf, Technical-Site ersetzt durch ihren gezielten Lauf und ein einzelner Redirects-to-Errors-Bericht durch den erneuten Diff. `source_reports` und `replaced_*` dokumentieren dies. Jeder Messbericht wird unverändert aus os-parity übernommen. `n06-ergebnisse.py` berechnet keine Zellwerte, Schwellen oder verified. Die unveränderten vollständigen Ursprungsjournale bleiben daneben erhalten. `roh/lauf.json` bleibt der historische Q02-Beleg.

Die endgültige Belegauswertung ist ausschließlich:

```sh
target/debug/os-parity coverage \
  --parity-report docs/plan/M2-acceptance/parity/roh/lauf-n06.json \
  --write docs/plan/M2-acceptance/parity/verified.json
```

`auswerten.py` schreibt daraus lediglich die CSV-Register und das Exportjournal. `belege.py` ist nur noch ein Aufruf von Rust-Coverage; alte Python-Verifikationslogik und generierte Q02-Kandidaten aus `evidence.toml` sind entfernt. `exporte.csv` enthält alle 221 Manifest-Einträge einschließlich Ergebnis, Status und Skip-/Fehlergrund. 216 wurden verglichen, davon 139 nach den unveränderten Schwellen bestanden; 77 bleiben fachlich rot. Der Abschlusslauf behält mit `OS_PARITY_MAX_EXAMPLES=100000` alle Zellbeispiele. Der Vorher-Lauf verwendet die normalen Beispielgrenzen; seine Einzelbefundzahl ist deshalb nicht direkt vergleichbar.

Die vier Skips sind ausdrücklich begründet: Content Near Duplicates und dessen Bulk-Export haben keine aktivierte Referenzanalyse in der Standardkonfiguration; Form On HTTP URL Inlinks existiert als Bulk-Export der Referenz nicht; Crawl Path wartet nach SQ-14 auf den CLI-Weg in N11. Die ersten drei stehen als unavailable mit Grund im Manifestkommentar, der letzte als Present mit skip-Grund im Journal. Keine vermeintliche Free-Sperre der neuen Referenzen wird dafür angeführt.

## Umsetzung nach Auftrag

- G01: fehlende nummerierte Referenzspalten akzeptieren ausschließlich leer oder Null; beidseitiges Fehlen erzeugt keine Lücke. Vorhandene Referenzspalten bleiben Pflicht. ONPAGE-008-Fixtures prüfen Page Titles mit sieben Referenzspalten sowie Canonical Link Element 2/3 auch mit dem echten internal_all-Mapping, einschließlich negativer Fälle.
- G02: eigene Spaltensätze für Content-Filter, Security-Bulk, Images-Missing-Size, Hreflang- und Canonical-Reports; SERP nutzt URL und positionsgenaue doppelte Spaltennamen. `optional_ref` trägt generationsabhängige Kopfzeilen. SQ-13f-Ignores nennen TECH-011/031/281. Alle Kettenmappings einschließlich Redirects-to-Errors prüfen die zusätzliche Endzielstation gegen Final Status Code / Final Status; eigentliche Sprünge bleiben verglichen. Referenzdateien bleiben unverändert.
- G03 / Q02-E01: Aufrufe je Kategorie mit Einzel-Fallback bei Gruppenfehlern; Regressionstest mit unbekanntem Report. `os_file` benennt Bulk- und Hreflang-Dateistämme ausdrücklich. Crawl Path nach SQ-14 mit OUT-023-Grund übersprungen; Detail-Test existiert in `crates/os-export/tests/crawl_path_reports.rs`, CLI folgt N11.
- G04 / Q02-P01…P06: M2-Overview und Inlinks aktiv; Link Score nach SQ-10 begründet ausgenommen. Coverage prüft URL-Menge, null Zellfehler, alle Spaltenlücken, angeforderte Belegspalten und nicht verglichene Present-Exporte. Site-/Dateiselektoren und negative Tests vorhanden. Gescheiterte aktuelle Belege nehmen einen alten verified-Status zurück. Nenner ohne Change Detection ist 155. OUT-084-Skip entfernt. `real_repo_files.rs`: nach K2-63 Erwartung Pending → Present (OUT-050/080); ausdrücklich unavailable deklarierte Exporte bleiben erlaubt. Historische M1-PDF-Ausnahmen DEV-016/022/024 geben keine M2-Gutschrift; DEV-022 existiert nicht mehr als aktiver Eintrag.
- G05: DEV-003/004 gelten auch für internal_m2/list_internal. Null-/Leer-Zähler nur für tatsächliche Konfigurations-/Renderingvoraussetzungen. TECH-141-Zeilenausnahmen sind dateigebunden, keine Ausnahme für Links:All. Ungültige Listenzeilen, Sitemaps-All und adressgenaue K-07-Inlinks sind dokumentiert. CSP 17 gegen 13 ist REF-ART-01, kein REF-FREE. Überholte Spec-bleibt-Ausnahmen zu entschiedenen SQ-Fällen entfernt. Keine neue Rundungs-/Prozenttoleranz nach SQ-08.
- G23-R: Chain Type nach SQ-13e/OUT-083 als DEV; Schleifenfinale und fehlende eigene Zeile des Zwischenglieds nach OUT-081 dokumentiert. Listenpopulation und übrige Produktfälle bleiben N11.
- SQ-16: ausschließlich der widersprüchliche ref_settings-Text der neuen Manifeste bereinigt.

## Offene Produktgruppen

| Paket | Gruppen / nächster Schritt |
| --- | --- |
| N07 | G09 Missing Size Attributes, G12 Inhaltstypfilter, G13 Inline-CSS-Entdeckung, G21 Sigma-Pixelbreiten, G19 ONPAGE-077, G24 Aside. |
| N08 | G06 drei Nachkommastellen, G07 Content-Spalten und Soft-404-URL-Schlüssel, G08 Lesbarkeit, G20 Wortgrenzen/Inhaltsbereich, G19 ONPAGE-147. **Direkter Blocker für (1)/(2).** SQ-09 mit M1-Gegenprobe/K-68 umsetzen. |
| N09 | G14 Grundmengen ohne Security, G15 Annotationen außerhalb head/ungültige Attribute, G16 Indexierbarkeit/HTTP Refresh, G17 Technikfilter/Linktypen, G19 ONPAGE-094. |
| N10 | G10 Bad Content Type, G11 nicht gecrawlte Ziele/Tiefe, G14 Security-Grundmengen, G19 TECH-132/133/137. |
| N11 | G03 CLI-Crawl-Path, G22 Sitemap lastmod/changefreq, G23 Listen-Ketten und übrige Fachbefunde, G25 Overview-Abschnitt. |

G18 bleibt nach SQ-10 als TECH-145 OS-Festlegung in M2 ausgenommen; Kalibrierung M3. Keine „Spec bleibt“-DEVs für SQ-02…SQ-13 außer den ausdrücklich entschiedenen Ausnahmen SQ-10/SQ-13e.

## Prüfungen und Übergabe

`cargo fmt -p os-parity -- --check`, `cargo clippy -p os-parity --all-targets -j 8 --locked -- -D warnings`, `cargo test -p os-parity -j 8 --locked -- --test-threads=1` und `bash scripts/check-neutral.sh` sind grün. 179 Tests, 0 ignoriert; Summe der Testbinärzeiten 2,34 s, zuzüglich kurzer inkrementeller Kompilierung. Ein paralleler Durchlauf zeigte einmal den vorhandenen Portreservierungs-Race in run_site; isolierte Gegenprobe und vollständiger serieller Lauf bestanden. Keine Testlogik zur Abschwächung geändert.

CI-Lauf-ID entfällt: laut Auftrag kein Push. Keine Fremdänderungen. `target/` nach Sicherung aller Journale entfernt. Finale Abnahme bleibt wegen G07/N08 offen; nächste Schritte: N07–N11 und anschließend Q02b.

**PAKET OFFEN** – Messkorrekturen geliefert; (1)/(2) benötigen Produktfixes in N08 und anschließend einen neuen vollständigen Lauf.
