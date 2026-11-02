# M3 Integrationsrunde g – M2-Stand bis int-26

## Stand und Umfang

02.11.2026, Branch `m3/int-g`. Basis `origin/m3/int-f` = `7f49938f`, integrierter
M2-Stand `origin/main` = `250d3b4a` (M2 bis int-26 einschließlich FIX-LIVE, N27, N28,
N30, N31, cargo-deny-Ausnahme RUSTSEC-2026-0429 und Spec-Festlegung 07 „Zeitbudgets im
Release-Build auf Referenzhardware“). Vor dem Merge: M3 191 Commits vor, 272 hinter main.

Merge-Basis `a0f78a5b` (`m2/int-21`, 31.10.) – derselbe M2-Stand, den int-e integriert hat.
Neu aus main sind damit int-22 bis int-26 (u. a. FIX-E-Port, FIX-STOP, FIX-TX, FIX-WAL,
N15, N19–N28, N30, N31, U31, FIX-ABOUT, FIX-LIVE, ci-e2e-limit).

Merge-Commit `d3b3927e`: `m3: Merge origin/main (M2 bis int-26, Spec-Festlegung 07) in m3/int-f`
(`git merge --no-ff origin/main`, Parents `7f49938f`, `250d3b4a`). Kein Rebase, kein
`cargo update`, kein Tag, keine Veröffentlichung.

## Konfliktlösung

16 Dateien mit Textkonflikt. Ausgangslage: FIX-E (CRAWL-126 Pause mit Hintergrund-Merge)
liegt auf M3 als Originalcommits (`m3/fix-e`, int-f) und auf main als angepasster Port
(int-23, `77a22061`), danach auf main weiterentwickelt (FIX-STOP, FIX-TX, FIX-WAL,
Analyse-Checkpoint CRAWL-141, N24 ff.). Prüfung je Datei: Hat M3 die Datei außerhalb von
FIX-E geändert (`a0f78a5b..m3/int-e`)?

- Bei 14 Dateien nein: alle M3-Änderungen stammen aus FIX-E, das main als Obermenge
  enthält. Gegenprobe: in allen 14 Dateien entfernt main gegenüber int-f nur Zeilen, die
  der Port oder spätere M2-Pakete bewusst umgebaut haben (z. B. `merge.rs` von FIX-STOP,
  `writer_thread.rs`/`worker.rs` von N22/N24), keine FIX-E-Funktion fehlt.
  Insgesamt haben beide Seiten 60 Dateien geändert; 48 davon hat M3 nur über FIX-E
  berührt, und alle 48 sind nach dem Merge bytegleich mit main (14 per Konfliktauflösung,
  34 automatisch gemergt).
- Bei 2 Dateien ja (`quit.rs`, `open.rs`): main-Fassung als Basis, M3-Funktion darauf.

| Datei | Wahl | Grund |
|---|---|---|
| `apps/desktop/src/features/detail/frame/DetailFrame.tsx` | main | M3 nur FIX-E; main zusätzlich `RetainedPanel`-Memo und Anpassung im Render (UI-145) |
| `apps/desktop/src/features/detail/frame/detail.test.tsx` | main | M3 nur FIX-E; main = FIX-E-Tests plus weitere |
| `apps/desktop/src/features/detail/frame/request.ts` | main | M3 nur FIX-E; main Tab-Klick per Microtask (UI-145) |
| `apps/desktop/src/features/sidebar/frame/sidebar.test.tsx` | main | M3 nur FIX-E; main = Obermenge |
| `apps/desktop/src/features/statusbar/Statusbar.tsx` | main | M3 nur FIX-E; main zusätzlich „Gestoppt – Datei wird zusammengeführt“ (FIX-STOP) und gefilterte Snapshot-Abos |
| `apps/desktop/src/features/statusbar/Statusbar.test.tsx` | main | M3 nur FIX-E; main = Obermenge |
| `apps/desktop/src/features/statusbar/i18n/de.ftl`, `en.ftl` | main | M3 nur FIX-E; main zusätzlich `status-stopped-merging` |
| `crates/os-app/src/session/merge.rs` (add/add) | main | M3 nur FIX-E; main-Port mit FIX-STOP-Erweiterung (laufender Merge macht weitere Runde) |
| `crates/os-app/tests/pause_merge.rs` (add/add) | main | M3 nur FIX-E; main = FIX-E-Tests plus FIX-STOP/FIX-WAL-Tests |
| `crates/os-engine/src/worker.rs` | main | M3 nur FIX-E; main Port plus spätere Schreibpfad-Änderungen |
| `crates/os-engine/src/writer_thread.rs` | main | M3 nur FIX-E; main Port plus Umbau Batch/Speicherzähler |
| `crates/os-store/src/writer.rs` | main | M3 nur FIX-E; main macht `checkpoint_within` öffentlich (CRAWL-141) |
| `docs/spec/01-crawl.md` | main | M3 nur FIX-E-Festlegung; main enthält sie wortgleich plus „Stoppen während der Rückführung“, erweitertes Beenden-Kriterium und CRAWL-141 Punkt 7. Nichts ergänzt. |
| `crates/os-app/src/quit.rs` | main + M3 | main ersetzt FIX-Es `merge::wait_all` in `request_quit` und `confirm` durch `close_cache` + `drain` (eigene Schreibaufträge zuerst, dann WAL, FIX-WAL). Der automatische Merge hätte beide Wartewege behalten; übernommen ist nur `drain`. Darauf die M3-U02-Zeile `result.is_ok() && crate::update::install::on_quit(app).await`: Update-Installation erst nach Drain und WAL-Rückführung; verweigert `on_quit` das Beenden, öffnet der `else`-Zweig von main den View-Cache wieder. |
| `crates/os-store/src/open.rs` | main + M3 | Einziger Konflikt in `merge_wal`: main öffnet mit `Connection::open_with_flags`, M3 mit `open_sqlite` (Windows-Langpfad-VFS, M3). Übernommen `open_sqlite`; M3s übrige `open_sqlite`-Stellen und `checkpoint_and_truncate` sind automatisch gemergt. |

Gegenprobe für die übrigen 12 von beiden Seiten geänderten Dateien: Bei allen entspricht
der Unterschied Merge ↔ main genau den M3-Änderungen außerhalb von FIX-E (`a0f78a5b..m3/int-e`),
z. B. `pool.rs`, `session/control.rs`, `session/feed.rs`, `os-fetch/src/client.rs`, `dto/mod.rs`, `commands.json`,
`registry/index.rs`, `manifest_contract.rs`, `help.test.tsx`, `release-macos.yml`.
Einzige Abweichung ist die oben beschriebene `open_sqlite`-Zeile in `open.rs`.

### Besonders geprüfte Bereiche (automatisch gemergt)

- **`deny.toml`:** nur main geändert; RUSTSEC-2026-0429 enthalten, M3-Stand sonst gleich.
- **`Cargo.lock`/`Cargo.toml`:** main hat seit der Basis keine Abhängigkeit geändert;
  der Lockfile ist der M3-Stand von int-f. Tokio `1.53.3`. `zip 8.6.0` (über calamine,
  Ausnahmen 0367/0368/0369/0429) und `zip 9.0.4` (M3) wie in int-f.
- **Workflows:** M3 hat seit der Basis nur `release-macos.yml` geändert (plus eigene
  `release*.yml`, `smoke-installed.yml`, `package-manager-smoke.yml`,
  `publish-packages.yml`, alle erhalten). main: `ci.yml`, `spec-coverage-m2.yml`,
  `ui-e2e.yml` (macos-webkit 90 min, übrige 60 min), neues `ui-nightly.yml`, in
  `release-macos.yml` der Schritt „App-Ticket heften (auch im DMG)“. Dieser Schritt
  sitzt im Merge zwischen M3s Tauri-Build (`--config macos-build.json`, Updater-Signatur)
  und M3s Schritt „DMG signieren, notarisieren, heften“; `BUNDLE_DIR` und
  `steps.check` existieren im M3-Job.
- **`docs/plan/*`:** keine Datei von beiden Seiten geändert; M2-Pläne/-Berichte
  (main) und M3-Pläne/-Berichte getrennt erhalten.
- **`docs/spec/*`:** außer `01-crawl.md` (oben) nur von main geändert
  (`04-custom-integrations.md`, `05-ui.md`, `07-performance.md`), übernommen.
- **Overview/Sidebar:** `Overview.tsx` (FIX-LIVE-Memoisierung), N27-Zählung und
  `SidebarFrame.tsx` hat M3 nicht geändert; Merge = main.

## Prüfungen

Linux-Build-Server (x86_64, 16 Kerne), eigener Worktree mit eigenem `target/`, Stand
`d3b3927e` per `git bundle` übertragen. `CARGO_INCREMENTAL=0`, Debugprofile
`line-tables-only`, `OPENSPIDER_SECRET_STORE=memory`; Workspace-Test über `~/ws-slot.sh`.

| Prüfung | Ergebnis |
|---|---|
| `npm ci` | grün |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | grün, Desktop-Crate eingeschlossen (1 min 29 s) |
| `cargo test --workspace --no-fail-fast --locked` | grün: 4.179 bestanden, 0 fehlgeschlagen, 50 ignoriert, 587 Testziele inkl. Doc-Tests (24 min) |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | grün: 105 Dateien, 899 Tests |
| `python3 -I docs/plan/M2-check.py` | OK: 150 Pakete, 590 Spec-IDs; kein M3-Prüfskript vorhanden |
| `bash scripts/check-neutral.sh` | grün |
| `cargo deny check licenses bans sources` | grün |

Erstlauf grün, keine Reparatur nötig. `cargo deny check advisories` lässt sich auf dem
Server nicht ausführen (lokales cargo-deny kann die CVSS-4.0-Einträge der aktuellen
Advisory-Datenbank nicht lesen, Umgebungsproblem); geprüft wird es im CI-Lauf.

CI und ui-e2e werden nach dem Push von `m3/int-g` per `workflow_dispatch` gestartet
(ci.yml mit `intel=false`; ui-e2e reagiert nicht auf Pushes nach `m3/**`). Ergebnisse
folgen im Nachtrag.

## Offene Punkte

- **Windows-Langpfade außerhalb von os-store:** main führt mit M2-N21 in
  `crates/os-analyze/src/views/membership.rs` (`data_version`) eine weitere direkte
  `Connection::open_with_flags` ein. M3 hat bisher nur os-store auf `open_sqlite`
  umgestellt (`write_queue.rs`, `compare/change/auto.rs` öffnen ebenfalls direkt);
  `data_version` liefert `Option` und fällt bei einem Fehler weich zurück. Keine Änderung
  in dieser Runde; Frage an den Besitzer des Langpfad-Pakets, ob os-analyze/os-app
  nachziehen sollen.
- **macOS-Heften und Updater-Archiv:** main heftet das Ticket nach dem Tauri-Build an App
  und DMG-Inhalt. Das von Tauri signierte `*.app.tar.gz` (M3-Updater) entsteht vorher und
  enthält die App ohne geheftetes Ticket (Gatekeeper prüft online, Signatur des Archivs
  bleibt gültig). Kein Merge-Fehler; Prüfung im nächsten nativen Release-Lauf.
- Aus int-f weiterhin offen: Testrennen `header.spec.ts:69`, `n04-live.spec.ts:74`
  (auf main mit FIX-LIVE behandelt, Beobachtung in ui-e2e dieser Runde) und CLI-Crawls
  ohne Hintergrund-Merge (`-wal` nach Pause/Ende bei fremdem Leser).
- Aus main übernommene Folgepakete (M2-N32, M2-N33 calamine/zip 9, M2-N34) gelten
  unverändert; nach M2-N33 entfallen die vier zip-Ausnahmen in `deny.toml` auch für M3.

## Fremde Dateien (K2-92)

Geändert wurden nur die Konfliktdateien (Auflösung wie oben) und dieser Bericht; keine
Produkt-, Test- oder Golden-Änderung über die Konfliktauflösung hinaus, keine
Erwartungsänderung (K2-63).

| Datei | Besitzer | Eingriff |
|---|---|---|
| `crates/os-app/src/quit.rs` | M2-FIX-WAL (main), M3-U02 | main-Fassung plus eine U02-Bedingung `on_quit` |
| `crates/os-store/src/open.rs` | M2-FIX-E-Port (main), M3 Windows-Langpfad | `merge_wal` öffnet über `open_sqlite` |
| 14 FIX-E-Dateien (Tabelle oben) | M3-FIX-E / M2-FIX-E-Port und Nachfolger | main-Fassung übernommen, ohne eigene Änderung |
| `docs/plan/M3-reports/int-g.md` | int-g | neu |
