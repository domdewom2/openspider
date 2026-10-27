# M2-N02 – Auflagenliste (K2-74, Planer, 27.10.2026)

Quellen: Fable-Gate Welle 3a (Runde 10: K09, T02, U13B, D22, D23) mit den Auflagen 1–7 und Fable-Gate Welle 3b (Runde 11 sowie C02: U23, I06, E06, K17, L01, C02) mit den Befunden „→ N02“, prompt_notes von M2-N01b/M2-N02, K2-68-Nachträge, Berichte n01.md, n01b.md, n02a.md (offene Punkte).
Geprüft wurde am Code von `origin/m2/int-10` (Runde 10, auf main) und an den Paketbranches `origin/m2/<k>` der Runde 11. Zeilenangaben beziehen sich auf diese Stände.

Zuschnitt: Zielpaket **M2-N02** (Rust/Kern und C9-Auflösung, Welle 4) und **M2-N02b** (UI, Welle 4) bekommen nur Auflagen an Pakete, die in Runde 10/11 gemergt sind. Auflagen, die den Merge von C02 voraussetzen, gehen an **M2-L02** (Welle 4, hängt an C02). Auflagen an C02 selbst gehen als Nachtrag in dessen prompt_notes. Niedrige Punkte ohne fachliche Wirkung kommen in `M2-backlog.md` (M3).

## 1 Zielpaket M2-N02 (13 Auflagen)

| Nr. | Herkunft | Datei(en) | Korrektur | Spec-ID | Status am Code |
|---|---|---|---|---|---|
| N02-1 | C9 Welle 2 (M2.md 2a) | `apps/desktop/e2e/crawls/compare.pending.spec.ts` (C01), `e2e/detail-fach/core-integration.spec.ts` (K09), `e2e/mcp/bridge.spec.ts` (5× K17), `e2e/tasks/bridge.spec.ts` (T01, X03) | Platzhalter-fixme entfernen, Testinhalt unverändert; rote Tests → Befund | je Test | offen (int-10 und m2/k17: fixme vorhanden; U23 entfernt das in `compare.pending`) |
| N02-2 | Gate W3a, Auflage 4 (D22) | `crates/os-render/src/snippets/runtime.js` und Rust-Seite in `snippets/**`, Fixture `tests/fixtures/integ-custom-js/` | `saveText`/`saveUrls` in das freigegebene Ausgabeverzeichnis, Pfad-Traversal ablehnen. Bisher wirft der Code „File access is not enabled“, das Kriterium ist nur trivial erfüllt | INTEG-072 | offen (Entscheidung K2-74: N02 statt M3; ohne die von R02/U17 gesperrten os-render-Dateien, sonst M3) |
| N02-3 | Gate W3b, I06-1 | `crates/os-integrations/src/crawl_glue/mod.rs:29`, `os-engine/src/crawl.rs:1101` (möglichst unverändert lassen) | Quota-Ledger `api-quotas.json` im Settings-Verzeichnis statt Sidecar je Crawl | INTEG-006 | offen (m2/i06: `path.with_extension("api-quotas.json")`) |
| N02-4 | Gate W3b, I06-3 | `crates/os-export/src/reports/orphan_pages.rs` | Orphan-Report ohne GA4/GSC: Verhalten bzw. Hinweis an OUT-099 abgleichen | OUT-099 | offen |
| N02-5 | Gate W3b, I06-2 | Moduldoc `crawl_glue` | Vorrang der Umgebung vor dem Schlüsselbund (`EnvironmentSecrets`) dokumentieren | INTEG-002 | offen |
| N02-6 | Gate W3b, E06-1 | `tests/fixtures/crawl-m2-list/export_tests.rs` → `crates/os-export/tests/list_m2_e06.rs` | Testcode aus der Fixture (über `#[path]` eingebunden) in die Testdatei verschieben | OUT-085 | offen (m2/e06: `#[path = "../../../tests/fixtures/…"]`) |
| N02-7 | Gate W3b, E06-2 | `crates/os-engine/src/serp_input.rs:108–125` | SERP-Seeding über Writer/WriteBatch statt Roh-SQL; url_counters und Overview gefüllt | CRAWL-031, OUT-098 | offen |
| N02-8 | Gate W3b, K17-2 | `crates/os-app/src/mcp/clients.rs:14`, `crates/os-app/tests/cmd_mcp_service.rs:103–111` | `.join(".codex").join("config.toml")` statt Literal mit Pfadtrenner | AGENT-001 (Spec-ID des vorhandenen Tests) | offen (m2/k17: `home.join(".codex/config.toml")`) |
| N02-9 | Gate W3a, Auflage 7 | – (nur Testlauf) | Stichprobe `cmd_mail_lifecycle`, `cmd_detail_ext_service`, `tab_mobile`, `tab_custom_js` auf main nach Runde 11 | K2-62 (6) | offen |
| N02-10 | Gate W3a, Auflage 3 | `crates/os-app/tests/manifest_contract.rs` | Konsistenz `compare.contentAvailable` und neuer Commands aus Runde 11 mit M2.md 3.6a | TECH-300 | Eintrag vorhanden (int-10, Zeile 114); 3.6a-Zeile trägt K2-74 nach; nur bestätigen |
| N02-11 | Gate W3a, Auflage 6 | `crates/os-fetch/src/net/mail_proxy_platform.rs`, `crates/os-analyze/src/mail_completion.rs` | Besitz für Folgewellen (keine Änderung ohne Befund) | OUT-280 | erledigt durch Besitzeintrag |
| N02-12 | K2-64-Rest (n01b.md 4.) | `render_chromium` (os-render, os-engine), `registry_performance`, `core_robots`, `gsc_inspection`, `tab_indexability`, `links_detail` | Laufzeit senken, Assertions unverändert; Ziel „Rust tests“ ≤ 15 min | A1/K-81 | offen (Windows ~24 min) |
| N02-13 | Gate W3b, L01-2 (Manifest) | `crates/os-cli/Cargo.toml`, `Cargo.lock` | `sys-locale` (MIT/Apache-2.0) als gebündelter Schnittstellenbedarf für L02 | OUT-198 | offen |

## 2 Zielpaket M2-N02b (1 Auflage)

| Nr. | Herkunft | Datei(en) | Korrektur | Spec-ID | Status am Code |
|---|---|---|---|---|---|
| N02b-1 | Gate W3b, U23-3 | `apps/desktop/src/features/compare/ComparisonTable.tsx`, `ComparisonLinks.tsx` | Vergleichsansicht auf DataGrid mit Spaltenkonfiguration statt schlichter HTML-Tabellen | CRAWL-144, OUT-140, TECH-301 | offen (m2/u23) |

## 3 Nachtrag M2-L02 (Welle 4, nach dem C02-Merge; 7 Auflagen)

| Nr. | Herkunft | Datei(en) | Korrektur | Spec-ID |
|---|---|---|---|---|
| L02-1 | Gate W3b, C02-3 | `crates/os-cli/src/plan.rs:351–359` | `--project-crawl-comparison` ohne Wert bzw. `TRUE` = true | OUT-202 |
| L02-2 | Gate W3b, K17-1 | `crates/os-mcp/src/tools/compare_crawls.rs:63–118`, `crates/os-mcp/tests/k17_tools.rs:112–114` | Change Detection über `compare::change::counts/ids`, Ignore „wartet auf M2-C02“ entfernen | AGENT-015 |
| L02-3 | Gate W3b, U23-1 | `crates/os-app/src/commands/compare/{ui,ui_change}.rs`, `crates/os-store/src/queries/app_support.rs:349` | Doppelimplementierung entfernen, UI auf C02-Funktionen | TECH-301 |
| L02-4 | Gate W3a, Auflage 2 (Teil 2) | `crates/os-app/tests/cmd_mail_lifecycle.rs`, `tests/fixtures/out-m2-mail/compare-counts.json` | auf die öffentliche Change-Detection-Schnittstelle umstellen, Golden ablösen | OUT-281, TECH-301 |
| L02-5 | Gate W3b, L01-1 | `crates/os-cli/tests/auth_profiles.rs:270`, `crates/os-cli/src/auth.rs:174` | Flake: Jar-Assertion nur mit Cookies ≥ 60 s Restlaufzeit | OUT-223 |
| L02-6 | Gate W3b, L01-2 | `crates/os-cli/src/column_headers.rs` | Systemsprache über `sys-locale`, Labels aus den UI-Übersetzungen | OUT-198 |
| L02-7 | Gate W3b, L01-4 | `crates/os-cli/src/registry.rs` | `Internal:Fonts/Media/XML` im CLI-Exportkatalog klären | ONPAGE-023 |

## 4 Nachtrag M2-C02 (prompt_notes, Fixrunde; 3 Auflagen)

| Nr. | Herkunft | Datei(en) | Korrektur | Spec-ID |
|---|---|---|---|---|
| C02-a | Gate W3a, Auflage 2 (Teil 1) | `crates/os-analyze/src/compare/change/**` (`Reader::new`) | Zählersemantik der Links-Elemente (Selbstlinks) nach TECH-301 entscheiden | TECH-300, TECH-301 |
| C02-b | Gate W3b, C02-2 | `crates/os-analyze/src/tabs/change_detection.rs:6–23` | Register-Tab mit echten Daten | TECH-301 |
| C02-c | Gate W3b, C02-4 | `crates/os-cli/src/exec.rs:181, 437–439`, `run_task.rs:45–46`, .ftl | `#[path]` nach os-cli, Exporte bei Vergleichsfehler nicht unterdrücken, i18n-Schlüssel ergänzen | OUT-202 |

Der IPC-Vertrag `elements: []` (Gate W3b, C02-1) gehört bereits zur laufenden Fixrunde von C02.

## 5 Planer, erledigt mit K2-74

| Herkunft | Erledigung |
|---|---|
| Gate W3a, Auflage 3 | `compare.contentAvailable` in M2.md 3.6a eingetragen |
| Gate W3a, Auflage 4 (Entscheidung) | INTEG-072 → N02 (N02-2), Eintrag in `M2-backlog.md` |
| Gate W3a, Auflage 6 | Besitz: `os-parse/src/mobile.rs` → D23, `os-store/src/queries/detail_ext.rs` → K09, Mail-Dateien → N02 (N02-11), `os-cli/src/mail.rs` → L02 (über `crates/os-cli/**`) |
| Gate W3b, E06-3 | Spec-Nachtrag CRAWL-030 (Sprunglimit auch für Canonical-Ketten) als Vorschlag an den Orchestrator, nicht in `docs/spec` eingetragen |

## 6 Ohne Paket (Prozess, Beobachtung, M3)

| Herkunft | Punkt | Verbleib |
|---|---|---|
| Gate W3a, Auflage 1 und W3b allgemein | CI-Matrix je Branch vor dem Merge | Integrator (Prozess) |
| Gate W3a, Auflage 5 | E2E `e2e/mcp/dialog.spec.ts` (AGENT-002, „Escape“) lokal rot | nur falls in ui-e2e reproduzierbar; liegt in N02-Besitz (`e2e/mcp/**`) |
| Gate W3b, U23-2, I06-4, E06-4, C02-5 | Merge-Konflikte (compare.rs, sidebar.rs, view.rs, os-cli args/plan/exec/run_task, crawl.rs, list_input.rs) | Integrator |
| Gate W3b, I06-2 | `os-app/src/session/control.rs:34` Credentials-Wrapper für alle Crawls | Hinweis, kein Auftrag |
| Gate W3b, K17-3 | `os-app/src/mcp/session.rs:52` MCP-`start_crawl` bei jeder laufenden App-Operation abgelehnt | beobachten (Q04) |
| Gate W3b, L01-3 | `os-export/src/csv.rs:27–55` thread_local-Header-Scope; NTLM-Client je Origin cachen (`os-fetch/src/client.rs:640–655`); MD4/HMAC-MD5 handgeschrieben | M3 (`M2-backlog.md`); keine fachliche Wirkung, Konfliktgefahr mit D21/U17 |
| n01b.md 3./6. | Lighthouse-/KI-Zeilen im API-Tab (`ApiKind`); `commands/viz.rs` `exclude_self_links = false` als UI-Standard | `M2-backlog.md` (I-/M4 bzw. Fable-Klärung) |
| n02a.md offene Punkte 2/3 | Zustandstext „Gestoppt“ statt „Completed“; WebKit-Wettlauf A01-10 (1 von ~30 Läufen) | `M2-backlog.md` bzw. Beobachtung in U32 |
| n01.md C16, n01b.md C03 | K2-68-Ignores | erledigt (C16 entfernt durch E06, C03 auf int-10 nicht mehr ignoriert) |
| n02a.md offener Punkt 1 | Resources-Spalten und -Typfilter in `view.rs` | erledigt durch N01b (UI-036) |
