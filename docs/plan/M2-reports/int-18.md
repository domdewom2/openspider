# Integrationsrunde 17 – m2/int-18

**PAKET FERTIG (Korrekturrunde P1/P2).** Q02b, N14 und N12 sind integriert; technische Auflagen und K2-97…99 sind umgesetzt. Rust und die gezielten UI-Suiten sind grün. Die Browser-Hauptsuite enthält einen WebKit-Zeitfehler mit grüner Einzelwiederholung. Die Korrekturrunde P1 ersetzt die historische feste Vergleichsuhr durch die OUT-123-Normalisierung im Paritätswerkzeug; aktuelle Prüfergebnisse stehen im Nachtrag unten.

Basis: origin/m2/int-17, f7ad3f29. Linux x86_64, Rust/Cargo 1.99.1, Node 22.23.4, Tokio unverändert **1.53.3**. Kein cargo update, keine Änderung an Cargo-/npm-Lockdateien, keine Referenzdatei verändert, kein Push.

## Merges

| Reihenfolge | Paket / Quellspitze | Merge-Commit | Fable |
|---|---|---|---|
| 1 | Q02b / aa15131f | 1aefb67f | approved mit Auflagen, 84 |
| 2 | N14 / ff6f205c | 78403043 | approved mit Auflagen, 85 |
| 3 | N12 / dd072aec | c39af984 | approved mit Auflagen, 82 |

Alle drei Merges waren konfliktfrei. Semantische Auflösung nach K2-99: N14-Migration und Sicht entfallen, derselbe SELECT steht inline. N13, N15, N16 und M3 wurden nicht gemergt. Eigene Commits verwenden den vereinbarten Co-Author-Trailer.

## Entscheide und Auflagen

- **K2-97:** OUT-031 Size (Bytes) = 0 für bekannte, ungecrawlte interne Ziele ratifiziert; Eintrag in M2.md Abschnitt 10. Spec-Regel aus Q02b bleibt bestehen.
- **K2-98:** OUT-095 in 06-output-ops.md präzisiert: Address = Quelle A, Hreflang URL = Ziel B des Verweises A → B, analog OUT-094.
- **K2-99:** Keine Migration 0006, Schema bleibt 5. Inline-Abfrage in crates/os-analyze/src/analysis/steps/links.rs mit Hinweis, den Autoindex-Namen bei Copy-Swap-Migrationen von url_ext_data zu prüfen. Verhindert eine vollständige OUT-184-Dateisicherung allein wegen einer Sicht.
- **N14-Planpflege:** Paket in M2-packages.json und M2.md ergänzt, tatsächlicher Pfad analysis/steps/links.rs, n14_*-Tests und Read-API-Goldens erfasst. Q04 hängt zusätzlich an N14.
- **Q02b:** DEV-039-bulk behält die ausdrücklich präzisierte Präfixreichweite: jedes interne Ziel des eigenen Start-Hosts samt Port je aufgelisteter Quelle, entsprechend TECH-141. Fremde localhost-Ziele bleiben verglichen. Die bestehenden 3-%-Toleranzen für content_soft404/serp_summary sind als **Lockerung** benannt. Eigenes Feld title_boundary ersetzt die Kopplung an count_list_items. Historische Erst-/Abbruchlogs ausgedünnt, finale Belege erhalten und Berichtverweise korrigiert.
- **N12:** UpperTable verweist nur bei gerenderter Tab-Leiste auf deren Tab-ID, sonst auf den übersetzten Tabellen-Namen. Visual-/Polish-Helfer erfassen DOM-Auswahl und sämtliche Geometrie synchron in einem evaluate. Statische Serialisierung funktioniert auch auf about:blank; Negativfälle bleiben wirksam. Keine neue Toleranz, keine Baselineänderung.
- **N12-Owns:** Bestätigte Ausweitung im Paketplan erfasst. Formale Überschneidung mit dem bereits integrierten N04 (d0ca97de) ist genau für dieses Paar in der vorgesehenen ERLAUBT-Liste des Planprüfers begründet. Prüfalgorithmus und übrige Besitzprüfungen unverändert; M2-check grün.
- **N14-Bericht:** RSS-Schätzung von ungefähr 100–200 MB bei 5M ausdrücklich **ungemessen**. Vergleichssnapshot als f7ad3f29 gekennzeichnet, N14-Kommentare deutsch. Historische Schemaaussagen durch verbindlichen Integrationsnachtrag ersetzt.
- **Runner:** ui-e2e.yml und spike-grid.yml auf macos-15 umgestellt.

## Erwartungsänderungen nach K2-63

OUT-230/OUT-234 und K2-99 begründen ausschließlich die Rücknahme der drei Read-API-Erwartungen schema_version 6 → 5 in out-basic-site.q.txt, out-basic-site.m2.q.txt und paused-crawl.q.txt. schema_v3_tables.rs erwartet wieder die Basis-Sichtinventur ohne analysis_link_facts. Der ausschließlich auf die verworfene Migration bezogene n14_migration.rs entfällt mit Migration 0006. Benchmark-Abfragen verwenden denselben SELECT inline.

Der N14-Differenztest gegen die vorherige Implementierung bleibt erhalten und ist grün, einschließlich alter Fakten und Änderungssequenzen. Keine Grenzwertänderung am WebKit-Zeittest und keine Anpassung von Referenzen, Mappings oder Paritätsschwellen durch die Integrationskorrekturen.

## Linux-Prüfung

| Prüfung | Ergebnis |
|---|---|
| cargo fmt --all --check | grün |
| cargo clippy --workspace --exclude openspider-desktop --all-targets -j 8 --locked -- -D warnings | grün, 3 min 10 s |
| cargo test --workspace --exclude openspider-desktop -j 8 --locked --no-fail-fast | **3.989 bestanden, 0 fehlgeschlagen, 44 regulär ignoriert**, 537 Test-/Doc-Test-Ziele; 29 min 26 s inklusive Build |
| os-app view/settle | keine Lastflakes beobachtet, keine Einzelwiederholung nötig; N13 separat |
| Desktop typecheck / test | grün; **90 Dateien / 858 Tests**, Vitest 296,02 s |
| CSS / check:static / npm-Lizenzen | grün; **29 statische Tests**, **474 Pakete** |
| npm lint | Skript nicht vorhanden; vorhandene CSS-Prüfung und check:static ausgeführt; kein Manifest geändert |
| cargo deny licenses bans sources | grün |
| Playwright-Hauptsuite, Chromium + WebKit, workers=4 | **976 bestanden, 1 fehlgeschlagen, 3 reguläre Skips**, 37 min 29 s |
| Chromium | **490/490 grün** |
| WebKit | **486 bestanden, 1 Zeitfehler, 3 bestehende Skalierungs-Skips** |
| a11y, beide Engines | **20/20 je Engine**, kritische/schwerwiegende Axe-Prüfungen grün |
| N12, beide Engines | **8/8 je Engine** |
| Polish | **29 Chromium / 26 WebKit bestanden**; drei WebKit-Skips für ausschließlich mit Chromium geprüfte Skalierungsfaktoren |
| Visual/U32-Layout | **120/120**, vier Chromium-Skalierungen und WebKit, drei Größen, de/en, hell/dunkel; 6,7 min |
| übrige Visual-Suite | **48 bestanden / 5 reguläre Sprach-Skips** nach K2-79; 2,1 min; vorhandene Baselines bestehen |
| Geometrie-Negativtest zusätzlich | **5/5 grün** |
| python3 -I docs/plan/M2-check.py | **129 Pakete, 590 Spec-IDs, OK** |
| bash scripts/check-neutral.sh | grün |

Linux schließt die Desktop-Hülle wie beauftragt aus. Native macOS-/Windows-Prüfung bleibt beim Orchestrator.

Der WebKit-Fehler betrifft e2e/beta/preferences.spec.ts:38, „theme and language reach both open windows within 200 ms“: korrekte Zustände, aber **315 ms > 200 ms** unter paralleler Serverlast. Unveränderte Einzelwiederholung **1/1 grün** (5,4 s). Der volle Lauf bleibt ausdrücklich **rot**; die Einzelwiederholung ersetzt ihn nicht.

Historische Belege vor P1: [Prüfzahlen](int-18/checks.json), [Rust-Summe](int-18/workspace-summary.json), [Rust-Gesamtlog](int-18/workspace.log.gz), [Browser-Summe](int-18/e2e-main-summary.json), [Browser-Gesamtergebnis](int-18/e2e-main.json.gz), [Einzelwiederholung](int-18/e2e-preferences-repeat.json.gz), [ursprünglicher 315-ms-Fehler](int-18/webkit-preferences-error.txt).

Der Browser-JSON enthält alle Ergebnisse, Fehler, Zeitwerte und Textanhänge. Die 224 eingebetteten PNG-Binärkörper sind zugunsten der Repo-Größenregel durch Namen, Typ, Base64-Länge und Prüfsumme ersetzt; keine Testresultate wurden entfernt.

## Parität vor/nach Integration

| Messung | Ergebnis |
|---|---|
| Q02b-Abschluss vor Integration | **218/218**, verified **129/155**, M1 **38/38** |
| origin/m2/int-17 Coverage-Basis | **129/155 verified** |
| integriertes parity-m2-links | **23/23**, Exit 0 |
| historischer Gesamtlauf vor P1 ohne Normalisierung | **215/218**, drei Sitemap-Datumsfehler |
| historischer Gesamtlauf mit fester Vergleichsuhr | **218/218**, nur Diagnosebeleg, keine CI-Lösung |
| Coverage nach Integration vor P1 | **129/155**, **83,226 %**, unverändert |
| M1/A3-Nachlauf vor P1 | **38/38**, vor/nach unverändert |

Vor P1 wichen ohne feste Uhr ausschließlich Lastmod-Zellen in sitemap_default.xml, sitemap_options.xml und sitemap_images.xml ab: **48/48/2 Zellen**, jeweils 31.10. statt 30.10. Alle übrigen Spalten und URL-Mengen stimmten. OUT-123 verwendet bei fehlendem HTTP-Last-Modified das Crawl-Datum. `exported_at = "2026-10-30"` und die Erzeugungsnotiz im vorhandenen Manifest `tests/golden/parity/parity-m2-onpage/manifest.toml` belegen das Referenzdatum; Metadaten mussten nicht ergänzt werden. Der [ursprüngliche ungefrorene Bericht](int-18/parity-unfixed.json.gz) bleibt erhalten.

Der historische Versuch mit libfaketime war ein Diagnosebeleg. Er erfüllt P1 nicht und wird durch die Normalisierung im Werkzeug ersetzt. [clock-cli.sh](int-18/clock-cli.sh) bleibt ausschließlich im Berichtsordner als Beleg; CI und aktuelle Prüfaufrufe verwenden direkt die unveränderte CLI und die echte Uhr. `.github/workflows/parity.yml` benötigt dafür keine Änderung.

Historische Belege: [Links](int-18/parity-links.json.gz), [Diagnoselauf mit fester Uhr](int-18/parity-all.json.gz), [Coverage](int-18/coverage-after.json.gz), [M1-Nachlauf](int-18/m1-after.json.gz), [M1-Skript](int-18/m1-nachlauf.py).

## Ressourcen und Bereinigung

Schwere Befehle liefen mit timeout 3600s nice -n 10, Cargo -j 8 --locked, CARGO_INCREMENTAL=0 und Dev-/Test-Debuginfo 0. Secret Store memory, Chromium über scripts/ci/linux-core.sh chromium. Vor Workspace- und Paritäts-Gesamtläufen wurde auf höchstens einen fremden Workspace-Lauf und mindestens 25 GiB freien Speicher gewartet.

Die Prozesszählung bindet das Muster an echte Cargo-Prozesse: ^([^ ]*/)?cargo test --workspace. Das ungebundene Muster zählt hier zusätzlich Agent-Auftragstexte und timeout-Eltern. Die tatsächliche Lastgrenze blieb erhalten; Wartezeiten und Startpunkte sind protokolliert.

Setup-Korrekturen sind offengelegt: erster Visual-Ansatz mit dynamischem Import scheiterte auf about:blank; statische Serialisierung und Negativnachweis folgten. Ein erster Hauptsuite-Start vor dem Build von os-testsite wurde mit zehn Setup-Fehlern abgebrochen, danach vollständig neu ausgeführt. Keine Produkt-/Testlockerung für diese Startfehler.

Nach Sicherung aller Belege wurden **target/**, eigene Testausgaben und der eigene node_modules-Symlink entfernt; ein eigener übriggebliebener Vite-Server wurde beendet. Keine fremden Caches oder Prozesse bereinigt. Nach Löschung rund **103 GiB frei**; [Cleanup-Beleg](int-18/cleanup.json).

## Offene CI-Abnahme / nächster Schritt

1. **Kein uneingeschränkt grüner UI-Gesamtlauf:** WebKit-Fensterabgleich 315 ms; Einzelwiederholung grün. Unter CI-/ruhiger Last bewerten, ohne den 200-ms-Grenzwert zu lockern.
2. **Native Matrix samt Lauf-ID offen:** Der Orchestrator holt den Branch, pusht und startet macOS/Windows. Hier kein Push, keine erfundene Lauf-ID; VoiceOver/NVDA und native Hardwareabnahmen bleiben dort.

Die serverseitige Korrekturrunde ist abgeschlossen. Nächster Schritt: Orchestrator bewertet den UI-Lastbefund in der CI und führt die native Matrix aus.

## Wiederaufnahmeprüfung am 31.10.2026

Vorhandene Integration und Belege geprüft; keine erneuten Merges und keine weiteren Paketstände übernommen. Fmt, M2-check (129 Pakete / 590 Spec-IDs) und Neutralitätsprüfung erneut grün. Runner-Kommentare und Regelverweis zur festen Uhr berichtigt. Keine Produkt- oder Teständerung; die oben dokumentierten Gesamtlaufbefunde bleiben offen. target/ bleibt entfernt.

## Korrekturrunde P1/P2 (31.10.2026)

**P1 – Umsetzung:** Der Runner beobachtet für jeden einzelnen Exporter-Aufruf (einschließlich Wiederholungen) den Start und das Ende in UTC. Liegen beide auf demselben Tag, ist damit das Datum aller URL-Abrufe dieses Crawls belegt. Bei Tageswechsel oder rückläufiger Uhr wird der Vergleich mit Fehler beendet; es wird kein Datum geraten und kein Intervall pauschal normalisiert. Die Referenzseite verwendet ausschließlich das vorhandene `exported_at` aus dem Manifest (Datum oder RFC-3339-Zeitstempel, UTC). Fehlende/ungültige Metadaten sind bei befülltem Lastmod ein Fehler.

Nur Sitemap-Exporte mit Server-Quelle erhalten in `Lastmod` den Platzhalter `<CRAWLDATE>`: OS-Zellen genau beim OS-Crawl-Datum, Referenzzellen genau beim Referenz-Exportdatum. Alle anderen Werte bleiben unverändert, ebenso benutzerdefinierte Datumswerte. In der Output-Spec ist keine weitere exportierte Datumsspalte mit diesem Ersatzwert definiert; insbesondere wird `Last Modified` in Tabellen nicht verändert. Referenzdateien, Mappings, Schwellen, Lockdateien und CI-Uhren bleiben unverändert.

**Tests/K2-63:** Additive OUT-123-Tests, keine bestehende Erwartung gelockert. Die temporäre Sitemap-Fixture belegt für Standard- und Bilder-Sitemaps den positiven Vergleich bei unterschiedlichen Crawl-Tagen sowie rote Vergleiche bei geändertem echtem Lastmod und beim auf der falschen Seite auftauchenden Referenzdatum. Benutzerdefinierte Daten bleiben rot. Zusätzliche Tests prüfen fehlende/ungültige Referenzmetadaten, UTC-Offset, Mitternacht und rückläufige Uhr. Erster Lauf des neuen Tests scheiterte an der fehlenden XML-Namespace-Angabe seiner neuen Fixture; diese wurde korrigiert, keine Produktregel geändert.

**P2 – erfüllt:** `e2e/beta/preferences.spec.ts:38` bleibt unverändert mit **200 ms**. Der historische WebKit-Wert **315 ms** bleibt als Lastbefund mit rotem Gesamtlauf und grüner Einzelwiederholung dokumentiert. Bewertung in ui-e2e macOS/Linux durch den Orchestrator; keine lokale Wiederholung als Ersatz für diese CI-Abnahme, keine erfundene Lauf-ID.

Aktuelle P1-Prüfung: `cargo fmt --all --check`, `cargo clippy -p os-parity --all-targets -j 8 --locked -- -D warnings` und `cargo test -p os-parity -j 8 --locked` grün: **189 bestanden, 0 fehlgeschlagen, 0 ignoriert, 11 Test-/Doc-Test-Ziele**. Clippy 14,75 s. M2-check erneut **129 Pakete / 590 Spec-IDs, OK**, Neutralitätsprüfung grün. Der bereits belegte vollständige Linux-Workspace- und Desktop-Stand oben bleibt bestehen; die Korrekturrunde wiederholt gemäß Nachtrag gezielt os-parity.

Belege: [Tests](int-18/p1-tests.log.gz), [Clippy](int-18/p1-clippy.log.gz), [Fmt](int-18/p1-fmt.log.gz), [Planprüfung](int-18/p1-plan.log.gz), [Neutralität](int-18/p1-neutral.log.gz), [Build](int-18/p1-build.log.gz), [Prüfaufruf mit Ressourcenschranke](int-18/p1-run.sh).

**P1 – erfüllt:** `os-parity run --all` mit echter Uhr und direkter CLI: **218/218 grün**, zehn Sites, drei bestehende unavailable-Skips, keine Fehler. `parity-m2-links` darin **23/23**; Sitemap default/options/images jeweils **100 %**. Lauf **02:01:28–02:07:06 UTC, 5 min 38 s**. Coverage vorher/nachher **129/155**, M1 vorher/nachher **38/38**. Keine Uhrmanipulation und keine CI-Anpassung nötig.

Ressourcenschranke: zwei Minuten gewartet, bei höchstens einem fremden Workspace-Test und **68 GiB frei** gestartet. Wie oben bindet die Prozesszählung das Muster an echte Cargo-Prozesse; der ungebundene Ausdruck zählte bei der Vorprüfung zehn Prozesse, davon nur zwei echte Workspace-Tests. Auftrags- und Shell-Texte erzeugen sonst eine dauerhafte Fehlblockade. Der ausgeführte Prüfaufruf und Start/Ende sind als Beleg erhalten.

Aktuelle Belege: [Prüfzahlen P1/P2](int-18/p1-checks.json), [Gesamtlauf ohne feste Uhr](int-18/p1-parity-all.json.gz), [Coverage](int-18/p1-coverage.json.gz), [M1-Nachlauf](int-18/p1-m1.json.gz), [Laufjournal](int-18/p1-run.log.gz). Implementierung: **8f581ae3**. Bestehende Merges nicht wiederholt, keine weiteren Branches übernommen. **P2 erfüllt** durch unveränderte Grenze und ausdrücklich dokumentierten Lastbefund; die native CI-Bewertung bleibt beim Orchestrator.

Nach Sicherung der Belege wurde das eigene **target/** vollständig gelöscht; **97.66 GiB frei**. [Bereinigungsbeleg](int-18/p1-cleanup.json). Kein Push.

**PAKET FERTIG – P1/P2 erfüllt. Nächster Schritt: Orchestrator übernimmt den Branch, pusht und führt die CI-/native Abnahme aus.**
