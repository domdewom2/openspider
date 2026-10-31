# M3-FIX-E – CI-Befunde der Integrationsrunde e

Stand: 31.10.2026. Branch `m3/fix-e`, Basis `origin/m3/int-e` (`a2b7bd61`).
Linux-Server; keine Veröffentlichung, kein Push. Native macOS-Bestätigung beim Orchestrator.
**Status: PAKET OFFEN** wegen verbleibender WebKit-Open-Latenz unter Doppel-Crawl-Last;
die unten aufgeführten grünen Standardgates ersetzen diesen offenen Befund nicht.

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

## Offener Restbefund und nächster Schritt

Die Linux-Stackkorrektur ist mit rotem Vorherlauf und kleinen Runtime-Stacks
belegt; die native SIGBUS-Zuordnung bleibt durch den Orchestrator zu bestätigen.
Die Detailkorrekturen beseitigen nachgewiesene Mehrfachscans, inkonsistente
Auswahlgenerationen und serielle Initial-Ladewege. Der WebKit-Doppel-Crawl-
Roundtrip von 1.152 ms ist damit jedoch **nicht geschlossen**. Das Paket wird
nicht allein aufgrund grüner serieller Wiederholungen als vollständig behoben
abgegeben.

Nächster technischer Schritt: im langsamen `view.open` getrennt messen:
Semaphore-/Blocking-Wartezeit der `detail-link-ids`-/`detail-window`-Lanes in
`ReadPool`, kaltes Öffnen der jeweiligen SQLite-Verbindung, SQL-Laufzeit sowie
Serialisierung/Antworttransport. Erst danach den verbleibenden Produktengpass
gezielt korrigieren und dieselbe Doppel-Crawl-Reihe ohne Budgetänderung wiederholen.
Parallel dazu führt der Orchestrator die oben beschriebene macOS-arm64-Bestätigung aus.
