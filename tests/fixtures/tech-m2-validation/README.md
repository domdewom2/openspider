# M2-D10 – Validation

Clean-Room: ausschließlich `docs/spec` und vorhandene Projektverträge genutzt;
Issue-Erklärungstexte ohne Einsicht in Referenz-Erklärungstexte verfasst.

Umgesetzt: TECH-008, CRAWL-138, TECH-280–290; zusätzliche Kriterien
CRAWL-140 [P1], TECH-003 [M2], UI-064 [P1].

`expectations.json` ist das unabhängig formulierte Golden für Token-Zählungen
und Filtermitgliedschaften. Die HTML-Dateien decken positive und negative
Strukturbeispiele ab. `scenario.toml` erzeugt große Antworten zur Laufzeit;
keine großen Binär-Fixtures werden gespeichert.

Die Analyse verwendet den HTML5-Tokenizer des bereits vorhandenen html5ever.
Sie betrachtet den Originalquelltext, ignoriert Template-Inhalt und wechselt
für Script/Rawtext/RCDATA in die entsprechenden Tokenizer-Zustände.
Zeichentoken zählen einzeln, sonstige Token einmal; Parsefehler zählen nicht
als Quelltexttoken. Dadurch hängen die nullbasierten Positionen nicht davon
ab, wie der Tokenizer Textläufe bündelt.

`validation.checked`, Tag-Zählungen, erste Positionen, erstes Element nach html
und Befunddetails gehen über `validation::to_ext` in `url_ext`. Die Struktur
wird ohne Validierungsoption nicht erhoben. Antwortgröße und Abbruchmarker
werden separat über `response_facts` erfasst. `untruncated_bytes` hat Vorrang;
Content-Length wird nur bei unkodierter, abgeschnittener Antwort als weitere
Größenangabe verwendet. Eine abgeschnittene Antwort bleibt im Größenfilter.

All zeigt interne HTML-Antworten mit Status 200. Der Ressourcenfilter ergänzt
interne JS/CSS-Antworten; Bilder und Fonts bleiben ausgeschlossen. Die
Coverage-Zustände berücksichtigen ausschließlich HTML-200, damit Ressourcen
keine vermeintlich ungeprüften HTML-Seiten erzeugen. Größen werden als Bytes
verglichen und im Filternamen nach dem vorhandenen Vertrag in MiB angezeigt.
Die Issue-Erklärungen verweisen auf die konfigurierte Größengrenze, ohne einen
festen Grenzwert im Text zu hinterlegen.

## Prüfungen

- Parser-Fixture-Goldens und Rohquelltext gegenüber abweichendem DOM.
- Validierung aus: keine Tokenfakten, Extraktion/Indexierbarkeit unverändert.
- Reale Loopback-Crawls: alle sieben Strukturfilter, Population und Spalten.
- Größen: genau 2 MiB, 2 MiB + 1 Byte (gzip), JS/CSS und negative Bild-Fixture;
  Neuklassifikation nach Konfigurationsänderung ohne weiteren Request.
- Store-Fixtures: Abbruchmarker, externe Ressourcen, Fonts und HTTP-Fehler.
- Pause/Fortsetzen/Neuabruf mit SimFetcher: NeedsRecrawl, Incomplete mit exakter
  Zahl ungeprüfter Seiten, Ready und Entfernung alter Tokenfakten bei Neuabruf.
- Neun Issue-Einträge mit Benchmark-Klassifizierung, de/en und generierten Docs.
- Separater ignorierter Benchmark gegen `crawl-site-100`, neun wechselnde Paare,
  Medianvergleich; erlaubtes Maximum 5 % Mehrkosten (TECH-008).

### Ergebnis vom 18.10.2026 (lokal, macOS arm64)

- `cargo fmt --all --check`: Exit 0.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: Exit 0 (abschließender Cache-Lauf 0,96 s).
- `cargo test --workspace --exclude openspider-desktop --locked`: Exit 0;
  2 327 bestanden, 0 fehlgeschlagen, 41 ignoriert, 261 Testgruppen;
  Laufzeit 295,31 s. Darin sind alle 20 neuen aktiven D10-Tests grün.
- `cargo deny check licenses bans sources`: Exit 0; alle drei Prüfungen ok.
- `scripts/check-neutral.sh`: Exit 0; keine Fundstellen. Bestehender Hinweis
  auf eine verwaiste Ausnahme für `.github/workflows/ci.yml`, unverändert.
- Kein Push, Merge oder PR; daher keine CI-Lauf-ID.
- Der zusätzliche TECH-008-Benchmark ist implementiert und mit dem erlaubten
  Benchmark-Ignore versehen. Kein Performance-Messwert: parallele Builds/Tests,
  zuletzt Load Average 4,99, oberhalb der Quiet-Grenze 2. Der Nachweis von
  höchstens 5 % Mehrkosten bleibt für einen ruhigen Benchmark-Lauf offen.

## Fremdergänzungen (K2-61)

F05 stellt keinen Transport für Token-Zählungen/Positionen in ValidationFacts,
keine originale Bytezahl im PageCtx und keine Coverage-Teilmenge eines Tabs bereit.
Die folgenden Ergänzungen füllen diese Lücken. Bestehende Signaturen bleiben
unverändert, Default-Verhalten ohne registrierte Coverage-Teilmenge bleibt erhalten.
Zwei CLI-Goldens erhalten ausschließlich die additive D10-Ausgabe (Diff unten).
Anpassungen von F05-Vertragstests stehen unter K2-60.

### `crates/os-model/src/parse_m2.rs`

```diff
diff --git a/crates/os-model/src/parse_m2.rs b/crates/os-model/src/parse_m2.rs
index 3f621ee..d1255cb 100644
--- a/crates/os-model/src/parse_m2.rs
+++ b/crates/os-model/src/parse_m2.rs
@@ -43,6 +43,8 @@ pub struct ValidationIssueFact {
 /// Fakten der HTML-Validierung einer Seite (TECH-008, `advanced.html_validation`).
 #[derive(Debug, Clone, PartialEq, Eq, Default)]
 pub struct ValidationFacts {
+    /// Struktur des Originalquelltexts; Token-Indizes beginnen bei 0.
+    pub tokens: ValidationTokens,
     /// Befunde in Dokumentreihenfolge.
     pub issues: Vec<ValidationIssueFact>,
     /// Anzahl Tokenfehler im `<head>`.
@@ -53,6 +55,18 @@ pub struct ValidationFacts {
     pub document_bytes: u64,
 }

+/// Token-Zusammenfassung ohne Speicherung des HTML-Quelltexts (TECH-280).
+#[derive(Debug, Clone, PartialEq, Eq, Default)]
+pub struct ValidationTokens {
+    pub html_count: u32,
+    pub head_count: u32,
+    pub body_count: u32,
+    pub first_html: Option<u64>,
+    pub first_head: Option<u64>,
+    pub first_body: Option<u64>,
+    pub first_element_after_html: Option<String>,
+}
+
 /// Ein Treffer eines Custom-Search-Filters (INTEG-020 ff.).
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct SearchHit {
```

### `crates/os-parse/src/lib.rs`

```diff
diff --git a/crates/os-parse/src/lib.rs b/crates/os-parse/src/lib.rs
index 0732618..6cf043b 100644
--- a/crates/os-parse/src/lib.rs
+++ b/crates/os-parse/src/lib.rs
@@ -250,6 +250,7 @@ pub fn parse_html(input: &ParseInput) -> ParsedPage {
     security::run(&ctx, &mut page);
     if cfg.html_validation {
         validation::run(&ctx, &mut page);
+        validation::set_document_bytes(&mut page, input.body.len() as u64);
     }
     if cfg.readability || cfg.minhash {
         readability::run(&ctx, &mut page);
```

### `crates/os-analyze/src/defs.rs`

```diff
diff --git a/crates/os-analyze/src/defs.rs b/crates/os-analyze/src/defs.rs
index 5a6b2f8..c206ff4 100644
--- a/crates/os-analyze/src/defs.rs
+++ b/crates/os-analyze/src/defs.rs
@@ -364,6 +364,30 @@ pub enum Requirement {
     Rendering,
 }

+type CoverageDomains = std::collections::BTreeMap<(&'static str, &'static str), &'static str>;
+
+fn coverage_domains() -> &'static Mutex<CoverageDomains> {
+    static DOMAINS: OnceLock<Mutex<CoverageDomains>> = OnceLock::new();
+    DOMAINS.get_or_init(|| Mutex::new(CoverageDomains::new()))
+}
+
+/// Registriert die prüfbare Teilmenge eines Tabs für einen Coverage-Marker.
+/// Ohne Registrierung gilt wie bisher die gesamte Tab-Population.
+pub fn register_coverage_domain(tab: &'static str, marker: &'static str, predicate: &'static str) {
+    coverage_domains()
+        .lock()
+        .expect("coverage domains")
+        .insert((tab, marker), predicate);
+}
+
+pub(crate) fn coverage_domain(tab: &str, marker: &str) -> Option<&'static str> {
+    coverage_domains()
+        .lock()
+        .expect("coverage domains")
+        .get(&(tab, marker))
+        .copied()
+}
+
 /// Sortierschlüssel.
 #[derive(Debug, Clone, PartialEq, Eq)]
 pub struct SortKey {
```

### `crates/os-analyze/src/state.rs`

```diff
diff --git a/crates/os-analyze/src/state.rs b/crates/os-analyze/src/state.rs
index 96ec9f7..b5d6efd 100644
--- a/crates/os-analyze/src/state.rs
+++ b/crates/os-analyze/src/state.rs
@@ -216,6 +216,23 @@ pub(crate) fn filter_states_for_file(
         return Ok(states);
     }
     let markers: Vec<_> = markers.into_iter().collect();
+    let domains: Vec<_> = markers
+        .iter()
+        .enumerate()
+        .filter_map(|(i, marker)| {
+            crate::defs::coverage_domain(tab.id, marker).map(|domain| (i, domain))
+        })
+        .collect();
+    let domain_sums = domains
+        .iter()
+        .map(|(i, domain)| {
+            format!(
+                ", COALESCE(SUM(CASE WHEN ({domain}) THEN 1 ELSE 0 END), 0), \
+         COALESCE(SUM(CASE WHEN ({domain}) THEN EXISTS(SELECT 1 FROM url_ext e \
+         WHERE e.url_id = u.id AND e.key = :coverage_{i} AND e.v_int = 1) ELSE 0 END), 0)"
+            )
+        })
+        .collect::<String>();
     let sums = markers.iter().enumerate().map(|(i, _)| format!(
         "COALESCE(SUM(EXISTS(SELECT 1 FROM url_ext e WHERE e.url_id = u.id AND e.key = :coverage_{i} AND e.v_int = 1)), 0)"
     )).collect::<Vec<_>>().join(", ");
@@ -223,7 +240,7 @@ pub(crate) fn filter_states_for_file(
         Some(hidden) => format!("({}) OR ({hidden})", tab.population),
         None => tab.population.to_string(),
     };
-    let sql = format!("SELECT COUNT(*), {sums} FROM urls u WHERE ({population})");
+    let sql = format!("SELECT COUNT(*), {sums}{domain_sums} FROM urls u WHERE ({population})");
     let mut stmt = file.conn().prepare(&sql)?;
     for (name, value) in (tab.params)(cfg) {
         let name = if name.starts_with(':') {
@@ -257,6 +274,14 @@ pub(crate) fn filter_states_for_file(
         .enumerate()
         .map(|(i, marker)| Ok((*marker, row.get(i + 1)?)))
         .collect::<Result<_, rusqlite::Error>>()?;
+    let domain_counts: BTreeMap<_, (u64, u64)> = domains
+        .iter()
+        .enumerate()
+        .map(|(offset, (i, _))| {
+            let column = 1 + markers.len() + offset * 2;
+            Ok((markers[*i], (row.get(column)?, row.get(column + 1)?)))
+        })
+        .collect::<Result<_, rusqlite::Error>>()?;
     for (def, state) in defs.iter().zip(&mut states) {
         if !state.yields_rows() {
             continue;
@@ -265,6 +290,10 @@ pub(crate) fn filter_states_for_file(
         for req in &def.requires {
             if let Requirement::Coverage { marker, .. } = req {
                 let covered = checked[marker];
+                let (total, covered) = domain_counts
+                    .get(marker)
+                    .copied()
+                    .unwrap_or((total, covered));
                 if total > 0 && covered == 0 {
                     *state = FilterState::NeedsRecrawl;
                     break;
```

### Additive CLI-Ausgabe (Goldens)

Die zwei konkret mit HTML-Tags beginnenden D10-Filter ergänzen die CLI-
Exportliste um zwei Tab-Exporte und zwei automatisch registrierte Inlink-Exporte.
Die beiden Goldens erhalten genau diese sechs zusätzlichen Zeilen; bestehende
Ausgabe und fremde Testdateien bleiben unverändert.

`tests/golden/out-cli/help_export_tabs.txt`:

```diff
diff --git a/tests/golden/out-cli/help_export_tabs.txt b/tests/golden/out-cli/help_export_tabs.txt
index fd90801..afd116c 100644
--- a/tests/golden/out-cli/help_export_tabs.txt
+++ b/tests/golden/out-cli/help_export_tabs.txt
@@ -294,10 +294,12 @@ Search Console:AMP URL Is Invalid
 Search Console:Rich Result Invalid
 Validation:All
 Validation:Invalid HTML Elements In <head>
+Validation:<head> Not First In <html> Element
 Validation:Missing <head> Tag
 Validation:Multiple <head> Tags
 Validation:Missing <body> Tag
 Validation:Multiple <body> Tags
+Validation:<body> Element Preceding <html>
 Validation:HTML Document Over 2MB
 Validation:Resource Over 2MB
 Link Metrics:All
```

`tests/golden/out-cli/list_exports.txt`:

```diff
diff --git a/tests/golden/out-cli/list_exports.txt b/tests/golden/out-cli/list_exports.txt
index 3b815ad..aa8c0ca 100644
--- a/tests/golden/out-cli/list_exports.txt
+++ b/tests/golden/out-cli/list_exports.txt
@@ -294,10 +294,12 @@ Search Console:AMP URL Is Invalid
 Search Console:Rich Result Invalid
 Validation:All
 Validation:Invalid HTML Elements In <head>
+Validation:<head> Not First In <html> Element
 Validation:Missing <head> Tag
 Validation:Multiple <head> Tags
 Validation:Missing <body> Tag
 Validation:Multiple <body> Tags
+Validation:<body> Element Preceding <html>
 Validation:HTML Document Over 2MB
 Validation:Resource Over 2MB
 Link Metrics:All
@@ -554,10 +556,12 @@ Search Console:AMP URL Is Invalid Inlinks
 Search Console:Rich Result Invalid Inlinks
 Validation:All Inlinks
 Validation:Invalid HTML Elements In <head> Inlinks
+Validation:<head> Not First In <html> Element Inlinks
 Validation:Missing <head> Tag Inlinks
 Validation:Multiple <head> Tags Inlinks
 Validation:Missing <body> Tag Inlinks
 Validation:Multiple <body> Tags Inlinks
+Validation:<body> Element Preceding <html> Inlinks
 Validation:HTML Document Over 2MB Inlinks
 Validation:Resource Over 2MB Inlinks
 Link Metrics:All Inlinks
```

## Stub-Ablösung (K2-60)

In `crates/os-engine/f05a-stubs.toml` entfallen ausschließlich die drei
Parser-Stubs von D10 sowie `tabs::validation::tab` (einschließlich des doppelten
F02-Eintrags derselben Funktion). Alle übrigen Stub-Einträge bleiben erhalten.

Der F05-Vertragstest `parser_extension_payloads_keep_their_url_and_values`
in `crates/os-engine/src/worker.rs` prüft nun zusätzlich `validation.checked`.
Die vorher positionsabhängige Prüfung des nachfolgenden hreflang-Zählers sucht
denselben Wert nach Schlüssel, da D10 nun davor echte Validierungsfakten liefert.
In `crates/os-analyze/tests/filters_registry.rs` sinkt die erwartete Zahl der
Stub-Tabs von 15 auf 14. Die erwartete Filterliste von Validation behandelt
die beiden konkreten Kennungen mit führendem `<head>`/`<body>` als D10-Filter
statt sie als dynamische Platzhalter zu überspringen. Die F05-Stubtabelle und
alle Erwartungen anderer Pakete bleiben erhalten.

## Abweichungen und offene Fragen

Die Plattform-CI wird gemäß Auftrag nicht ausgelöst (kein Push/PR).
Ein Performance-Nachweis verlangt eine ruhige Maschine; der Benchmark ist
explizit ausführbar und wird nicht als normaler Funktionstest behandelt.
Keine offenen fachlichen Fragen.
