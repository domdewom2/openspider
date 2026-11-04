# B3-DIFF – „Was hat sich seit dem letzten Crawl geändert?“ (beta.3, Abschnitt 4a)

Anlass: Nutzerfrage vom 04.11.2026 (Vergleich zweier Crawls für Deployments). Branch `m3/diff` (Mac-Worktree
`openspider-wt/c-diff`), Server-Worktree `m3-diff` (`srv/diff`). Nicht gepusht, nicht gemergt.

## Ergebnis

1. **CLI-Export mit Unterfiltern (OUT-151):** `--export-tabs "<Tab>:<Filter>:<Unterfilter>"` mit Added/New/Removed/Missing,
   nur im Vergleich (`--crawl-comparison`, `--compare-with-previous`); sonst Exit-Code 2 vor dem Crawl. Missing-Zeilen tragen
   die Previous-Werte (z. B. `/gone.html` mit `Title 1` aus v1), `Removed` enthält die behobene 404.
2. **Change Detection (TECH-300/301, neu TECH-309/310):** Elemente 15 `Canonical` und 16 `Redirect Target`, Standard an,
   hinter den 14 Referenzelementen. Population „All“ umfasst jetzt auch Weiterleitungen (sie tragen meist keinen
   HTML-Content-Type); für eine Weiterleitung sind nur Indexability und Redirect Target erhoben. Ziel-URLs des
   Previous-Crawls laufen vor dem Vergleich durch das URL-Mapping.
3. **Report „Crawl Changes“ (OUT-154, neu):** acht Abschnitte (Spalte `Change`): `Page Gone`, `New Error`, `New Redirect`,
   `Redirect Target Changed`, `New Redirect Chain`, `Longer Redirect Chain`, `Canonical Changed`, `Indexability Changed`,
   jeweils mit Werten vorher/nachher (Status, Indexierbarkeit + Grund, Canonical, Weiterleitungsziel, Kettenlänge).
   App: Berichte → „Änderungen seit dem letzten Crawl“ (nur im Vergleichsmodus, auch ohne geöffneten Crawl);
   CLI: `--save-report "Crawl Changes"`; MCP: `compare_crawls(a, b, changes=true[, change])`.
4. **Handbuch:** neuer Abschnitt „Deployment prüfen“ / „Checking a deployment“ mit App, CLI, Report und SQL-Beispiel
   (`ATTACH`), SQL gegen echte Crawl-Dateien der Fixture geprüft.

## Commits

- `a3b2f461` docs: Crawl-Vergleich für Deployments – Unterfilter im CLI-Export, Change Detection Canonical/Redirect Target, Report „Crawl Changes“ (Nutzerentscheidung 04.11.2026)
- `e522b266` test: Fixture out-b3-diff (v1 → v2) und Golden crawl_changes.csv für OUT-154, OUT-151, TECH-309/310
- `1c572a6b` analyze: Change Detection Canonical/Redirect Target, Population mit Weiterleitungen, Report-Kern „Crawl Changes“
- `c073dd6a` export: Report „Crawl Changes“ im Vergleich (OUT-154), ohne Vergleich Kopfzeile und Warnung
- `d34d628b` cli: --export-tabs "Tab:Filter:Unterfilter" im Vergleich (OUT-151) und --save-report "Crawl Changes"
- `bac48f94` mcp: compare_crawls mit changes/change liefert den Report „Crawl Changes“ (AGENT-015, OUT-154)
- `7ef466f4` analyze: Registry und Goldens für Change Detection Canonical/Redirect Target und Report „Crawl Changes“
- `d03057d5` app: Report „Crawl Changes“ über die Vergleichssitzung des Fensters (export.report, suggestName, Katalog; OUT-154)
- `0035f26c` ui: Berichte → „Änderungen seit dem letzten Crawl“ im Vergleichsmodus, Elemente Canonical/Redirect Target (DE/EN)
- `fe7b8f4b` web: Handbuch-Abschnitt „Deployment prüfen“ / „Checking a deployment“ mit Vergleich, Report und SQL-Beispiel
- `dba0f42c` app: rustfmt
- (dieser Bericht) `docs: Bericht B3-DIFF`

## Spec-Änderungen (Nutzerentscheidung 04.11.2026)

| ID | Änderung |
|---|---|
| OUT-151 | Unterfilter-Syntax `Tab:Filter:Unterfilter` (Begründung: Hierarchie Tab › Filter › Unterfilter, verschachtelte `:` wie OUT-193, je Eintrag wählbar), Herkunft der Werte, Validierung, Auto Compare ohne Vorlauf, fünf Akzeptanzkriterien |
| OUT-154 (neu) | Report „Crawl Changes“: Begriffe, Spalten, acht Abschnitte, Akzeptanzkriterien inkl. Golden, Tausch, Segment, Abbruch |
| OUT-113, OUT-192, OUT-194, OUT-202, OUT-203 | Verweise auf Unterfilter und Report; Validierung |
| TECH-300 | 16 Elemente, Standard 15 (ohne Content); gespeicherte Elementlisten bleiben |
| TECH-301 | Population mit Weiterleitungen; Spalten `Canonical`, `Redirect Target` |
| TECH-309, TECH-310 (neu) | Elementfilter „Canonical“, „Redirect Target“ |
| 03 Anhang A Nr. 37 | PA-Vermerk |
| AGENT-015 | Parameter `changes`, `change` |
| PARITY.md | Zeilen OUT-154, TECH-309, TECH-310 |
| `docs/plan/M2-filters.toml` | Filter `Canonical`, `Redirect Target` im Tab Change Detection (Registry) |

## Beispielausgabe (Fixture `out-b3-diff`, v1 → v2), Auszug `crawl_changes.csv`

```
Change,Address,Previous Address,Status Code (Previous),Status Code (Current),Indexability (Previous),Indexability (Current),Indexability Status (Previous),Indexability Status (Current),Canonical (Previous),Canonical (Current),Redirect Target (Previous),Redirect Target (Current),Redirect Chain Length (Previous),Redirect Chain Length (Current)
Page Gone,http://www.site.test/broken-now.html,http://www.site.test/broken-now.html,200,404,Indexable,Non-Indexable,,Client Error,,,,,0,0
Page Gone,http://www.site.test/gone.html,http://www.site.test/gone.html,200,,Indexable,,,,,,,,0,
New Error,http://www.site.test/broken.html,,,404,,Non-Indexable,,Client Error,,,,,,0
New Redirect,http://www.site.test/moved.html,http://www.site.test/moved.html,200,301,Indexable,Non-Indexable,,Redirected,,,,http://www.site.test/keep.html,0,1
Redirect Target Changed,http://www.site.test/rt,http://www.site.test/rt,301,301,Non-Indexable,Non-Indexable,Redirected,Redirected,,,http://www.site.test/end1.html,http://www.site.test/end2.html,1,1
New Redirect Chain,http://www.site.test/r1,http://www.site.test/r1,301,301,Non-Indexable,Non-Indexable,Redirected,Redirected,,,http://www.site.test/end1.html,http://www.site.test/r1b,1,2
Longer Redirect Chain,http://www.site.test/c1,http://www.site.test/c1,301,301,Non-Indexable,Non-Indexable,Redirected,Redirected,,,http://www.site.test/c2,http://www.site.test/c2,2,3
Canonical Changed,http://www.site.test/canon.html,http://www.site.test/canon.html,200,200,Non-Indexable,Non-Indexable,Canonicalised,Canonicalised,http://www.site.test/canon-a.html,http://www.site.test/canon-b.html,,,0,0
Indexability Changed,http://www.site.test/idx.html,http://www.site.test/idx.html,200,200,Indexable,Non-Indexable,,noindex,,,,,0,0
```

Vollständig (18 Zeilen): `tests/golden/out-b3-diff/crawl_changes.csv`.

## CLI-Beispiel (Server, echte Crawls der Fixture über `os-testsite`)

```
$ openspider --crawl-comparison 01a1a726-983d-… 01a1a726-a178-… --save-report "Crawl Changes" \
    --export-tabs "Internal:HTML:Missing,Response Codes:Client Error (4xx):Removed" --output-folder out
Export: out/internal_html_missing.csv (1 Zeilen)
Export: out/response_codes_client_error_4xx_removed.csv (1 Zeilen)
Export: out/crawl_changes.csv (18 Zeilen)
openspider: Fertig: … 3 Exportdateien, 0 Warnungen (Exit-Code 0)
```

`internal_html_missing.csv` enthält `/gone.html` mit `Title 1` = „Weg“ (v1) und `Comparison Status` = `Missing`.
Das SQL-Beispiel des Handbuchs lieferte an denselben Dateien `/broken-now.html` und `/gone.html` (verschwunden),
`/canon.html` (`canon-a` → `canon-b`) und `/c3`, `/moved.html`, `/r1b` (neue Weiterleitungen) – deckungsgleich mit dem Report.

## Tests

Neue Tests (je mit `// spec:`): `crates/os-analyze/tests/b3_diff.rs` (6), `crates/os-export/tests/b3_diff_exports.rs` (4),
`crates/os-cli/tests/b3_diff_cli.rs` (3), `crates/os-mcp/tests/b3_diff_tools.rs` (1), `crates/os-app/tests/b3_diff_report.rs` (1),
App: `fach.test.tsx` (16 Elemente), `export.test.tsx` (Menüeintrag, Übersetzungen). Angepasst an die Spec-Änderung
(keine Lockerung): Registry-Zahlen 14 → 16 / 13 → 15 (`change_detection.rs`, `config_v2.rs`), Goldens `default.osconfig`,
`osconfig.schema.json`, `list_exports.txt`, `help_export_tabs.txt`, `list_reports.txt`, `out-m2-mail/compare.txt`
(zwei Zeilen `Canonical`/`Redirect Target` mit 0).

Ergebnisse auf dem Build-Server (`m3-diff`, Stand nach `app: rustfmt`):

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -- -D warnings` (mit `~/ws-slot.sh`) | grün (Exit 0); nach dem reinen Formatierungs-Commit `cargo clippy -p os-app --all-targets -- -D warnings` erneut grün |
| `cargo test --workspace --no-fail-fast` (mit `~/ws-slot.sh`, `CARGO_PROFILE_*_DEBUG=0`) | 4303 bestanden, 51 ignoriert, 20 fehlgeschlagen – **alle 20 mit `database or disk is full` / `No space left on device`**: Die Serverplatte lief während des Laufs auf 0 Byte frei (parallele Workspace-Builds `m3-perf` 115 GB, `m3-mcp-connect` 78 GB) |
| Wiederholung der 14 betroffenen Testziele nach Freigabe (os-engine `serp_m2`, `serp_seed_overview`; os-export `links_detail`, `list_m2_e06`, `pipeline_core`, `sitemap_gen`; os-integrations `gsc_inspection`; os-loadtest `gen_ui_large_fixture`; os-model `config_io`; os-parity `real_repo_files`; os-store `crawl_files`, `n28_encountered`, `scale_storage`, `schema_create`) | 111 bestanden, 0 fehlgeschlagen |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` (vitest, alle Dateien) | 115/115 Testdateien grün |
| `npm run -w apps/desktop lint` | grün |
| `node website/check-site.mjs` | OK: 20 Seiten |

Hinweis Server: Ein früherer Lauf brach beim Linken mit `No space left on device` / `ld … Bus error` ab; ich habe daraufhin
nur das eigene `target/` von `m3-diff` gelöscht (121 GB) und ohne Debuginfo neu gebaut. Fremde Worktrees nicht angefasst.

### Abschlusslauf nach Fable-Review und `git merge origin/main` (Merge `212af321`)

Server-Worktree neu gespiegelt, vorher 124 GB frei, `~/ws-slot.sh`; ein Lauf in einem Stück (`CARGO_PROFILE_*_DEBUG=0`):

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0 |
| `cargo test --workspace --no-fail-fast` | Exit 0: 604 Testziele, 4325 bestanden, 0 fehlgeschlagen, 51 ignoriert |
| `npm run -w apps/desktop typecheck` | Exit 0 |
| `npm run -w apps/desktop test` | Exit 0: 115 Testdateien, 991 Tests bestanden |
| `node website/check-site.mjs` | OK: 20 Seiten |

## Spec-Fragen / Unklarheiten

1. **Removed-Werte – Orchestrator-Entscheid 04.11.2026 (Fable-Empfehlung):** Removed-Zeilen tragen die Current-Werte des
   Partners, wie der Unterfilter „Removed“ der Haupttabelle der App (`os-app` `compare/ui.rs`); die Current-Zeile erklärt,
   warum die URL den Filter verlassen hat (behobene 404 jetzt 200). Vorher-Werte liefert „Crawl Changes“. In OUT-151 so
   vermerkt.
2. **Bestehende Konfigurationen:** Crawl-Dateien/Profile mit ausdrücklicher Elementliste (beta.1/beta.2) bekommen
   `Canonical`/`Redirect Target` nicht automatisch; der Report hängt nicht davon ab. Eine Migration „alte Standardliste →
   neue“ wäre möglich, ist aber nicht spezifiziert.
3. **„vorher 2xx/indexierbar“** ist als „vorher indexierbar“ (damit 2xx) umgesetzt; nicht indexierbare 2xx-Seiten, die
   verschwinden, erscheinen nur in „Internal:HTML“ › Missing.
4. Die Mail nach Auto Compare listet jetzt auch `Canonical`/`Redirect Target` (bei 0 wie die übrigen Elemente).
5. **Segmentregel im App-Export (offen):** OUT-154 legt die Segmentregel fest (Current-URL im Segment bzw. Previous-URL
   ohne Partner im Segment des Previous-Crawls); umgesetzt und getestet ist sie in `os-export::compare::write_crawl_changes`
   für CLI/Scheduling (`--segment`). Der App-Export „Berichte → Änderungen seit dem letzten Crawl“ schreibt den Report
   ohne Segment, weil die Vergleichssitzung kein aktives Segment kennt; offen, ob die App dafür eine Segmentwahl bekommt.
6. **Nachtrag Fable-Review (88/100):** `New Error` zählt eine URL, die im Previous-Crawl keine Antwort hatte (Status 0/leer),
   nicht als neu (Wortlaut „ohne Fehler antwortete“, wie `All Errors`); Code angepasst. `Indexability Changed` vergleicht
   eine Current-URL ohne Antwort mit ihrem Grund `No Response`. `compare_crawls.change`: Schema kanonisch (`enum`), Server
   tolerant (AGENT-015). Der konsolidierte Segment-Export (`--segment … --single-workbook`) warnt bei Unterfiltern ohne
   Vergleich wie die übrigen Wege, statt sie stumm zu überspringen.

## Fremde Dateien (K2-92)

| Datei | Bereich | Art der Änderung |
|---|---|---|
| `crates/os-model/src/enums_m2.rs`, `config/v2.rs` | Modell | zwei Enum-Varianten, Doku |
| `crates/os-app/src/commands/export.rs`, `compare.rs` | App-Dienstschicht | Report über Vergleichssitzung (nicht `os-app/src/mcp/**`) |
| `apps/desktop/src/features/export/ExportHost.tsx`, `i18n/exportNames.ts`, `i18n/exports/i18n/*.ftl` | UI Export | Menüeintrag, Übersetzungen |
| `apps/desktop/src/features/config/pages/change-detection/*`, `custom-search/i18n/*.ftl`, `features/compare/model.ts` | UI Konfiguration/Vergleich | zwei Elemente |
| `docs/plan/M2-filters.toml`, `crates/os-analyze/src/tabs/mod.rs` | Filter-Registry | zwei Filter |
| `docs/spec/schemas/osconfig.schema.json`, `tests/fixtures/config/default.osconfig`, `tests/fixtures/out-m2-mail/compare.txt`, `tests/golden/out-cli/*.txt` | Goldens | neu erzeugt |
| `integrations/*/references/tools.md` | Skill-Doku | eine Tabellenzeile `compare_crawls` |
| `website/site/handbuch/index.html`, `website/site/en/manual/index.html` | Website | neuer Abschnitt am Ende + ein Inhaltslink; MCP-Abschnitt unberührt |

Nicht angefasst: `crates/os-store/**`, `link_score*`, `crates/os-mcp/src/client_config.rs`, `print_config.rs`,
`crates/os-app/src/mcp/**`, `apps/desktop/src/features/mcp/**`, MCP-Abschnitte der Website, `site.css`.
