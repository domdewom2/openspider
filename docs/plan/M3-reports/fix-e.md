# M3-FIX-E – CI-Befunde der Integrationsrunde e

Stand: 31.10.2026. Branch `m3/fix-e`, Basis `origin/m3/int-e` (`a2b7bd61`).
Linux-Server; keine Veröffentlichung, kein Push. Native macOS-Bestätigung beim Orchestrator.
**Status: PAKET FERTIG (Linux).** Die geforderten Linux-Nachweise sind vollständig;
zusätzlich besteht der unveränderte N04-Test jetzt auch isoliert mit zwei Workern
20/20 ohne Retry. Die historische Überschreitung bei gleichzeitigem Workspace-Lauf
bleibt unten dokumentiert. Native macOS-Bestätigung wie beauftragt beim Orchestrator.

## Befund 1: MCP-Crawl und kleine Thread-Stacks

Die als Fremddaten gelesene Datei `~/ci-logs/int-e/inte-arm.log` gehört zu Lauf
**41190697264**. Das Binary startet erst um **10:36:35.311850 UTC**, meldet drei
grüne Tests und endet um **10:36:35.709775 UTC** mit SIGBUS. Die rund neun Minuten
liegen **vor** diesem Binary im Workspace-Lauf; kein neunminütiger MCP-Hänger.

Die drei nicht als bestanden gemeldeten Tests:

- `http_crawl_uses_app_session_and_target_policy`
- `persistent_permissions_and_markdown_share_the_tool_list`
- `server_lifecycle_token_rotation_and_autostart`

Linux-Einzelprüfung des unveränderten Binaries: alle drei bei 512 KiB und 2 MiB
grün; auch das ganze Binary mit `ulimit -s 512`, `RUST_MIN_STACK=524288` und
`--test-threads=1` grün. Bei 256 KiB bricht ausschließlich der Crawl-Test mit
`thread tokio-rt-worker has overflowed its stack` / SIGABRT ab. Die beiden anderen
Tests bleiben sogar bei 64 KiB grün. Stackbedarf und Signalsymptom unterscheiden
sich zwischen Linux x86_64 und macOS arm64; der native SIGBUS ist damit noch nicht
als auf macOS selbst vermessen ausgegeben.

Ein lokal unter `/tmp/fix-e-debugger` entpackter GDB zeigt zuerst die Kette
`Access::launch → Handle::block_on(start_crawl) → start_spider → CrawlFile::create
→ stamp_config → CrawlConfig::from_toml_str → AdvancedConfig::visit_map`.
Nach Entlastung der Start-Future wird der zweite Engpass sichtbar:
`worker::run_item → robots.check → ProtectedFetcher → HttpFetcher → reqwest/hyper
→ TCP connect`. Die GDB-Frame-Adressen zeigen unter anderem rund **119.488 Bytes**
im `run_item`-Pollingpfad. Das sind kumulierte Frames großer, ineinander
enthaltener Async-Zustandsmaschinen, keine unendliche Rekursion.

Die Produktkorrektur setzt gezielte Heap-Grenzen mit `Box::pin`: Spider-Start,
Worker-Phasen `run_item`, Robots-Prüfung, Analyse und Zusammenbau des Schreibbatches
sowie geschützter HTTP-Abruf und dessen Exchange-Timeout. Thread-Stackgrößen im
Produkt und Abhängigkeiten bleiben unverändert. Ein explorativer Versuch, die
TOML-Deserialisierung umzustellen, wurde verworfen; Modell und TOML-Verhalten sind
unverändert. Die erste teilweise Korrektur reichte unter wechselnder
Polling-Reihenfolge noch nicht aus; nur die vollständige Fassung zählt im Gate.

Der bestehende MCP-Fixturetest verwendet jetzt einen expliziten Tokio-Runtime
mit **320 KiB** für Worker- und Blocking-Threads. Damit hängt der Regressionstest
nicht allein von `RUST_MIN_STACK` ab. Mit unverändertem Produktcode und diesem
Test: **SIGABRT/Stacküberlauf** (`regression-before-cmd_mcp_service.log`). Keine
Testabschaltung, keine größer gesetzten Produktstacks.

Prüfung der Alternativhypothesen: keine eigene `mmap`-/`memmap`-Verwendung und keine
relevanten großen Stack-Arrays in den untersuchten MCP-/App-/Engine-/Store-Pfaden.
SQLite nutzt WAL/Shared Memory; das ist nicht mit einer eigenen memmap-Datei
gleichzusetzen. `checkpoint_and_truncate` gehört zum Update-Installationspfad,
der hier nicht aufgerufen wird; AppConfig::new deaktiviert den automatischen
Update-Start. Repositoryweite Suche nach `stack_size`, `thread_stack_size`,
`mmap`/`memmap`, großen Array-Literalen und Truncate-Pfaden wurde auch auf Export,
Fetch und die Desktop-Hülle ausgedehnt. Die bestehende CLI startet für ihren
Task-/Vergleichspfad mit 8 MiB Stack; Store-Checkpoint-/Prefetch-Threads setzen
keine eigene Größe. Die zwei 16-KiB-Puffer des TLS-Proxy-Relays laufen im
Loopback-HTTP-Fixturepfad nicht. Keine Änderung dieser Thread-/Pufferpfade.
Der beobachtete Linux-Abbruch liegt im Stackschutz und nicht in SQLite. M3-Update/Release und M2-Q02b-Export werden im fehlschlagenden Crawl nicht
ausgeführt. Tokio bleibt laut unverändertem Lockfile **1.53.3**.

Zusätzliche Audit-Notiz auf der Workspace-Binary-Variante: Eine erste
Wiederholungsserie endete im dritten Durchlauf beim letzten Lifecycle-Test abrupt
(der Crawl-Test war bereits grün). Das erste Diagnoseskript speicherte dabei
keine genaue Signalnummer; der Abbruch wird **nicht** als Erfolg oder als
nachgewiesener Stacküberlauf ausgegeben. Danach: neue Serie beider Binaries je
20/20 grün; außerdem **200/200** MCP-Läufe mit einzeln gespeichertem Exitstatus,
Dateigröße und mtime vor/nach jedem Lauf sowie **100/100** normale Exits unter GDB,
mit `prlimit --stack=524288`, `RUST_MIN_STACK=524288` und Signal-Stopp bei
SIGSEGV/SIGBUS. Kein Core entstand, kein Signal wurde reproduziert. Protokolle:
`cache-repeat-cmd_mcp_service.log` (einmaliger Abbruch), `cache-repeat2-*`,
`mcp-signal-audit.{log,jsonl}`, `mcp-debugger-audit.log`. Dieser unklassifizierte
Einzelfall bleibt ein Hinweis für die native Bestätigung, keine verschwiegene
grüne Erstserie.

## Befund 2: live Link-Details

CI-Lauf **41190723578**, Log `~/ci-logs/int-e/inte-lwk.log`; zusätzlich Original-
Artefakt **13039449175** über das GitHub-Plugin gelesen und unter
`/tmp/m3-fix-e-logs/ci-webkit` ausgewertet. Die Aufrufspur enthält echte Zeilen:
Retry-Inlinks: `view.open` 123 ms, `view.rows` 648 ms / 13 Zeilen; erster Versuch:
Outlinks-Zeilen 1.406 ms / 31 Zeilen. Gleichzeitig laufen Live-View-Updates.
Die Daten fehlen nicht im Writer oder Detail-Export. Der unveränderte Test ist
lokal einzeln grün (1,6 Minuten), unter vier gleichzeitigen Wiederholungen jedoch
**3/4 rot**, einschließlich identischer Inlinks-Platzhalter.

`view.open` und `view.update` zählten bisher die Linkauswahl und lasen eine erste
Zeile; erst `view.rows` materialisierte die vollständige Link-ID-Liste nochmals.
Bei laufenden Crawls kann der eingehende Linkindex noch zurückgestellt sein.
Dann entstehen drei Scans der wachsenden Linktabelle vor den ersten sichtbaren
Zeilen, zusätzlich zu Metadaten-/Zeilen-Rundreisen. Auch die Gesamtzahl konnte
aus einer anderen Auswahlgeneration stammen als die später gelesenen IDs.

`detail_page` materialisiert nun die bereits vorhandene `LinkView` bei
Open/Update genau einmal. Die Gesamtzahl stammt aus dieser Liste; ein begrenztes erstes Fenster mit bis
zu 200 Zeilen und folgende Fenster werden nach diesen Primärschlüsseln gelesen.
Die bestehenden begrenzten Fenster, Reihenfolge, Filter, expliziten Versionen,
Retention und Aktualisierung bleiben erhalten. Es werden keine breiten Tabellen
vollständig im Speicher gehalten. Nicht materialisierte Detailarten behalten
ihren bisherigen Pfad.

Die Backend-Korrektur allein beseitigte die Vier-Worker-Stressfehler noch nicht.
Eine temporäre, danach bytegleich entfernte Browserinstrumentierung zeigte:
initiale Daten wurden korrekt übernommen, kamen aber erst nach Open, einem
zusätzlichen React-Commit und anschließendem Rows-IPC an. Beispiel: 707 ms Open
plus 457 ms Rows. Der sichtbare `TablePanel` lädt jetzt das erste begrenzte Fenster
unmittelbar nach Open, bevor er das Grid veröffentlicht. Damit entfällt der
zusätzliche Render-/Effect-Schritt unter Live-Tick-Last. Ein spät fertiggestellter
Handle eines inzwischen versteckten Tabs startet weiterhin keine Zeilenabfrage.
Alle Retention-/Hidden-/Revisionstests bleiben erhalten. Ein neuer Vitest prüft
bei gezielt verzögerten Rows, dass das Grid erst mit geladenen ersten Zeilen
veröffentlicht wird.

Ein weiterer Chromium-Befund nach sechs grünen Wiederholungen zeigte bei nur
rund 409 ms Open+Rows weiterhin eine verspätete Grid-Veröffentlichung. Der
Detailrahmen abonnierte das gesamte, je Fortschrittstick neu erzeugte
`CrawlStatus`-Objekt, obwohl er daraus nur Crawl-ID und Crawl-Modus verwendete.
Jeder Tick baute damit auch die behaltenen Detailpanels erneut auf. Der Rahmen
abonniert jetzt genau diese beiden stabilen Werte. Die datenabhängigen
Revisionen der Tabellen bleiben unabhängig davon aktiv. Ein Regressionstest
sendet zehn echte Store-Fortschrittsupdates: unveränderter Rahmen rendert das
Panel jedes Mal erneut (13 statt 3 Aufrufe im roten Vorherlauf), korrigierter Rahmen bleibt stabil;
eine neue Crawl-ID aktualisiert weiterhin Panel und Konfiguration.

Die danach nochmals ausgeführte WebKit-Reihe zeigte nach 15 grünen Durchläufen
einen verbleibenden seriellen Leseweg: 485 ms Open und 591 ms erste Rows. Deshalb
bereitet Open/Update jetzt das erste **200-Zeilen-Fenster** vollständig vor
(gleiche Blockgröße wie die vorhandene Grid-Datenquelle). `view.rows` überträgt
dieses Fenster bei passenden Spalten/Offsets aus dem gespeicherten Handle,
ohne weiteren Datenbankzugriff. Versions-/Owner-Prüfung findet weiterhin vorher
statt; andere Spalten oder spätere Fenster behalten den normalen Pfad. Nach
dem ersten passenden Abruf wird die Backend-Kopie freigegeben. Die Zeilenhaltung bleibt begrenzt. Ein weiterer Lastlauf zeigte jedoch, dass
auch ein datenbankfreier zweiter IPC-Aufruf das Budget überschreiten kann.


Die abschließende IPC-Korrektur ergänzt `ViewHandle` additiv um das optionale
`initialRows`-Fenster (Rust: `initial_rows`). Open überträgt die vorbereiteten
Linkzeilen sofort und gibt seine Cachekopie frei. Die Datenquelle prüft View-ID,
Version, Offset, Spalten und Blockgrenze und übernimmt gültige Zeilen direkt in
Block 0. Alte Handles ohne Feld funktionieren unverändert; spätere Fenster und
Update verwenden die vorhandenen Pfade. Compare/Stub initialisieren nur `None`;
der generierte TypeScript-Typ wurde aus Rust regeneriert. `commands.json` bleibt
bytegleich. Zwei Datenquellentests prüfen direkten Cachezugriff ohne zweiten IPC,
spätere Fenster/Versionen und das Zurückweisen falscher Generationen.

Eine anschließende Instrumentierung maß zwischen Klick und Datenanzeige
1.185 ms: Timer 165 ms, Open 455 ms, erste Rows 394 ms, übriger Render-/Transportweg.
Mit gebündeltem Open blieb in einer WebKit-Wiederholung ein langsamer Open-Aufruf
von 659 ms plus Auswahl-Debounce. Deshalb starten explizite Tabklicks für die
bestehende Auswahl ohne zusätzlichen 100-ms-Auswahl-Debounce; echte URL-Wechsel
behalten ihn. Der Request startet im Layout-Effect vor der nächsten Paint-
Verschiebung. Die Dringlichkeit gehört nicht zu den Lifecycle-Abhängigkeiten:
behaltene Tabellen werden beim Tabwechsel nicht entsorgt. Ein Fake-Timer-Test
prüft unmittelbares Tab-Open, unveränderte 100 ms für neue URLs und Handle-Retention.

Der verstärkte Regressionstest blockiert abwechselnd `detail-link-ids` und
`detail-window` nach Open/Update. Die vorherige Fassung scheitert nun
deterministisch am zweiten Leser (`initial-page-before.log`, Exit 101); die
vollständige erste Seite muss ohne beide Leser übertragbar sein.

Regressionen in `beta_detail.rs`:

- Nach Open/Update wird wahlweise der feste `detail-link-ids`- oder
  `detail-window`-Leser blockiert. Erste Inlinks-
  und Outlinks-Zeilen müssen dennoch lesbar sein. Alter Produktstand: Timeout,
  neuer Stand: ohne weiteren Materialisierungsscan.
- Ein real eingefügter Link zwischen Open und erstem Zeilenabruf verändert die
  geöffnete Auswahl nicht; erst Update liefert die zusätzliche Zeile samt passender
  Gesamtzahl. Die bisherigen Live-/Offset-Generationstests bleiben erhalten.

Abgleich mit `origin/m2/int-21`: `commands/view.rs`, `views/mod.rs`, `TablePanel.tsx`
und `ViewDataSource.ts` sind auf der Paketbasis identisch. Es wurde also kein
M2-Detailpfad durch einen M3-B01-Exportpfad ersetzt. Der M3-Unterschied im Feed ist
die Absicherung bestätigter Zustandswechsel (`refresh_state`), nicht eine andere
Detailquelle. Ein fehlerhafter Konfliktentscheid in den Detaildateien ist nicht
belegt. Behoben wird der durch die Integrationsprüfung sichtbar gewordene
vorhandene Mehrfachscan; die grüne int-21-Matrix widerlegt dessen Lastabhängigkeit
nicht. Auch `os-engine/src` ist zwischen int-21 und int-e unverändert. Die Store-Diffs
betreffen den Windows-Long-Path-Wrapper (auf Linux/macOS weiterhin derselbe
`Connection::open_with_flags`-Aufruf) und den hier nicht aufgerufenen Update-
Checkpoint. Eine abweichende Writer-/Exportoption als Ursache ist dadurch nicht
belegt. N18-Reset, Pause/Resume und Writer-/Exportoptionen werden nicht umgebaut.
Der originale N04-Test, sein 1000-ms-Budget und seine Erwartungen bleiben unverändert.

## Besitz, K2-92 und K2-63

K2-92 erlaubt die erforderlichen minimalen Produktänderungen dieses Fixpakets.
Keine fremden laufenden Paketstände wurden überschrieben.

| Datei | Minimaler Eingriff / Spec |
| --- | --- |
| `crates/os-engine/src/crawl.rs` | Spider-Start-Future auf Heap; AGENT-020, CRAWL-001 |
| `crates/os-engine/src/worker.rs` | Heap-Grenzen der großen URL-Phasen; AGENT-020, CRAWL-001 |
| `crates/os-fetch/src/client.rs` | Heap-Grenzen des geschützten Abrufs/Timeouts; AGENT-020, CRAWL-095 |
| `crates/os-app/src/commands/view.rs` | Eine materialisierte Auswahl für Count und Fenster bei Open/Update; UI-018/-032/-033/-145 |
| `crates/os-app/tests/cmd_mcp_service.rs` | Reale Fixture bei explizit kleinem Runtime-Stack; AGENT-020 |
| `crates/os-app/tests/beta_detail.rs` | Leserblockade und echte Insertion als Regression; UI-018/-032/-033/-145 |
| `apps/desktop/src/features/detail/frame/TablePanel.tsx` | Erstes begrenztes Fenster vor Grid-Mount, Hidden-Tab bleibt lazy; UI-145 |
| `apps/desktop/src/features/detail/frame/detail.test.tsx` | Verzögerte Rows, stabile Fortschrittsticks, sofortige Tabwechsel bei erhaltenem Auswahl-Debounce; UI-018/-030/-145 |
| `apps/desktop/src/features/detail/frame/DetailFrame.tsx` | Nur Crawl-ID/Modus abonnieren, explizite Tab-Absicht übergeben; UI-018/-030/-145 |
| `apps/desktop/src/features/detail/frame/request.ts` | Tabwechsel ohne Auswahldebounce, frühes Scheduling, Retention erhalten; UI-030/-145 |
| `crates/os-app/src/dto/view.rs` | Optionales begrenztes Initialfenster; UI-018/-145 |
| `crates/os-app/src/commands/compare.rs`, `crates/os-app/src/stub/mod.rs` | Neue optionale Handle-Eigenschaft nur mit `None` initialisieren; UI-018 |
| `apps/desktop/src/ipc/gen/ViewHandle.ts` | Mechanisch regenerierter optionaler Typ; UI-018 |
| `apps/desktop/src/data/ViewDataSource.ts` | Validiertes Initialfenster in bestehenden Blockcache übernehmen; UI-018/-145 |
| `apps/desktop/src/data/ViewDataSource.test.ts` | Initialfenster, Legacy-Handle, weitere Blöcke/Versionen und falsche Generation; UI-018/-145 |

Minimale Produkt-Diffauszüge (Spec-Zuordnung jeweils in der Tabelle):

```diff
# crates/os-engine/src/crawl.rs
- start_spider(fetcher, req, deps, start).await
+ Box::pin(start_spider(fetcher, req, deps, start)).await
# crates/os-engine/src/worker.rs
- run_item(&sh, &item).await
+ Box::pin(run_item(&sh, &item)).await
- sh.gate.check(&*sh.fetcher, &url).await
+ Box::pin(sh.gate.check(&*sh.fetcher, &url)).await
- analyze(sh, item, &url, &result).await
+ Box::pin(analyze(sh, item, &url, &result)).await
- assemble(sh, item, analysis).await?
+ Box::pin(assemble(sh, item, analysis)).await?
# crates/os-fetch/src/client.rs
- pinned.fetch_impl(req).await
+ Box::pin(pinned.fetch_impl(req)).await
- tokio::time::timeout(budget, exchange).await
+ tokio::time::timeout(budget, Box::pin(exchange)).await
# crates/os-app/src/commands/view.rs – Open und Update
- page(..., 0, 1, None).await?
+ detail_page(..., links.as_ref()).await?
# detail_page nutzt die vorhandene ID-Auswahl für erste PK-Seite und Count:
+ let ids = links.ids(app, selection, req).await?;
+ // page_query mit ordered=true und ids.iter().take(INITIAL_LINK_ROWS)
+ p.total = ids.len() as u64;
+ // OpenView hält die erste begrenzte Seite; initial_rows überträgt sie einmal.
+ initial.take();
# apps/desktop/src/features/detail/frame/TablePanel.tsx
+ source.active = currentlyVisible.current && !signal.aborted;
+ await source.ensureRange(0, 29);
  return { handle, source };
# apps/desktop/src/features/detail/frame/DetailFrame.tsx
- const crawl = useSession(s => s.crawl);
+ const crawlId = useSession(s => s.crawl?.crawlId);
+ const crawlMode = useSession(s => s.crawl?.mode);
+ <DetailRequestUrgent.Provider value={urgentSelection === selectionToken}>
# apps/desktop/src/features/detail/frame/request.ts
- useEffect(() => {
+ useLayoutEffect(() => {
- }, SELECTION_DEBOUNCE_MS);
+ }, urgentRef.current ? 0 : SELECTION_DEBOUNCE_MS);
# crates/os-app/src/dto/view.rs
+ #[serde(default, skip_serializing_if = "Option::is_none")]
+ #[ts(optional = nullable)]
+ pub initial_rows: Option<RowWindow>,
# crates/os-app/src/commands/view.rs – Open
+ h.initial_rows = initial_rows(&opened, &id, h.version, 0, INITIAL_LINK_ROWS, &[]);
# crates/os-app/src/commands/compare.rs und src/stub/mod.rs
+ initial_rows: None,
# apps/desktop/src/ipc/gen/ViewHandle.ts – generiert
+ initialRows?: RowWindow | null,
# apps/desktop/src/data/ViewDataSource.ts – nach Identitäts-/Grenzprüfung
+ this.blocks.set(0, { keys: initial.rowKeys, cells: initial.cells });
```

Vollständige Diffs: `04e552e8` (Stack), `208d482b` (Link-Generation),
`682663a9` (initiales Detailfenster), `2a04c504` (stabiler Detailrahmen),
`d861cb54` (Vorbereitung der ersten Seite), `ff16c5cb` (optionales Initialfenster),
`b3b1ecd1` (Tabwechsel ohne erneuten Auswahl-Debounce).
Keine Manifest-/Lockfile-, Golden-, Screenshot- oder Erwartungsfortschreibung; **K2-63 wird nicht beansprucht**.
Clean-Room/Neutralität eingehalten, keine neuen externen Testzugriffe.

## Linux-Prüfung

Endgültiger Produktstand: `b3b1ecd1`; vollständige Commitfolge zusätzlich
`ff16c5cb` (gebündeltes Initialfenster) und `b3b1ecd1` (Tab-/Auswahl-Scheduling).
Alle Produktcommits tragen den verlangten Co-Author-Trailer.

| Prüfung | Ergebnis / Protokoll unter `/tmp/m3-fix-e-logs` |
| --- | --- |
| `cargo fmt --all --check` | Grün; `fmt-final.log` |
| Clippy Workspace/all-targets, `-D warnings`, kein Ausschluss | Grün; `warm-clippy.log`, Rust-Stand danach unverändert |
| `cargo test --workspace --no-fail-fast` einschließlich Desktop und Doc-Tests | Grün: 4.089 bestanden, 0 Fehler, 49 bestehende Ignores, 564 Ziele; `workspace-warm-final.log` |
| Desktop-Typecheck und vollständiger Vitest-Lauf | Grün; 869 Tests / 92 Dateien, `desktop-complete-final.log` |
| Statische Frontendverträge | 29/29 grün; `contracts-final.log` |
| npm-Lizenzen | 476 Pakete positiv; `contracts-final.log` |
| `cargo deny --locked check licenses bans sources` | Grün; `deny.log`, danach unveränderter Dependency-Stand |
| Neutralität | Grün; `contracts-final.log` |
| MCP-Binary, alle 6 Tests, 20 Wiederholungen | 20/20 grün; `rust-findings-final20.log` |
| Detail-Binary, alle 5 Tests, 20 Wiederholungen | 20/20 grün; `rust-findings-final20.log` |
| Sechs gezielte Frontendregressionen, 20 Wiederholungen | 20/20 grün; `frontend-regressions20.log` |
| Beta + A11y WebKit / Chromium, je 2 Worker | Beide je 34/34 grün: WebKit 7,9 min, Chromium 6,3 min; `final-*-beta-a11y.log` |
| N04 WebKit / Chromium, 20 Wiederholungen | Chromium 20/20 grün (16,5 min, 2 Worker); WebKit 20/20 grün (30,7 min, 1 Worker), beide ohne Retry; `browser-final-pipeline.log`, `final-webkit-serial20.log`, `webkit-serial-orchestration.log` |
| Zusätzliche isolierte N04-WebKit-Doppel-Crawl-Reihe | **20/20 grün**, 2 Worker, kein Retry, 16,1 min; `resume-clean-webkit20.log`, `.jsonl`, `resume-clean-webkit20-summary.json` |

Die finalen Rust-Wiederholungen setzen Hauptthread-Stacklimit und
`RUST_MIN_STACK` auf 512 KiB, `--test-threads=1`; der MCP-Crawl setzt intern
320 KiB. Binary-SHA256, Größe und mtime sind protokolliert, Exitstatus und
unveränderte Metadaten werden für jeden Durchlauf geprüft. Davor bestanden zwei
vollständige Workspace-Läufe mit je 4.088 Tests und 49 bestehenden Ignores;
der endgültige Lauf enthält zusätzlich den Legacy-DTO-Test (Summe der Ziel-Testlaufzeiten 1.409,12 s). Zwischenstände der
Browserkorrektur hatten bereits je 20/20 Chromium beziehungsweise WebKit bestanden;
diese gelten ausdrücklich nicht als Nachweise des endgültigen Produktstands.

Lastdiagnostik: Vier gleichzeitige N04-Crawls sowie einzelne Doppel-Crawl-
Serien überschritten das unveränderte 1000-ms-Budget. Die finale Doppel-Crawl-
WebKit-Serie während Workspace-/Frontend-Prüfung endete nach zwei Erfolgen mit
einem Inlinks-Timeout (ein weiterer Test unterbrochen, 16 nicht ausgeführt).
Die finale Aufrufspur zeigt `view.open` für Inlinks mit **1.152 ms** und bereits
**14 vollständig gebündelten Initialzeilen**. Hier überschreitet schon der
Open-Roundtrip allein das 1000-ms-Budget; es fehlen keine Zeilen und es wird kein
zweiter initialer Rows-IPC mehr benötigt. Beleg: `tab-final-failed-commands.json`.
Sie ist **rot**, kein 20/20-Nachweis. Die vollständigen Browser-Suites wurden
für beide Engines ausgeführt; die gesonderte serielle WebKit-Reihe prüfte das
Einzel-Fixture-Verhalten ohne zwei gleichzeitige 5000-URL-Crawls. Budgets,
Erwartungen, Goldens und der originale N04-Test bleiben unverändert.

In zwei verschachtelten Diagnoseblöcken wurde `nice` versehentlich zweimal
angewendet (effektiv 19 statt 10). Korrigiert: Schlussläufe setzen Nice 10 genau
einmal; gestartete Playwright-/Bridge-Prozesse wurden mit `ps` geprüft. Die davon
betroffenen roten Diagnoseläufe gelten nicht als reguläre Schlussläufe.
Ein explorativer Aufruf mit gemeinsamem Vite-Server wurde mit Exit 130 beendet;
die Schlussläufe nutzen den vorhandenen Header-Konfigurationswrapper auf eigenem
ephemerem Port. Ein weiterer gestarteter Lauf wurde für den abschließenden
Auswahl-Token-Fix beendet, nicht als Testfehler gezählt. Eigene verwaiste
Bridge-/Fixture-Prozesse aus abgebrochenen Läufen wurden gezielt beendet.

Schwere Befehle: `timeout 3600s nice -n 10`; Rust `-j 8 --locked`;
`~/ws-slot.sh` vor Workspace-Läufen. Chromium-Pfad gemäß Auftrag,
`OPENSPIDER_SECRET_STORE=memory`. Testsite, CLI und Parity wurden vor den
Browserläufen gebaut, ebenso `os-app-bridge --features e2e-bridge`; abschließender
Werkzeugbuild grün (`tools-complete-final.log`). Kein `cargo update`,
Lockfile/Tokio 1.53.3 unverändert. Logs liegen außerhalb des Repositorys. Nach Ende aller Prüfungen wurden das
eigene `target/` und `apps/desktop/test-results/` gelöscht; vor dem Löschen waren
keine eigenen Target-Binaries mehr aktiv. Abschließende Neutralitätsprüfung
inklusive Bericht grün (`neutral-report-final.log`). Kein Push.

## Native macOS-Bestätigung

Der Orchestrator prüft auf macOS arm64 zunächst alle sechs Tests von
`cargo test --locked -j 8 -p os-app --test cmd_mcp_service -- --test-threads=1`, danach
20 Wiederholungen des Crawl-Tests sowie das Binary mit normaler Parallelität.
Der Crawl-Test setzt seinen Runtime-Stack selbst auf 320 KiB. Zusätzlich den
Workspace-Lauf mit den normalen CI-Profilen ausführen. Bei erneutem SIGBUS:
Crash-Report/LLDB-Backtrace inklusive abstürzendem Thread und Stackgrenzen sichern,
um den nativen Befund exakt mit den Linux-Ketten abzugleichen.

Auf dem Mac mini den unveränderten `beta/n04-live.spec.ts` mit WebKit wiederholen
und anschließend das native Gate ausführen. Linux-WebKit ersetzt keine Prüfung
von WKWebView. Signierung, Installation und Veröffentlichung bleiben außerhalb
dieses lokalen Pakets.

## Fortsetzung: Diagnose und isolierter Doppel-Crawl-Nachweis

Der vorangehende Berichtsstand blieb wegen des oben genannten roten Doppel-Crawl-
Laufs während Workspace-/Frontend-Prüfung offen. Die Fortsetzung untersucht dessen
vermuteten Pool-/Open-Engpass, ohne Produkt- oder Testbudgets weiter zu ändern.
Alle sechs MCP-Tests bestanden erneut (`/tmp/m3-fix-e-mcp-baseline.log`).

Temporäre Messungen in `ReadPool` trennten Wartezeit einschließlich Checkout und
SQL-Laufzeit; eine weitere Messung in der Bridge trennte Task-Wartezeit,
Command-Verarbeitung und Antwortserialisierung. Messcode wurde anschließend
**bytegleich entfernt**; `git diff` auf den Produktdateien ist leer. Keine neuen
Produktänderungen nach `b3b1ecd1`, keine neue dauerhafte Instrumentierung.

Die erste isolierte Viererreihe mit zwei Workern bestand 4/4. Maxima über 234
Link-ID-Abfragen: 11,9 ms vor SQL und 175,9 ms SQL; über 233 erste Zeilenfenster:
14,9 ms vor SQL und 15,3 ms SQL. Eine zweite Diagnosereihe bestand ebenfalls
vier vollständige Tests und wurde dann gezielt für den uninstrumentierten
Schlusslauf beendet (Exit 130; **kein** 20/20-Nachweis). In ihren 39 Open-Antworten
lagen die Maxima bei 0,236 ms Task-Wartezeit, 103,6 ms Command-Verarbeitung und
3,83 ms Serialisierung. Die Detail-Lanes lagen höchstens bei 15,5 ms vor SQL,
184,3 ms Link-ID-SQL und 20,9 ms Fenster-SQL. Unterbrochene eigene Kindprozesse
wurden gezielt beendet. Belege: `resume-timing.log`, `resume-pool-timing-summary.json`,
`resume-bridge-timing-run.log`, `resume-bridge-timing-summary.json`.

Diese Diagnosen reproduzierten keinen langsamen initialen Open-Aufruf. Sie
belegen daher **keine nachträglich bewiesene Ursache** des historischen
1.152-ms-Ausreißers. Sie liefern auch keinen Grund, Poolgrößen, SQLite-Open oder
Thread-Stacks pauschal zu vergrößern. Längere Wartezeiten während der späteren
Abschlussanalyse sind nicht mit dem initialen Detail-Open gleichzusetzen.

Nach Entfernen des Messcodes und erneutem Bridge-Build wurde der originale
N04-Test vollständig **20-mal mit zwei WebKit-Workern und ohne Retry** ausgeführt.
Lauf vom 31.10.2026, 16:12:46 bis 16:28:52 UTC: **20/20 grün**, Exit 0.
Währenddessen kein Cargo-/Vitest-Lauf in diesem Worktree; die anderen Pakete auf
dem gemeinsamen Server liefen weiter. Jedes Ergebnis umfasst Live-HTML,
Inlinks/Outlinks/Retention, Pause/Resume, Analyseabschluss und Recent nach
Prozessneustart. Externer Reporter und Logs liegen ausschließlich unter `/tmp`.

```sh
OPENSPIDER_SECRET_STORE=memory U03_E2E_ALL=1 \
timeout 3600s nice -n 10 npm run -w apps/desktop e2e -- \
  --config=e2e/header/playwright.config.ts beta/n04-live.spec.ts \
  --project=webkit --workers=2 --repeat-each=20 --retries=0 --max-failures=1
```

Der Schlusslauf nutzte zusätzlich einen externen Reporter und ein externes
Ausgabeverzeichnis. Alle 20 vorhandenen `live-latencies-ms`-Anhänge sind im
JSONL-Protokoll gespeichert. Maxima: HTML 1.268 ms, erster Inlinks-Wechsel
1.016 ms, Outlinks 1.097 ms, Rückwechsel Inlinks 790 ms. Diese Werte enthalten
bereits den Playwright-Klick einschließlich dessen Wartezeiten; die unveränderte
1000-ms-Assertion beginnt **nach** Abschluss des Klicks und bestand in allen Fällen.
Das sind verschiedene Messintervalle; die Anhangswerte werden nicht als reine
1000-ms-Antwortlatenz ausgegeben.

Die vollständigen oben aufgeführten Linux-Gates gelten weiterhin für denselben
Produktstand; sie wurden in dieser Fortsetzung nicht als neu ausgeführt ausgegeben.
Zusätzlich wurden zum Abschluss Formatierung und Neutralität erneut geprüft.
Werkzeuge und Bridge wurden vor den neuen Browserläufen mit `--locked -j 8`
gebaut. Der Messcode wurde nicht committet. `target/` wurde nach Ende aller
Prüfungen erneut entfernt; kein Push.

## Abschluss und Grenzen

Der auf Linux nachgewiesene Stacküberlauf und der ursprüngliche Detail-Ladepfad
sind durch Produktkorrekturen und rote Vorher-/grüne Nachherregressionen abgedeckt.
Alle geforderten Linux-Prüfungen einschließlich der 20-fachen Befundtests sind grün;
der zusätzliche isolierte WebKit-Doppel-Crawl-Nachweis ist jetzt ebenfalls grün.
Damit ist das Linux-Fixpaket abgeschlossen.

Die frühere Überschreitung bei gleichzeitigem Workspace-Lauf bleibt als bekannte
Zusatzlastgrenze bestehen; der neue Lauf beweist keine obere Antwortzeit unter
beliebiger zusätzlicher CPU-/I/O-Last. Bei erneutem Auftreten die oben getrennt
vermessenen Pool-, SQL-, Bridge- und Browserintervalle am konkreten Ausreißer
sichern. Das ist keine Behauptung, der historische Ausreißer sei durch eine weitere
Produktänderung verschwunden.

Nächster verbindlicher Schritt: Der Orchestrator führt die oben beschriebene
macOS-arm64-Bestätigung einschließlich nativer Crashdiagnose bei erneutem SIGBUS
sowie die Mac-mini-/WKWebView-Prüfung aus. Linux ersetzt diesen nativen Nachweis nicht.

## Nachbesserung 2 (Claude)

Stand 31.10.2026, Übernahme von Codex (Kontingent erschöpft) auf `b1ed83de` plus
dessen unkommittierten Zwischenständen. Auslöser: CI `41220198430` (windows-x64,
`cmd_mcp_service`: `tokio-rt-worker` überläuft, `STATUS_STACK_OVERFLOW`) und
ui-e2e `41220204813` (linux-webkit, `overview.spec.ts:8` zweimal rot,
`header.spec.ts:23` flaky). Code lokal im Worktree, Bauen/Prüfen nur auf dem Server.

### Befund 1: Stacküberlauf unter Windows

**Ursache.** `04e552e8` ließ den MCP-Crawltest auf 320-KiB-Worker- und
Blocking-Stacks laufen (vorher 2 MiB). Die dort gesetzten `Box::pin(f(..)).await`
verkleinern zwar den Zustand, nicht aber den Pollrahmen: in unoptimierten Builds
entsteht die Future `f(..)` zuerst als Temporärwert im Rahmen des Aufrufers und
bleibt dort reserviert, solange die inneren Futures laufen. Gemessen (Linux x86_64,
Debug, GDB-Rahmenadressen beim Überlauf): `worker::run_item` allein **102.400 Byte**,
darüber reqwest/hyper rund 100 KB. Linux brauchte mehr als 256 KiB (3/3 Überlauf
bei 256 KiB, 3/3 grün bei 320 KiB); Windows mit eigener Rahmen- und Guard-Aufteilung
lag über 320 KiB. Weitere tiefe Ketten: MCP-Werkzeug → Konfiguration → Launch →
TLS-Wurzeln; Blocking-Task-Ergebnisse mit `CrawlConfig`, die durch jeden
Tokio-/Unwind-Rahmen kopiert wurden; die TOML-Deserialisierung von `CrawlConfig`
(Serde-Visitor 52 KB, gesamt rund 106 KB) am Ende dieser Ketten.

**Änderung (plattformneutral, keine Stackvergrößerung, keine neue Abhängigkeit).**

- `os-engine`: Hilfsfunktion `heap(|| f(..))` (nie inline) baut die Future in einem
  kurzlebigen Rahmen und legt sie auf den Heap; damit bleibt nur der Zeiger im
  Aufrufer. Angewendet auf die URL-Phasen (`run_item`, Zielprüfung, robots,
  Analyse, Zusammenbau, Senden, Sitemaps), die drei Startvarianten, die
  Inline-Awaits des Runners und den gespawnten Runner selbst.
- `os-fetch`: Der reqwest-Versand läuft als eigene Task (Abbruch beim Verwerfen der
  Future, also bei Timeout/Pause/Stopp wie bisher; Panics werden weitergereicht).
  Der tiefe Verbindungsaufbau beginnt damit an der Basis eines Runtime-Threads.
- `os-mcp`: `start_crawl` und `crawl_status` haben eigene Rahmen statt eines
  gemeinsamen 42-KB-Rahmens.
- `os-app`: MCP-Launch geteilt (Fetcher/TLS auf frischem Blocking-Thread, Engine-
  Start und Registrierung als Heap-Futures); `ReadPool` reicht Ergebnisse geboxt
  durch Unwind- und Blocking-Task-Rahmen.
- Codex-Zwischenstände geprüft: Heap-Grenzen für List/Serp übernommen (jetzt über
  `heap`), `Box::pin` um `block_on` durch den Split ersetzt, die drei
  Größentests übernommen und für die geänderten Pfade verschärft.

**Regressionstests (alle Plattformen, Debug).** Feste Obergrenzen der
Future-Größen: `run_item` ≤ 8 KiB (vorher 13.928 B, jetzt 3.856 B), `process`
≤ 1 KiB (176 B), Runner ≤ 16 KiB (vorher 20.728 B, jetzt 11.736 B), MCP-Launch und
Engine-Start je ≤ 16 KiB (7.024 B bzw. 7.120 B). Der bestehende 320-KiB-Test bleibt
unverändert. Neu `timed_out_request_releases_its_connection` (CRAWL-095): nach dem
Antwort-Timeout schließt die Versand-Task die Verbindung.

**Messung danach** (temporäre, nicht committete Sonde mit gleicher Fixture,
Stack-Hochwassermarke je Thread bei 1 MiB, fünf Läufe): höchstens **186.563 Byte**
(Startkette mit TOML-Deserialisierung), übrige Threads 114–177 KB;
vorher über 256 KiB. Die TOML-Deserialisierung (≈ 106 KB) ist jetzt der
größte Einzelposten; sie hängt am Modell und wurde nicht umgebaut. Native
Windows-Werte sind auf Linux nicht messbar; die Bestätigung bleibt beim CI.

### Befund 2: Overview in Linux-WebKit

**Ursache.** Im CI-Fehlerbild (Artefakt der linux-webkit-Jobs) ist der
Overview-Tabpanel leer, ohne „Loading…“: Die Abfrage hatte Daten, aber
`useDeferredValue(overview.data)` lieferte noch `undefined`. Unter laufenden
Fortschrittsticks wird das verzögerte Rendering der großen Übersicht immer wieder
von dringenden Updates unterbrochen. `b1ed83de` veröffentlicht die erste Antwort
sofort und verzögert nur spätere Aktualisierungen. Bestätigt unter Last
(WebKit, 6 Worker, kein Retry): Stand vor `b1ed83de` **11/12** (ein Lauf ohne
jede Overview-Mutation), mit `b1ed83de` **36/36** (12 + 24). Seriell sind beide
Stände lokal grün; der CI-Runner ist langsamer.

**Header-Flake (`header.spec.ts:23`).** Laut CI-Snapshot war der Crawl beim
zweiten Pausieren bereits „Completed“ (101/101 URLs, 0:08): Die Fixture ist auf
langsamen Runnern vor der zweiten Pause fertig. Das betrifft weder Pause noch
Fortsetzen aus diesem Paket; Test und Fixture bleiben unverändert (K2-63).
Lokal 20/20 (beide Header-Tests je 10× seriell, WebKit).

### Prüfung (Server, Stand `6a47d809` + Bericht)

| Prüfung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --all-targets -j 6 --locked -- -D warnings` | grün |
| `cargo test -p os-app --test cmd_mcp_service` | 6/6; zusätzlich Binary 10/10 normal und 10/10 mit `RUST_MIN_STACK=1048576` |
| `cargo test --workspace -j 6 --locked` (nach `ws-slot`) | grün: 4.094 bestanden, 0 Fehler, 49 bestehende Ignores, 566 Ziele |
| `npm run -w apps/desktop typecheck` / `test` | grün; Vitest 870/870 in 92 Dateien |
| Bridge-, Testsite-, CLI-, Parity-Builds | grün |
| `e2e/beta`, Chromium + WebKit, 2 Worker | 28/28 grün (2,9 min) |
| `overview.spec.ts` 10× seriell WebKit, ohne Retry | 10/10 |
| `header/header.spec.ts` 10× seriell WebKit, ohne Retry | 20/20 (zwei Tests) |
| `bash scripts/check-neutral.sh` | grün |

Kein Push, kein `cargo update`, Tokio 1.53.3 und Lockfile unverändert; die
Messsonde lag nur im Server-Worktree und ist gelöscht. `target/` und
Playwright-Ausgaben im Server-Worktree danach entfernt.

**Offen:** Windows-x64-Bestätigung von `cmd_mcp_service` und linux-webkit-ui-e2e
im CI des Orchestrators.

### Nachtrag: Pause-Latenz (CI 41239868087)

CI auf `2f4a4cab`: windows-x64 `cmd_mcp_service` grün. In ui-e2e scheiterte
`beta/control.spec.ts:6` jedoch auf windows-chromium und linux-webkit, jeweils auch im
Retry. Die Anhänge `pause-ack-ms` aus den Traces: Chromium/Windows 3.391 und 3.648 ms
(IPC 2.439 und 2.558 ms), WebKit/Linux 3.111 und 3.159 ms (IPC 1.549 und 1.622 ms).

**Abgleich mit `e4fb1fe1`.** Gleiche UI, nur die Bridge ausgetauscht; die Binärdateien
wurden abwechselnd gemessen (Server, kein Retry). Seriell, 10× je Engine: alt
Chromium Median 1.126 ms (IPC 410), WebKit 1.224 ms (IPC 636); Stand `2f4a4cab`
Chromium 1.146 ms (IPC 477), WebKit 1.236 ms (IPC 594). Unter Last (6 Worker,
3 Runden à 8, abwechselnd) gab es keinen systematischen Unterschied; die Mediane
überlappen. Ein Lauf von `2f4a4cab` lag bei 3.032 ms; alte Läufe erreichten bis zu
2.834 ms. Die Stack-Commits verursachen also keine messbare Regression; das
3-s-Budget war schon vorher auf langsamen Runnern knapp.

**Ursache des großen Anteils.** Eine temporäre Zeitmessung im Pause-Pfad (nicht
committet) zeigte: Der größte Block liegt zwischen Schleifenende und dem Ende des
laufenden Workers. Bei gesetztem `max_urls_per_sec` lässt der Host-Pacer die nächste
URL durch. Danach wartete der Worker in jedem Intervall rund 440 ms im Rate-Limiter
des Fetchers (der robots.txt-Abruf belegt dort einen Slot zusätzlich). Diese Wartezeit
kann eine Pause nicht abbrechen: Die Quittung wartete bis zum nächsten Slot, und
**danach begann der Abruf noch**, obwohl die Pause schon angefordert war (CRAWL-126).

**Änderung (`94db9704`).** `Fetch::start_delay()` ist eine additive Trait-Methode
(Vorgabe: null). `HttpFetcher` meldet darüber die offene Wartezeit seines Limiters,
ohne einen Slot zu reservieren; `ProtectedFetcher` und `PrimedFetch` reichen sie durch.
Der Worker wartet diese Zeit mit dem abbrechbaren Schlaf des Host-Pacers und stellt
die URL bei Pause oder Stopp zurück. Rate, Test und 3-s-Budget bleiben unverändert.
Neue Regression `pause_cancels_the_wait_for_the_shared_start_slot`
(CRAWL-126/-113): Vorher startet die zurückgestellte Seite 3 s nach der Pause (rot),
nachher startet nach der Pause kein Abruf (grün).

**Messung nachher** (seriell, 10× je Engine): Chromium Median 932 ms (IPC 301,
max. 1.680), WebKit 1.166 ms (IPC 410, max. 1.559). Unter Last, abwechselnd mit alt:
Chromium-IPC-Median 292/329 statt 810/1.001 ms. Der IPC-Wert von WebKit unter Last
blieb ähnlich (893–1.234 ms). Laut Backendmessung dauerte die Pause dort nur
150–550 ms; den Rest verbringt die Antwort im Browser bei sechs parallelen
WebKit-Instanzen, außerhalb des Pause-Pfads.

| Prüfung (Stand `94db9704`) | Ergebnis |
| --- | --- |
| fmt, clippy `-D warnings` | grün |
| `cargo test --workspace` | 4.095 bestanden, 0 Fehler, 49 Ignores |
| `e2e/beta` Chromium + WebKit, 2 Worker | 28/28 |
| `control.spec.ts` 10× seriell je Engine | 10/10 und 10/10 |

### Nachtrag 2: CI 41254618922 / 41254617243 auf `7cca5846`

**linux-webkit `control.spec.ts:6`.** Werte aus den Traces: Latenz 3.869 und 3.903 ms,
IPC 2.647 und 2.579 ms. Nach der IPC-Antwort vergehen also noch 0,7 bis 0,8 s
(Antwort annehmen und rendern), vorher 0,5 s künstliche Testverzögerung.

Das mitgeschnittene WebSocket-Protokoll zeigt: Die Pause-Antwort (+2.647 ms) kommt
unmittelbar nach den Lesekommandos, die bei der Pause liefen oder kurz danach
starteten. Das sind `catalog.filterCounts` (−102 → +2.176), `sidebar.overview`
(+514 → +2.604) und `export.catalog` (+513 → +2.646).

Reproduktion auf dem Server mit `taskset -c 0,1` (WebKit, seriell, Median 2,0–2,7 s,
vereinzelt über 3 s). Temporäre, nicht committete Messung mit Wanduhr-Abgleich von
Bridge und Browser:

- Backend-Pause 0,36–1,16 s; davon `wal_checkpoint(TRUNCATE)` 0,34–1,14 s.
- Zustellung im WebSocket/Browser zusätzlich 0,4–0,5 s.
- UI nach der Antwort 0,2–1,5 s.

TRUNCATE wartet im Busy-Handler (45 bis 216 Aufrufe) und endet in 5/5 Läufen genau
mit dem letzten überlappenden `ReadPool::run_snapshot`-Leser. Das sind Übersicht,
Katalog und Export; jeder dieser Leser braucht auf 2 Kernen 0,6–1,1 s.

Gegenproben (2 Kerne, abwechselnd), alle ohne messbaren Unterschied:

- Bridge `e4fb1fe1` gegen aktuell: alt ebenfalls ein Lauf mit 3.116 ms.
- Seitenleiste vor gegen nach `b1ed83de`: der alte Stand war eher langsamer.
- SQLite mit `opt-level 3`.
- UI-Aktualisierungen während „Pausing …“ zurückhalten.

Das ist also keine Regression dieses Pakets: Die Pause-Quittung enthält den
abschließenden TRUNCATE-Checkpoint (CRAWL-126 „Pause + Checkpoint“, portable Datei mit
zusammengeführtem WAL). Dieser Checkpoint muss laufende Snapshot-Leser abwarten.

Probe ohne Commit: TRUNCATE-Wartezeit auf ≈ 100 ms begrenzt. Der IPC-Median sank
damit auf 0,44–0,80 s, die Datei bliebe aber bis zum Ende der Leser unzusammengeführt.
Mögliche Wege, die eine Entscheidung brauchen:

- (a) Begrenztes TRUNCATE. Dauerhaft ist die Pause trotzdem, weil das WAL synchronisiert
  ist; ein nachgelagerter TRUNCATE nach der Quittung macht die Datei wieder portabel.
- (b) Laufende Snapshot-Leser bei Pause unterbrechen. Die UI müsste „Cancelled“ still
  verwerfen; `QueryFeedback` zeigt es heute als Alarm.
- (c) Übersicht, Katalog und Export als Leser verbilligen.

Keine Produktänderung ohne Entscheidung.

**linux-chromium `table/bridge.spec.ts:7`.** Beide Versuche hängen am Klick auf „Recrawl“.
Im CI ist kein Menü offen; lokal ist der Menüpunkt deaktiviert. Abwechselnd je 15× auf dem
Server: Bridge `e4fb1fe1` 1/15 rot, aktuell 2/15 rot, beide mit demselben Bild. Der
Test setzt keine URL-Rate, daher ist `start_delay` null und der Pfad aus `94db9704`
wird nicht betreten. Vorbestehender UI-Flake.

**macos-arm64 `os-render` `requires_chromium_retries_target_crash_exactly_once`.**
Zeitüberschreitung beim Warten auf das Wiederholungsziel nach dem Chromium-Crash.
`os-render` ist in diesem Paket unverändert und hängt nicht von `os-fetch` ab.
Runner-Flake, kein Codebezug.

### Nachtrag 3: Option (a) umgesetzt (Entscheidung vom 01.11.2026)

Die Spec-Ergänzung unter CRAWL-126 steht als eigener Commit (`e3d33150`).

**Umsetzung.**

- **Dauerhaftigkeit vor der Quittung (`218c2a44`, `858062d0`).** Vor dem End-Batch
  sendet die Engine eine Markierung `durable` an den Writer. Die Senke stellt dann
  `synchronous = FULL` ein, sodass der Commit des End-Batches das WAL synchronisiert.
- **Begrenzter Checkpoint.** Der abschließende `TRUNCATE`-Checkpoint wartet höchstens
  `CHECKPOINT_READER_WAIT` = 100 ms auf Leser (bisher bis zum Busy-Timeout von 5 s).
- **Hintergrund-Merge.** `os_store::open::merge_wal` versucht den Merge in kurzen,
  abbrechbaren `TRUNCATE`-Durchläufen (je höchstens 100 ms, dazwischen 50 ms Pause).
- **App (`c3d01b46`).**
  - Nach Abschluss der Engine (`settle`) startet der Merge im Hintergrund.
  - „Fortsetzen“ (UI und MCP) bricht ihn vorher ab und wartet darauf.
  - `quit::confirm` und `request_quit` warten auf `wait_merged` innerhalb des
    3-s-Budgets.
  - `crawls.reveal` und `crawls.move` warten auf den Merge, `crawls.delete` bricht ihn ab.
  - Der Export per `VACUUM INTO` bleibt unabhängig.
  - `CrawlStatus` hat ein optionales Feld `merging`; es wird nur gesendet, wenn es
    gesetzt ist, deshalb bleiben die Wire-Fixtures unverändert.
- **UI (`0953ecf0`).** Die Statuszeile zeigt „Pausiert – Datei wird zusammengeführt“
  bzw. „Paused – merging file“ (`status-paused-merging`, DE/EN).

**Tests (`// spec: CRAWL-126`).**

| Test | Prüft |
| --- | --- |
| `os-store` `pause_merge` | Checkpoint mit haltendem Leser kehrt nach ≤ 300 ms zurück (Balanced und Maximale Geschwindigkeit); der Merge wartet auf den Leser; danach ist das WAL 0 Byte groß; ein abgebrochener Merge kehrt sofort zurück |
| `os-engine` `lifecycle_crash` | Ein Kindprozess pausiert, während ein Leser das WAL festhält, und wird nach der Quittung hart beendet: Die Datei ist „Pausiert“, `integrity_check` ist ok, die Warteschlange ist vollständig |
| `os-engine` `lifecycle_pause` | Export während des laufenden Merges: keine Begleitdateien, Status „Pausiert“ |
| `os-engine` `n10_frontier` (erweitert) | Nach dem Merge nur die Hauptdatei kopieren, fortsetzen, Golden-Vergleich |
| `os-app` `pause_merge` | Fortsetzen 50 ms nach der Quittung ohne Fehler; Beenden wartet auf den Merge (≤ 3 s), danach hat das WAL keinen Inhalt |
| Vitest Statuszeile | Hinweis in DE und EN, Rückkehr zu „Paused“ |

**Messung** (Server, `taskset -c 0,1`, WebKit, `control.spec.ts` 10× seriell, zwei Reihen):

| Reihe | Latenz Median | Latenz Max | `ipcMs` Median | `ipcMs` Max |
| --- | --- | --- | --- | --- |
| 1 | 2.067 ms | 2.414 ms | 571 ms | 1.223 ms |
| 2 | 2.134 ms | 2.499 ms | 566 ms | 1.132 ms |

Vorher unter gleichen Bedingungen: Median 2,0–2,7 s, Maximum bis 3,2 s, `ipcMs`-Median
1,0–1,4 s. Das 3-s-Budget bleibt unverändert (K2-63).

Alle Playwright-Läufe mit `CI=true` (kein `reuseExistingServer`); Port 1420 war vor den
Messungen frei, der Dev-Server stammt also aus diesem Worktree.

**Prüfungen.**

- fmt und clippy (`-D warnings`): grün.
- Workspace: im ersten Lauf 4.099 bestanden, 1 Fehler (`dto_fixtures`, `merging: null`
  im Wire-Format). Behoben mit `skip_serializing_if`; die betroffenen App-Suiten sind
  danach grün. Schlusslauf: 4.100 bestanden, 0 Fehler, 49 Ignores, 568 Ziele.
- Typecheck und Vitest: 871/871.
- `e2e/beta` Chromium und WebKit: 28/28.

Hinweis: Unter „Maximale Geschwindigkeit“ schreiben Hintergrund-Checkpoints während des
Crawls weiterhin ohne fsync (bestehendes Profilverhalten). Der End-Batch ist jetzt
synchronisiert; die Hauptdatei wird erst beim abschließenden Merge mit `NORMAL`
synchronisiert.

### Nachtrag 4: CI auf `324cd5a7` (macos-arm64, linux-chromium)

**macos-arm64 `os-store` `pause_merge`.** Gemessen wurden 677 ms statt höchstens 300 ms.
Ursache: `busy_timeout` begrenzt nur die Summe der *angeforderten* Schlafpausen des
SQLite-Busy-Handlers (1, 2, 5, 10, 15, 20, 25 ms …), nicht die tatsächlich verstrichene
Zeit. macOS verlängert einzelne Pausen merklich, unter Runner-Last stark.

Gemessen auf dem Mac mini (M1, eigener Klon `~/Projects/wt-fix-e`, `nice`, Start bei
Last < 3, Diagnosetest nicht committet):

| Messung | Ergebnis |
| --- | --- |
| `PASSIVE`-Checkpoint mit haltendem Leser | 1,4 ms, also keine Synchronisationskosten |
| `TRUNCATE` mit `busy_timeout(100 ms)` | 135–137 ms |
| Abschluss-Checkpoint der Senke | 147 ms |
| `fullfsync` / `checkpoint_fullfsync` | 0 |

F_FULLFSYNC scheidet damit aus; die Zeit liegt vollständig im Busy-Handler.

**Änderung.** Ein eigener Busy-Handler mit Wanduhr-Frist (thread-lokal, Schlafschritte
≤ 2 ms) gilt für den Abschluss-Checkpoint und jeden Merge-Versuch. Danach wird der normale
Busy-Timeout von 5 s wiederhergestellt. Grenze und Test bleiben unverändert (K2-63).

Mac mini danach: Abschluss-Checkpoint 100,4–100,5 ms, `pause_merge` 10/10.

**linux-chromium `auth/bridge.spec.ts:26`.** Kein Bezug zu diesem Paket. Der Test
scheitert schon im ersten Durchlauf beim Warten auf das CDP-Ziel des Login-Browsers
(`LoginSession::open`, `os-render`). In diesem Test wird kein Crawl gestartet, deshalb
laufen weder Pause- und Endpfad noch Merge. `os-render` und `auth_login` sind in diesem
Paket unverändert. Auf demselben Branch lief der Test in den drei Läufen davor grün.
Einzustufen als vorbestehender Flake beim Kaltstart von Chromium.

Linux-Server nach der Änderung: fmt und clippy (`-D warnings`) grün; Workspace 4.100 bestanden,
0 Fehler, 49 Ignores.

### Nachtrag 5: Auflage aus der Freigabe (Fable, 01.11.2026)

**„Im Finder zeigen“ ist jetzt begrenzt.** `crawls.reveal` wartet auf den Merge höchstens
`REVEAL_BUDGET` = 2 s (für alle gewählten Crawls zusammen, analog zu `QUIT_BUDGET`). Danach
zeigt die App die Datei in jedem Fall. Läuft der Merge noch, erscheint der Hinweis
`crawl-merge-pending` (DE/EN): Die Datei wird noch zusammengeführt; beim Kopieren gehört
die „-wal“-Datei bis dahin dazu.

Neuer Test `reveal_waits_for_the_merge_only_within_its_budget` (CRAWL-126): Ein fremder
Snapshot hält den Merge offen. Der Aufruf kehrt nach ≤ 3 s zurück, zeigt den Pfad und
meldet den Hinweis. Nach dem Ende des Snapshots ist das WAL leer, und „Zeigen“ kehrt
sofort zurück. Ohne Budget wartet der Aufruf, solange der Snapshot gehalten wird.

**Optionale Hinweise umgesetzt.**

- (a) `checkpoint_within`: Ein Drop-Guard löscht die Frist und stellt den Busy-Timeout
  auch bei einem Panic wieder her.
- (b) Der Writer-Thread überspringt den abschließenden Checkpoint, wenn nach einem
  erfolgreichen expliziten Checkpoint kein Batch mehr kam. Mit haltendem Leser fallen so
  die zweiten 100 ms weg. Ohne expliziten Checkpoint bleibt der Abschluss-Checkpoint.
- (c) Unter CRAWL-126 stehen drei neue Akzeptanzpunkte (Hintergrund-Merge mit Hinweis in
  der Statusleiste, Beenden wartet, Zeigen wartet höchstens 2 s).

Prüfungen auf dem Server:

- fmt, clippy (`-D warnings`) und Neutralität grün.
- Workspace 4.101 bestanden, 0 Fehler, 49 Ignores.
- Typecheck grün; Vitest 871/871.
- `os-app` `pause_merge` 2/2, `os-store` `pause_merge` 2/2, `writer_thread`-Unit-Tests 10/10.

### Nachtrag 6: ui-e2e rot auf `d9e96d9f` (Lauf 41287318605)

**Befund.** Rot waren `beta/n04-live.spec.ts:17` (Zeile 74, Inlinks/Outlinks-Grid nicht
innerhalb 1 s) auf allen drei Plattformen und `header/header.spec.ts:23` (Zeile 69, zweite
Pause) auf Linux. Beides ist **kein Regress aus `deba2e77`/`152f8c55`**, sondern sind
vorbestehende Flakes. Es gibt keinen Code-Fix für die Specs. Test, Budgets und Fixtures
bleiben unverändert (K2-63).

**Belege aus CI.**

- Im „grünen“ Lauf 41279051227 auf `5f500611` scheiterte n04 auf linux-chromium und
  linux-webkit im ersten Versuch an derselben Zeile 74 („element(s) not found“). Grün war
  der Lauf nur dank des Retrys. Header und n04 fielen schon früher so aus, unter anderem
  auf `main` `bc964d71` (Lauf 41238238849, beide Versuche).
- Die Runner waren im roten Lauf langsamer. Median der Testdauern rot/grün über rund 490
  gemeinsame Tests: linux-webkit 1,31, windows-chromium 1,06, linux-chromium 1,00.
- Der Rerun derselben Jobs auf unverändertem `d9e96d9f` ist auf allen vier Plattformen
  grün, n04 und Header jeweils im ersten Versuch. Flaky war je ein anderer Test:
  linux-chromium `beta/quit.spec.ts:6`, linux-webkit `config/bridge.spec.ts:69`.
- Header-Trace (linux-chromium): Beim zweiten Klick auf „Pause“ waren 86–91 von 96–101
  URLs fertig. Der Klick brauchte 0,5–0,9 s, weil der Haupt-Thread der Seite belegt war.
  Danach zeigte der Snapshot „Completed 101/101“. Die Fixture (101 URLs à 200 ms) ist also
  vor der zweiten Pause fertig. Das ist dasselbe Bild wie im Abschnitt „Header-Flake“ oben.
- n04-Trace: `view.open` für Inlinks dauerte 155–1.300 ms, und der Tab-Klick selbst bis
  2,5 s. Dieses Bild ist bei `5f500611` (erste Versuche) und bei `d9e96d9f` gleich.

**Bisect auf dem Server.** Hetzner, `taskset -c 0-3` (vier Kerne wie der Runner),
`--retries=0`. Als Basis dienten die sechs zwischen `5f500611` und `d9e96d9f` geänderten
Quelldateien (Engine, Store, App, FTL), jeweils dateiweise auf den alten Stand gesetzt.

| Spec, Engine | `5f500611` rot | `d9e96d9f` rot |
|---|---|---|
| header, chromium | 2/70 | 5/70 |
| header, webkit | 5/20 | 3/20 |
| n04, chromium | 2/6 (alle Zeile 74) | 3/6 (alle Zeile 74) |

Zusammen sind das beim Header 7/90 gegen 8/90. Ein Unterschied ist nicht nachweisbar.

Pause-Latenz aus Traces (chromium, je 30): Klick bis „Paused“ im Median 1.871 ms gegen
1.722 ms, Maximum 2.762 gegen 2.544 ms. Fortsetzen bis „Running“ 827 gegen 844 ms. Das
Überspringen des zweiten Abschluss-Checkpoints macht die Pause also eher schneller.
Langsamer macht es sie nicht.

**`checkpointed` und das busy-Flag (Hinweis aus der int-23-Prüfung).** `wal_checkpoint`
wertet nur die Frame-Zahl aus. Nach einem expliziten Checkpoint, den ein Leser
unvollständig ließ, entfällt deshalb der abschließende Checkpoint. Das ist so gewollt:
CRAWL-126 begrenzt das Warten auf Leser auf 100 ms. Den Rest führt die App im Hintergrund
zurück (`merge::start` nach `settle`; MCP-Crawls laufen über dieselbe Sitzung). Ohne App
(CLI) räumt SQLite das WAL beim Schließen der letzten Verbindung auf, sofern kein fremder
Leser es hält. Hält ein fremder Leser es, blieb die `-wal` auch vor `152f8c55` nach 2 × 100 ms
stehen. Neu ist nur der Fall, dass ein Leser genau zwischen 100 und 200 ms loslässt. Mit
den roten Specs hängt das nicht zusammen, denn beide laufen in der App mit Merge. Der
Kommentar im Writer-Thread sagt jetzt, dass „erfolgreich“ kein leeres WAL bedeutet. Drei
Unit-Tests sichern das Verhalten (`dd02fe65`, CRAWL-126):

- Checkpoint ohne folgenden Batch: kein zweiter Checkpoint.
- Batch danach: Der Abschluss-Checkpoint läuft.
- Ohne expliziten Checkpoint: Der Abschluss-Checkpoint läuft.

**Prüfung auf `dd02fe65` (Server, alle Kerne).** Header chromium 5/5 und webkit 5/5,
n04 chromium 3/3 und webkit 3/3. `writer_thread`-Unit-Tests 13/13.
Außerdem grün: fmt, clippy (`-D warnings`), Workspace 4.104 bestanden, 0 Fehler,
49 Ignores. Typecheck grün, Vitest 871/871.
