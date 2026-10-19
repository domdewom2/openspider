# M2-D13 – Paketbericht und Fix-Runde

Stand: 2026-10-19; Branch `m2/d13`; Paketbasis `123de3e`; Review von Fable: `M2-D13.md`, needs_work 72.
Der Bericht liegt im eigenen Fixture-Verzeichnis gemäß `docs/plan/M2-packages.json`.

## Umsetzung und Spec-Abdeckung

- TECH-216: Mehrfachmitgliedschaften zählen verschiedene `urlset`-Dateien; doppelte Einträge derselben Datei und Indexeinträge zählen nicht. Store-Fixture und aktiver HTTP-Crawl mit XML/gzip prüfen `/both` und `/dup-same-file`.
- TECH-217: `entries > 50000`, live für urlset und Index; genau 50000 bleibt ausgeschlossen.
- TECH-218 (D13-Teil, Anhang B): gespeicherte entpackte Größe `> 52428800`, live; Grenze und NULL getestet. Streaming und Größenmessung bleiben bei E02.
- TECH-219/OUT-057: sieben CSV-/XLSX-Bulk-Exporte; Sitemap-Zuordnungen und Inlinks, Folge-Orphans mit `Found Via`, unveränderte Hyperlink-Zähler.
- OUT-099: Sitemap-Anteil des Orphan-Pages-Reports, dedupliziert je URL. GA4/GSC-Anteile bleiben bei I06.
- UI-064 [P1] (Anhang B): sechs Sitemap-Issues mit Einstufung, Analysepflicht, deutschen/englischen Texten und synchronen Dokumentationsseiten.

## Review-Befunde und Nachweise

1. **Namenskollisionen:** Alle sieben kanonischen Bulk-Namen wieder mit ` Details`, OUT-057-Kurznamen und frühere ` Inlinks`-Namen als Aliase. Die Filterableitung entfernt ` Details`. `tests/golden/out-cli/list_exports.txt` ist bytegleich zur Paketbasis `123de3e`; die Erwartungsänderung aus `54fbb5a` ist vollständig zurückgenommen. Nachweise: `catalog_wiring::no_two_exports_share_a_file_name`, `pipeline_core::no_two_registered_exports_share_a_file_name`, CLI-Katalog-Golden und `all_seven_exports_match_fixture_and_short_and_legacy_aliases_resolve` (alle Aliase exportieren dieselben Fixture-Zeilen; Tab-/Bulk-Auflösung getrennt).
2. **Planregister:** Entscheidung und Minimaldiff unter „Fremdergänzungen (K2-61)“. Kein fremder Vertragstest wird abgeschwächt.
3. **Fehlender Bericht/falsche Ausnahmezuordnung:** Dieser Bericht erfasst das Gesamtpaket und korrigiert die historischen Commit-Beschreibungen: ausschließlich die beiden Stub-Listeneinträge fallen unter K2-60; die sechs Count-Goldens unter K2-63. Die CLI-Golden-Umbenennung bleibt nicht im Paketdiff.
4. **Spalte Sitemaps:** Bewusste Verhaltensänderung gegenüber M1: `COUNT(*)` aller `sitemap_entries` wird durch `COUNT(DISTINCT e.sitemap_id)` ausschließlich über `kind = 'urlset'` ersetzt. TECH-211 zeigt die Zahl der Sitemap-Dateien, TECH-216 präzisiert verschiedene urlsets. Doppelte loc-Werte und Indexverweise erhöhen den UI-/Exportwert somit nicht. Doc-Kommentar präzisiert „Anzahl verschiedener urlset-Dateien“. Nachweise: `multiple_sitemaps_waits_for_analysis_and_counts_distinct_urlsets` (inklusive Indexeintrag in Store-Fixture) und `live_crawl_counts_distinct_urlsets_from_xml_and_gzip_fixtures`.
5. **Prädikatduplikate:** Gemeinsames `page!()` mit `PAGE` und `concat!` für alle sechs Seitenfilter. Gemeinsamer Kommentar erklärt `hidden_population` und den Ausschluss: Sitemap-Dateien erscheinen ausschließlich in den beiden Grenzfiltern TECH-217/218. Nachweise: `tab_sitemaps` und `tab_sitemaps_m2`, insbesondere `sitemap_files_are_exclusive_to_limit_filters_even_if_previously_discovered_as_pages`.

## Empfehlungen

- Direkte Bulk-Exporte rufen `catalog::export_table` auf. Dort wird `requirement_warnings(..., &[])` genutzt; dieses ignoriert `SitemapProcessed`. `prereq::availability` ist damit am direkten Aufrufpfad kein zuverlässiger Hinweis. `write_filter` liefert jetzt `ExportError::Unsupported` mit „requires a processed sitemap“ bzw. „requires the Sitemaps crawl analysis“. Das gilt auch für den gemeinsamen Sitemap-Anteil des Reports. Nachweis: `availability_and_direct_exports_explain_missing_sources_and_pending_analysis`, einschließlich aller sieben Bulk-Exporte ohne Quelle und der fünf analyseabhängigen Exporte vor der Analyse.
- Statuscode 0 (No Response) bleibt bewusst leer wie bei den Link-Exporten; Modulkommentar und Fixture-Test `no_response_status_is_an_empty_export_cell` sichern diese Festlegung.
- Aktiver Crawl-Test gegen die vorhandene Spec-Fixture `tech/sitemaps` mit `sitemap-pages.xml`, `sitemap-posts.xml.gz`, `/both` und `/dup-same-file`. Die Fixture wird nur gelesen. Kein neuer Ignore und kein K2-26-Platzhalter: Dieser Integrationsfall ist schon mit dem vorhandenen Engine-Vertrag ausführbar. Ein Test des noch ausstehenden E02-Streaming-Parsers wird damit nicht behauptet.
- Linux-CI: kein neuer Lauf, weil der Auftrag ausdrücklich Push und PR untersagt. Der im Review genannte Lauf `39135478516` gehört zum alten Stand `54fbb5a` und war rot. Der Integrator muss nach Veröffentlichung des Fix-Commits `linux-core` und die Plattformmatrix abwarten.

## Stub-Ablösung (K2-60)

In `crates/os-engine/f05a-stubs.toml` wurden ausschließlich die zwei D13-Einträge entfernt:

- `crates/os-export/src/catalog/bulk_sitemaps.rs`, Funktion `entries`.
- `crates/os-export/src/reports/orphan_pages.rs`, Funktion `entries`.

Beide Einträge besitzen jetzt Implementierungen. Sonstige Stub-Einträge bleiben unverändert.

## Fremdergänzungen (K2-61)

Einzige Fremdergänzung: `docs/plan/M2-filters.toml`, Filter TECH-218. Der Eintrag bleibt gemäß der ausdrücklich zugelassenen Alternative aus Review-Befund 2 bestehen. Dies ist eine dokumentierte Review-Ausnahme für ein Planregister, keine allgemeine Ausweitung von K2-61 auf Plandateien. UI-064 verlangt „Issue / High“; K2-15 legt den Fachspec-Namen `XML Sitemap With Over 50MB` fest.

```diff
-  { id = "XML Sitemap With Over 50MB", spec = "TECH-218", kind = "live" },
+  { id = "XML Sitemap With Over 50MB", spec = "TECH-218", kind = "live", issue = true },
```

Gegenprobe: Den Eintrag auf die Paketbasis zurückgesetzt und `cargo test -p os-analyze --test catalog_split catalog_and_register_agree_through_issue_true_in_the_filter_files` ausgeführt. Ergebnis: rot mit `Katalogeintrag ohne issue = true: ("sitemaps", "xml sitemap with over 50mb")`. Es scheitert also der gemeinsame Registervertrag in `catalog_split.rs` (nicht der im Review beispielhaft genannte Fachtest). Entfernen des Issues würde UI-064/TECH-218 verletzen; Ändern des fremden Tests würde den Vertrag abschwächen. Daher bleibt genau dieses additive Flag erhalten. Der Integrator kann es gezielt übernehmen; bei Verwerfen muss der Planer den Register-/Katalogkonflikt auflösen. Keine weitere Fremdergänzung.

## Erwartungsänderungen (K2-63)

Die drei neuen Filter TECH-216/217/218 müssen in vollständigen Filterzählungen erscheinen. Deshalb sind folgende sechs fremde Goldens zwingend anzupassen. Alte Erwartung jeweils: alle drei Zeilen fehlen. Neue Erwartung: die in der Tabelle genannten Zeilen werden angehängt; alle vorhandenen Zeilen bleiben unverändert. Keine fremde Testlogik wurde geändert.

| Datei unter `tests/golden/tech/` | TECH-216: `URLs In Multiple Sitemaps` | TECH-217: `XML Sitemap With Over 50k URLs` | TECH-218: `XML Sitemap With Over 50MB` |
|---|---|---|---|
| `crawl-analysis/counts_after.txt` | `0` | `0` | `0` |
| `crawl-analysis/counts_before.txt` | `needs_crawl_analysis` | `0` | `0` |
| `crawl-analysis/counts_sitemaps-off.txt` | `needs_crawl_analysis` | `0` | `0` |
| `sitemaps/counts_after.txt` | `1` | `0` | `0` |
| `sitemaps/counts_before.txt` | `needs_crawl_analysis` | `0` | `0` |
| `sitemaps/counts_without-sitemap.txt` | `0` | `0` | `0` |

Format je neue Zeile: `<Filtername>: <Wert>`. TECH-216 ist erst nach Analyse auswertbar; die beiden Dateigrenzen TECH-217/218 sind live und in den kleinen Fixtures nicht überschritten. Ohne verarbeitete Sitemap sind alle drei Zähler 0. Testnachweis: `crates/os-analyze/tests/tab_sitemaps.rs`.

## Abweichungen/Befund an den Planer

- K2-50 fordert kanonische Bulk-Namen ohne „Details“. Diese kollidieren mit den bestehenden OUT-008-Tab-Exporten. Gemäß verbindlicher Review-Entscheidung bleiben die F05-Namen mit „Details“ kanonisch; OUT-057-Namen funktionieren als Bulk-Aliase. Eine Umbenennung der fremden Tab-Exporte liegt außerhalb D13; der Planer muss K2-50 nachziehen.
- Die M1-Erweiterungsspalte `Sitemaps` folgt nun der urlset-Semantik von TECH-211/216 (oben beschrieben); dies ist eine beabsichtigte UI-/Export-Verhaltensänderung.
- Lokale Prüfung auf macOS statt Cloud; kein Push/PR und damit kein neuer CI-Lauf gemäß Auftrag. Plattformübergreifende CI bleibt Integrationsaufgabe.

## Prüfungen und Laufzeiten

Plattform: macOS / Darwin arm64; Rust 1.99.1; Node 22.20.0; npm 10.9.3. Rust mit `OPENSPIDER_SECRET_STORE=memory`, `CARGO_INCREMENTAL=0`, Dev-/Test-Debuginfo `line-tables-only`. Testnetz nur lokale Fixtures/Mocks. Clean-Room: ausschließlich Projekt-Specs und eigener Code, keine Referenzressourcen.

Abgeschlossene Prüfungen:

| Prüfung | Ergebnis | Laufzeit |
|---|---|---|
| Gezielte Analyze-Tests (`tab_sitemaps`, `tab_sitemaps_m2`, `issue_catalog_sitemaps`, `catalog_split`) | 28 grün | 6,91 s inkl. Build |
| Sitemap-Bulk-/Report-Tests (`cargo test -p os-export --lib bulk_sitemaps`) | 6 grün | 16,88 s inkl. Build |
| `cargo fmt --all --check` | grün | 1,24 s |
| `cargo clippy --workspace --all-targets -- -D warnings` | grün, einschließlich Desktop | 89,62 s |
| `npm ci` | grün | 7,01 s |
| `npm run -w apps/desktop typecheck` | grün | 2,35 s |
| `npm run -w apps/desktop test` | 133 Tests / 13 Dateien grün | 11,11 s |
| `python3 -I docs/plan/M2-check.py` | grün | 7,73 s |
| `npm run -w apps/desktop check:static` | grün | 3,01 s |
| `npm run -w apps/desktop check:licenses` | grün | 0,31 s |
| `cargo deny check licenses bans sources` | grün (bestehende Dependency-Warnungen; Lockfile unverändert) | 2,10 s |

`cargo test --workspace`: **grün**, einschließlich Desktop und Doc-Tests; 2336 bestanden, 0 fehlgeschlagen, 41 bereits vorhandene Ignores. Laufzeit **826,61 s** (13 min 46,61 s), einschließlich Testbuild. Beide im Review genannten Namenskollisionstests und das CLI-Katalog-Golden sind dabei grün. Keine neuen Ignores.

Target-Verzeichnis: **17G**, mit reduziertem Debuginfo und ohne inkrementellen Build. Der vollständige Workspace einschließlich Desktop wurde auftragsgemäß gebaut und getestet. Keine neuen Abhängigkeiten; Manifeste und Lockfiles unverändert. Kein neuer CI-Lauf und keine Aussage über Linux/Windows dieses Fix-Stands; der alte rote CI-Lauf ist oben abgegrenzt.

Besitzprüfung des gesamten Paketdiffs gegen `123de3e`: ausschließlich eigene Pfade plus die oben einzeln dokumentierten Ausnahmen. `git diff --check` grün. Das CLI-Katalog-Golden ist bytegleich zur Paketbasis.
