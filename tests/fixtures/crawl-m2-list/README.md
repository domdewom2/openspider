# M2-E06: Fixtures und Paketbericht

## Umfang

| Spec | Ergebnis |
|---|---|
| CRAWL-029 | Bestehenden Upload-Export verifiziert: zehn Eingaben, drei Duplikate, Rohschreibweise, normalisierte Adresse und ausgeschlossene Eingabe. CSV und XLSX behalten zehn Zeilen. |
| CRAWL-030 | Always Follow für HTTP-/JavaScript-/Refresh-Weiterleitungen und HTML-/HTTP-Canonicals; gilt in Spider und List, auch extern und jenseits des Tiefenlimits. Sprunglimit, Include/Exclude, URL-Limit und Schleifenschutz bleiben wirksam. Beim Fortsetzen bleiben externe Kettenziele HTML-Seiten und das Sprungbudget erhalten. |
| CRAWL-031 | Offline-Import über `os_engine::serp_input::{read,parse}` und `CrawlSource::Serp`: TXT/CSV/XLS/XLSX, englische/deutsche Kopfzeilen, verständliche Fehler, Platzhalter für leere URLs, unveränderte Unicode-Texte und gemeinsame Zeichen-/Pixelmessung. Keine HTTP-Anfragen oder Rendering-Aufträge. |
| TECH-087 | Bestehende Hreflang-/CDN-Integration verifiziert: ausgeschaltet kein Abruf, extern nur Status, CDN-Host intern mit HTML-Auswertung und Rücklinkdaten. |
| OUT-085 [M2] | Listenketten einschließlich des bislang ignorierten Export-Vertragstests aktiviert und geprüft. |
| OUT-098 [M2] | SERP-Summary-Kopfzeilen werden direkt akzeptiert; CSV-/XLSX-Export und erneuter Import erhalten Texte und Messwerte. |

`scenario.toml` enthält eigene lokale Redirect-, Canonical- und Hreflang-Seiten.
`dupes.txt` übernimmt die spezifizierte Zeilenfolge der bestehenden Listenfixture.
`serp.csv`, `serp.xls` und `serp.xlsx` enthalten dieselben fünf Entwürfe; die
Arbeitsmappen sind reine Testdaten ohne Formeln. `serp-de.txt` ist tabgetrennt.

## Implementierungsentscheidungen

- SERP bleibt eine Engine-API; dieses Paket ändert keine Desktop-Feature-Oberfläche.
- Jede SERP-Zeile erhält einen privaten URN-Schlüssel, sodass auch doppelte oder
  fehlende Adressen eigenständige Entwürfe bleiben. Die sichtbare Adresse bleibt
  unverändert; leere Adressen heißen `Zeile N` (einschließlich Kopfzeile gezählt).
- Entwürfe nutzen eine synthetische HTML/200-Einstufung für die vorhandenen
  Element- und Duplikatfilter. HTTP-Header, Abrufzeitstempel und Transfermessungen
  werden nicht erfunden; die Engine meldet null HTTP-Abrufe. Ein erneuter Upload
  ersetzt das Fortsetzen oder erneute HTTP-Prüfen eines SERP-Datensatzes.
- Spider/List behalten bei ausgeschalteten Always-Follow-Optionen ihr Verhalten.
  Der Import nutzt dieselben Messfunktionen wie die HTML-Extraktion.

## Fremdergänzungen (K2-61)

`crates/os-engine/src/lifecycle.rs`, CRAWL-030: zusätzlicher Aufruf nach dem
bestehenden `resume::restore`, um die Kettenoptionen auch nach einem Limit oder
Pause zu erhalten. Die Implementierung steht im eigenen `frontier.rs`; bei
beiden ausgeschalteten Optionen ist sie wirkungslos.

```diff
+    crate::frontier::restore_follow_chains(&file, &cfg, &mut restored.core)?;
```

## Fremdänderung (K2-72)

Alle folgenden Ergänzungen gehören zu CRAWL-031 / OUT-098. Bestehende Match-Arme
und Signaturen bleiben unverändert. Zusätzlich zu den ausdrücklich benannten
Stellen werden die drei weiteren erschöpfenden Matches in Session-Control,
MCP-Auflistung und Overview um den zwingend nötigen neuen Arm erweitert.

| Datei | Additiver Diff |
|---|---|
| `crates/os-model/src/enums.rs` | `+ Serp => "serp",` |
| `crates/os-engine/src/serp_input.rs` | Neue Datei: `SerpRow`, `read`, `parse`, transaktionales Seeding mit gemeinsamer Text-/Pixelmessung. |
| `crates/os-engine/src/lib.rs` | `+ pub mod serp_input;` |
| `crates/os-engine/src/lifecycle.rs` | `+ CrawlMode::Serp => return Err(EngineError::Invalid("SERP-Entwürfe werden offline importiert; für neue Werte die Datei erneut hochladen".into())),` |
| `crates/os-engine/src/rescan.rs` | Derselbe neue `Serp`-Arm; verhindert HTTP-Neuprüfung. |
| `crates/os-app/src/commands/crawls.rs` | `+ CrawlMode::Serp => CrawlModeDto::Serp,` |
| `crates/os-app/src/session/feed.rs` | `+ os_model::CrawlMode::Serp => CrawlModeDto::Serp,` |
| `crates/os-app/src/session/control.rs` | `+ os_engine::CrawlSource::Serp { label, .. } => label.clone(),` |
| `crates/os-mcp/src/tools/list_crawls.rs` | `+ CrawlMode::Serp => "serp",` |
| `crates/os-export/src/reports/overview.rs` | `+ CrawlMode::Serp => ("SERP File", list_source),` und `+ CrawlMode::Serp => "SERP",` |

## Erwartungsänderung (K2-63)

`docs/spec/schemas/osconfig.schema.json`, CRAWL-031: neu erzeugtes Schema enthält
als einzigen Unterschied den zusätzlichen Enumwert `"serp"` nach `"list"`.
Der unveränderte Schema-Golden-Test bestätigt die generierte Datei. Dies ist
zugleich eine ausschließlich durch die additive Modellerweiterung verursachte
Golden-Anpassung gemäß K2-61.

`crates/os-model/tests/config_set.rs`, CRAWL-031: ausschließlich die erwartete
Fehlermeldung für einen ungültigen Modus wird von `einer von spider, list erwartet`
auf `einer von spider, list, serp erwartet` erweitert. Die gültigen Enumwerte
stehen automatisch im Fehlertext; Testlogik und übrige Erwartungen bleiben gleich.

## Freigegebene Testaktivierung (K2-68)

`crates/os-export/tests/pending_w1_d12.rs`, OUT-085:

```diff
-#[ignore = "Klärung K2-68: wartet auf M2-E06 (Welle 3), N01-C16"]
```

Testlogik und Erwartungen bleiben unverändert. Keine Stub-Listenänderungen
(K2-60) und keine Platzhalter-Ablösung (K2-71) waren erforderlich.

## Validierung

Eigene Rust-Tests: `list_m2` (sechs), `serp_m2` (zwei),
`hreflang_crawl_m2` (einer); alle mit Spec-Tags und lokalen Fixtures.
Zusätzlicher lokaler Integrationslauf mit dem echten Exporter: zwei aufeinander
folgende CSV-/XLSX-SERP-Rundreisen, Titel-/Beschreibungs-Tab und Duplikatfilter,
zehn Upload-Zeilen in beiden Exportformaten sowie ausgeschlossener Status in CSV.

Abschlussprüfungen am 22.10.2026 auf macOS:

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `OPENSPIDER_SECRET_STORE=memory cargo test --workspace --exclude openspider-desktop --locked --no-fail-fast`: grün, 3636 bestanden, 52 bestehende Ignores, 480 Ergebnisblöcke einschließlich Doc-Tests; Laufzeit 14 min 40 s einschließlich Build.
- `cargo deny check licenses bans sources`: grün.
- `scripts/check-neutral.sh` und `git diff --check`: grün.
- Die zuvor veraltete Fehlertext-Erwartung in `config_set` wurde gemäß K2-63
  korrigiert; anschließend lief die komplette Suite erneut erfolgreich.

Der letzte Commit steht im Übergabebericht. Keine neuen
Abhängigkeiten, kein Frontend-Diff, kein Push, Merge oder PR. Die CI-Matrix wird
entsprechend dem lokalen Auftrag nicht ausgelöst. Keine offenen Spec-Fragen.
