# M3 – Integrationsrunde c

Stand: 31.10.2026 · Branch `m3/int-c` · Basis `m3/int-b` (`2ae74d23`).

## Merges und Konfliktlösung

Die bei Übernahme des Arbeitsverzeichnisses bereits vorhandenen Merge-Commits wurden geprüft und beibehalten:

1. `5155c5c2` – `m3: Merge r03 (Fable FREIGEGEBEN 80)`, Paketstand `16112ad2` (Auflagen erfüllt).
2. `f3a8ba4f` – `m3: Merge d01 (Fable approved 86)`, Paketstand `1148abba` (Auflagen F1–F5 erfüllt).

Beide sind eigene Merge-Commits mit dem geforderten Co-Author-Trailer. Der erste Merge basiert unmittelbar auf `2ae74d23`, der zweite auf dem R03-Merge.

Der Inhaltskonflikt in `.github/workflows/release.yml`, Job `assemble`, ist im D01-Merge gelöst (mit `git show --remerge-diff` nachgeprüft): Release-Notes mit `mkdir -p release-reports` und R03-`echo` → unveränderter D01-Archivschritt aus dem Paketbranch → SHA256SUMS → Uploads. Kein Zugriff auf `release-reports/R03.txt`, kein D01-STUB. R03-Job `smoke-installed` und `draft.needs: [prepare, assemble, smoke-installed]` sind enthalten.

Keine zusätzlichen Code-, Test-, Golden- oder Manifeständerungen in dieser Integrationsrunde; deshalb keine Erwartungsänderung nach K2-63. Tokio bleibt im unveränderten Lockfile auf `1.53.3`; kein `cargo update`.

## Linux-Prüfung

Alle geforderten Linux-Prüfungen sind grün. Rohlogs dieser Runde: `/tmp/openspider-int-c-validation/`.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets -j 6 --locked -- -D warnings` | grün, Exit 0; Linux-Ausnahme Desktop-Hülle, native Prüfung beim Orchestrator |
| `cargo test --workspace --exclude openspider-desktop -j 6 --locked` | grün, Exit 0; 3.971 bestanden, 0 fehlgeschlagen, 43 regulär ignoriert; 541 Testprogramme/Doc-Test-Gruppen, Kompilierung 4 min 47 s |
| Desktop `typecheck` und `test` | grün, Exit 0; 91 Dateien, 858/858 Tests (149,27 s) |
| Desktop `check:static` (zusätzlich) | grün; 29/29 Tests, 27 IPC-Wrapper aktuell, npm-Lizenzen 476/476 zulässig |
| Node: `tests/release/*.test.mjs`, `tests/fixtures/migrations/node/*.test.mjs`, `scripts/release/licenses/generate.test.mjs` | grün, 25/25, keine Skips; einschließlich beider geforderten Konflikt-Vertragstests |
| Python: `python3 -m unittest discover -s tests/smoke -v` | grün, 5/5 |
| Linux-Smoke-Trockenlauf nach `M3-acceptance/smoke/README.md` | grün, Exit 0; Text 0,45 s, JavaScript 9,90 s; Exporte, Sitemaps, JS-Probe und direkte `.ostask`-Ausführung bestanden |
| actionlint 1.7.12: `release.yml`, `smoke-installed.yml` | grün, Exit 0, keine Zusatzkonfiguration |
| `bash scripts/check-neutral.sh` | grün, keine Fundstellen |
| `cargo deny --locked check licenses bans sources` (zusätzlich) | grün: bans, licenses, sources jeweils `ok` |

Umgebung: Linux x86_64, Node `v22.23.4`. Cargo mit `--locked`, `-j 6`, `CARGO_INCREMENTAL=0` und Debugprofilen `line-tables-only`; schwere Befehle mit `timeout 3600s nice -n 10`. Vor dem Workspace-Test wurde `~/ws-slot.sh` erfolgreich abgewartet (mehr als 25 GB frei). Chromium: `/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`; Schlüsselbundtests mit `OPENSPIDER_SECRET_STORE=memory`.

Prüfumgebung korrigiert, ohne Repo-Änderungen: Der erste Node-Lauf hatte 24/25 Erfolge, da `zip` fehlte. Das Distributionspaket wurde nur unter `/tmp/openspider-int-c-tools/` entpackt und für den Wiederholungslauf dem PATH vorangestellt. Dieser Lauf bestand 25/25 Tests. actionlint 1.7.7 kannte die beiden Runner-Labels `macos-15-intel` und `windows-11-arm` nicht; mit dem bereits vorhandenen actionlint 1.7.12 sind beide Workflows ohne Zusatzkonfiguration grün.

Der bekannte Lastflake `os-integrations/tests/api_framework.rs:617`, `sliding_window_and_retry_limit_are_observed_by_mock`, bestand im vollständigen Workspace-Lauf direkt. Keine Einzelwiederholung nötig; kein Ignore, keine Maskierung und keine Teständerung.

Smoke: `cargo build -p os-cli -p os-testsite -j 6 --locked` bestand in 35,19 s. Anschließend `python3 scripts/smoke/installed.py --install-dir target/debug --testsite target/debug/os-testsite --chromium "$OPENSPIDER_CHROMIUM" --output /tmp/openspider-int-c-validation/linux-smoke --dry-run` mit den oben genannten Ressourcenoptionen. Drei zusätzliche Text-Crawls mit je 18 URLs: 0,455 / 0,464 / 0,536 s, Median 38,78 URLs/s (einschließlich Prozessstart, kein Referenzhardware-Benchmark). `result.json` bestätigt `dry_run=true`, CLI `openspider 0.1.0 (f3a8ba4f95, linux-x86_64)` und Chrome for Testing 156.0.8078.4. Evidence bleibt außerhalb des Repositories im genannten Ausgabeordner.

## Übergabe

`target/` am Ende gelöscht; keine zusätzlichen Produktänderungen. Der Bericht ist der einzige neue Pfad dieser Prüfrunde; Merge-Commits bleiben unverändert. Kein Push.

## Offene native Nachweise

Push und native CI führt der Orchestrator aus. Insbesondere trägt er die echte R03-Smoke-Lauf-ID nach: **ausstehend**. Linux-Trockenläufe ersetzen weder Installation noch native OS-Aufgabenplanung oder den plattformübergreifenden Bytevergleich. Es wurden keine Workflows remote gestartet, keine Releases erzeugt und nichts gepusht.
