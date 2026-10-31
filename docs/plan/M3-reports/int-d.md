# M3 – Integrationsrunde d

Stand: 31.10.2026 · Branch `m3/int-d` · Basis `m3/int-c` (`14b643d9`).

## Merges

In der beauftragten Reihenfolge, jeweils konfliktfrei mit `git merge --no-ff`:

1. `a8d80d20` – `m3: Merge b01 (Fable freigegeben 88)`, Paketstand `b218152f` (Nachprüfung einschließlich NTLM-Pool).
2. `881a161e` – `m3: Merge p01 (Fable freigegeben mit Auflagen 82)`, Paketstand `48bb9b11` (Auflagen 1/2/6 in p01fix erfüllt).

Beide Merge-Commits tragen `Co-Authored-By: OpenAI Codex <noreply@openai.com>`.
Der erste Parent des B01-Merges ist die Basis, der erste Parent des P01-Merges der B01-Merge.
Keine Konfliktauflösung oder zusätzliche Produkt-, Test- oder Golden-Änderung durch den Integrator; keine Erwartungsänderung nach K2-63.
Der Lockfile-Diff gegen die Basis ergänzt ausschließlich `hmac` bei den direkten Abhängigkeiten von `os-fetch`; vorhandene Version wiederverwendet. Tokio bleibt `1.53.3`, kein `cargo update`.

## Linux-Prüfung

Alle geforderten Linux-Prüfungen sind grün. Prüfprotokolle außerhalb des Repositories: `/tmp/openspider-int-d-validation/`.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün, Exit 0 |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets -j 6 --locked -- -D warnings` | grün, Exit 0, keine Warnungen; 1 min 40 s |
| `cargo test --workspace --exclude openspider-desktop -j 6 --locked` | grün, Exit 0; 3.978 bestanden, 0 fehlgeschlagen, 43 regulär ignoriert, 543 Test-/Doc-Test-Gruppen; Kompilierung 3 min 53 s, summierte Testzeit 1.201,20 s |
| `npm run -w apps/desktop typecheck` | grün, Exit 0 |
| `npm run -w apps/desktop test` | grün, Exit 0; CSS-Prüfung grün, 91/91 Dateien und 858/858 Tests, 108,32 s |
| `node --test tests/release/*.test.mjs tests/fixtures/migrations/node/*.test.mjs packaging/homebrew/*.test.mjs scripts/release/licenses/generate.test.mjs` | grün mit temporären Werkzeugen, 30/30 bestanden, 0 Skips; erster Lauf wegen fehlendem `zip` 29/30, siehe unten |
| `python3 packaging/winget/test_validate.py` | grün, 3/3 Tests |
| actionlint 1.7.12 für alle geänderten Workflows: `package-manager-smoke.yml`, `publish-packages.yml` | grün, Exit 0, ohne Zusatzkonfiguration |
| `node website/check-site.mjs` | grün, 18 Seiten; Startseiten 76.358/76.116 Bytes |
| `bash scripts/check-neutral.sh` | grün, keine Fundstellen |
| Desktop `check:static` (zusätzlich) | grün, 29/29 Tests, 27 IPC-Wrapper aktuell, 476/476 npm-Lizenzen zulässig |
| `ruby packaging/homebrew/lifecycle.test.rb` (zusätzlich, temporäres Ruby) | grün, 3/3 Prüfungen: Upgrade, Bootstrap-Wiederholung, unabhängige Registrierungen |
| `cargo deny --locked check licenses bans sources` (zusätzlich) | grün, alle drei Kategorien `ok`; 65 Hinweise zu mehrfach vorhandenen Crate-Versionen, keine Versionsänderung im Lockfile |
| `git diff --check` | grün |

Keine Lastflakes und keine Einzelwiederholung im Rust-Lauf. Der bekannte Test `os-integrations/tests/api_framework.rs::sliding_window_and_retry_limit_are_observed_by_mock` bestand direkt. Die B01-Prüfungen `ntlm_client_cache` (3/3, 12,12 s) und `csv_header_scope` (2/2) bestanden innerhalb des vollständigen Workspace-Laufs. Keine neuen Ignores oder Maskierungen.

Umgebung: Linux x86_64, Rust `1.99.1`, Node `v22.23.4`; Cargo mit `-j 6 --locked`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=line-tables-only` und `CARGO_PROFILE_TEST_DEBUG=line-tables-only`. Schwere Befehle mit `timeout 3600s nice -n 10`; vor jedem Workspace-Lauf `bash ~/ws-slot.sh` erfolgreich abgewartet. `OPENSPIDER_SECRET_STORE=memory`, Chromium über `OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`. Bei den Plattenplatz-Stichproben mindestens 135 GB frei; `target/` vor Abschluss rund 50 GB.

Der erste Typecheck konnte mangels installierter npm-Abhängigkeiten nicht starten (`tsc: not found`, Exit 127). `npm ci --offline` installierte aus dem vorhandenen Cache erfolgreich 421 Pakete ohne Lockfile-Änderung; anschließender Typecheck und vollständige Frontend-Prüfung grün.

Der erste Node-Lauf bestand 29/30 Tests; `tests/release/orchestration.test.mjs:166` (CLI-Archive mit L01-Hinweisen) scheiterte mit `zip: command not found` (Exit 127 des Archivschritts). Ruby war ebenfalls nicht im Standard-PATH, vom Cask-Test als Diagnose gemeldet. Keine Tests geändert oder übersprungen. Wiederholung mit bereits vorhandenen temporären Werkzeugen aus `/tmp/openspider-int-c-tools/extracted/usr/bin` und `/tmp/p01-ruby` (Ruby-Bibliotheken über `LD_LIBRARY_PATH`/`RUBYLIB`): 30/30 bestanden, 0 Skips, einschließlich Ruby-Syntaxprüfung des generierten Casks. Keine Systeminstallation, keine Repo-Änderung für Werkzeuge.

## Übergabe und offene native Nachweise

`target/` nach den Prüfungen gelöscht. Nur dieser Integrationsbericht wird zusätzlich zu den Paket-Merges committet. Kein Push; keine entfernten Workflows oder Veröffentlichungen gestartet.

Der Orchestrator übernimmt den Branch, pusht ihn und ergänzt:

- Native CI: ausstehend, Lauf-ID und Ergebnis nachtragen. Linux-Clippy und Rust-Tests schließen `openspider-desktop` aus; der rust-ini-Pin aus M2-N13 ist in M3 noch nicht enthalten.
- Nativer Workflow `package-manager-smoke`: ausstehend, Lauf-ID und Ergebnis nachtragen. Lokale Generator-, Schema- und Lifecycle-Prüfungen ersetzen keine native Homebrew-/winget-Installation.
