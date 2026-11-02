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

Alle Merges liefen mit `--no-ff` textuell konfliktfrei (`docs/spec/05-ui.md` automatisch zusammengeführt: UI-018- und UI-061-Festlegung an getrennten Stellen). N27 und N28 basieren auf `a2f4a58f` (int-25 vor dem Berichts-Commit), N30 auf `ef543f55`.

## Inhalt

- **N27 (UI-018, UI-145):** Overview, Issues und `catalog.filterCounts` lesen eine gemeinsame Zählung je Datenstand. Während des Crawls gedrosselt: nächste Zählung frühestens max(2 s, 3 × letzte Dauer), seriell auf einer Leseverbindung. Nach Pause/Stopp/Ende sofort eine exakte Zählung ohne Schreiben (`Mode::Settled`, Auflage A1). Overview und Dashboard zeigen „Stand vor N s“ (`OverviewDto.ageMs`, auch nach Pause bis zur exakten Antwort, Auflage A2). `useIssues` hält die letzte Liste sichtbar. Messung (6.000 Seiten × 150 Links, 4 CPUs): Bridge-CPU im Lauf 250 → 126–143 %, in der Pause 85 → 32 %; Overview-Antwort 7,8 s → 61 ms; Filterzähler 3,1 s → 76 ms; Alter im Lauf ~8 s Median.
- **N28 (UI-061, CRAWL-130):** `os_store::counters::encountered` = eindeutige URLs ohne Hilfsabrufe (robots.txt-Zeilen, Sitemap-Dateien), eine Definition für Overview, Bericht, Statusleiste, Öffnen, Crawl-Liste und Engine-Fortschritt (auch nach Resume). „Total URLs Crawled“ in der Overview gleich (K-54, CRAWL-130). Gespeicherte Summary-Stücke von vor N28 werden einmal neu berechnet (`summary_signature`). Feed parst robots.txt je Origin einmal, Crawl-delay-Hinweis einmal. Pagination-Filter „Non-200 Pagination URLs“ mit fester Join-Reihenfolge. `fx-ui-basic`: gefunden/gecrawlt 29 → 27.
- **N31 (OUT-229, CRAWL-134, CRAWL-129):** Die Aufbewahrung (beim Start und stündlich) läuft auf einer eigenen Wartungsspur der Schreibwarteschlange mit Sperre gegen gleichzeitiges Schreiben in den Crawl-Index; Nutzeraufträge der Hauptspur warten nicht mehr dahinter, Beenden wartet nicht auf eine laufende Runde (absturzsicher, nächster Start wiederholt). Segmenttest wartet auf `segments.assignment_seq`, das 1-s-Budget aus CRAWL-134 bleibt als Prüfung (unter fsync-Last 0/20 → 20/20). Speicherdrosselung `max(Start, live)/2`.
- **N30 (CRAWL-112, UI-141):** Eine geänderte Thread-Zahl wirkt sofort auf die laufende Engine; scheitert das Speichern, wird sie zurückgesetzt. Ursache des Windows-Fehlschlags war die Startaufbewahrung vor dem Speicherauftrag in der einzigen Schreibwarteschlange. Der Test `n21/views.rs` wartet auf eine ruhende Sitzung, bevor er Wiederverwendung erwartet (N21-Festlegung „Cache-Probe wartet nicht auf Live-Pflege“). Unter Last: Threadtest Basis 0/20 → 20/20, n21 794/800 → 800/800.

## Auflagen und Entscheidungen

- **N27:** A1 `settling`-Flag (`188f0a82`), A2 Altersanzeige (`afe5329a`). Abweichung von A1: Der Test prüft „kein `view_cache`-Eintrag für den pausierten Datenstand“ statt „`wal_bytes` unverändert“, weil die stille Vorberechnung nach dem Öffnen ältere Stücke schreibt (Begründung im N27-Bericht). Entscheidungen: Live ≤ 1 s nur für Chip/Listeneintrag des aktiven Filters und den Fuß (Spec-Präzisierung UI-018 in `1806cc68`); seriell bleibt; Drosselung während der Crawl-Analyse als Folgepaket.
- **N28:** A1 `n21-profile.rs` auf `counters::encountered`, A2 Kommentar 27 (`9ce1e731`); Nachtrag `summary_signature` (`352bebe7`). Entscheidungen: Spec-Festlegung CRAWL-130 „gecrawlt“ als Folgepaket (Wortlaut in `M2-backlog.md` Abschnitt 9), doppelter Crawl-delay-Hinweis behoben, kein CLI-Feld „gefunden“.
- **N30:** keine Auflagen.
- **N31:** Auflagen (Kommentar `config.rs`, Modulkopf `write_queue.rs` zu Beenden und Wartungsspur, Backlog-Eintrag `retain_with`) bei der Integration erfüllt. Entscheidung: Drosselformel `max(Start, live)/2` bleibt (N04-Festlegung, sichere Richtung über `min(live, throttle)`); Verwaltung wartet weiter auf eine laufende Runde – Folgepaket nach der Beta.

K2-63: Geänderte fremde Erwartungen nur in N28 (`e2e/alpha/detail-sidebar.spec.ts`, `e2e/polish/visual-heights.spec.ts`: 29 → 27), zwingend aus der UI-061-Festlegung; keine Budgetänderung.

## Planung

- Neue Pakete in Welle 4n: M2-N27, M2-N28, M2-N30, M2-N31; Owns aus den K2-92-Tabellen der Berichte.
- `ERLAUBT` (M2-check.py) um 20 sequenzielle Überschneidungen mit Grund ergänzt.
- Backlog Abschnitt 9: Drosselung während der Crawl-Analyse, Zielwert exakter Stand nach Pause, CRAWL-130 „gecrawlt“ mit Randfällen, Beobachtungspunkte aus N30.
- `python3 -I docs/plan/M2-check.py`: OK (149 Pakete, 590 Spec-IDs).

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

`n04-live:74` war auf linux-webkit zuvor grün (main `ef543f55`, N27 allein, N28 allein, N30) und ist auch auf `m2/n31` (`940b310f`, enthält N27+N28+N30) in beiden Versuchen rot. Behandlung als Regression der Runde, nicht als Flake: Ursachensuche per Bisect auf dem Server läuft (Paket M2-FIX-LIVE, Budget unverändert, K2-63). int-26 geht erst nach dem Fix nach main.

## Folgearbeiten

- M2-N29 (Kennzeichnung nicht gebauter Funktionen).
- Drosselung der gemeinsamen Zählung während der Crawl-Analyse; CRAWL-130 „gecrawlt“ mit Bericht/Engine-Angleichung.
- Inkrementelle Zähler nach UI-145 (nach der Beta).
- Auswertung der Nutzer-Crawl-Datei mit `n25-tail.py` (Ratenabfall nach Fortsetzen).
