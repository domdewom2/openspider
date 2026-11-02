# M2 Integrationsrunde 26

Stand: 02.11.2026. Branch `m2/int-26`, Basis `ef543f55` (origin/main: int-25 abgeschlossen). Bauen und Prüfen auf dem Linux-Server; Git-Operationen lokal. Kein Rebase, kein `cargo update` (Tokio bleibt 1.53.3), Schema bleibt v5.

## Nachtrag int-25

- CI 41368451527, Versuch 2: alle Jobs grün, auch `ci (windows-x64)`. Der Fehlschlag in Versuch 1 (`n21/views.rs:230`) ist in dieser Runde als Testrennen belegt und behoben (M2-N30).
- Fable-Auflage A2 (FIX-ABOUT im nativen Build prüfen): steht aus bis zum DMG-Build von `ef543f55`; Ergebnis folgt im Nachtrag dieses Berichts.

## Integration

| Reihenfolge | Paket | Quellstand | Merge | Fable |
|---|---|---|---|---|
| 1 | N28 (`m2/n28`, Spec `90a2dfcb`) | `352bebe7` | `531a723f` | approved mit Auflagen, Auflagen erfüllt; Nachtrag `summary_signature` |
| 2 | N27 (`m2/n27`, Spec `575bbded`) | `d3f52f95` | `e57c6e85` | approved mit Auflagen, Auflagen erfüllt |
| 3 | N30 (`m2/n30`) | `bd29d178` | `b6d529d4` | approved |
| 4 | N31 (`m2/n31`) | `940b310f` | `9d6f5803` | approved mit Auflagen, Auflagen bei der Integration erfüllt |
| 5 | FIX-LIVE (`m2/fix-live`, Basis `41ae1fb3`) | `a49e6755` | `7f62ac90` | approved mit Auflagen, Auflagen erfüllt |

Alle Merges liefen mit `--no-ff` textuell konfliktfrei (`docs/spec/05-ui.md` automatisch zusammengeführt: UI-018- und UI-061-Festlegung an getrennten Stellen). N27 und N28 basieren auf `a2f4a58f` (int-25 vor dem Berichts-Commit), N30 auf `ef543f55`, FIX-LIVE auf `41ae1fb3` (konfliktfrei).

## Inhalt

- **N27 (UI-018, UI-145):** Overview, Issues und `catalog.filterCounts` lesen eine gemeinsame Zählung je Datenstand. Während des Crawls gedrosselt: nächste Zählung frühestens max(2 s, 3 × letzte Dauer), seriell auf einer Leseverbindung. Nach Pause/Stopp/Ende sofort eine exakte Zählung ohne Schreiben (`Mode::Settled`, Auflage A1). Overview und Dashboard zeigen „Stand vor N s“ (`OverviewDto.ageMs`, auch nach Pause bis zur exakten Antwort, Auflage A2). `useIssues` hält die letzte Liste sichtbar. Messung (6.000 Seiten × 150 Links, 4 CPUs): Bridge-CPU im Lauf 250 → 126–143 %, in der Pause 85 → 32 %; Overview-Antwort 7,8 s → 61 ms; Filterzähler 3,1 s → 76 ms; Alter im Lauf ~8 s Median.
- **N28 (UI-061, CRAWL-130):** `os_store::counters::encountered` = eindeutige URLs ohne Hilfsabrufe (robots.txt-Zeilen, Sitemap-Dateien), eine Definition für Overview, Bericht, Statusleiste, Öffnen, Crawl-Liste und Engine-Fortschritt (auch nach Resume). „Total URLs Crawled“ in der Overview gleich (K-54, CRAWL-130). Gespeicherte Summary-Stücke von vor N28 werden einmal neu berechnet (`summary_signature`). Feed parst robots.txt je Origin einmal, Crawl-delay-Hinweis einmal. Pagination-Filter „Non-200 Pagination URLs“ mit fester Join-Reihenfolge. `fx-ui-basic`: gefunden/gecrawlt 29 → 27.
- **N31 (OUT-229, CRAWL-134, CRAWL-129):** Die Aufbewahrung (beim Start und stündlich) läuft auf einer eigenen Wartungsspur der Schreibwarteschlange mit Sperre gegen gleichzeitiges Schreiben in den Crawl-Index; Nutzeraufträge der Hauptspur warten nicht mehr dahinter, Beenden wartet nicht auf eine laufende Runde (absturzsicher, nächster Start wiederholt). Segmenttest wartet auf `segments.assignment_seq`, das 1-s-Budget aus CRAWL-134 bleibt als Prüfung (unter fsync-Last 0/20 → 20/20). Speicherdrosselung `max(Start, live)/2`.
- **N30 (CRAWL-112, UI-141):** Eine geänderte Thread-Zahl wirkt sofort auf die laufende Engine; scheitert das Speichern, wird sie zurückgesetzt. Ursache des Windows-Fehlschlags war die Startaufbewahrung vor dem Speicherauftrag in der einzigen Schreibwarteschlange. Der Test `n21/views.rs` wartet auf eine ruhende Sitzung, bevor er Wiederverwendung erwartet (N21-Festlegung „Cache-Probe wartet nicht auf Live-Pflege“). Unter Last: Threadtest Basis 0/20 → 20/20, n21 794/800 → 800/800.

- **FIX-LIVE (UI-145, UI-018):** Ursache der n04-live-Regression auf linux-webkit: Seit N27 liefert die gemeinsame, gedrosselte Zählung während des Crawls häufiger neue Overview-Antworten (auch nur mit neuem `ageMs`); die Overview baute je Antwort alle 401 Filterzeilen neu auf und blockierte unter WebKit den Main-Thread wiederholt für 0,5–1,5 s. Der Fix memoisiert Filterzeilen und Tab-Abschnitte; über Structural Sharing von TanStack Query behalten unveränderte Filter ihre Objekte, Rückrufe bleiben über `useLatest` stabil. Eine Antwort mit nur neuem Alter rendert keine Zeile, eine geänderte Zählung nur die betroffenen Zeilen (`overview-rows.test.tsx`).

## Auflagen und Entscheidungen

- **N27:** A1 `settling`-Flag (`188f0a82`), A2 Altersanzeige (`afe5329a`). Abweichung von A1: Der Test prüft „kein `view_cache`-Eintrag für den pausierten Datenstand“ statt „`wal_bytes` unverändert“, weil die stille Vorberechnung nach dem Öffnen ältere Stücke schreibt (Begründung im N27-Bericht). Entscheidungen: Live ≤ 1 s nur für Chip/Listeneintrag des aktiven Filters und den Fuß (Spec-Präzisierung UI-018 in `1806cc68`); seriell bleibt; Drosselung während der Crawl-Analyse als Folgepaket.
- **N28:** A1 `n21-profile.rs` auf `counters::encountered`, A2 Kommentar 27 (`9ce1e731`); Nachtrag `summary_signature` (`352bebe7`). Entscheidungen: Spec-Festlegung CRAWL-130 „gecrawlt“ als Folgepaket (Wortlaut in `M2-backlog.md` Abschnitt 9), doppelter Crawl-delay-Hinweis behoben, kein CLI-Feld „gefunden“.
- **N30:** keine Auflagen.
- **N31:** Auflagen (Kommentar `config.rs`, Modulkopf `write_queue.rs` zu Beenden und Wartungsspur, Backlog-Eintrag `retain_with`) bei der Integration erfüllt. Entscheidung: Drosselformel `max(Start, live)/2` bleibt (N04-Festlegung, sichere Richtung über `min(live, throttle)`); Verwaltung wartet weiter auf eine laufende Runde – Folgepaket nach der Beta.

- **FIX-LIVE:** Auflage 1 Kommentar im Test (Render-Zählung gilt ohne StrictMode, `7fc076d8`); Auflage 2 dieser Bericht mit Bisect und Profil; Auflage 3 n04-live original unter 2 Kernen/1 Worker auf dem finalen Stand und auf main (Abschnitt Prüfungen); Auflagen 4/5 Backlog Abschnitt 9 (M2-N32, Flake `visual-heights`, Spec-Frage Zeitbudgets). Entscheidung main-Merge (Fable): mit grünem ui-e2e; ein im Retry grüner linux-webkit-Lauf genügt; sind beide Versuche rot, wird nicht gemergt.

K2-63: Geänderte fremde Erwartungen nur in N28 (`e2e/alpha/detail-sidebar.spec.ts`, `e2e/polish/visual-heights.spec.ts`: 29 → 27), zwingend aus der UI-061-Festlegung; keine Budgetänderung.

## Planung

- Neue Pakete in Welle 4n: M2-N27, M2-N28, M2-N30, M2-N31, M2-FIX-LIVE; Owns aus den K2-92-Tabellen der Berichte (FIX-LIVE: `Overview.tsx`, `overview-rows.test.tsx`).
- `ERLAUBT` (M2-check.py) um 20 sequenzielle Überschneidungen mit Grund ergänzt, dazu 4 für FIX-LIVE (`Overview.tsx` gehört auch U12, N02a, U23, N03; andere Wellen, daher nur zur Dokumentation).
- Backlog Abschnitt 9: Drosselung während der Crawl-Analyse, Zielwert exakter Stand nach Pause, CRAWL-130 „gecrawlt“ mit Randfällen, Beobachtungspunkte aus N30; aus FIX-LIVE Folgepaket M2-N32, Flake `visual-heights.spec.ts:81/87`, Spec-Frage Zeitbudgets.
- `python3 -I docs/plan/M2-check.py`: OK (150 Pakete, 590 Spec-IDs).

## Prüfungen

Server (Linux x86_64), Integrationsbaum:

| Prüfung | Stand | Ergebnis |
|---|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` | `1806cc68` (N27+N28) und `a391962e` | grün |
| os-app-Pakettests (`pause_merge`, `fix_tx_*`, `stop_analysis_merge`, `n25_*`, `pending_w1_o02`, `n28_counts`) | `1806cc68` | 15/15 |
| os-analyze (`n26_*`, `n25_*`, `fix_wal_checkpoint`, `tab_hreflang*`, `n28_pagination_plans`) | `1806cc68` | 35/35 |
| os-store/os-engine `n28_encountered` | `1806cc68` | 4/4 |
| `cargo test --workspace --no-fail-fast` | `1806cc68` | 4.115 bestanden, 0 fehlgeschlagen |
| `cargo test -p os-app --no-fail-fast` (nach N30) | `a391962e` | 484 bestanden, 0 fehlgeschlagen |
| `cmd_config_runtime`, `n21` (nach N30) | `a391962e` | 12/12, 9/9 |
| `npm run -w apps/desktop typecheck`, `test` | `1806cc68` | grün, 102 Dateien, 894 Tests |

N30 ändert nur os-app (ein Quell- und zwei Testdateien); der volle Workspace lief auf N30s Basis grün (4.099/0, N30-Bericht).

Integrationsbaum mit N31 (`41ae1fb3`): `cargo fmt`, `cargo clippy --workspace --all-targets -D warnings` grün; `cargo test --workspace --no-fail-fast` 4.118 bestanden, 0 fehlgeschlagen; M2-check OK.

CI und ui-e2e des Integrationsbaums (`41ae1fb3`; erste Läufe auf `a391962e` wegen der Aufnahme von N31 abgebrochen):

| Lauf | Ergebnis |
|---|---|
| CI 41390554949 | grün (alle Jobs ohne Intel) |
| ui-e2e 41390556980 | linux-chromium, macos-webkit, windows-chromium grün; linux-webkit rot: `beta/n04-live.spec.ts:74` (Grid „Inlinks“ nicht innerhalb von 1 s während des Live-Crawls), in beiden Versuchen; flaky `polish/visual-heights.spec.ts:81` |

`n04-live:74` war auf linux-webkit zuvor im Integrationslauf grün (N27 allein, N28 allein, N30) und ist auch auf `m2/n31` (`940b310f`, enthält N27+N28+N30) in beiden Versuchen rot. Schon vor int-26 war n04-live im ersten Versuch rot (main `ef543f55`, n25 `f585094f`) und erst im Retry grün; der Test ist also vorbelastet, int-26 hat die Rate deutlich verschlechtert. Behandlung als Regression der Runde, nicht als Flake: Ursachensuche per Bisect auf dem Server, Fix M2-FIX-LIVE (Budget unverändert, K2-63).

### Bisect n04-live (Server, Diagnosekopie, 4 Kerne/2 Worker, 10 Läufe je Stand)

| Stand | Nachlauf > 1000 ms | Nachlauf Median / Max | Main-Thread-Blockaden ≥ 500 ms |
|---|---|---|---|
| `ef543f55` (main) | 4/10 | 278 / 2895 ms | 12 |
| `531a723f` (nur N28) | 3/10 | 325 / 1861 ms | 14 |
| `1806cc68` (N28+N27) | 5/10 | 598 / 2830 ms | 30 |
| `a391962e` (+N30) | 5/10 | 475 / 4231 ms | 29 |
| `41ae1fb3` (int-26) | 7/10 | 484 / 3470 ms | 25 |
| Fix `a49e6755` | 4/10 | 435 / 2129 ms | 15 |

CPU-Profil (Chromium, gleicher Ausschnitt): Main-Thread belegt 12,1 s (main) → 26,6 s (int-26) → 11,4 s (Fix); davon `OverviewContent` 1,1 s → 8,5 s → ~0,3 s. Der Sprung kommt mit N27 (häufigere Overview-Antworten), der Fix bringt Blockaden und Main-Thread-Last auf das Niveau von main.

### n04-live original (je 10 Läufe, WebKit, Server)

| Konfiguration | vorher (`41ae1fb3`) | nachher (Fix) | main `ef543f55` |
|---|---|---|---|
| 4 Kerne / 1 Worker | 10/10 grün | 10/10 grün | – |
| 4 Kerne / 2 Worker | 7/10 | 8/10 | – |
| 2 Kerne / 1 Worker | 4/10 | **0/10** | 4/10 |

Der 2-Kerne-Ausreißer (Fix 0/10) ist ungeklärt und wird ausdrücklich nicht weggelassen. 2 Kerne werden als Bewertungsgrundlage verworfen: Der Main-Thread ist dort zu 93–96 % blockiert, auch main schafft nur 4/10; die Diagnose unter 2 Kernen zeigte für den Fix einen besseren Nachlauf-Median (550 statt 1314 ms). Nachmessung auf dem finalen Stand: Fable-Auflage 3, Abschnitt unten.

Fable-Entscheidung main-Merge: mit grünem ui-e2e; ein im Retry grüner linux-webkit-Lauf genügt; sind beide Versuche rot, wird int-26 nicht gemergt.

### Auflage 3: n04-live original, 2 Kerne / 1 Worker, finaler Stand und main

Server, WebKit, `taskset -c 0,1`, `--workers=1 --retries=0 --repeat-each=10`, Debug-Bridge wie CI, nacheinander (nicht parallel) gelaufen. Finaler Stand = `d0ea1bf0` (Code `a49e6755` + Testkommentar), Baum per `git archive` auf den Server-Worktree `m2-int-26` gebracht und gegen den Commit verglichen; main in einem temporären Worktree.

| Stand | grün | Fehlschläge | Klick-Latenzen der grünen Läufe (Inlinks/Outlinks/Inlinks) |
|---|---|---|---|
| finaler int-26 `d0ea1bf0` | 6/10 | 4 × `n04-live.spec.ts:74` (Grid „Inlinks“ nicht innerhalb von 1 s) | 1,5–3,2 s |
| main `ef543f55` | 5/10 | 5 × `n04-live.spec.ts:74` (gleiche Stelle) | 1,4–3,5 s |

Bewertung: Kein STOPP (Regel: finaler Stand 0/10 bei main ≥ 3/10). Der frühere 0/10-Ausreißer des Fixes unter 2 Kernen ist nicht reproduziert; unter 2 Kernen liegen finaler Stand und main gleichauf (6/10 bzw. 5/10), beide messen hier einen überlasteten Main-Thread, keine Abnahme. Die Latenzen der grünen Läufe liegen über 1 s, obwohl der Test grün ist: Die angehängten Werte messen ab vor dem Klick (inklusive Playwright-Auto-Wait des Klicks auf den blockierten Main-Thread), das 1-s-Zeitlimit der Prüfung beginnt erst nach dem Klick (Spec-Datei Zeile 72–75). Latenz im Live-Crawl: M2-N32.

Server-Gates auf `d0ea1bf0` (Rust unverändert gegenüber `41ae1fb3`, daher kein voller Workspace-Test): `cargo fmt --all --check` grün; `npm run -w apps/desktop typecheck` grün; `npm run -w apps/desktop test` 103 Dateien, 895 Tests grün; `python3 -I docs/plan/M2-check.py` OK (150 Pakete).

## Folgearbeiten

- M2-N29 (Kennzeichnung nicht gebauter Funktionen).
- Drosselung der gemeinsamen Zählung während der Crawl-Analyse; CRAWL-130 „gecrawlt“ mit Bericht/Engine-Angleichung.
- Inkrementelle Zähler nach UI-145 (nach der Beta).
- Auswertung der Nutzer-Crawl-Datei mit `n25-tail.py` (Ratenabfall nach Fortsetzen).
- M2-N32 „Live-Latenz Detailfenster“ (`view.open` Inlinks/Outlinks im Live-Crawl 300–600 ms, unter Last 1,8 s, Ziel UI-145 150 ms erste Seite; erst messen, dann erste Seite ohne Gesamtzahl, Renderlast senken; Abnahme im Release-Build auf Referenzhardware, n04-live linux-webkit 10 Läufe). Wortlaut in `M2-backlog.md` Abschnitt 9.
- Flaky `e2e/polish/visual-heights.spec.ts:81/87` (Dashboard-Kennzahlen „–“), Zusammenhang mit N27 möglich, nicht untersucht.
- Spec-Frage Zeitbudgets (Fable-Empfehlung, wartet auf Nutzerbestätigung, Spec unverändert): UI-145/UI-143/PERF-004 gelten im Release-Build auf der Referenzhardware (`docs/spec/07-performance.md`); E2E gegen den Dev-Server ist Regressions-Smoke mit eigenen Schwellen.
