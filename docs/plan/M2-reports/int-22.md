# M2 Integrationsrunde 22

Stand: 31.10.2026. Branch `m2/int-22`, Basis `7fb4cc57` (origin/main: int-21, Release-Fix und Test-Zeitlimit). Linux x86_64, Kernel 7.0.0-30-generic, Rust 1.99.1, Node 22.23.4. Kein Push, kein Rebase.

## Integration

| Reihenfolge | Paket | Quellstand | Merge |
|---|---|---|---|
| 1 | N20-final einschließlich N15F (`b35f00ef`) | `a7b510a4` | `62164954` |
| 2 | N19 | `713f51e5` | `a0157fc9` |
| 3 | A03-plan | `3b68c000` | `38775e64` |

Alle Merges konfliktfrei mit `--no-ff`. N15F Fable approved, N20 Fable approved mit Auflagen. N19 ergänzt die exakte PERF-001-Wachstumsdiagnose in `os-loadtest/src/engine.rs`; N15F/N20 ergänzen Writer-/Nutzdaten-Diagnosen sowie RSS-Abtastung in anderen Dateien. Beide Feldmengen bleiben erhalten, keine Umbenennung. A03 enthält Build-Nachweis, Durchklickplan und Befundvorlage; diese Integration führt keine menschliche Abnahme durch.

Nicht integriert: `m2/u31`, `m2/n21`, `m2/n22`, `m2/n23` sowie der verworfene mimalloc-Ansatz `m2/n15`.

## Planung und Testregeln

Pakete N15F, N19, N20 in `M2-packages.json` und `M2.md` ergänzt; N21/N22/N23 nur als Vormerkungen in Welle 4n mit Abhängigkeit U31 und zunächst ohne Dateireservierung. Zusatz-IDs und Besitz gemäß Integrationsauftrag. M2-check erlaubt die sequenzielle N15F/N20-Überlappung sowie additive N19-Diagnose. Weitere Überschneidungen mit den bereits integrierten N04/N13 sind einzeln begründet. 137 Pakete, 590 primäre Spec-IDs; Planprüfung grün.

Keine Test-, Golden-, Manifest- oder Lockfile-Änderung durch den Integrator. N19 bringt die bereits im Paketbericht begründete HTML-Bereitschaftskorrektur für `beta_n04` mit; Assert-Grenzen und Timeouts bleiben erhalten. Keine zusätzliche Erwartungsänderung nach K2-63. Tokio bleibt 1.53.3; kein `cargo update`.

## Prüfbedingungen

`CARGO_INCREMENTAL=0`, Dev-/Test-Debug `line-tables-only`, `OPENSPIDER_SECRET_STORE=memory`. Schwere Befehle mit `timeout 3600s nice -n 10`, Cargo `-j 6 --locked`; vor Workspace-Läufen `~/ws-slot.sh`. Chromium aus `/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`, Playwright mit zwei Workern, echter Bridge und Chromium/WebKit. Shared Server mit parallelen Paketen N21–N23; Messungen erst bei Ein-Minuten-Last < 6 und ohne eigene parallele Prüfläufe.

Prüfergebnisse:

| Prüfung | Ergebnis | Dauer |
|---|---|---|
| Format | grün | siehe Abschlussprotokoll |
| Clippy gesamter Workspace einschließlich Desktop, alle Targets, `-D warnings` | grün | 1 min 51 s |
| `cargo test -p os-engine -p os-store -p os-loadtest -j 6 --locked` | 666 bestanden, 15 bestehende Ignores, 102 Testprogramme inkl. Doc-Tests | 6 min 35 s |

| `cargo test --workspace -j 6 --locked` ohne Ausschluss | 4031 bestanden, 49 bestehende Ignores, 546 Testprogramme inkl. Doc-Tests; keine Fehler oder Wiederholungen | 25 min 5 s |

Workspace-Test: 18:07:57–18:33:02 UTC. `beta_n04`: alle fünf Tests bestanden, einschließlich des 5.000-Seiten-Crawls (25,81 s). Kein Lastflake. Danach ausschließlich abgeschlossene Test-Executables aus diesem Worktree entfernt (626 Dateien, rund 56,6 GB); Bibliotheken für die Werkzeugbauten erhalten. Übrige Prüfergebnisse und Messwerte werden nach Abschluss ergänzt. Rohprotokolle dieser Ausführung liegen unter `/tmp/openspider-int22`.

## Offene Folgearbeiten

- N20-Fable-Folgeauflage: Steuer-Nachrichten in `crawl.rs` im `select!` nicht wartend senden. Auf ausdrücklichen Auftrag in int-22 nur dokumentiert, keine Implementierung.
- N19: macOS-arm64-Ursachenklärung der historischen PERF-001-Streuung bleibt beim Folgepaket; Linux-Messungen ersetzen diese nicht.
- U31/N21/N22/N23 und die menschliche A03-Runde laufen separat; kein Abnahmenachweis durch bloßes Zusammenführen der A03-Unterlagen.

## Abschluss durch den Orchestrator (31.10.2026 abends)

Der Codex-Lauf brach während der Browserprüfung am Kontingentlimit ab (Merges, Format, Clippy, Paket- und Workspace-Tests waren abgeschlossen). Die übrigen Prüfungen hat der Orchestrator mit demselben Prüfskript ab der Browserprüfung neu gestartet (`/tmp/openspider-int22/rest.sh`, gleiche Umgebung):

| Prüfung | Ergebnis |
|---|---|
| Playwright e2e/beta, e2e/mcp, e2e/a11y, Chromium + WebKit, 2 Worker, echte Bridge | 92 bestanden, keine Fehler (27,7 min); abgebrochener Erstversuch (63 bestanden, 0 Fehler) archiviert als `e2e-abgebrochen.log` |
| os-parity run --all / coverage M2 | grün; verified 129/155 (83,2 %) |
| M2-check.py | OK (137 Pakete, 590 spec_ids) |
| check-neutral.sh | zunächst 1 Fundstelle in `n19.md:15` (Versionsmuster neben dem Wort für den Vergleichslauf); Formulierung angepasst, danach keine Fundstellen |
| cargo deny check licenses / cargo metadata --locked | grün |
| PERF-001 small (Last < 6) | bestanden, Spitzen-RSS 170.700.800 Byte, 2.004,7 URLs/s |
| PERF-003 large (Last < 6) | bestanden, Spitzen-RSS 411.516.928 Byte, 1.050,2 URLs/s |

GitHub Actions auf `75afc4ac` (vor dem Doku-Fix): CI 41231849144 grün (Linux, Windows, macOS arm64; Intel nach Vorgabe nicht), Spec coverage M2 41231849702 grün, ui-e2e 41231849663 rot nur in `e2e-a (linux-webkit)`: `e2e/beta/overview.spec.ts:8` fehlgeschlagen, `control`, `detail-during-crawl`, `n04-live` flaky. Dieselben WebKit-Beta-Tests sind bereits auf `main` (int-21, Lauf 41206162190) rot bzw. flaky; keine Regression dieser Runde. Die Ursache wird im M3-Paket FIX-E behoben (Overview-Erstantwort, Live-Detail) und anschließend in die M2-Linie übernommen.

Stand: **PAKET FERTIG** (Integration), offen nur die übernommene WebKit-Stabilität aus FIX-E.
