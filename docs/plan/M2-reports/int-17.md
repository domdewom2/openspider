# Integration r16 – m2/int-17

**PAKET OFFEN:** Alle neun freigegebenen Paketstände sind integriert; die vollständige Rust-Prüfung und alle abschließenden UI-Suiten sind grün. `os-parity run --all` bleibt wegen **23 Exportabweichungen für Q02b** rot. Das vereinbarte verified-Ziel ist mit **129/155** erfüllt. Nächster Schritt: Q02b-Restliste abarbeiten/entscheiden; anschließend Plattformabnahme durch den Orchestrator. Kein Push.

Basis: `origin/m2/int-16` = `233d00de` einschließlich K2-82/Late-Paint-Fix. Geprüfter letzter Code-/Teststand: `381d7c52`; spätere Änderungen betreffen Dokumentation und Belege. Linux, 16 Kerne, Rust 1.99.1, Node 22.23.4, Playwright 1.64.1. Tokio unverändert **1.53.3**, kein cargo update; Cargo.toml/Cargo.lock gegenüber der Basis unverändert. Q03 und M3-Branches wurden nicht gemergt.

## Merges und Konfliktlösungen

| Reihenfolge | Paket | Merge | Freigabe / Lösung |
|---|---|---|---|
| 1 | origin/m2/base-4r | 5c70de74 | N06/Q02/Plan und Referenzexporte; konfliktfrei, Late-Paint-Fix erhalten |
| 2 | m2/n11 | 031c17c7 | Fable 84, Auflagen unten; konfliktfrei |
| 3 | m2/n10 | 35c390c8 | Fable 80 einschließlich Korrekturrunde; konfliktfrei |
| 4 | m2/n09 | 60e6db8b | Fable 78 einschließlich Korrekturrunde; decide_crawl auto-gemergt |
| 5 | m2/n07 | 4d49d8d7 | Fable Nachprüfung 88; out-issues/Overview auto-gemergt, anschließend erneut geprüft |
| 6 | m2/n08 | 57a23a0d | Fable Nachprüfung 82; drei Konflikte wie unten gelöst |
| 7 | m2/j01 | e4984359 | Astra 97, docs-only, konfliktfrei |
| 8 | m2/u30 | c0907ec0 | Fable 82; vollständige E2E-A-Matrix vor Commit, Forced-Colors-Auflage umgesetzt |
| 9 | m2/u32 | 475b82d8 | Fable 72; konfliktfrei, Auflagen und Endwerte unten |

N08: `tests/parity/fixtures_check.rs` behält die strengere N07-Variante einschließlich `Plugins => Class::Flash`; `drop.is_empty() ||` wurde verworfen. `internal_all.csv` und `.xlsx.txt` wurden vollständig aus dem kombinierten Stand regeneriert: N08-Header und N07-Zellwerte, keine manuelle Mischung. Schema und default.osconfig mit UPDATE_GOLDEN=1 regeneriert und anschließend geprüft. config_io **15/15**, config_schema **5/5**, cli_crawl **11/11**. Korrektur zur Commit-Nachricht: Dort steht für config_io versehentlich 10/10 statt 15/15; das Regenerationslog belegt 15.

Alle eigenen Commits tragen den vereinbarten Trailer; ausschließlich explizite eigene Pfade wurden gestaged.

## Auflagen, Integrationsfixes und K2-63

- **N11 A/C/D erfüllt:** N09 registriert External Redirection (JavaScript) mit needs_rendering und Zähler 0 nach ONPAGE-057. Overview-Workaround und Testsonderfall entfernt. OUT-085-Nullsprungzeilen bleiben ausschließlich in Redirect Chains und Redirect & Canonical Chains; All Redirects verlangt ≥1 Sprung. Eigener Listen-Fixture-Test. Overview, Sitemap-Generatoren und n11_* im abschließenden Workspace-Lauf erneut grün.
- **N11 B:** upload_order_matches gilt nur für `report:`-Exporte. Crawl-Path-Freischaltung über `--export-crawl-path` und Verbindungs-Layout im Mapping bleiben ausdrücklich **Q02b**; kein ungeprüftes Mapping oder Referenz-Eingriff.
- **N09/N10 erfüllt:** Directives-All im out-issues-Golden **7**. Validation-All auf parity-m2-technical **67/67**, identische URL-Menge (vorher 68/67). Vier Headerfilter auf parity-m2-onpage jeweils **58** statt OS 59; `/background.png` nach N07 korrekt. Deren Overview enthält nur noch die bekannte Soft-404-Abweichung 2/3. Stichproben decide_crawl/tab_internal vollständig grün.
- **N07/N08 erfüllt:** freigegebene Korrekturstände integriert; Schema/Default und Exporte geprüft. engine_adapter einschließlich des RSS-Kindprozesstests aus **703f2529** grün. Diese Q03-fremde Änderung bei späterem Q03-Merge ausdrücklich abgleichen.
- **Plan/Spec erfüllt:** K2-92–95 in M2.md Abschnitt 10; ONPAGE-149 einschließlich kalibrierter W/S/Syl-Regeln nachgezogen. Standardausschluss nav/footer/header/aside in ONPAGE-140/141, **M1.md Tabelle 3.4** und den Technik-/Integrations-Querverweisen konsolidiert. Historischer M1-Default bleibt als Hinweis erkennbar.
- **R01:** Der vorhandene release-macos.yml startet bereits nur bei v-Tags oder workflow_dispatch. Die vom Orchestrator genannte Änderung am ungemergten M3/R01-Stand bleibt R01 zugeordnet; kein M3-Merge.

**App/Kern-Fix (ONPAGE-142/149/153, OUT-001):** Die unveränderte E2E-Bytegleichheitsprüfung zeigte unterschiedliche Standardspalten zwischen App und CLI. `catalog.columns.defaultVisible` berücksichtigt jetzt dieselben generischen Needs::Config/AnyConfig-Voraussetzungen wie die Kern-Ansicht. Deaktivierte Spalten bleiben explizit auswählbar. Neuer Store-Fixture-Test vergleicht beide Auswahlen bei ein-/ausgeschalteten Funktionen; cmd_catalog_basic 4/4. E2E CSV/XLSX danach in beiden Browsern grün.

**K2-63-Erwartungsänderungen:**

| Datei | Alt → neu / Begründung |
|---|---|
| out-basic-site/internal_all.csv und .xlsx.txt | vollständig neu erzeugte kombinierte Spalten/Zellen nach ONPAGE-013/140/141/142/149, N07/N08 |
| out-basic-site/crawl_overview.csv und .xlsx.txt | neu erzeugte kombinierte Filter/Populationen nach N07/N09; nur temporärer Writer zur Regeneration, Golden-Hilfe anschließend unverändert wiederhergestellt |
| os-analyze/tests/tab_internal.rs | Text Ratio 8/271×100: **3,0 → 2,952**, ONPAGE-013/SQ-08; Testname/Kommentar aktualisiert, Toleranz und Logik unverändert |
| out-m2-mail/compare.txt | ausschließlich Validation / All **9→10 (+11,11 %) → 11→12 (+9,09 %)**, TECH-281/K2-85; vollständiger Abgleich mit öffentlicher Change Detection unverändert |
| e2e/visual/layout.spec.ts | 48 durch U30 behobene Chromium-test.fail-Erwartungen entfernt, UI-001/UI-121/K2-75; Assertions, neue-Befund-Prüfung und Baselines unverändert |

reports_issues mit UPDATE_GOLDEN=1 erneut grün; out-issues hatte keinen weiteren Inhaltsdiff. Keine Schwellen/Referenzdaten zur Verschleierung geändert.

## U30/U32 und native Übergabe

**U30 A1:** Vollständige Matrix vor Merge: 786 Fälle, zunächst 763 bestanden / 20 rot / drei bestehende WebKit-DPR-Skips, null Flakes. Nach fertigem Werkzeug-Build, explizitem Chromium-Pfad und App/Kern-Fix wurden alle 15 sonst roten Fälle erneut geprüft: **15/15 grün**. Je Fall letzter Nachweis: **778 bestanden, fünf bekannte U30-Fehler bis N12, drei bestehende Skips**. Ein Settings-Fall überschritt unter Last das unveränderte 200-ms-Budget (216,2 ms), Nachlauf grün. Erstfehler und Wiederholung bleiben in den Belegen sichtbar.

**U30 A2/A3:** Vorhandene U32-test.fail-Mechanik übernommen; alle Assertions bleiben aktiv. Grid nur Chromium und ausschließlich After-grid-Fokusfehler; axe nur exakte bekannte Regel/Impact/Zielmenge. Markierung am Testende nur ohne fremde Fehler; zusätzliche Befunde bleiben unerwartet rot, vollständig behobene bekannte Fehler werden unerwartete Erfolge. Abschließende a11y-Suite: **35 bestanden + fünf erwartete Fehler**, null unerwartete Fehler/Skips/Flakes. Die kollidierende Auswahlregel aus ui.css ist entfernt; Systemfarben liegen gemeinsam in primitives/contrast.css.

**U30 A4/A5:** U30-F01/F03/F04 bleiben **bekannt rot bis N12**; VoiceOver lokal durch den Orchestrator, NVDA auf Windows. Keine Screenreader-Abnahme auf Linux behauptet.

**U32 A1 – gemessene Abweichung von der ursprünglich verlangten Zahl:** U30 behebt alle **48 Chromium-Textfälle**. Der erste integrierte Visual-Lauf meldete 48 unerwartete Erfolge, zwölf erwartete WebKit-Fehler und fünf fixme. Nur die überholten Chromium-Erwartungen wurden nach K2-63 entfernt. Der vollständige Wiederholungslauf bestätigt **156 bestanden, zwölf erwartete Fehler, fünf M3-fixme**; die alte Zahl 60 wäre für diesen Stand falsch. Bericht u32.md, befunde.md, Visual-README und linux-ergebnisse.json enthalten diese echten Endwerte samt geprüftem Quellcommit. Alle **112 Linux-Bilder** ohne Baseline-Update bestätigt.

**U32 A2/A3/A4:** U32-002/003 als Werkzeugbefunde mit ungeklärter Produktwirkung eingestuft; U32-002 unter Linux/Chromium nach U30 behoben. **N12 vor macOS-/Windows-Baselines**; native Erstläufe zunächst Werkzeugprüfung, macOS **--workers=1**. Native MJS-tsc/node--check grün; keine Swift-/PowerShell-/OS-Abnahme behauptet. U32-004 rust-ini/deep-link und U32-005 api_framework.rs:617 an **N13**. Der Zeitfenstertest war im Sammellauf erneut rot, ist im finalen vollständigen Lauf 24/24 grün und wurde nicht verändert; Determinismusbedarf bleibt dokumentiert.

## Prüfungen

Schwere Befehle: timeout 3600s nice -n 10; Cargo -j 8 --locked, CARGO_INCREMENTAL=0, DEBUG=0 gemäß Serverumgebung. Maximal ein eigener Workspace-Testlauf gleichzeitig; keine fremden Prozesse beendet. Endwerte gelten für vollständige Läufe bzw. ausdrücklich ausgewiesene letzte Nachweise je E2E-Fall.

| Prüfung | Ergebnis |
|---|---|
| cargo fmt --all --check | grün |
| cargo clippy --workspace --exclude openspider-desktop --all-targets -j 8 --locked -- -D warnings | grün, 1:13 min; Desktop wegen rust-ini/deep-link ausgeschlossen |
| cargo test --workspace --exclude openspider-desktop -j 8 --locked --no-fail-fast | **grün: 3978 bestanden, 0 fehlgeschlagen, 43 bestehende Ignores**, 536 Targets inklusive Doc-Tests; **22:55 min** Wandzeit, 1292,09 s summierte Target-Zeiten |
| explizite Pflichtziele | decide_crawl 30; tab_internal 42 + 1 Ignore; cli_crawl 11; n11_crawl_path 2; n11_exports 4; reports_issues 15; reports_overview 20; sitemap_gen 18 + 1 Ignore; sitemap_gen_m2 21; engine_adapter **8/8** |
| npm typecheck / test | grün; **90 Dateien, 857 Tests**, 137,51 s |
| npm lint | **Skript fehlt**; keine Manifeständerung. C4-check:static stattdessen grün: **29 Tests**, 474 lizenzgeprüfte npm-Pakete |
| E2E-A Chromium/WebKit, workers=4 | vollständige 786-Fall-Matrix + alle erforderlichen Nachläufe wie oben; keine unerwarteten Endfehler |
| a11y Chromium/WebKit | **35 bestanden + 5 erwartete Fehler**, 400,14 s |
| U32 Visual, vier Chromium-Skalierungen + WebKit | **156 bestanden + 12 erwartete Fehler + 5 fixme**, 234,18 s |
| U32 keyboard-suite Chromium/WebKit | **178/178**, 131,50 s |
| cargo deny --locked check licenses bans sources | bans/licenses/sources grün; erlaubte Duplikat-Hinweise |
| os-parity run --all | **rot: 23/217 verglichene Exporte**, zehn Sites, vier Exporte übersprungen; Q02b-Liste unten |
| os-parity coverage | **36/155 → 129/155 verified**, 83,226 %, Ziel ≥93 erfüllt; vor/nach durch os-parity selbst ermittelt |
| historischer M1-Nachlauf | **38/38**, Word Count **0 Zellfehler** |
| python3 -I docs/plan/M2-check.py | grün: 128 Pakete, 590 spec_ids |
| bash scripts/check-neutral.sh | grün, keine Fundstellen |

Vorher für verified: N06-Volljournal aus base-4r (`roh/lauf-n06-voll.json`). Kein eigener Python-Verifikationsmaßstab. Der erste Rust-Lauf fand die alte Text-Ratio-Erwartung; der Sammellauf drei rote Targets (Text Ratio, Mail-Golden, bekannter N13-Zeitfenstertest). Nach den dokumentierten Erwartungskorrekturen ist der **vollständige** finale Lauf grün; Erstfehler werden nicht durch isolierte Erfolge ersetzt.

Belege: [Verzeichnis int-17](int-17/), insbesondere workspace-final.json, komprimierte Befehlslogs, E2E-Einzeltestnachweise, vollständiges komprimiertes Paritätsjournal, coverage-before/after.json und m1-k68.json. Das eigene target/ (54 GiB) und die lokalen U32-Browserausgaben wurden nach Sicherung gelöscht. Keine CI-Lauf-ID, da kein Push erlaubt ist.

## Q02b und nächster Schritt

Vollständige maschinenlesbare Restliste: [q02b-failures.json](int-17/q02b-failures.json). Darunter TECH-133/CRAWL-089 (inkonsistente Referenz, N09-Indexierbarkeit berücksichtigen), TECH-137 external-99 (Schwelle 100 bleibt, Overview 2 statt 1), ONPAGE-147 deutsche Soft-404 als OS-Festlegung/DEV, ONPAGE-094 kurze Body-Titel, Lesbarkeits-/Overview-Restfälle, Links-/Listen-Reihenfolge, Hreflang-/Canonical-Reports sowie Crawl-Path-Freischaltung und Verbindungs-Layout. ONPAGE-043 folgt K2-94; keine neue Ausnahme erfunden.

Der Branch ist lokal abholbar. Orchestrator: Q02b-Restparität entscheiden/abarbeiten; N12/N13 und lokale VoiceOver-/native Werkzeugprüfung vor den nativen Baselines; anschließend Push des Integrationsbranches und macOS-/Windows-CI. Der Q03-Merge muss 703f2529 abgleichen. Keine Paketbranches gepusht.
