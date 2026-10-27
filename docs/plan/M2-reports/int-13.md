# Integrationsrunde 13

Basis `origin/m2/int-12` 2046085a; Pakete D21 a5ca564d, R02 73b062db, U17 2ee44349. Runde 12 läuft parallel in der CI; ein späterer Nachmerge obliegt dem Orchestrator.

## Konfliktauflösungen

- `crates/os-analyze/tests/filters_registry.rs`: Stub-Zähler für D21 und R02 jeweils dekrementiert; Filter und dynamische Spaltenquellen beider Seiten erhalten.
- `crates/os-analyze/tests/framework_registry.rs`: Custom JavaScript und JavaScript gleichermaßen als renderingabhängig behandeln.
- `crates/os-app/src/commands/view.rs`: vorhandene Kontextschlüssel und A11y-Knotenschlüssel kombiniert.
- `crates/os-app/tests/manifest_contract.rs`: abgelöste D21- und U17-Besitzer entfernt; beide Katalogergänzungen erhalten, leere Besitzerprüfung entfernt.
- `crates/os-engine/src/render_glue.rs`, `worker.rs`: Snippet- und Accessibility-Optionen bzw. Verarbeitung additiv erhalten.
- `crates/os-render/src/api.rs`, `page.rs`: Snippet- und Accessibility-Felder samt Defaults und Ergebnis kombiniert.
- `crates/os-render/src/intercept.rs`: D22-Snippet-Sperre vor R02-Ablehnungsgrund; beide Verhaltensweisen erhalten.
- `crates/os-engine/src/lib.rs`, `lifecycle.rs`: SERP-Modul und Sitzungsschutz sowie Wiederherstellung beider Zweige erhalten; Initialisierung nach Resume.
- `crates/os-store/src/queries/mod.rs`: Detail-Abfragen und Sitzungsschutz-Abfrage erhalten.
- `tests/golden/out-cli/help_export_tabs.txt`, `list_exports.txt`: Konflikt zunächst mit Integrationsstand aufgelöst, anschließend mit `UPDATE_GOLDEN=1` über die CLI-Registry-Ausgaben neu erzeugt und auf Vereinigung geprüft. Der CLI-Golden-Test wertet die Variable selbst nicht aus. Gegenüber Runde 12 nur 93 bzw. 95 Ergänzungen; keine fehlenden Namen aus den Paket-Deltas.

Automatische Merges geprüft: `worker.rs` enthält Original-nofollow direkt nach noindex, Bildressourcen nach Bild-Merge und `observe_redirect`; `RenderOpts.login_storage` und `ResourcePolicy::blocked_reason` bestehen nebeneinander; Screenshot-Route und Accessibility-Export bleiben erhalten. Tauri-Bundle enthält die drei axe-Ressourcen. Tokio bleibt 1.53.3.

## Offene Prozessauflagen

CI-Matrix einschließlich Intel, Linux-Core und UI-E2E auf dem Integrationsstand sowie Gegenprüfung der zwei E2E-Befunde stehen aus; ohne GitHub-Schreibrechte kann dieser lokale Branch nicht in CI gestartet werden. Lauf-IDs und Ergebnisse sind anschließend in `u17.md` bzw. `r02.md` nachzutragen. R02-Laufnachweise stehen in `r02.md`: Coverage abgebrochen, die drei anderen Läufe erfolgreich. R02-Nachtrag zu Fremdänderungen obliegt dem Orchestrator. D21: Coverage-Scanner liest unter `crates` und `tests` ausschließlich `.rs`-Dateien (`coverage/m2.rs`, `scan.rs`); die `.patch`-Nachweise sind damit ausgeschlossen. vollständiger CI-Coverage-Nachweis offen; Folgeauflagen an N02 und weitere Pakete gemäß Fable-Berichten bleiben bestehen.

## Lokale Prüfungen

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`: grün auf dem Endstand.
- Tests der sieben angeforderten Crates: `os-render`, `os-engine`, `os-analyze`, `os-app`, `os-cli`, `os-export`, `os-model` jeweils einzeln grün. Der gemeinsame Lauf wurde beim Linken wegen nur noch 4,8 GiB freiem Speicher abgebrochen; Test-Binärdateien wurden zwischen den Einzelläufen entfernt. Der erste App-Lauf scheiterte an einem älteren Accessibility-Fixture; nach Anpassung an D21 (registrierte Regel, gespeichertes Ergebnis, gültige Sortierspalte) gezielt und im vollständigen App-Lauf grün. Der Linker meldete bei App-Tests eine zu große Unwind-Tabelle.
- `python3 -I docs/plan/M2-check.py`: grün. `npm ci`: erfolgreich, Node 22.20.0 unter der von einigen Paketen verlangten Version 22.22.2; npm meldet acht Audit-Befunde. Desktop-Typecheck und Vitest: grün (76 Dateien, 771 Tests). `check:static` inklusive `gen:ipc --check` und npm-Lizenzen: grün.
- `OPENSPIDER_CHROMIUM` nicht gesetzt; Chromium-Tests und E2E-A `e2e/auth` nicht als Browsernachweis prüfbar. CI-Matrix bleibt offen.
- `os-parity coverage --milestone M2 --merged D21,R02,U17` mit den lokalen Rust- und Vitest-Ergebnissen: Exit 1, zwei blockierende IDs (`OUT-232`, `OUT-272`) und sieben ungültige Ignore-Gründe in `e2e/mcp/bridge.spec.ts` und `e2e/tasks/bridge.spec.ts`; vollständige Workspace-/Playwright-Nachweise fehlen. TECH-250…257 und TECH-259, OUT-062, OUT-109 und UI-064 [P1] bei D21 stehen auf `implemented`. TECH-258 ist P2 und nicht im M2-Katalog. Der Scanner wertet als erfolgreich zurückkehrende Chromium-Tests ohne Browser ebenfalls als grün; dies ist kein echter Chromium-Nachweis.
