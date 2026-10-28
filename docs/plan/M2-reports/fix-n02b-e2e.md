# Fixpaket fix-n02b-e2e

Stand: 28.10.2026. Branch `m2/fix-n02b-e2e`, Basis `m2/n02b` / `a9ffb763`.
Vorlage: `/Users/dominikwojcik/Projects/logs/fable-n02b.md`, vollständig gelesen,
insbesondere Befunde 1–3 und die U23-Selektorliste. Clean-Room eingehalten.

## Auflagen: Änderung, Test, Ergebnis

| Auflage | Änderung | Test | Ergebnis |
|---|---|---|---|
| A1 / Befund 1 | `keyed_link_rows` vergibt vor dem Filtern per `enumerate` die bisherigen Schlüssel: Current `index + 1`, Previous `-index - 1`. Ein einmaliger HashMap-Aufbau ersetzt die lineare Suche je Zeile; erwartete Gesamtlaufzeit O(k). | `ten_thousand_links_keep_original_keys_across_sources_and_filters` (`TECH-301`): je 10.000 Current- und Previous-Linkzeilen, alle drei Presence-Zustände, Quellen, Seiten und Änderungsfilter; vollständiger Vergleich der Schlüssel und Exportwerte mit den ursprünglichen Indexwerten. | Grün; alle zehn Compare-Tests zusammen in 3,35 s. Kein quadratischer Referenzalgorithmus im Belastungstest. |
| A2 / Befund 2 | `expect("link belongs to comparison")` entfällt. Fehlende Zuordnung liefert über `runtime::error::internal` einen `CmdError`; Weitergabe mit `?`. | Compare-Tests prüfen unveränderte erfolgreiche Seiten- und Schlüsselermittlung; Fehlerweitergabe am Code geprüft. | Grün; kein Panic-Pfad für fehlende Linkzuordnung im Handler. |
| A2 / Befund 3, Link-Paging | Bestehender Test `link_sort_precedes_paging_and_preserves_keys` um echtes `skip(100).take(100)` ergänzt. | Bei 205 Zeilen je Richtung 100 Zeilen auf Seite 2, exakte erste/letzte Werte und Schlüssel; bisherige Positionsassertionen erhalten. | Grün. |
| Befund 3, URL-Sortierung | Die bereits vorhandene Übernahme von `q.sort` in `query.sort` im URL-Zweig bleibt erhalten und erhält einen Rust-Integrationstest. | `url_query_sort_orders_rows_and_keys_before_paging` (`OUT-140`, `TECH-301`) mit bestehender SQLite-Crawl-Fixture und echter Compare-Sitzung: Address asc/desc, alle zwölf Schlüssel und Werte, Offset 2 / Limit 3, gleichbleibendes Total, ungültige Sortierspalte. | Grün. |
| U23 / E2E | Vergleichsselektoren auf DataGrid umgestellt; Auswahl per begrenzter Tastaturnavigation. Viewport 1600 × 800; zusätzlich horizontales Scrollen zur Spalte Previous Anchor. Sämtliche bisherigen Prüfabsichten einschließlich CSV erhalten. | Reale Crawl-Paare über die gebaute Bridge, Chromium und WebKit, ein Worker. | 2 bestanden; erneuter Gesamtlauf 44,0 s. |
| A3 / Integration | Lokale E2E-Prüfung ist grün. | Plattformmatrix macOS arm64/x64, Windows, linux-core und CI-Job `ui-e2e` wurden nicht gestartet. | Vor Merge weiterhin durch den Integrator auszuführen; kein Push, Merge oder PR beauftragt oder erfolgt. |

## Selektorliste alt → neu

Zeilenangaben beziehen sich auf die Fable-Vorlage und den Ausgangsstand.

| Stelle | Alt | Neu / erhaltene Prüfung |
|---|---|---|
| Z. 51 | `table`, Name Comparison table | `grid`, gleicher exakter Name; Sichtbarkeit bleibt geprüft. |
| Z. 57, 103 | `table`, Name Comparison overview | Unverändert: echte HTML-Tabelle, auch in der Sidebar. |
| Z. 59 | Vergleichs-`table` mit zwei `row` | Vergleichs-`grid` mit zwei `row`; Kopfzeile plus eine Datenzeile. |
| Z. 62 | URL-Button `/start` klicken | `selectRow(grid, /\/start$/)`: Fokus, Home, ArrowDown bis zum passenden `gridcell` der ausgewählten Zeile, begrenzt durch `aria-rowcount`. |
| Z. 65, 78–79 | Outlinks-`row` zählen | Unverändert vier bzw. zwei Zeilen; `sizes[change] = count - 1` und Summenprüfung bleiben erhalten. |
| Z. 66 | `cell` mit `/b`, sichtbar | `gridcell` mit `/b`, Anzahl eins nach Removed. |
| Z. 67 | `cell` mit `/a`, sichtbar | `gridcell` mit `/a`, Anzahl eins nach Added. |
| Z. 68 | `cell` Neu / Alt, sichtbar | Je ein exaktes `gridcell`; Alt wird nach Scrollen von `.os-grid-scroll` zum rechten Rand geprüft, anschließend zurückscrollen. Previous Anchor folgt tatsächlich allen 15 Standardspalten; 1600 px allein reichen deshalb nicht. |
| Z. 69, 84 | Export-Button und deaktivierte Optionen | Unverändert; CSV-Kopf und beide Ankertexte sowie deaktivierte Änderungsfilter bleiben geprüft. |
| Z. 71 | `cell` mit `/b` bei Source Previous | `gridcell` mit `/b`, Anzahl eins. |
| Z. 83 | URL-Button `/a` klicken | `selectRow(grid, /\/a$/)` mit derselben begrenzten Tastaturnavigation. |
| Z. 85 | `cell` mit `/same`, sichtbar | `gridcell` mit `/same`, Anzahl eins. |
| Z. 86 | `columnheader` Title 1 (Previous) | Unverändert; Sichtbarkeit bleibt geprüft. |
| Z. 87 | Vergleichs-`table` mit vier `row` | Vergleichs-`grid` mit vier `row`. |
| Z. 101 | Hauptfenster: `[data-region="table"]` → `table` | Gleicher Bereich → `grid`, Name Comparison table exakt. |
| Z. 104 | Hauptfenster: URL-Button `/start` klicken | `selectRow(mainTable, /\/start$/)`. |
| Z. 105 | Region inlinks | Unverändert; Sichtbarkeit bleibt Erfolgskriterium. |

Die Muster wurden aus `detail-fach/detail-fach.spec.ts` und dem tatsächlich
vorhandenen `e2e/table-advanced/table-advanced.spec.ts` gelesen; der in der
Aufgabe genannte Pfad `e2e/table/table-advanced.spec.ts` existiert nicht.

## Gesamtprüfung

Alle langen Befehle liefen im Vordergrund über `subprocess.run(..., timeout=3600)`
(3.600.000 ms). Vorhandene Node-Abhängigkeiten wurden verwendet, daher kein `npm ci`.

| Befehl | Ergebnis |
|---|---|
| `cargo build -p os-app --bin os-app-bridge --features e2e-bridge --locked` | Exit 0. |
| `cargo test -p os-app --lib compare` | 10 bestanden, 0 Fehler, 3,35 s. |
| `cargo fmt --all --check` | Exit 0. |
| `cargo clippy -p os-app --all-targets -- -D warnings` | Exit 0. |
| `npm run -w apps/desktop typecheck` | Exit 0. |
| `npm run -w apps/desktop test` | 77 Dateien, 777 Tests bestanden, 31,32 s. |
| `npm run -w apps/desktop e2e -- e2e/compare --project=chromium --project=webkit --workers=1` | Erneut 2 bestanden: Chromium 23,5 s, WebKit 18,8 s. |
| `git diff --check` | Grün. |

Beim Bridge-Build und Rust-Test meldet der macOS-Linker die bekannte Warnung
`__eh_frame section too large`; beide Befehle endeten erfolgreich. Clippy mit
`-D warnings` war grün. Keine Änderungen zur Unterdrückung von Warnungen.

## Commits und Fremdänderungen

Schritte 1–3 waren bei Fortsetzung bereits als Paket-Commits vorhanden und
wurden geprüft; die Prüfungen wurden erneut ausgeführt:

1. `536cdcde`: Voraussetzungen und vollständige Vorlagenprüfung (leerer Prüfcommit).
2. `656e69c6`: U23-Selektoren und Tastaturauswahl; beide Engines bestanden.
3. `f27dc9fb`: A1/A2 und Rust-Tests; zehn Compare-Tests bestanden.
4. `bf04cef8`: vollständige erneute Gesamtprüfung (leerer Prüfcommit).
5. Der abschließende Berichtscommit enthält ausschließlich diese Datei.

Alle Paket-Commits tragen `Co-Authored-By: OpenAI Codex <noreply@openai.com>`.
Es wurden nur die drei freigegebenen Pfade geändert und einzeln gestagt.

Die aus N02b geerbte, nach K2-74 freigegebene Änderung in `ui.rs` umfasst
neben Linkschlüsseln und Sortierung vor Paging ausdrücklich auch
`query.sort = view::sorts(...)` im normalen URL-Zweig. Das Fixpaket ergänzt
hier den fehlenden Regressionstest. Grid-Implementierung,
`comparison.test.tsx` und `crates/os-store/src/queries/app_support.rs` bleiben
unverändert. Keine fremden Quelltextänderungen angefasst.

Bei Beginn dieser Fortsetzung enthielt `git status` bereits die unversionierten
Pfade `node_modules` (Symlink auf den N02b-Worktree) und
`apps/desktop/test-results/.last-run.json`. Beide bleiben außerhalb der
Commit-Auswahl erhalten. Playwright aktualisiert die Ergebnisdatei mit
`status: passed`. Getrackte Dateien sind nach den Paket-Commits sauber;
ein vollständig leeres `git status --short` ist wegen dieser Pfade nicht
erreicht. Die Bereinigung liegt außerhalb der drei freigegebenen Dateien
und wurde gemäß Besitzregel nicht vorgenommen. Damit bleibt in Schritt 5
ausschließlich diese geforderte vollständige Statusbereinigung offen.
