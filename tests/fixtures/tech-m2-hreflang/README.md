# M2-D07 fixture contracts

`relations.sql` models `hreflang/`, `out-hreflang` and `outside-head` from the spec
under the package directory required by K2-24. It is loaded into a fresh schema-v3
crawl file in analyze and export tests. URL IDs are deterministic and deliberately
cover overlapping defects: target 6 is both noindex and canonicalised; target 7
is canonicalised and returns an inconsistent code. Page 2 confirms via a sitemap.
Targets 8 and 9 cannot have their return annotations checked. Page 16 supplies
HTTP annotations and can be checked. Page 12 repeats a code across two sources.

`outside.html` exercises a head, noscript in head, an early head-ending img, and
a body annotation. The parse test runs regardless of validation; the full crawl
coverage test waits for D10 to implement the validation marker.

`detail.csv` is the expected single-selection detail export for URL 1. It uses
the confirmation precedence from TECH-089 with K2-46's Not Applicable status.
The SQL also has separate HTML/HTTP and sitemap truncation counters (2 + 3).

## Paketbericht M2-D07

Umgesetzt: TECH-075, TECH-077, TECH-078, TECH-079, TECH-081, TECH-082,
TECH-083, TECH-084, TECH-086, TECH-088, TECH-089 und OUT-090 bis OUT-096.
Zusatzkriterien: TECH-071 [M2] (Indexability-Spalten) und TECH-008 [M2]
(Outside-head mit Coverage, NeedsConfig, NeedsRecrawl und Incomplete).

- Neun Filter, davon Unlinked als idempotenter, abbrechbarer Nachlauf mit
  atomarem Flag-Austausch und Änderungsfeed für Quellen und Ziele.
- Gemeinsame Beziehungsprädikate für Filter, Bestätigungsstatus und Reports.
- Detailansicht mit Mehrfachauswahl, Sortierung, Paging und Kürzungshinweisen;
  vollständiger CSV/XLSX-Auswahlexport sowie sieben beziehungsbasierte Reports.
- Alter OUT-094-Name als Alias, auch bei anderer Schreibweise und Leerzeichen.
- Neun Issue-Klassifizierungen mit eigenen deutschen/englischen Texten und
  Dokumentationsseiten im bestehenden Generierungsformat.

### Tests und Grenzen

23 neue aktive Fixture-Tests: 11 Analyse-/Filtertests, 1 Parser-Test und 11
Report-/Exporttests, einschließlich CSV-Golden, XLSX, Alias-Aufruf über die
öffentliche Exportfunktion und Exporte über mehrere SQL-Seiten.
Der vorhandene Hreflang-Test umfasst zusätzlich 13 grüne Regressionstests.

Abschlussprüfung auf macOS arm64:

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo deny check licenses bans sources`: grün.
- `scripts/check-neutral.sh`: grün (bestehende Allowlist-Warnungen siehe unten).
- `cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`:
  **nicht grün**, Fremdtest-Blocker siehe unten. Zwei zusätzlich aufgetretene
  Golden-Abweichungen in `tab_sitemaps` sind durch die sechs unten dokumentierten
  Golden-Ergänzungen behoben; das gesamte Modul `tab_sitemaps` lief anschließend
  mit 14 Tests grün. Der gesonderte CLI-Doctest war wegen eines Artefaktversions-
  konflikts rot (`E0460`); `cargo test -p os-cli --doc --locked` lief anschließend
  ohne Codeänderung grün.
  Im vollständigen Prüfversuch: 2327 Tests bestanden, 3 Assertions fehlgeschlagen,
  41 Tests ignoriert; summierte Testlaufzeiten der einzelnen Binärdateien
  288,63 Sekunden (ohne Kompilierung). Von den drei fehlgeschlagenen Assertions
  waren zwei die inzwischen behobenen Goldens; der MCP-Blocker ist separat mit
  `cargo test -p os-mcp --test tools_read issues_that_need_the_crawl_analysis_are_named_instead_of_counted --locked`
  reproduziert. Keine pauschale Workspace-Freigabe.
  Die Fachtests von D07 sind grün (23 aktiv, ein C9-Ignore).

Keine Frontend-Dateien oder Abhängigkeiten geändert. Keine CI-Lauf-ID, da kein
Push beauftragt ist. Die Neutralitätsprüfung ist grün; sie meldet vier bereits
verwaiste Allowlist-Einträge zur CI-Datei, die außerhalb dieses Pakets bleiben.

Abweichungen: keine neuen Fachentscheidungen. K2-44 und K2-46 sind umgesetzt:
Validierungs-Coverage statt bedingungsloser Outside-head-Zählung, Report-Auswahl
nach Filter, OUT-094 Expected = Selbstcode von P / Actual = Rücklinkcode von Q
und Bestätigungsstatus Not Applicable für nicht prüfbare Rücklinks.

Offener Abnahmeblocker: der unten beschriebene fremde MCP-Test.
Ein vollständiger Crawl-Integrationstest liegt gemäß C9
in `crates/os-analyze/tests/pending_w1_d07.rs` mit
`ignore = "wartet auf M2-D10"`: D10 muss den Validierungsmarker liefern.
Die Fachlogik selbst ist ohne Ignore gegen Store-Fixtures geprüft; N01 löst den
Integrationstest auf. Externes Hreflang-Crawling bleibt wie geplant bei M2-E06.

### Stub-Ablösung (K2-60)

`crates/os-engine/f05a-stubs.toml`: die fünf D07-Einträge für Analyse, Details,
URL-Beiträge, Reports und Detail-Export entfernt. Ebenso entfernt ist der
zusätzliche F02-Inventareintrag für dieselbe nun implementierte D07-Funktion
`analysis/steps/hreflang.rs::run`. Keine fremde Stub-Funktion geändert.

### Fremdergänzungen (K2-61)

Keine fremden Laufzeitdateien oder Schnittstellen ergänzt. Ausschließlich die
folgenden sechs Goldens wurden wegen der eigenen additiven Filterausgabe neu
erzeugt. Jede vorhandene Zeile bleibt erhalten; hinzu kommen jeweils neun
Hreflang-Zeilen. Vollständiger Diff gegen den Paket-Ausgangsstand:

```diff
diff --git a/tests/golden/tech/crawl-analysis/counts_after.txt b/tests/golden/tech/crawl-analysis/counts_after.txt
index f39b04e..7ed8271 100644
--- a/tests/golden/tech/crawl-analysis/counts_after.txt
+++ b/tests/golden/tech/crawl-analysis/counts_after.txt
@@ -4,6 +4,15 @@ Non-200 Hreflang URLs: 0
 Missing Return Links: 1
 Incorrect Language & Region Codes: 0
 Missing: 3
+Unlinked Hreflang URLs: 1
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 0
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 2
+Outside <head>: needs_recrawl
 All: 5
 URLs In Sitemap: 4
 URLs Not In Sitemap: 1
diff --git a/tests/golden/tech/crawl-analysis/counts_before.txt b/tests/golden/tech/crawl-analysis/counts_before.txt
index bcb048a..abc1653 100644
--- a/tests/golden/tech/crawl-analysis/counts_before.txt
+++ b/tests/golden/tech/crawl-analysis/counts_before.txt
@@ -4,6 +4,15 @@ Non-200 Hreflang URLs: 0
 Missing Return Links: 1
 Incorrect Language & Region Codes: 0
 Missing: 3
+Unlinked Hreflang URLs: needs_crawl_analysis
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 0
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 2
+Outside <head>: needs_recrawl
 All: 5
 URLs In Sitemap: needs_crawl_analysis
 URLs Not In Sitemap: needs_crawl_analysis
diff --git a/tests/golden/tech/crawl-analysis/counts_sitemaps-off.txt b/tests/golden/tech/crawl-analysis/counts_sitemaps-off.txt
index bcb048a..ec2772a 100644
--- a/tests/golden/tech/crawl-analysis/counts_sitemaps-off.txt
+++ b/tests/golden/tech/crawl-analysis/counts_sitemaps-off.txt
@@ -4,6 +4,15 @@ Non-200 Hreflang URLs: 0
 Missing Return Links: 1
 Incorrect Language & Region Codes: 0
 Missing: 3
+Unlinked Hreflang URLs: 1
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 0
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 2
+Outside <head>: needs_recrawl
 All: 5
 URLs In Sitemap: needs_crawl_analysis
 URLs Not In Sitemap: needs_crawl_analysis
diff --git a/tests/golden/tech/hreflang/counts_store-off.txt b/tests/golden/tech/hreflang/counts_store-off.txt
index 19f9e01..50670c7 100644
--- a/tests/golden/tech/hreflang/counts_store-off.txt
+++ b/tests/golden/tech/hreflang/counts_store-off.txt
@@ -4,3 +4,12 @@ Non-200 Hreflang URLs: 0
 Missing Return Links: 0
 Incorrect Language & Region Codes: 0
 Missing: 0
+Unlinked Hreflang URLs: needs_crawl_analysis
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 0
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 0
+Outside <head>: 0
diff --git a/tests/golden/tech/hreflang/counts_store-on_crawl-off.txt b/tests/golden/tech/hreflang/counts_store-on_crawl-off.txt
index 2cfc431..65f6a17 100644
--- a/tests/golden/tech/hreflang/counts_store-on_crawl-off.txt
+++ b/tests/golden/tech/hreflang/counts_store-on_crawl-off.txt
@@ -4,3 +4,12 @@ Non-200 Hreflang URLs: 1
 Missing Return Links: 1
 Incorrect Language & Region Codes: 1
 Missing: 2
+Unlinked Hreflang URLs: needs_crawl_analysis
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 1
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 7
+Outside <head>: needs_recrawl
diff --git a/tests/golden/tech/hreflang/counts_store-on_crawl-on.txt b/tests/golden/tech/hreflang/counts_store-on_crawl-on.txt
index 90c5465..9cada8a 100644
--- a/tests/golden/tech/hreflang/counts_store-on_crawl-on.txt
+++ b/tests/golden/tech/hreflang/counts_store-on_crawl-on.txt
@@ -4,3 +4,12 @@ Non-200 Hreflang URLs: 1
 Missing Return Links: 2
 Incorrect Language & Region Codes: 1
 Missing: 3
+Unlinked Hreflang URLs: needs_crawl_analysis
+Inconsistent Language & Region Return Links: 0
+Non-Canonical Return Links: 0
+Noindex Return Links: 0
+Multiple Entries: 1
+Missing Self Reference: 0
+Not Using Canonical: 0
+Missing X-Default: 7
+Outside <head>: needs_recrawl
```

### Abbruch: nicht additive Fremdänderung erforderlich

`crates/os-mcp/tests/tools_read.rs:1717`, Test
`issues_that_need_the_crawl_analysis_are_named_instead_of_counted` (AGENT-013),
setzt voraus, dass *alle* nicht auswertbaren Issues ausschließlich den Status
`needs_crawl_analysis` haben. Der nun implementierte und als Issue registrierte
Hreflang-Filter `Outside <head>` liefert für eine Datei ohne
`validation.checked` gemäß K2-44 dagegen korrekt `needs_recrawl`.
`os-mcp/src/tools/list_issues.rs` übernimmt alle nicht auswertbaren Filter samt
Status; das ist für diesen neuen Status korrekt.

Den Marker liefert der noch offene F05-Stub von M2-D10. Eine falsche
Ready-Markierung oder das Verbergen des Hreflang-Issues würde die Fachanforderung
verletzen. Eine Integration von D10 ist hier nicht beauftragt. Deshalb bleibt die
Assertion unverändert. Sie ist weder ein F05-Vertragstest eigener Stubs (K2-60)
noch eine strikt additive Schnittstellenergänzung (K2-61).

Beispiel für die **nicht angewandte**, außerhalb von owns erforderliche
Testanpassung, die den ursprünglichen Analyse-Check erhält und den neuen
Coverage-Status gezielt berücksichtigt:

```diff
--- a/crates/os-mcp/tests/tools_read.rs
+++ b/crates/os-mcp/tests/tools_read.rs
@@
-    assert!(pending.iter().all(|p| p["state"] == "needs_crawl_analysis"));
+    assert!(pending.iter().any(|p| p["state"] == "needs_crawl_analysis"));
+    assert!(pending.iter().all(|p| {
+        p["state"] == "needs_crawl_analysis"
+            || (p["tab"] == "Hreflang"
+                && p["filter"] == "Outside <head>"
+                && p["state"] == "needs_recrawl")
+    }));
```

Nach der ausdrücklichen Abbruchregel im Paketauftrag wird M2-D07 deshalb nicht
als abnahmefähig gemeldet. Es wurden keine fremden Tests geändert. Alternativ
kann die Integration der echten D10-Validierung diesen konkreten Fixture-Konflikt
auflösen; das gehört nicht zu M2-D07.
