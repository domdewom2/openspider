# Integrationsrunde 18 (M2), Branch int-19

Stand: 31.10.2026, Linux x86_64. Arbeitsbranch `m2/int-19`, Basis `origin/m2/int-18` = `1503ca76` (Q02b/N14/N12). Kein Push; native CI folgt durch den Orchestrator.

## Merge und Auflagen

`m2/n13` (`b05cc985`) wurde textuell konfliktfrei mit dem geforderten Merge-Commit **`5a9e5449` – `m2: Merge n13 (Fable approved mit Auflagen 86)`** integriert. Die Commit-Trailer nennen OpenAI Codex. Nacharbeiten: `ff17118a` (A3) und `2d46c6af` (A2).

| Auflage | Ergebnis |
|---|---|
| A1 – native Nachweise | Offen beim Orchestrator: Branch abholen/pushen, neue macOS-arm64/x64- und Windows-x64-CI ausführen und Lauf-IDs ergänzen. Linux ersetzt diese Nachweise nicht. Beobachtungspunkte und bisherige Plattformbefunde stehen in [n13.md](n13.md), insbesondere Chromium-Erststart und Windows-Dateisperren. |
| A2 – Sitemap-Test nach Port-0-Umstellung | Erfüllt. `crates/os-parity/tests/run_site.rs::sitemap_crawl_dates_normalize_but_real_lastmod_differences_fail` nutzt `{BASE}` in der temporären Referenz und `req.base_url.trim_end_matches('/')` im Exporter. Kein `<loc>` mit Port 0 mehr. `run_site` **26/26**; Parität **218/218**, Coverage **129/155**. |
| A3 – Owns und Pin dokumentieren | Erfüllt, **K2-100** in M2.md Abschnitt 10 und konsistente N13-Owns/Notizen in M2-packages.json. M2-check.py besteht unverändert; keine neue Ausnahme für Besitzüberschneidungen nötig. |

**K2-63 / OUT-123:** Nur die Origin-Darstellung der temporären Testfixture wurde angepasst: vorher `http://127.0.0.1:0`, jetzt `{BASE}` in der Referenz und die tatsächlich gebundene Origin im Export. URL-Pfade, Lastmod-Werte und sämtliche Assertions bleiben gleich. Drei Varianten (default, explizit server, images) prüfen jeweils einen positiven und zwei negative Fälle; der zusätzliche custom-Fall bleibt negativ. Damit bleiben drei Positiv- und sieben Negativfälle aktiv. `crates/os-parity/src/crawl_date.rs` ist unverändert, einschließlich der grünen Fail-closed-Tests für Tageswechsel/rückläufige Uhr und fehlende/ungültige Referenzmetadaten. Echte Referenzexporte, Schwellen und Vergleichsregeln wurden nicht geändert.

**Owns-Erweiterung, Grund Orchestrator-Nachtrag:** `crates/os-app/tests/{cmd_api_framework,cmd_copy_page_fixture,pending_w1_i01,pending_w1_k02,pending_w1_k04}.rs` für Fixture-Synchronisierung/Dateisperren; `crates/os-parity/tests/run_site.rs` für Ports/Origin; `crates/os-engine/src/testing.rs` sowie `crates/os-analyze/tests/tab_accessibility.rs` für vorbereitete und wiederverwendete echte Browser. Fachliche Erwartungen der integrierten N13-Nachträge siehe deren K2-63-Begründungen in [n13.md](n13.md).

**Abhängigkeiten:** rust-ini auf 0.21.1 gepinnt (nur Cargo.lock), bis tauri-plugin-deep-link > 2.6.2 mit rust-ini 0.21.3 kompatibel; CI immer --locked. Der Lockfile-Stand entspricht exakt N13; gegenüber int-18 nur rust-ini 0.21.3 → 0.21.1 samt Prüfsumme und dessen Abhängigkeit trim-in-place 0.1.8. Tokio bleibt **1.53.3**. In dieser Integration wurde kein cargo update ausgeführt.

Die optionale Drop-Guard-Erweiterung wurde nicht vorgenommen; die vorhandenen expliziten `browser.close().await` bleiben bestehen.

## Linux-Prüfung

Vor jedem Workspace-Aufruf wurde `~/ws-slot.sh` ausgeführt. Schwere Befehle liefen mit `timeout 3600s nice -n 10`, Cargo mit `-j 6 --locked`, `CARGO_INCREMENTAL=0`, beiden Debug-Profilen `line-tables-only` (geerbtes Strip `debuginfo`). Secret Store `memory`. Chromium: installierter Playwright-Chromium, über `OPENSPIDER_CHROMIUM` ausgewählt; `scripts/ci/linux-core.sh chromium` grün. Die unten angegebenen Dauern enthalten gegebenenfalls Bau und Cargo-Lock-Wartezeit und sind keine Performance-Messung.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -j 6 --locked -- -D warnings` | Exit 0; **ohne Ausschluss, einschließlich openspider-desktop**, Cargo-Dauer 2 min |
| `cargo test --workspace -j 6 --locked --no-fail-fast` | Exit 0; **4.012 bestanden, 0 fehlgeschlagen, 45 bestehende Ignores**, 543 Ergebnisblöcke einschließlich leerer Targets/Doctests; **28 min 33,71 s** |
| Desktop-Anteil im Workspace-Test | **20 bestanden**, 1 bestehender P2-Ignore; Desktop unter Linux baubar, kein Ausschluss erforderlich |
| `cargo test -p os-parity --test run_site -j 6 --locked` | Exit 0; **26 bestanden**, 0 fehlgeschlagen/ignoriert; **23,68 s** einschließlich Bau, reine Tests 0,04 s |
| `npm run -w apps/desktop typecheck` | Exit 0 |
| `npm run -w apps/desktop test` | Exit 0; **90 Dateien / 858 Tests**, 126,15 s |
| `npm run -w apps/desktop check:static` | Exit 0; **29 Tests** |
| `npm run -w apps/desktop check:licenses` | Exit 0; **474 Pakete** unter Positivliste |
| `cargo deny check licenses bans sources` | Exit 0; alle drei Kategorien grün |
| `os-parity run --all` | Exit 0; **218/218 grün**, 10 Sites present, 3 bestehende unavailable-Skips, keine Fehler; **4 min 12,18 s** |
| `os-parity coverage --milestone M2 --parity-report …` | Exit 0; **129/155 verified = 83,226 %**, unverändert gegenüber int-18 |
| `python3 -I docs/plan/M2-check.py` | Exit 0; **129 Pakete, 590 spec_ids**, OK |
| `bash scripts/check-neutral.sh` | Exit 0; keine Fundstellen |

Der Workspace-Test lief am 31.10. von **04:14:05 bis 04:42:39 UTC**; die gezielte run_site-Suite danach bis 04:43:03 UTC. Keine Lastflakes, keine Fehlversuche bei diesen Tests, keine Einzelwiederholungen nötig. Insbesondere die beiden Accessibility-Browsertests, der Geheimnis-Dateiscan, Copy/I01/K02/K04, Sliding-Window-Test und A2 sind im vollständigen Lauf grün. Keine neuen Ignores oder gelockerten Fachassertions. N13 bereitet die echten Fixture-Browser mit 60 s Startbudget vor; die Produkt-/Seitenzeitlimits bleiben unverändert (Begründung in n13.md). A2 ändert keine Zeitbudgets.

Der Paritätslauf lief mit **echter Systemuhr**, ohne Uhrmanipulation, von **04:43:39 bis 04:47:52 UTC**. Die direkten Binärdateien stammen aus dem abgeschlossenen Workspace-Bau. Befehl: `target/debug/os-parity run --all --openspider <Workspace>/target/debug/openspider --out /tmp/int-19/parity`. Sitemap default/options/images jeweils **100 %**. Die drei unavailable-Skips bleiben die beiden Near-Duplicate-Exporte und Security/Form-On-HTTP-Inlinks; kein neuer Skip. Coverage wurde anschließend aus genau diesem Gesamtreport berechnet.

N13 und die Nacharbeiten berühren keine Frontend-/E2E-Dateien; daher gemäß Auftrag keine Playwright-Läufe. Native A1 bleibt ausdrücklich beim Orchestrator.

## Belege und Übergabe

Dauerhafte Belege: [Prüfzahlen](int-19/checks.json), [Befehle und Laufzeiten](int-19/results.json), [Workspace-Test](int-19/workspace-test.log.gz), [run_site](int-19/run-site.log.gz), [Clippy](int-19/clippy.log.gz), [Frontend](int-19/frontend.log.gz), [Lizenzprüfung](int-19/deny.log.gz), [Chromium](int-19/chromium.log.gz), [Paritäts-Gesamtreport](int-19/parity-all.json.gz), [Coverage](int-19/coverage.json.gz), [Format/Plan/Neutralität](int-19/other-checks.log). Rohbelege zusätzlich unter `/tmp/int-19/`.

Nach Sicherung der Belege wurde das eigene `target/` vollständig gelöscht und seine Abwesenheit geprüft. Kein Push. **PAKET FERTIG** für diese Linux-Integration; nächster Schritt: Orchestrator holt `m2/int-19` ab, pusht und ergänzt die native CI unter A1.
