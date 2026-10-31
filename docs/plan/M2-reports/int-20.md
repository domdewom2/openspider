# Integrationsrunde 19 (M2), Branch int-20

Stand: 31.10.2026, Linux x86_64. Branch `m2/int-20`, Basis `origin/m2/int-19` = `d7b7a6d6` (Integrationsrunde 18 mit N13). Kein Push. N15 wurde nicht integriert.

## Merges und Nachträge

In vorgegebener Reihenfolge textuell konfliktfrei integriert:

1. `ccfe1f88` – `m2: Merge n16 (Fable approved mit Auflagen 84)`, Paketspitze `28648507`, Korrekturrunde erfüllt.
2. `fdf2d7ab` – `m2: Merge n17 (Fable, CI-Korrektur)`, Paketspitze `a30cc1a3`; ausschließlich Workflow und N17-Bericht.

N16-Befund 7: N16 und N17 als Welle 4n in `M2-packages.json` und `M2.md` ergänzt, mit den beauftragten Owns und N16-extra_spec_ids PERF-007/ONPAGE-144. M2-check.py: 131 Pakete, 590 spec_ids, OK. Die übermittelten Mac-mini-M1-Abnahmewerte (31.10., 16 GB, macOS 26.3, rustc 1.99.1; Nullpaar- und 5-%-Near-Fixture, 100k/1M, vier/acht Threads) stehen vollständig in [n16.md](n16.md). Last vor jedem Lauf <4, Budget <600 s / <2 GB in allen Fällen erfüllt.

Keine Test-/Golden-Erwartung in dieser Integration geändert; keine K2-63-Anpassung erforderlich. `Cargo.lock` und Manifeste gegenüber der Basis unverändert, Tokio 1.53.3; kein cargo update. Der N16-Merge enthält die bestehende Architektur-Dokumentation des Verfahrens. N17 stellt die E2E-Werkzeuge und die native macOS-WebKit-Tastaturnavigation im Workflow bereit; keine Produktänderung.

## Linux-Prüfung

Vor jedem Workspace-Aufruf `~/ws-slot.sh`; schwere Befehle `timeout 3600s nice -n 10`, Cargo `-j 6 --locked`. `CARGO_INCREMENTAL=0`, Debug-Profile `line-tables-only`, Secret Store `memory`. Chromium aus dem beauftragten Playwright-Pfad. Keine Performance-Messung auf dem Linux-Integrationsserver.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -j 6 --locked -- -D warnings` | Exit 0, ohne Ausschluss, einschließlich Desktop; 123,14 s |
| `cargo test --workspace -j 6 --locked --no-fail-fast` | Exit 0; **4.024 bestanden, 0 fehlgeschlagen, 49 Ignores**, 544 Ergebnisblöcke; **1.463,83 s** einschließlich Build |
| Desktop im Workspace-Test | 20 bestanden, 1 bestehender P2-Ignore |
| `OPENSPIDER_NEAR_DUP_THREADS=1 cargo test -p os-analyze --test near_dup_n16 --test near_dup_d02 -j 6 --locked` | Exit 0; **22/22**, einschließlich vier produktquelltextinterner Grenztests |
| Derselbe gezielte Lauf mit `OPENSPIDER_NEAR_DUP_THREADS=8` | Exit 0; **22/22**, 21,68 s |
| Desktop-Typecheck und Unit-Tests nach `npm ci` | Exit 0; **90 Dateien / 858 Tests**, 115,79 s |
| Desktop `check:static` | Exit 0; IPC-Generator aktuell, 474 Lizenzprüfungen, 29 Static-/Token-Tests |
| Bridge-Build mit Feature `e2e-bridge` | Exit 0; 91,99 s |
| E2E-Werkzeuge `cargo build -j 6 --locked -p os-testsite -p os-cli -p os-parity --bins` | Exit 0; 17,84 s |
| `actionlint .github/workflows/ui-e2e.yml` (1.7.12) | Exit 0 |

Workspace-Test: **05:22:56 bis 05:47:20 UTC**, keine Lastflakes und keine Einzelwiederholung nötig. Gegenüber int-19 zwölf zusätzliche bestandene Tests und vier zusätzliche `Benchmark`-Ignores aus N16 (100k/1M mit Near-Duplicates sowie zwei reine Fixture-Vorbereitungen); kein bestehender Test neu ignoriert. Keine Erwartung abgeschwächt. Native Messungen dieser großen Fälle sind im N16-Bericht belegt.

Umgebungsnachtrag: Der erste Frontend-Aufruf endete vor Testbeginn mit `tsc: not found` (Exit 127), weil `node_modules/` fehlte. Nach `npm ci` waren Typecheck, Tests und Static-Gate vollständig grün; kein Lockfile-/Quellfix. Ein zuerst gefundenes actionlint 1.7.7 versteht den bestehenden YAML-Alias bei `pull_request.paths` nicht; der beauftragte Workflow besteht mit actionlint 1.7.12 ohne Änderung.

Playwright: `OS_E2E_TRANSPORT=bridge npm run -w apps/desktop e2e -- e2e/beta e2e/mcp e2e/a11y --project=chromium --project=webkit --workers=4`, unter dem genannten Timeout-/Nice-Rahmen. **92/92 bestanden**, Exit 0, **1.062,66 s / 17,7 min**, keine Retries, Skips oder Fehler. Beide Engines prüfen die echte Bridge mit vorher gebautem Fixture-Server, CLI und Paritätswerkzeug. Alle sieben in N17 beschriebenen macOS-a11y-Befundfälle sind unter Linux-WebKit grün. Die beiden MCP-Sessionfälle warten den echten zehnminütigen Rust-Timeout ab; kein Fake-Time-Ersatz.

| Abschlussprüfung | Ergebnis |
|---|---|
| `os-parity run --all --openspider … --out /tmp/int-20/parity` | Exit 0; **218/218 Vergleiche grün**, 10 Sites present, 0 Fehler, 0 Warnungen; 303,84 s |
| `os-parity coverage --milestone M2 --parity-report /tmp/int-20/parity/report.json --write /tmp/int-20/coverage.json` | Exit 0; **129/155 verified = 83,226 %** |
| `python3 -I docs/plan/M2-check.py` | Exit 0; 131 Pakete, 590 spec_ids, OK |
| `bash scripts/check-neutral.sh` | Exit 0, keine Fundstellen |
| `cargo deny check licenses bans sources` | Exit 0; alle drei Kategorien grün |

Paritätslauf **06:08:55 bis 06:13:59 UTC**, mit echter Systemuhr. Drei bestehende unavailable-Skips unverändert: `content_near_duplicates.csv`, `bulk_content_near_duplicates.csv`, `bulk_security_form_on_http_url_inlinks.csv`. Keine Referenz, kein Mapping und keine Schwelle geändert. Das Ergebnis bleibt 218/218 und Coverage 129/155 wie in int-19. Ein erster Coverage-Aufruf verwendete irrtümlich `parity-all.json` statt der vom Werkzeug geschriebenen `report.json` und endete mit Exit 2; nach Pfadkorrektur Exit 0, ohne Produktänderung.

Umgebung: rustc/cargo 1.99.1, Node 22.23.4, npm 10.9.9, actionlint 1.7.12. Die Cloud-Runtime-Statusschnittstelle und `/etc/codex/network-policy.json` sind in dieser Umgebung nicht verfügbar; bestehende Netzwerk-/Proxykonfiguration blieb unverändert. `npm ci` nutzte das vorhandene Lockfile.

## Belege und Abschluss

Dauerhafte Belege: [Prüfzahlen](int-20/checks.json), [Befehle und Laufzeiten](int-20/results.json), [Workspace-Test](int-20/workspace-test.log.gz), [Clippy](int-20/clippy.log.gz), [Thread 1](int-20/near-1.log.gz), [Thread 8](int-20/near-8.log.gz), [Frontend nach Installation](int-20/frontend-retry.log.gz), [E2E](int-20/e2e.log.gz), [Werkzeug-Build](int-20/tools-build.log.gz), [Paritätsreport](int-20/parity-all.json.gz), [Coverage](int-20/coverage.json.gz), [Lizenzprüfung](int-20/deny.log.gz); weitere kleine Prüflogs im selben Ordner. Rohbelege zusätzlich unter `/tmp/int-20/`. Fehlaufrufe sind mitgesichert; finale Prüfergebnisse sind grün.

Nach Sicherung der Belege wurden das eigene `target/` und die erzeugten Playwright-Testresultate entfernt und ihre Abwesenheit geprüft. Keine Änderungen an N15, kein Push. **PAKET FERTIG** für diese Linux-Integration; nächster Schritt: Orchestrator holt `m2/int-20` ab und ergänzt nach Push die native CI.


## Native Übergabe

N17: macOS-WebKit/Windows-Chromium und vollständige CI-Matrix durch den Orchestrator nach Abholung/Push prüfen; besonders die sieben macOS-a11y-Fälle und elf bisherigen Werkzeug-Startfehler je Plattform. Linux bestätigt keine native macOS-Einstellung. N13s native A1 aus int-19 bleibt beim Orchestrator. Der beauftragte Verzicht auf Push bleibt bestehen.
