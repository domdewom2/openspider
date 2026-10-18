# M2-D08 – Umsetzungsbericht

**Status: Fachumsetzung vorhanden, Paket wegen nicht-additiver Fremdänderungen
und einer Dateinamensentscheidung noch nicht abnahmefähig.**

## Umfang

Umgesetzt im Kern: TECH-136, TECH-137, TECH-138, TECH-139, TECH-140,
TECH-141, TECH-142, TECH-143, TECH-145, TECH-146, ONPAGE-017, UI-081,
OUT-033, OUT-034, OUT-035, OUT-036, OUT-037, OUT-038, OUT-039, OUT-040,
OUT-041 und OUT-068. Zusatzkriterien: TECH-130 [M2] (Inlinks als sichtbare
Standardspalte), OUT-031 [P1] (mailto:/tel:-Rohziele über WriteBatch),
PERF-007 (Abbruch und Lasttestdefinition).

Link Score folgt ausschließlich TECH-145: Auflösung von Weiterleitungen und
Canonicals bis zehn Schritte, eindeutige Zielmenge einschließlich nofollow
im Nenner, zehn Durchläufe in fester Reihenfolge, logarithmische Normierung
und interne lineare Vergleichsoption. Kanten bleiben in SQLite-Tempdateien;
Routingdaten, Ausgangsgrade und Rangvektoren liegen im Speicher. Identische
Neuberechnung erzeugt keine neuen Änderungssequenzen. Ergebnisse
und Änderungssequenz werden atomar veröffentlicht; Abbruch erhält den
vorherigen vollständigen Datenstand. Ausstehende Analyse blendet Altwerte aus.

Die Links-Nachlaufanalyse speichert gemischte/alleinige nofollow-Inlinks und
Inlinks ausschließlich nicht indexierbarer Quellen im eigenen Namensraum.
Schreibtransaktionen beginnen unmittelbar, damit parallel laufende
Analyseschritte keinen SQLite-Snapshot auf Schreibzugriff hochstufen müssen.
Anchors aggregiert in SQLite, einschließlich Mehrfachauswahl, Richtung,
Self-/Nofollow-Ausschluss, numerischen Spaltentypen, Paging und Drill-down.
Die acht neuen Issues haben eigene deutsche und englische Texte und Hilfeseiten.

## Prüfungen

Lokal unter macOS arm64 (Darwin), keine Frontend-Änderungen:

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`:
  grün, letzter Lauf 8,60 s.
- `cargo deny check licenses bans sources`: grün (bestehende Mehrversionshinweise).
- `scripts/check-neutral.sh`: grün; bestehende verwaiste Allowlist-Einträge gemeldet.
- Eigene Fachtests: 9 Link-Score-, 7 Links-/Anchors- und 5 Bulk-Export-Tests grün.
  Nach der Idempotenzkorrektur: 13,14 s / 0,90 s; letzter Export-Nachtest 0,34 s.
- Bestehende Links-Tests: 9 grün; automatischer Analyse-Hook: 4 grün.
  Registry und Issue-Texte: jeweils 10 grün.
- Der vollständige Workspace-Testlauf mit `--no-fail-fast` ist **rot**:
  2.321 bestanden, 6 fehlgeschlagen, 43 ignoriert, 264 Testziele einschließlich
  Doc-Tests. Summierte Testlaufzeiten 272,51 s, davor Build 52,52 s.
  Fehlgeschlagene Ziele: `os-analyze/tab_canonicals` sowie
  `os-export/{catalog_stubs,catalog_wiring,links_response_codes,pipeline_core}`.
  Die unten beschriebenen Fremdtest-/Dateinamenskonflikte bleiben bestehen.
  Sitemap-Wiederholung, CLI-Tests, CSV-/XLSX-/Issue-Goldens und Doc-Tests sind
  im abschließenden vollständigen Lauf grün.
  Danach wurde OUT-033 noch auf sämtliche gespeicherten Hyperlink-Zeilen
  einschließlich Rohzielen abgestimmt, das eigene Golden erneuert und alle
  fünf D08-Exporttests erneut ohne Update-Modus grün ausgeführt.
- Die beiden Lasttests und der M2-E01-Integrationstest bleiben wie beauftragt ignoriert.
- Kein npm-Lauf: `apps/desktop/src` wurde nicht geändert.
Die Tests enthalten Spec-Tags und eigene SQL-/HTML-Fixtures. Das vorgegebene
Fünf-Knoten-Beispiel und fünf deterministische Zufallsgraphen gegen eine
unabhängige Python-Implementierung prüfen die Formel. Weitere Fälle prüfen
Auflösungsgrenze/Schleifen, effektives nofollow, Wiederholung, leere Graphen,
Anzeigequellen und Abbruch während der Iteration. Exporttests prüfen Auswahl,
Spalten, Golden, Katalog-/Filterkonsistenz und WriteBatch-Rohziele.

## Fremdergänzungen (K2-61)

`crates/os-analyze/src/details/mod.rs`: zusätzlicher Aufruf am Anfang der
Detail-Registry. Der vorhandene generische Store-Adapter reicht keine
`DetailReq.params` weiter und setzt alle Spaltenarten auf Text. Der zusätzliche
D08-Eintrag macht die eigenen Anchors-Optionen und numerischen Spaltenarten
zugänglich. Signatur und vorhandene Einträge bleiben bestehen; der neue Hook
liefert ausschließlich für `DetailKind::Anchors` einen Eintrag.

```diff
 pub fn detail_source(kind: DetailKind) -> (&'static str, DetailFn) {
+    if let Some(source) = crate::tabs::links::detail_source(kind) {
+        return source;
+    }
     use DetailKind::*;
```

Additive Ausgabe-Goldens außerhalb von `owns` (K2-61), ausschließlich wegen D08:

- `tests/golden/out-cli/list_exports.txt`: zehn zusätzliche `Links:`-Bulk-Namen
  unmittelbar nach `Links:All Outlinks`; alle bisherigen Zeilen unverändert.
- `tests/golden/out-basic-site/internal_all.csv`: zusätzliche letzte Spalte
  `Link Score`, 18 Datenzeilen. Alle bisherigen Zellen bytegleich erhalten.
- `tests/golden/out-basic-site/internal_all.xlsx.txt`: dieselbe zusätzliche
  letzte Spalte in der textuellen XLSX-Baseline; alle bisherigen Zellen erhalten.

- `tests/golden/out-issues/issues/issues_overview.csv`: eine zusätzliche Zeile
  für das eigene Issue „Links: Outlinks To Localhost“.
- `tests/golden/out-issues/issues/issue_links_outlinks_to_localhost.csv`: neue
  Ergebnisdatei ausschließlich für dieses Issue.
- `tests/golden/out-issues/issues/warning_links_pages_without_internal_outlinks.csv`:
  zusätzliche Standardspalte `Inlinks` nach TECH-130; bisherige Zellen erhalten.

Weitere Fremdergänzungen oder fremde Golden-/Fixture-Änderungen: keine.
Der vorherige eigene Abbruchbericht
`docs/issues/links-m2-d08-interface-blocker.md` wurde nach Auflösung seines
Befunds entfernt; im Hilfeseitenordner sind ausschließlich Katalogseiten erlaubt.
Dieser Paketbericht liegt deshalb im eigenen Fixture-Ordner.

## Stub-Ablösung (K2-60)

In `crates/os-engine/f05a-stubs.toml` wurden ausschließlich die sechs D08-Einträge
entfernt: `link_score::run`, `links::run`, `links::column_source`,
`links::url_details`, `anchors::detail_rows`, `anchors::register_collations`.
In `crates/os-analyze/tests/filters_registry.rs` wurde die leere
Dynamikquellen-Erwartung ausschließlich um den jetzt implementierten
Internal-Link-Score-Eintrag erweitert. Andere Paket-Stubs bleiben unverändert.

## Abweichungen und offene Nachweise

- K2-28 angewandt: aria-label zählt auch im Export OUT-036 nicht als Ankertext.
- Nenner von `% of Total`: HTML-2xx nach K2-47.
- Die beiden Lasttests (30 Mio. Kanten < 5 min; 5 Mio. Knoten/100 Mio. Kanten
  ≤ 60 s/≤ 2 GB) sind gemäß Auftrag als `Benchmark` ignoriert. Ihre Budgets
  sind nicht gemessen. Der Benchmark prüft unter Linux zusätzlich den Prozess-
  Spitzen-RSS; der macOS-Speichernachweis erfolgt bei der Referenzmessung.
- `pending_w1_d08.rs` enthält einen vollständigen, nach C9/K2-26 ignorierten
  Integrationstest für die Erfassung nicht crawlbarer Navigation durch M2-E01.
  Die D08-Fachlogik läuft ohne Ignore gegen Store-/WriteBatch-Fixtures.
- UI-/Command-Anteile liegen gemäß K2-25 bei den jeweiligen Folgepaketen;
  hier wurden deren Kern-Datenquellen umgesetzt. Kein Frontend geändert.
- CI-Lauf-ID: entfällt, da auf ausdrücklichen Auftrag weder Push noch PR erfolgt.
- Offene Fragen: verbindliche Auflösung der Tab-/Bulk-Dateinamenskollisionen
  und Freigabe der beschriebenen M1-Fremdtest-Anpassungen. Zusätzlich stehen
  die Integrations- und Referenzhardware-Nachweise aus.

## Noch nicht freigegebener Fremdtestbedarf

Der M1-Test `the_step_is_repeatable_and_stamps_the_change_feed` in
`crates/os-analyze/tests/tab_canonicals.rs` startet alle Analyseschritte,
erwartet aber ausschließlich die sieben Canonical-Zeilen im Änderungsfeed.
Durch die aktiven D08-Schritte werden zusätzlich Link-Score-/Links-Ergebnisse
veröffentlicht (im abschließenden Lauf 25 statt 7 Zeilen). Diese Ausgabe zu unterdrücken wäre fachlich
falsch. Vorgeschlagene Änderung nur im Aufbau dieses einen Tests:

```diff
 async fn the_step_is_repeatable_and_stamps_the_change_feed() {
-    let c = crawl("tech/canonical-basic", CrawlConfig::default(), false).await;
+    let mut cfg = CrawlConfig::default();
+    cfg.analysis.link_score = false;
+    cfg.analysis.links = false;
+    let c = crawl("tech/canonical-basic", cfg, false).await;
```

Die bisherigen Canonical-Erwartungen bleiben damit vollständig erhalten.
Die Änderung ist keine additive Produktions-Erweiterung nach K2-61 und kein
expliziter F05-Stub-Test; sie wird ohne zusätzliche Freigabe nicht angewandt.


## Weitere Blocker aus dem vollständigen Workspace-Lauf

1. **Dateinamen (OUT-008/OUT-068):** Acht Links-Bulk-Exporte heißen exakt wie
   die gleichnamigen Tab-Exporte. Der globale Kollisionscheck schlägt deshalb
   in `crates/os-export/tests/{catalog_stubs,catalog_wiring,pipeline_core}.rs`
   fehl. Betroffen sind Internal Nofollow Outlinks, Internal Outlinks With No
   Anchor Text, Non-Descriptive Anchor Text In Internal Outlinks, Follow &
   Nofollow Internal Inlinks To Page, Internal Nofollow Inlinks Only, Outlinks
   To Localhost, Pages With Uncrawlable Internal Outlinks und Non-Indexable
   Page Inlinks Only. Beispielsweise belegen Tab und Bulk beide
   `links_non_indexable_page_inlinks_only.csv`; genau diesen Bulk-Dateinamen
   verlangt OUT-068 ausdrücklich. Eine global eindeutige Benennung würde
   bestehendes Tab-Export-Verhalten außerhalb von `owns` ändern, insbesondere
   im Naming-/Tab-Export-Pfad (`crates/os-export/src/{naming,tab_export}.rs`),
   und benötigt eine Entscheidung zum Namensvertrag. Weder solche Änderungen
   noch Änderungen/Lockerungen der Kollisionsprüfungen wurden vorgenommen.

2. **M1-Katalog-Testannahmen:**
   `crates/os-export/tests/links_response_codes.rs` erwartet in
   `all_36_entries_are_registered_with_distinct_file_names` genau 38 Einträge
   im früheren Links-/Response-Code-Katalog (jetzt 48). In
   `every_entry_exports_against_the_registered_tabs` wird außerdem für jeden
   Katalogeintrag der Standard-Link-Header vorausgesetzt; OUT-033 verlangt
   jedoch ausdrücklich einen abweichenden Header. Sinnvoll wäre, diese
   bestehenden Response-Code-Tests auf ihren bisherigen Teilkatalog zu
   begrenzen. Das ist eine nicht-additive Fremdteständerung und wurde nicht
   ausgeführt. Die neuen D08-Formate haben eigene grüne Tests.

Die Zusatzfreigabe zum Canonical-Test wurde angefragt. Sie allein würde die
oben genannten übrigen Blocker noch nicht auflösen. Keine der hier genannten
nicht-additiven Fremdänderungen wurde ohne Freigabe vorgenommen.
