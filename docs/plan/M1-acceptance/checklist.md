# Abnahmebericht M1 (M1-N24, Welle 7c)

Stand: 2026-10-16 · Umsetzer: Sonnet 5.5 · Branch `m1/m1-n24` · Basis `origin/main` f8357b5 (Plan Welle 7c/7d, K-80 … K-82; letzter Code- und Berichtsstand: Merge von M1-N21, 6d8237f, Welle 7b; letzter abgeschlossener main-Lauf: [38818635508](https://github.com/domdewom2/openspider/actions/runs/38818635508) auf cb3a58a, grün) · Welle 7b abgeschlossen, Welle 7c läuft (M1-N22 … M1-N25)  
Kriterien A1–A9: `docs/plan/M1.md` Abschnitt 6 in der Fassung vom 16.10.2026 (A1 misst nach K-81 die reine Testausführung, `cargo-deny` prüft nach K-82 auch `advisories`). Belege: dieser Bericht, `docs/plan/M1-status.json`, `docs/plan/M1-acceptance/parity/` (Paritätsbericht), `docs/plan/M1-acceptance/loadtest/` und `benchmarks/` (M1-N14), CI-Läufe (Links unten).

## 0 Kurzfassung

Stand `origin/main` f8357b5 (16.10.2026), Welle 7c. Das Fable-Gate M1 auf cb3a58a ergab **needs_work, 82/100** (vorher Recheck Welle 6: 84/100, Gate M1: 70/100). Der Planer hat die Befunde am 16.10.2026 als K-80 … K-82 entschieden: PERF-002 (A4) → M1-N22, Laufzeit von A1 → M1-N23, Advisories → M1-N25; dieser Bericht (M1-N24) bringt die Zahlen von A1, A2 und A9 auf den main-Stand. Der Code- und Testbestand ist der Merge von M1-N21 (6d8237f, Wellen 7 und 7b: M1-P32, M1-P33, M1-N19, M1-N20, M1-N21); seitdem ändern sich nur Dateien unter `docs/` (Planung, Spec, Roadmap), auch durch f8357b5. Der Paritätsbericht (A3) stammt von M1-N18; der Job `parity` ist in den main-Läufen weiter grün. **Abschnitt A4 und die Zeile A4 der Kurzfassung bleiben auf dem Stand von M1-N21; sie werden von M1-N26 nachgeführt.**

| Nr. | Kriterium | Ergebnis | Beleg |
|---|---|---|---|
| A1 | Build/Qualität | **nicht erfüllt (Laufzeit windows-x64/macos-x64)**, Nachbesserung M1-N23 nach K-81 – alle Jobs der 10 main-Läufe seit dem N18-Merge grün, aber der Schritt „Rust tests“ liegt in 5 von 10 Läufen über 15 min (windows-x64 bis 16 min 58 s, macos-x64 einmal 17 min 04 s); nach K-81 misst A1 künftig die reine Testausführung, `cargo-deny` prüft ab M1-N25 auch `advisories` (K-82) | Tabelle in Abschnitt A1; main-Läufe [38688658122](https://github.com/domdewom2/openspider/actions/runs/38688658122) … [38818635508](https://github.com/domdewom2/openspider/actions/runs/38818635508) |
| A2 | Spec-Abdeckung | **erfüllt** (242/242: implemented 237, verified 5, 0 open, 0 ungültige Ignores; Zusatz-IDs 7 mit PERF-002) | `docs/plan/M1-status.json` aus dem Artefakt `spec-coverage` des main-Laufs [38818635508](https://github.com/domdewom2/openspider/actions/runs/38818635508) (cb3a58a), Abschnitt A2 |
| A3 | Parität (SF Free) | **erfüllt** – 5 von 5 Sites, 38 von 38 Vergleichen erfüllen die Schwelle, `os-parity run --all` endet mit Exit 0; in der CI erzwungen | `docs/plan/M1-acceptance/parity/`, Abschnitt A3 |
| A4 | Performance | **nicht erfüllt (PERF-002 Abschnitte)** – 1-Mio.-Crawl PERF-001: Mittel 2 098,8 URLs/s (>= 500), aber die zehn 10-%-Abschnitte liegen bei 68 % bis 186 % des Mittelwerts (erlaubt 70 % bis 130 %, K-72); Nachbesserungsauftrag in `loadtest/README.md`. Alle übrigen Kriterien der Großläufe nach Welle 7 erfüllt (PERF-001 RSS 471,1 MB, PERF-003 RSS 468,1 MB, Stillstand 6,1 s / 4,3 s, PERF-005, CRAWL-124, 18 von 18 Benchmark-Ignores) | Abschnitt A4, `loadtest/README.md`, `benchmarks/README.md` (M1-N21) |
| A5 | CLI | **erfüllt** | Abschnitt A5 |
| A6 | Crawl-Datei | **erfüllt** | Abschnitt A6 |
| A7 | MCP | **erfüllt** | Abschnitt A7 |
| A8 | Clean-Room | **erfüllt (mechanische Prüfung)**; Textstichprobe: Fable | Abschnitt A8 |
| A9 | Statusbericht | **erfüllt** (keine ID `open`; Pakete bis M1-N21) | `docs/plan/M1-status.json` (Stand cb3a58a), Abschnitt A9 |

**Gate-Weg:** Fable-Gate M1 (needs_work, 82/100) auf cb3a58a → **Welle 7c** (M1-N22 PERF-002 Konstanz im 1-Mio.-Crawl, K-80; M1-N23 CI-Laufzeit A1, K-81; M1-N24 dieser Bericht; M1-N25 Advisories, K-82) → Merge der vier Pakete → **drei grüne main-Läufe** → **M1-N26** (Welle 7d, allein auf ruhiger Maschine: `perf-001 --size large` zweimal mit leerem `crawler_violations`, `perf-003 --size large`, `perf-005 --size large --kills 10`, danach die lokale Gegenprobe; trägt A1 (main-Läufe nach Welle 7c), A2/A9, A4, Kurzfassung und Gate-Weg nach) → erneutes Fable-Gate M1. Die Großläufe von M1-N21 (15.10.2026, Commit `88fd424`) bleiben bis M1-N26 der Stand von A4. Offen sind A1 (Laufzeit) und A4 (PERF-002 Abschnitte).

Offene Punkte für Orchestrator/Planer:
- **A1, Laufzeit „Rust tests“ → M1-N23 (K-81):** Der Schritt dauerte in 5 der letzten 6 main-Läufe auf windows-x64 über 15 min (15:09 bis 16:58), auf macos-x64 einmal 17:04 (Tabelle in A1). M1-N23 trennt Kompilieren („Rust tests (build)“) und Testausführung, erzwingt 15 min per `timeout-minutes` und strebt < 12 min an. Lange Integrationstests aufzuteilen wäre ein Befund an den Planer (Testdateien gehören nicht zum Besitz von N23). Der Beleg nach der Merge-Reihe folgt in M1-N26.
- **Advisories → M1-N25 (K-82):** `cargo deny check advisories` ist auf main rot (rustls RUSTSEC-2026-0346 bis -0349, ttf-parser, Angabe des Planers in `docs/plan/M1.md`, Welle 7c); der Job `cargo-deny` der CI prüft bisher nur licenses, bans und sources und ist grün. M1-N25 hebt die Pakete gezielt an bzw. ignoriert Befunde ohne Fix in `deny.toml` und nimmt `advisories` in den Job auf.
- **A4, PERF-002 Abschnitte → M1-N22 (K-80):** Im 1-Mio.-Crawl von PERF-001 fällt der Durchsatz stetig von rund 3 900 auf rund 1 400 URLs/s (Abschnitte 149, 186, 173, 164, 139, 116, 100, 75, 72, 68 % des Mittelwerts 2 098,8 URLs/s; zweiter Lauf: 138 bis 172 % in den Abschnitten 1 bis 4). Nachbesserungsauftrag mit Spec-ID, Crates, Messwerten und Befehl in `loadtest/README.md`. PERF-003 large ist mit Welle 7 erfüllt (468,1 MB, Stillstand 4,3 s). Der Nachweis folgt in M1-N26.
- **Last-Flakes durch 20-s-Fristen in Goldens** (`goldens_canonical_basic`, `chains_over_different_redirect_types_are_followed_as_one`): M2-Backlog, Priorität niedrig, kein Paket in M1.
- **Lasttest `--profile max-speed`:** wird seit N19 nicht mehr mit den PERF-001-RSS-Grenzen bewertet (K-16); bei 1 Mio. Seiten 5 437 MB RSS und kein Geschwindigkeitsvorteil gegen Balanced (-9,3 %), siehe `loadtest/README.md`, Beobachtungen. Kein Kriterium.
- **Windows-Durchsatz:** Quote in A4 (Tabelle) weiter beobachten.
- **Aufräumen:** Der Branch `parity/sf-reference` existiert nicht mehr auf dem Remote; die Testsite-Ports 8902–8905 sind frei.
- **Annahmen ohne Beleg in den Paritäts-Sites (M2):** A-19b (Vorlauf einer Schleife, `/tail`; K-66), A-35b (`<li>` ohne Link, K-68), `HTTP Refresh Redirect` auf die eigene URL (K-69), Randfälle der Kollation `OS_TEXT` (K-70). Siehe `docs/plan/M2-backlog.md`.

## A1 Build/Qualität

**Status A1: erfüllt (nach K-81/K-82, M1-N26).** Nach den Merges der Welle 7c (f1867ad M1-N22, 7e81cd8 M1-N23, 8ef30d1 M1-N24, ac41d7c M1-N25) liegen **drei grüne main-Läufe** vor ([38867684400](https://github.com/domdewom2/openspider/actions/runs/38867684400) auf 221726d, [38868486042](https://github.com/domdewom2/openspider/actions/runs/38868486042) auf f1f5913, [38868822232](https://github.com/domdewom2/openspider/actions/runs/38868822232) auf 5f65ffc; je 16 von 16 Jobs grün). Die drei Commits ändern gegenüber ac41d7c nur Dateien unter `docs/`; der Code- und Teststand ist der von Welle 7c. Der Schritt „Rust tests“ (reine Testausführung, K-81) braucht auf jeder Plattform höchstens 8 min 38 s (Grenze 15 min, Ziel < 12 min) und ist im Workflow per `timeout-minutes: 15` erzwungen; der Job `cargo-deny (licenses, bans, sources, advisories)` ist grün (K-82).

Quelle der Zeiten: `gh api repos/domdewom2/openspider/actions/runs/<id>/jobs` (`started_at` bis `completed_at` je Schritt, Job `build (<Plattform>)`).

| Lauf | Commit | Rust tests (build) macos-arm64 | macos-x64 | windows-x64 | Rust tests macos-arm64 | macos-x64 | windows-x64 | Clippy macos-arm64 | macos-x64 | windows-x64 |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| [38867684400](https://github.com/domdewom2/openspider/actions/runs/38867684400) | `221726d` | 2 min 23 s | 10 min 22 s | 10 min 48 s | 4 min 10 s | 5 min 12 s | 8 min 38 s | 0 min 29 s | 7 min 17 s | 4 min 17 s |
| [38868486042](https://github.com/domdewom2/openspider/actions/runs/38868486042) | `f1f5913` | 2 min 24 s | 6 min 25 s | 10 min 50 s | 4 min 00 s | 3 min 55 s | 7 min 54 s | 0 min 26 s | 3 min 54 s | 3 min 55 s |
| [38868822232](https://github.com/domdewom2/openspider/actions/runs/38868822232) | `5f65ffc` | 2 min 51 s | 9 min 54 s | 7 min 31 s | 4 min 22 s | 5 min 17 s | 7 min 22 s | 0 min 33 s | 6 min 02 s | 2 min 27 s |

Bewertung nach K-81: Testausführung (Schritt „Rust tests“, Spalten 5 bis 7) höchstens **8 min 38 s** (windows-x64, Lauf 38867684400), macos-x64 höchstens 5 min 17 s, macos-arm64 höchstens 4 min 22 s; alle neun Werte liegen unter dem Ziel von 12 min und weit unter 15 min (kleinste Reserve zur Grenze 6 min 22 s). Das Kompilieren (Schritt „Rust tests (build)“, `--no-run`) steht nur im Bericht: 2 min 23 s bis 2 min 51 s (macos-arm64), 6 min 25 s bis 10 min 22 s (macos-x64), 7 min 31 s bis 10 min 50 s (windows-x64); die Obergrenze ist das Job-Timeout. Die Summe aus Kompilieren und Ausführen wäre auf windows-x64 in den ersten beiden Läufen mit 19 min 26 s und 18 min 44 s über 15 min gewesen, nach der bisherigen Definition also nicht erfüllt; sie ist nach K-81 kein Kriterium mehr. Clippy: höchstens 7 min 17 s (macos-x64). Der Schritt „Rust tests“ ist die Summe aller Testausführung (alle Workspace-Tests einschließlich Doctests, `cargo test`, kein `nextest`); `spec-coverage` bleibt bei `cargo test` (Testausgabe des Laufs 38868822232: 2 046 bestanden, 0 fehlgeschlagen, 36 ignoriert).

`cargo-deny` mit `advisories` (K-82): `deny.toml` hat einen Eintrag unter `[advisories] ignore` (RUSTSEC-2026-0192, `ttf-parser` 0.25.1 unmaintained, nur dev-dependency von `os-parse`, nicht in ausgelieferten Binaries, Begründung in der Datei); die rustls-Advisories RUSTSEC-2026-0346 bis -0349 sind durch die Anhebung auf rustls 0.23.46 behoben (Commit 246e565, M1-N25). Der Job `cargo-deny (licenses, bans, sources, advisories)` ist in allen drei Läufen grün; die Prüfung von `advisories` ist damit in der CI erzwungen.

Weitere Jobs im jüngsten Lauf 38868822232 (16 Jobs, alle grün): build (macos-arm64, macos-x64, windows-x64); cargo-deny (licenses, bans, sources, advisories); rustfmt; crawlfile create und crawlfile load je auf macos-arm64, macos-x64, windows-x64; loadtest-small auf allen drei Plattformen; spec-coverage (`--strict`, 0 open); parity (macos-arm64, erzwungen).

Die lokale Gegenprobe (Debug-Build, `cargo test --workspace --exclude openspider-desktop --locked` in frischem Target ohne `RUSTC_WRAPPER`) steht im Abschnitt „Lokale Gegenprobe“.

### Vorgeschichte: main-Läufe vor Welle 7c nach der bisherigen Definition (Stand M1-N24)

Bis zum Merge der Welle 7c war A1 **nicht erfüllt (Laufzeit windows-x64/macos-x64)**: Der Schritt „Rust tests“ (`cargo test --workspace`, **einschließlich Kompilieren**, vor K-81) lag in 5 von 10 main-Läufen seit dem Merge von M1-N18 über 15 min. Die Tabelle bleibt als Beleg erhalten; die Nachbesserung war M1-N23 (Trennung in „Rust tests (build)“ und „Rust tests“, `timeout-minutes: 15`).

main-Läufe seit dem Merge von M1-N18 (317920a, 15.10.2026) bis zum Stand dieses Berichts, alle Gesamtergebnis **success**:

| Lauf | Commit | Rust tests macos-arm64 | Rust tests macos-x64 | Rust tests windows-x64 | Clippy macos-arm64 | Clippy macos-x64 | Clippy windows-x64 | Bewertung (Grenze 15 min) |
|---|---|---:|---:|---:|---:|---:|---:|---|
| [38688658122](https://github.com/domdewom2/openspider/actions/runs/38688658122) | `317920a` Merge m1-n18 (Welle 6b) | 5 min 58 s | 7 min 29 s | 14 min 21 s | 0 min 25 s | 1 min 09 s | 0 min 51 s | erfüllt (Reserve windows-x64 0 min 39 s) |
| [38705346547](https://github.com/domdewom2/openspider/actions/runs/38705346547) | `46b8d6f` Plan Welle 7/7b | 6 min 12 s | 9 min 29 s | 12 min 00 s | 0 min 23 s | 0 min 47 s | 0 min 36 s | erfüllt |
| [38717556960](https://github.com/domdewom2/openspider/actions/runs/38717556960) | `741ae32` M2-Umsetzungsplan | 6 min 46 s | 13 min 16 s | 14 min 28 s | 0 min 32 s | 1 min 39 s | 0 min 51 s | erfüllt (Reserve windows-x64 0 min 32 s) |
| [38720486616](https://github.com/domdewom2/openspider/actions/runs/38720486616) | `807854e` M2-Plan Rev. 2 | 5 min 05 s | 7 min 35 s | 14 min 36 s | 0 min 21 s | 0 min 46 s | 0 min 55 s | erfüllt (Reserve windows-x64 0 min 24 s) |
| [38784370685](https://github.com/domdewom2/openspider/actions/runs/38784370685) | `88fd424` Merge M1-N20 (Welle 7) | 7 min 54 s | **17 min 04 s** | **16 min 58 s** | 1 min 57 s | 5 min 28 s | 2 min 57 s | **nicht erfüllt** (macos-x64, windows-x64) |
| [38815668770](https://github.com/domdewom2/openspider/actions/runs/38815668770) | `1857385` spec(10) AGENT-060-062 | 6 min 25 s | 10 min 26 s | **15 min 21 s** | 0 min 25 s | 1 min 05 s | 1 min 07 s | **nicht erfüllt** (windows-x64) |
| [38815952548](https://github.com/domdewom2/openspider/actions/runs/38815952548) | `ba006c5` Roadmap nach 1.0 | 6 min 28 s | 9 min 28 s | **16 min 45 s** | 0 min 32 s | 1 min 29 s | 1 min 06 s | **nicht erfüllt** (windows-x64) |
| [38818554473](https://github.com/domdewom2/openspider/actions/runs/38818554473) | `7f5c4b2` spec(01) CRAWL-145/146 | 6 min 13 s | 10 min 31 s | **15 min 09 s** | 0 min 30 s | 1 min 11 s | 1 min 10 s | **nicht erfüllt** (windows-x64) |
| [38818568488](https://github.com/domdewom2/openspider/actions/runs/38818568488) | `4112486` plan(M2) CRAWL-145/146 | 6 min 39 s | 7 min 30 s | **15 min 36 s** | 0 min 34 s | 1 min 05 s | 0 min 51 s | **nicht erfüllt** (windows-x64) |
| [38818635508](https://github.com/domdewom2/openspider/actions/runs/38818635508) | `cb3a58a` spec(01) CRAWL-147 | 7 min 05 s | 9 min 13 s | 12 min 25 s | 0 min 31 s | 1 min 23 s | 0 min 40 s | erfüllt |

Die ersten drei Zeilen sind die **drei grünen main-Läufe nach dem Merge von M1-N18**; alle drei liegen auf allen Plattformen unter der Grenze (windows-x64 14 min 21 s, 12 min 00 s, 14 min 28 s). Die Zeilen 88fd424 (Merge M1-N20, Welle 7) und 1857385, ba006c5, 7f5c4b2, 4112486, cb3a58a sind die sechs main-Läufe, die das Fable-Gate M1 nennt. Der Merge von M1-N21 (6d8237f) hat keinen eigenen main-Lauf; sein Stand ist im Lauf auf 1857385 enthalten (6d8237f ist Vorfahre).

Auswertung:

- **windows-x64:** 12 min 00 s bis 16 min 58 s; über 15 min in 5 von 10 Läufen (15:09, 15:21, 15:36, 16:45, 16:58), in 5 der 6 jüngsten (nur cb3a58a mit 12:25 liegt darunter). Die Läufe 317920a, 741ae32 und 807854e (14:21, 14:28, 14:36) blieben nur knapp unter der Grenze (Reserve 0:24 bis 0:39).
- **macos-x64:** 7 min 29 s bis 17 min 04 s; einmal über 15 min (88fd424: 17:04; dort auch Clippy 5:28 gegenüber sonst 0:46 bis 1:39), sonst 7:29 bis 13:16.
- **macos-arm64:** 5 min 05 s bis 7 min 54 s, unkritisch.
- **Streuung:** Dieselbe Plattform schwankt zwischen aufeinanderfolgenden Läufen um mehrere Minuten (windows-x64 12:00 bis 16:58, macos-x64 7:29 bis 17:04). Die Läufe 1857385 bis cb3a58a gehören zu Commits, die gegenüber dem Merge von M1-N21 (6d8237f) nur Dateien unter `docs/` ändern; dennoch reicht windows-x64 von 12:25 bis 16:45. Nach K-81 liegt die Ursache in Kompilierdauer, Cache-Treffer von `rust-cache` und Runner-Hardware, nicht in den Tests; belegt ist das hier nicht, weil Kompilieren und Ausführung nicht getrennt gemessen wurden. M1-N23 liefert die Trennung (Aufgabe (0): Zeiten aus den Log-Zeitstempeln) und die Messung der reinen Testausführung.
- **Bewertung:** Nach der bisherigen Definition ist A1 **nicht erfüllt**. Eine Überschreitung von 15 min ist ein Befund für den Planer und liegt in 5 von 10 Läufen vor. Der Beleg nach K-81 (Testausführung < 15 min, Ziel < 12 min, in der CI per `timeout-minutes` erzwungen) folgt mit M1-N23; M1-N26 trägt die main-Läufe nach Welle 7c nach.

Zur Einordnung der Bewertung vor Welle 7: Der N18-Branchlauf ([38680316333](https://github.com/domdewom2/openspider/actions/runs/38680316333), `workflow_dispatch` auf `m1/m1-n18`, Commit fd8f57a) lag bei 5 min 58 s (macos-arm64), 10 min 23 s (macos-x64) und 14 min 16 s (windows-x64); der main-Lauf vor N18 (Welle 6) bei 14 min 42 s auf windows-x64. Seit dem Merge von Welle 7 (88fd424) liegt windows-x64 in 5 von 6 Läufen über 15 min; ob die zusätzlichen Tests (M1-P32, M1-P33, M1-N19) oder die Schwankung von Runner und Cache die Ursache sind, ist nicht gemessen (Aufgabe (0) von M1-N23).

Weitere Jobs im Lauf [38818635508](https://github.com/domdewom2/openspider/actions/runs/38818635508) (cb3a58a, vor Welle 7c, 16 Jobs, alle grün): `cargo-deny` damals nur mit licenses, bans, sources (`advisories` war auf main rot, behoben mit M1-N25).

## A2 Spec-Abdeckung

`docs/plan/M1-status.json` ist **unverändert** das Artefakt `spec-coverage` des jüngsten grünen main-Laufs nach Welle 7c ([38868822232](https://github.com/domdewom2/openspider/actions/runs/38868822232), Commit 5f65ffc, 16.10.2026; `gh run download 38868822232 -n spec-coverage`, Datei byte-gleich übernommen). Der Job erzeugt sie mit `cargo test --workspace --exclude openspider-desktop --no-fail-fast --locked` und `os-parity coverage --packages docs/plan/M1-packages.json --test-output out.txt --parity-report target/parity/report.json --evidence tests/parity/evidence.toml --write docs/plan/M1-status.json --strict` (Exit 0). Ergebnis: **242 IDs – implemented 237, verified 5, deferred 0, open 0**; Zusatz-IDs (`extra`, nicht gewertet): **7** (CRAWL-075, CRAWL-128, OUT-084, **PERF-002** mit 31 Tests aus `os-loadtest` und Paketen M1-P32, M1-N19, M1-N21, M1-N22, M1-N26, TECH-008, TECH-030, TECH-068); ignorierte Tests ohne gültigen Grund: 0; Warnungen: 0. Die Testausgabe des Laufs zählt 2 046 bestandene, 0 fehlgeschlagene und 36 ignorierte Tests (cb3a58a: 2 037 / 0 / 36; die neun zusätzlichen Tests kommen mit den Commits seit cb3a58a, also Welle 7c, hinzu).

Gegenüber der Datei aus M1-N24 (Stand cb3a58a): gleiche Statuszahlen und 1 872 Test-Zuordnungen; die Pakete M1-N22 (PERF-002, Diagnose des Nachladens) und M1-N26 (Nachweis A4, Eigentümer von PERF-001, PERF-003, PERF-005) stehen jetzt in der Paketliste dieser drei IDs und von PERF-002 (die Pakete der übrigen IDs sind unverändert, die Datei enthält Pakete bis M1-N22; M1-N26 schreibt keine Tests). Geändert gegenüber M1-N24 sind genau die Einträge von PERF-001, PERF-003, PERF-005 und der Zusatz-ID PERF-002.

Benchmark-Ignores: 18 Tests in 12 Dateien tragen den Ignore-Grund `Benchmark …` (`grep -rn 'ignore = "Benchmark' crates --include='*.rs'` ohne `crates/os-parity/tests/data`: `os-engine` `lifecycle_list.rs` 1, `lifecycle_pause.rs` 1, `perf_store.rs` 2; `os-export` `links_bench.rs` 1, `pipeline_bench.rs` 1, `sitemap_gen.rs` 1, `xlsx_cells.rs` 1; `os-fetch` `http_status.rs` 2; `os-parse` `pixel_width.rs` 1, `sitemap_gzip.rs` 1; `os-store` `bench_writer.rs` 5; `os-testsite` `synthetic.rs` 1). Sie zählen für den Status neutral (Statusregeln 5.4 Punkt 4), stehen nicht unter den ungültigen Ignores und sind in A4 mit 18 von 18 bestanden belegt.

Beobachtung ohne Folge für den Status: Für `AGENT-001` steht der Test `os-mcp` › `error.rs::tests::usage_errors_exit_with_two_everything_else_with_one` in der Datei mit `no_result` (in der Datei aus N18: `ok`). Die Testausgabe des Laufs enthält ihn als bestanden; die Zeile trägt das Präfix `openspider mcp: ` (Ausgabe des Binärs vor der Zeile des Testharness); der Parser von `os-parity coverage` erkennt sie vermutlich deshalb nicht (Ursache nicht untersucht). Der Status von AGENT-001 bleibt `implemented` (41 von 43 Tests mit Ergebnis `ok`, einer `no_result`, einer ignoriert mit `M2:`). Eine Anpassung des Parsers gehört nicht in M1-N24 (Befund an den Planer).

Stichprobe der dünn belegten IDs (Nachtrag Welle-1-Sichtung):

| ID | Status | Tests mit Ergebnis ok |
|---|---|---|
| OUT-231 | implemented | 4: `paths_registry.rs::default_crawl_folder_is_the_one_defined_in_out_227`, `paths_registry.rs::fixture_crawl_in_openspider_home_is_listed_found_and_opened_from_the_crawl_folder`, `paths_registry.rs::changing_the_storage_location_leaves_existing_crawls_in_place`, `paths_registry.rs::default_crawl_folder_is_separate_from_the_override_and_never_inside_it` |
| ONPAGE-006 | implemented | 1: `issues_catalog.rs::end_to_end_samples` |

Mit Paritätsbeleg (`verified`, Regeln und Belegzuordnung in `tests/parity/evidence.toml`, nur IDs, deren Spec-Umfang die verglichenen Spalten vollständig abdecken und die ohne Referenzeinschränkung SF Free belegt sind): **5** – ONPAGE-012, ONPAGE-120, ONPAGE-130, TECH-010, TECH-011.

## A3 Parität (SF 24.3 Free)

Referenz: SF 24.3 Free (macOS arm64, englische Oberfläche), exportiert am 14.10.2026; **5 von 5 Sites mit Status `present`** (Anforderung >= 4), 39 Exporte laut Manifest, davon 38 verglichen und 1 übersprungen. Lauf: `os-parity run --all --openspider target/debug/openspider --out target/parity` unter `lockf` (Bericht: `docs/plan/M1-acceptance/parity/report.md`, `report.json`), Stand M1-N18 (Basis `origin/main` 86bace1 plus die Änderungen dieses Pakets).

Ergebnis: **38 von 38 Vergleichen erfüllen die Schwelle aus A3** (URL-Menge identisch nach dokumentierten Abweichungen, Zellübereinstimmung >= Schwelle), Exit-Code **0**. Vorher (M1-P29): 26 von 38. Alle Vergleiche liegen bei 100,000 % der verglichenen Zellen; verbleibende Unterschiede sind ausschließlich die unten aufgeführten, begründeten Abweichungen und die Referenzeinschränkungen SF Free.

| Site | Export | Zellen | Übereinstimmung | Schwelle | URL-Menge | Ergebnis |
|---|---|---:|---:|---:|---|---|
| out-basic-site | internal_all | 894 | 100.000 % | 99 % | identisch | ok |
| out-basic-site | external_all | 5 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | response_codes_all | 152 | 100.000 % | 99 % | identisch | ok |
| out-basic-site | links_all | 100 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | all_inlinks | 1067 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | redirect_chains | 0 | 100.000 % | 97 % | identisch | ok |
| out-basic-site | crawl_overview | 124 | 100.000 % | 100 % | identisch | ok |
| parity-onpage | internal_all | 4761 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | external_all | 44 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | response_codes_all | 911 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | url_all | 945 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | page_titles_all | 756 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | meta_description_all | 756 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | h1_all | 588 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | h2_all | 588 | 100.000 % | 99 % | identisch | ok |
| parity-onpage | content_all | 336 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | images_all | 57 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | all_inlinks | 3322 | 100.000 % | 97 % | identisch | ok |
| parity-onpage | crawl_overview | 118 | 100.000 % | 100 % | identisch | ok |
| parity-response | internal_all | 2158 | 100.000 % | 99 % | identisch | ok |
| parity-response | external_all | 24 | 100.000 % | 97 % | identisch | ok |
| parity-response | response_codes_all | 407 | 100.000 % | 99 % | identisch | ok |
| parity-response | all_inlinks | 1064 | 100.000 % | 97 % | identisch | ok |
| parity-response | all_error_inlinks | 209 | 100.000 % | 97 % | identisch | ok |
| parity-response | redirect_chains | 281 | 100.000 % | 97 % | identisch | ok |
| parity-response | redirects_to_errors | – | – | – | – | übersprungen: außerhalb des M1-Umfangs: Report Redirects to Errors (OUT-084) ist P1 und kein M1-Umfang (M1.md 7.2); OpenSpider hat in M1 nur Redirects:All Redirects und Redirects:Redirect Chains |
| parity-response | crawl_overview | 121 | 100.000 % | 100 % | identisch | ok |
| parity-synthetic | internal_all | 20652 | 100.000 % | 99 % | identisch | ok |
| parity-synthetic | links_all | 3410 | 100.000 % | 97 % | identisch | ok |
| parity-synthetic | all_inlinks | 50105 | 100.000 % | 97 % | identisch | ok |
| parity-synthetic | crawl_overview | 125 | 100.000 % | 100 % | identisch | ok |
| parity-technical | internal_all | 6065 | 100.000 % | 99 % | identisch | ok |
| parity-technical | canonicals_all | 1272 | 100.000 % | 99 % | identisch | ok |
| parity-technical | directives_all | 855 | 100.000 % | 99 % | identisch | ok |
| parity-technical | hreflang_all | 1696 | 100.000 % | 97 % | identisch | ok |
| parity-technical | links_all | 950 | 100.000 % | 97 % | identisch | ok |
| parity-technical | sitemaps_all | 0 | 100.000 % | 97 % | identisch | ok |
| parity-technical | all_inlinks | 4849 | 100.000 % | 97 % | identisch | ok |
| parity-technical | crawl_overview | 121 | 100.000 % | 100 % | identisch | ok |

Hinweise zur Aussagekraft:

- Die Zellübereinstimmung bezieht sich auf die verglichenen Zellen **nach** den dokumentierten Abweichungen (2662 entschuldigte Zellen/Zeilen insgesamt, davon 115 Referenzeinschränkungen SF Free; Tabellen unten). Offene Befunde gibt es keine.
- `redirect_chains` bei `out-basic-site` vergleicht 0 Zellen: beide Seiten haben keine Kette mit mindestens zwei Sprüngen.
- `Redirects to Error(s)` (OUT-084, P1) gehört nicht zum M1-Umfang und ist im Manifest mit `skip` übersprungen; `All Error Inlinks` wird über `Response Codes:Internal & External:All Error Inlinks` verglichen.
- Zähler im Crawl Overview: Die Schwelle für `crawl_overview` ist 100 % (Zähler der M1-Filter identisch nach Abweichungen). `parity-response` Internal Redirect Chain = 9 ohne Eintrag (K-66, vorher 7 mit DEV-033).
- Der CI-Job `parity` (macos-14, Lauf [38680316333](https://github.com/domdewom2/openspider/actions/runs/38680316333)) endet mit Exit 0: Der Schritt „Paritätslauf (os-parity run --all)“ ist **nicht mehr `continue-on-error`** (entfernt in M1-N18). A3 wird damit in der CI erzwungen; ein Rückfall bricht den Job `parity`. Bericht weiterhin als Artefakt `parity-report` und in der Job-Zusammenfassung.
- Die Schwellen gelten je Export und Site (nicht gepoolt); die `% of Total`-Spalte ist abgeleitet und wird nicht verglichen.

### Begründete Paritätsabweichungen (`tests/parity/deviations.toml`, DEV-nnn)

Jede steht mit Spec-ID und Kennzeichen in der Datei; „Anzahl“ = entschuldigte Zellen bzw. Zeilen im Lauf. **Neu in M1-N18:** DEV-035, DEV-036 (K-71). **Gestrichen:** DEV-021 und DEV-022 (K-56, K-71: ersetzt durch DEV-035/-036), DEV-033 (K-66: Redirect Chain mit Schleifen zählt seit M1-N18 wie die Referenz). **Kennzeichen geändert:** DEV-005 von `Paritaetsabweichung` auf `OS-Festlegung` (K-67). Seit M1-P31 bereits gestrichen: DEV-001, DEV-002, DEV-015, DEV-019, DEV-020, DEV-032. Kein Grund enthält mehr „Entfaellt …“ oder „Offen fuer den Planer“.

| Kennung | Spec | Kennzeichen | Anzahl | Begründung |
|---|---|---|---:|---|
| DEV-003 | ONPAGE-011 | OS-Festlegung | 2326 | Nicht-HTML-URLs sowie Weiterleitungen, Fehlerseiten (4xx/5xx) und URLs ohne Antwort haben leere Element-, Laengen- und Pixelspalten, auch wenn ihr Koerper <title>/<h1> enthaelt (ONPAGE-011, A-5, K-60, Befund N-01), und eine Seite ohne Element leere Wertspalten (ONPAGE-008); die Referenz schreibt 0. |
| DEV-004 | ONPAGE-013 | OS-Festlegung | 85 | Nicht-HTML-URLs sowie Weiterleitungen, Fehlerseiten (4xx/5xx) und URLs ohne Antwort: Word Count und Text Ratio leer (ONPAGE-013, K-60, Befund N-01); die Referenz schreibt 0. |
| DEV-005 | ONPAGE-065 | OS-Festlegung | 9 | ONPAGE-065 (K-67): OpenSpider meldet die tatsaechliche Ursache, Connection Refused bei abgelehntem Verbindungsaufbau und Connection Timeout bei ausbleibender Antwort; die Referenz meldet fuer den geschlossenen Port localhost:1 Connection Timeout. |
| DEV-006 | OUT-031 | Paritaetsabweichung | 7 | Status Code ist in den Link-Exporten leer, wenn das Ziel nicht abgerufen wurde (OUT-031, Not Crawled); die Referenz schreibt 0 fuer robots.txt-blockierte Ziele. |
| DEV-006b | OUT-031 | Paritaetsabweichung | 2 | wie DEV-006 (Fehler-Inlinks auf nicht abgerufene Ziele). |
| DEV-007 | OUT-031 | Paritaetsabweichung | 12 | Rel = alle rel-Werte des Elements in Originalreihenfolge (OUT-031); die Referenz laesst rel="stylesheet" leer. |
| DEV-008 | OUT-031 | OS-Festlegung | 5 | Follow=False auch bei Meta/X-Robots nofollow der Quellseite (OUT-031); die Referenz bewertet nur das Link-Attribut. |
| DEV-009 | OUT-031 | Paritaetsabweichung | 37 | Die Spec kennt Link Origin nur HTML/Rendered HTML (OUT-031); die Referenz schreibt HTTP fuer Links aus Antwort-Headern und Weiterleitungen. |
| DEV-010 | TECH-130 | OS-Festlegung | 4 | K-07: Selbstlinks zaehlen im Links-Tab nicht (TECH-130); die Referenz zaehlt sie. Betrifft die Seite mit Selbstlink. |
| DEV-011 | TECH-130 | OS-Festlegung | 8 | K-07 (wie DEV-010), Seiten mit Selbstlink in out-basic-site. |
| DEV-012 | TECH-051 | OS-Festlegung | 3 | Population des Directives-Tabs: alle internen URLs mit HTTP-Antwort, jeder Status (TECH-051); die Referenz fuehrt Weiterleitungen und Fehlerseiten nicht. |
| DEV-013 | TECH-051 | OS-Festlegung | 5 | wie DEV-012: Zaehler des Abschnitts Directives (Population aller internen URLs mit Antwort). |
| DEV-014 | ONPAGE-024 | OS-Festlegung | 8 | Klassifizierung ONPAGE-024: fehlender Content-Type, keine Antwort und robots.txt-Sperre sind Unknown; die Referenz fuehrt Antworten ohne Content-Type (z. B. Weiterleitungen) unter Other. Der Filter Other ist P1 (M1.md 7.2). |
| DEV-016 | ONPAGE-166 | OS-Festlegung | 1 | PDF-Inhalte (Word Count, Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2); in M1 nur Titel und Keywords aus den PDF-Eigenschaften. |
| DEV-017 | OUT-081 | OS-Festlegung | 8 | OUT-081: die Hop-Spalten n beschreiben die Spruenge (Redirect Type n, Redirect URL n, Status Code n, Status n); die Referenz haengt als letzten Hop das Endziel an. |
| DEV-018 | ONPAGE-171 | OS-Festlegung | 3 | IMG Inlinks (OpenSpider-Erweiterung) zaehlt auch Hyperlinks auf Bilder (ONPAGE-170 Nr. 2); die Referenz zaehlt nur eingebettete Bilder. |
| DEV-023 | TECH-064 | OS-Festlegung | 2 | Der Filter Refresh umfasst Meta Refresh und den Refresh-Header (TECH-064); die Referenz zaehlt den Refresh-Header im Response-Codes-Filter Redirection (HTTP Refresh). |
| DEV-024 | ONPAGE-166 | OS-Festlegung | 1 | PDF-Inhalte (Content-Tab-Zeile) sind P1 (ONPAGE-166, M1.md 7.2), siehe DEV-016. |
| DEV-025 | TECH-001 | OS-Festlegung | 2 | Indexability Status nennt genau den ersten zutreffenden Grund in der Pruefreihenfolge (TECH-001); die Referenz nennt bei noindex und fremdem Canonical beide, kommagetrennt. |
| DEV-026 | TECH-064 | OS-Festlegung | 1 | Die Spalte Meta Refresh im Directives-Tab zeigt auch den Wert des Refresh-Headers (TECH-064: Refresh umfasst Meta und Header); die Referenz laesst sie bei reinem Header-Refresh leer. |
| DEV-027 | TECH-071 | OS-Festlegung | 1 | Population des Hreflang-Tabs: auch gecrawlte hreflang-Ziele (TECH-071); die Referenz fuehrt das umgeleitete Ziel (301) nicht. |
| DEV-028 | ONPAGE-170 | OS-Festlegung | 1 | Grundmenge des Images-Tabs: auch per Bild-Referenz entdeckte Bild-URLs mit Fehlerantwort (Images-Intro, ONPAGE-170); die Referenz fuehrt das nicht abrufbare Bild (404) nicht. |
| DEV-029 | ONPAGE-170 | OS-Festlegung | 1 | wie DEV-028: Zaehler des Abschnitts Images enthaelt das per Referenz entdeckte, nicht abrufbare Bild. |
| DEV-030 | OUT-081 | OS-Festlegung | 10 | Bei einer Weiterleitungsschleife ist Final Address leer und Final Status = Redirect Loop (OUT-081); die Referenz nennt die letzte Station der Schleife samt ihrem Status. Das gilt auch fuer den Selbst-Refresh (Weiterleitung auf sich selbst, K-57): die Referenz nennt die Seite selbst mit 200/OK. |
| DEV-031 | OUT-081 | OS-Festlegung | 1 | Meta Refresh und Refresh-Header gelten nur mit Verzoegerung > 0 als temporaer (OUT-081); die Referenz wertet den Meta Refresh ohne Verzoegerung als temporaer. |
| DEV-034 | TECH-071 | OS-Festlegung | 1 | wie DEV-027: Die Population des Hreflang-Tabs enthaelt auch gecrawlte hreflang-Ziele (TECH-071), hier das umgeleitete Ziel /hl/es-old/ (301); die Referenz zaehlt nur interne Seiten mit 2xx. |
| DEV-035 | TECH-015 | OS-Festlegung | 2 | K-71: Die Grundgesamtheit der Referenz heisst 'Internal HTML or PDF pages with 2xx response' und enthaelt jede PDF; die Canonicals-Grundmenge von OpenSpider fuehrt Nicht-HTML-Ressourcen nur mit mindestens einer HTTP-Canonical (TECH-011) und nimmt eine PDF ohne Link-Header nicht in Missing auf (TECH-015). PDFs als Seiten sind P1 (ONPAGE-166). parity-onpage hat eine PDF ohne Canonical (/ctype/t-pdf.pdf): Referenz 85/81, OpenSpider 84/80. |
| DEV-036 | TECH-133 | OS-Festlegung | 1 | K-07/K-71 (wie DEV-010): Selbstlinks zaehlen im Links-Tab nicht, eine Seite nur mit Selbstlink hat 0 Outlinks (TECH-130, TECH-133); die Referenz zaehlt sie mit. Betrifft /links/self-only.html: Referenz 1, OpenSpider 2. |

### Referenzeinschränkungen SF Free (`SF-FREE-nn`, K-53) – zählen nicht als Paritätsabweichung

Die Free-Version sperrt die Spider-Konfiguration: Hreflang und Pagination wurden nur gespeichert, verlinkte XML-Sitemaps nicht gecrawlt, robots.txt-Sitemaps nicht abgerufen. `tests/parity/profiles/parity.osconfig` bildet das nach (`spider.hreflang.crawl = false`, `sitemaps.crawl_linked = false`).

| Kennung | Spec | Anzahl | Begründung |
|---|---|---:|---|
| SF-FREE-01 | TECH-210 | 110 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): verlinkte XML-Sitemaps und robots.txt-Sitemaps werden nicht abgerufen; die Referenz listet im Tab Sitemaps dennoch alle internen URLs, OpenSpider ohne verarbeitete Sitemap keine (TECH-211: Tab leer). |
| SF-FREE-02 | TECH-210 | 5 | SF-Free-Einschraenkung (Spider-Konfiguration gesperrt): ohne Sitemap-Crawl zaehlt die Referenz alle URLs im Abschnitt Sitemaps, OpenSpider 0 (TECH-211). |

Betroffene Filter (Sitemaps-Tab und -Filter wie Orphan URLs / URLs not in Sitemap / Non-Indexable URLs in Sitemap, Hreflang-Filter Non-200 Hreflang URLs / Missing Return Links) erhalten **keinen** Status `verified`; ihr Beleg bleibt der spec-abgeleitete Fixture-Test (A2). `tests/parity/evidence.toml`: Die Spalten `HTTP rel="next" 1` und `HTTP rel="prev" 1` stehen nicht mehr in der Regel für TECH-010/-011 (Fable-Gate 5), `verified` beruht damit nur auf tatsächlich verglichenen Spalten.

### Profilangleichungen an die tatsächliche Referenzkonfiguration

Beide Einstellungen im Paritätsprofil (`tests/parity/profiles/parity.osconfig`) sind entschieden:

- `robots.unreachable_policy = "allow_all"`: **bewusste OS-Festlegung** (K-55, RFC 9309, Abschnitt 2.3.1.4: „Host sperren“ als OpenSpider-Voreinstellung). Die Referenz crawlt URLs hinter nicht erreichbarer robots.txt (geschlossener Port `localhost:1`) trotzdem; das Profil stellt die Referenz mit der dafür vorgesehenen Option nach (CRAWL-100, Q30). Keine Paritätsabweichung, kein DEV-Eintrag.
- `advanced.respect_self_ref_meta_refresh = false`: seit K-57 Standard (TECH-068), das Profil nennt ihn nur zur Klarheit. Ein Selbst-Refresh wirkt nur mit der Option auf die Indexierbarkeit, wie in der Referenz.

### Entscheidungen K-66 … K-71 (Fable-Recheck Welle 6, 84/100; Planer 15.10.2026)

| Nr. | Gegenstand | Entscheidung | Status |
|---|---|---|---|
| K-66 | Redirect Chain mit Schleifen (A-19, DEV-033, ONPAGE-060/-061) | Parität: Redirect Chain = interne URL mit >= 2 Hops, auch wenn die Kette in einer Schleife endet; eine Selbst-Weiterleitung (1 Hop) steht nur in Redirect Loop. Vorlauf einer Schleife (`/tail`) gleich (Annahme A-19b, nicht belegt) | **erledigt durch M1-N18**: `os-analyze` `tabs/response_codes.rs` (Makro `redirect_chain!`: `u.redirect_target_id <> u.id` statt `NOT reaches_loop!()`), Tests `tab_response_codes.rs`, Goldens `response_codes__redirect_chain.csv` / `__internal_redirect_chain.csv` (nur URLs hinzugefügt: `/loop1`, `/loop2`, `/qloop`, `/tail`); `parity-response` Internal Redirect Chain 9 ohne Eintrag; DEV-033 gestrichen |
| K-67 | DEV-005 (ONPAGE-065) | OS-Festlegung bleibt: `Connection Refused` bei abgelehntem Verbindungsaufbau, `Connection Timeout` bei ausbleibender Antwort | **erledigt durch M1-N18**: DEV-005 mit Kennzeichen `OS-Festlegung` |
| K-68 | A-35b (ONPAGE-141) | + 1 je `<li>` im Inhaltsbereich statt + 1 je `<a href>` | **Spec angeglichen (Planer)**; Umsetzung von M1-P31 bleibt, keine Codeänderung |
| K-69 | K-57, Header-Fall | `Refresh`-Header ohne URL ist keine Weiterleitung; Selbst-Weiterleitung = Meta Refresh ohne URL oder Refresh auf die eigene URL | **Spec angeglichen (Planer)**; Umsetzung von M1-P31 bestätigt, keine Codeänderung |
| K-70 | A-4/K-58 (ONPAGE-005) | Kollation `OS_TEXT` bleibt (Annahme A-4b); Verifikation und indexfähige Variante nach M2 (M2-S04) | **Spec angeglichen (Planer)**, keine Codeänderung |
| K-71 | Canonicals-Grundmenge (PDF ohne Canonical) und Links › Pages Without Internal Outlinks | OS-Festlegung für M1 (PDFs als Seiten sind P1, ONPAGE-166; Selbstlinks zählen nicht, K-07) | **erledigt durch M1-N18**: DEV-021/-022 gestrichen, DEV-035 (TECH-015, `parity-onpage`, delta -1) und DEV-036 (TECH-133, `parity-technical`, delta +1) eingetragen; Spec TECH-015/-133 vom Planer ergänzt |

Weitere Empfehlungen des Rechecks, von M1-N18 erledigt: `continue-on-error` im Job `parity` entfernt; `evidence.toml` geschärft; Kommentare in `parity.osconfig` an K-55/K-57 angepasst; Berichte und `M1-status.json` neu erzeugt; A4 aus den N14-Berichten übernommen.

**TECH-012 (Flake `contains_canonical_counts_html_and_header_canonicals`):** M1-P31 hat keinen reinen Testfehler gemeldet. 50 Wiederholungen des Tests auf dem Merge-Stand (Basis 86bace1 plus N18-Änderungen, Debug-Build, einzeln und mit allen 22 Tests der Datei `tab_canonicals.rs`): 50 von 50 bestanden, 0 Fehlläufe. Aufgenommen ist allein der Vorschlag aus dem P31-Bericht, die Assert-Meldung zu erweitern (Differenzmengen, Endzustand des Crawls, Zeilen des Filters „All“, Abrufprotokoll der Testsite); die Behauptungen sind unverändert. **Grenze:** Der Fehlertext des Abrufs (`FetchOutcome::NoResponse.detail`) wird weder gespeichert noch an den Test durchgereicht; er ist aus `tab_canonicals.rs` nicht erreichbar. Tritt der Flake wieder auf, zeigt die neue Meldung, welche URL fehlt und ob der Abruf stattfand. Ein Zugriff auf `detail` bräuchte eine Änderung in `os-engine` (Befund an den Planer, nur falls der Flake wiederkehrt).

## Befunde und Nachbesserungsaufträge aus dem Paritätslauf

**Stand M1-N18: alle Befunde sind erledigt, es gibt keine offenen Nachbesserungsaufträge aus dem Paritätslauf.** Der Text der Befunde ist der Bericht von M1-P29 (Stand vor Welle 6), jeweils ergänzt um den Status. Klassifizierung nach `docs/plan/M1-packages.json` (P29): *Implementierungsfehler* → Nachbesserungsauftrag (kein Fix außerhalb des Besitzes von P29, keine Ausnahme in `deviations.toml`); *Spec-Abweichung/Annahme/OS-Festlegung* → Eintrag in `deviations.toml` (siehe oben). Alle Beispiele sind mit `os-parity run --site <site> --openspider target/debug/openspider` reproduzierbar; „Zellen“ = abweichende Zellen im Lauf.

### Nachbesserungsaufträge (Implementierungsfehler bzw. offene Abweichungen)

**N-01 · Fehlerseiten (4xx/5xx) liefern Inhaltswerte** (Priorität hoch) – Spec ONPAGE-009 (A-5: Inhalt von Fehlerseiten wird nicht ausgewertet), ONPAGE-011 (Internal-Werte = Werte der Element-Tabs), ONPAGE-013. Crate: `os-parse`/`os-engine` (Extraktion nur bei 2xx) oder `os-analyze` `tabs/internal.rs` (Spalten nur bei 2xx); Ursachenort nicht untersucht.
Status: **erledigt durch M1-P31** (K-60: Internal-Elementwerte, Word Count und Text Ratio nur für interne HTML-Seiten mit 2xx; für Fehlerseiten leer, DEV-003/-004 erweitert). Paritätslauf M1-N18: `internal_all` bei allen Sites >= 99 %.
Minimales Beispiel: Fixture `parity-response`, `/e410` (Status 410, Body mit `<title>Error 410</title>` und `<h1>Error 410</h1>`): `Internal:All` liefert `Title 1` = „Error 410“, `Title 1 Length` 9, `Title 1 Pixel Width` 83, `H1-1` = „Error 410“, `H1-1 Length` 9, `Word Count` 2, `Text Ratio` 6.9; die Referenz lässt Titel/H1 leer und schreibt 0.
Umfang: 24 Seiten in 4 Sites (`out-basic-site` `/missing.html`; `parity-response` `/e400 … /e503`, `/s999`; `parity-synthetic` 14 Seiten; `parity-technical` `/canon/gone-404.html`), je 24 Zellen in den Spalten Title 1, Title 1 Length, H1-1, H1-1 Length, Text Ratio, dazu Pixel Width und Word Count. Das ist der häufigste Grund, warum `internal_all` bei `out-basic-site`, `parity-response`, `parity-synthetic` und `parity-technical` unter 99 % bleibt.

**N-02 · Page-Titles- und Meta-Description-Export ohne Pixel-Width-Spalten** – Spec ONPAGE-090 und ONPAGE-100 (Spaltenlisten enthalten `Title 1/2 Pixel Width` bzw. `Meta Description 1/2 Pixel Width`), UI-038. Datei: `crates/os-analyze/src/tabs/page_titles.rs` (Z. 30–50) und `meta_description.rs`: die vier Pixel-Spalten sind mit `default_visible = false` definiert, der Standardexport lässt sie weg (Internal:All zeigt sie).
Status: **erledigt durch M1-P31** (K-60: Pixel-Spalten im Standardexport Page Titles/Meta Description). `page_titles_all` und `meta_description_all` 100,000 %.
Minimales Beispiel: `openspider --crawl http://127.0.0.1:8902/ --export-tabs "Page Titles:All"` (Fixture `parity-onpage`) → die Kopfzeile hat keine `Title 1 Pixel Width`; die Referenz hat sie (Wert z. B. 332 für `/`). 336 Zellen (84 Zeilen × 4 Spalten); `page_titles_all` und `meta_description_all` stehen deshalb bei 77,8 %.

**N-03 · Word Count weicht bei Seiten mit Links ab** (Priorität hoch) – Spec ONPAGE-141 (Tokenisierung an Unicode-Leerraum, A-35), ONPAGE-013. Beobachtung: Die Referenz zählt auf jeder geprüften Seite genau so viele Wörter mehr, wie die Seite Link-Elemente (`<a>`) im Inhaltsbereich hat – nicht so viele, wie Wörter in den Links stehen. `parity-synthetic` `/s1/p1.html` (H1 „Seite 1“ = 2 Wörter, Absatz 90 Wörter, 10 × `<li><a>Seite n</a></li>` = 20 Wörter): OpenSpider 112 (= 90 + 2 + 20, nach Spec korrekt), Referenz 122; `parity-onpage` `/` (8 Links mit zusammen 11 Wörtern): 79 gegen 87; `parity-technical` `/dir/` (13 Links in `<main>`, 2 in `<nav>`): 81 gegen 94 (Differenz 13); `parity-response` `/`: 191 gegen 234 (43 Links).
Umfang: 430 Zellen (`parity-synthetic` 393 von 393 HTML-Seiten), `Word Count` in `internal_all` und `content_all`, dazu Crawl Overview Content › Low Content Pages (parity-response 9 statt 8). Entscheidung: Spec/Annahme A-35 ändern oder die Abweichung als OS-Festlegung bestätigen (dann Eintrag in `deviations.toml`).
**→ Entschieden 14.10.2026 (K-59), präzisiert 15.10.2026 (K-68): Parität, Annahme A-35b (+ 1 je `<li>` im Inhaltsbereich), ONPAGE-141 ergänzt.** Status: **erledigt durch M1-P31** (zellgenau auf allen 211 HTML-Seiten; `<li>` ohne Link ohne Beleg, M2-S06).

**N-04 · Pixelbreiten außerhalb der Toleranz** (UI-038: ≤ 3 %) – 39 Zellen in `internal_all` (`Title 1 Pixel Width` 28, `Meta Description 1 Pixel Width` 11): `parity-onpage` `/titles/pixel-i70.html` (70 × „i“, 20 px): Referenz 350, OpenSpider 311 (−11,1 %); `/titles/pixel-W60.html` (60 × „W“): 1080 gegen 1133 (+4,9 %); `/meta/dup-case.html` (Meta Description): 735 gegen 802; `out-basic-site` `/faq.html` (Titel): 142 gegen 148 (+4,2 %); `parity-response` `/meta-refresh-self.html`: 246 gegen 238; `parity-synthetic` Meta Description der 10 Seiten `/s*/p*.html`: 151 gegen 156 (+3,3 %). Auffällig: Die Referenzbreite der beiden Wiederholungsfixtures ist ein Vielfaches ganzer Pixel je Zeichen (i = 5 px, W = 18 px; OpenSpider 4,44 und 18,88 px). Prüfauftrag: Schriftprofil und Rundung von UI-038 (⚖ Annahmen) gegen die Golden-Werte der Referenz nachziehen.
Status: **erledigt durch M1-P31** (K-64, Kalibrierung der Pixelbreiten-Engine, UI-038: alle Werte der fünf Paritäts-Sites <= 3 %).

**N-05 · Folder Depth bei doppeltem Schrägstrich** – Spec ONPAGE-014 / A-8: `/urls/a//b.html` → Referenz 2, OpenSpider 3 (das leere Segment zählt mit). 1 Zelle (`parity-onpage`). Minimal: leere Pfadsegmente bei der Zählung überspringen.
Status: **erledigt durch M1-N16** (K-60, `os-url`: leere Pfadsegmente zählen nicht für Folder Depth).

**N-06 · Hash für Nicht-HTML und leere Antworten** – Spec ONPAGE-018 („MD5 … der HTML-Seite“). OpenSpider schreibt `Hash` für jede abgerufene URL (Bilder, CSS, JS, Text, SVG, JSON, 301-Antworten mit leerem Körper: `d41d8cd98f00b204e9800998ecf8427e`); die Referenz nur für HTML-Seiten. Beispiel: `out-basic-site` `/assets/logo.png`, `/old-page` (301). 111 Zellen in `internal_all` und 21 in `url_all`. (Die Referenz schreibt MD5-Werte mit führender 0 um die Null verkürzt – 31 statt 32 Zeichen; das ist reine Darstellung und im Mapping per `hex_trim_zeros` neutralisiert, kein Befund.)
Status: **erledigt durch M1-P31** (K-60: `Hash` nur für HTML-Antworten).

**N-07 · Selbstreferenzierende und leere Meta-Refresh-Seiten** (Profil `respect_self_ref_meta_refresh = false` wie die Referenz) – Spec TECH-068, ONPAGE-018, OUT-031, OUT-081. Seiten `parity-response` `/meta-refresh-self.html` (`content="30"`), `/http-refresh-nourl.html`, `parity-technical` `/dir/refresh-bare.html` (`content="60"`): OpenSpider lässt `Redirect URL` und `Redirect Type` leer, legt keine Link-Zeile `Meta Refresh` (Quelle = Ziel) in `Links:All Inlinks` an und führt die Seite nicht in `Redirects:Redirect Chains`; die Referenz schreibt `Redirect URL` = eigene Adresse, `Redirect Type` = `Meta Refresh Redirect`, die Inlinks-Zeile und eine Redirect-Chains-Zeile (Loop). Folgen im Crawl Overview: Response Codes › Redirection (Meta Refresh) 4 statt 5 (response) bzw. 1 statt 2 (technical), Internal Redirect Chain 7 statt 9, Redirect Loop 2 statt 3 bzw. 0 statt 1. Zellen: Redirect URL/Type 2 + 2, Inlinks 2 Zeilen, Redirect Chains 1 Zeile, Crawl Overview 5 + 2 Zähler.
**→ Entschieden 14.10.2026 (K-57): Parität (M1-P31).** Status: **erledigt durch M1-P31** (K-69 präzisiert den Header-Fall: `Refresh` ohne URL ist keine Weiterleitung).

**N-08 · Nur über Hreflang entdeckte URLs bei `spider.hreflang.crawl = false`** – Spec TECH-071 / CRAWL-047-Konfiguration. `parity-technical` `/hl/it/` (Status 404, nur per `<link rel="alternate" hreflang>` verlinkt): OpenSpider listet die URL in `Internal:All` als Zeile ohne Status und führt die Hreflang-Link-Zeile (`Hreflang | /hl/en/ | /hl/it/`) in `Links:All Inlinks`; die Referenz kennt weder die Zeile noch die Link-Zeile. Folge: Crawl Overview Total URLs Encountered 118 statt 116, Internal All/Security All/HTTP URLs 113 statt 111 (neben DEV-002). 1 Zeile `internal_all`, 1 Inlinks-Zeile, 7 Zähler.
**→ Entschieden 14.10.2026 (K-62): Parität (M1-P31).** Status: **erledigt durch M1-P31**.

**N-09 · Link Position und Link Path für Links im `<head>`** – Spec OUT-031 (kennt die Position `Head` nicht). Die Referenz schreibt für `<link>`/`<meta>` im Kopf `Link Position` = `Head` und `Link Path` = `//head/link[n]` bzw. `//head/meta[n]`; OpenSpider schreibt für Canonical, Hreflang und Meta Refresh beide Spalten leer und für CSS/JavaScript im Kopf `Link Position` = `Content`. Beispiel: `out-basic-site` `Canonical | / | /`, `CSS | / | /assets/site.css`. 426 Zellen `Link Position`, 414 `Link Path` (alle Sites). Aufgabe: Spec um `Head` ergänzen und `os-parse` Link-Position/-Pfad für Kopfelemente setzen. (Body-Links stimmen: `Navigation`/`Content`/`Footer` und der Pfad als Elementfolge sind in allen Sites gleich.)
**→ Entschieden 14.10.2026 (K-63): Parität, `Head` und Kopfpfad (M1-P31, `os-model`-Nachtrag 3.13).** Status: **erledigt durch M1-P31**.

**N-10 · Externe `Inlinks` zählen Redirect- und Bild-Verweise** – Spec ONPAGE-025/-026 („Anzahl Link-Instanzen von internen Seiten“, Linktypen nicht festgelegt). `parity-response` `http://localhost:1/`: Referenz 1 (ein Hyperlink), OpenSpider 2 (Hyperlink + HTTP-Redirect von `/r-to-dead`); `parity-onpage` `{EXT}/img/size-102401.png`: Referenz 0, OpenSpider 1 (Bild-Link). 2 Zellen. Aufgabe: Spec präzisieren (nur Hyperlinks) und Zählung anpassen.
**→ Entschieden 14.10.2026 (K-61): nur Hyperlinks, ONPAGE-025 präzisiert (M1-N16).** Status: **erledigt durch M1-N16**.

### Entscheidungsbedarf (Spec/Annahme vs. Referenz)

**B-01 · robots.txt als Tabellenzeile (K-40, CRAWL-098)** – Die Referenz listet die robots.txt des internen Hosts in keinem Tab; OpenSpider führt sie als URL-Zeile (200, `text/plain`, Klasse Unknown) in Internal, Response Codes, URL, Directives. Das verschiebt alle Gesamtzähler im Crawl Overview um 1 und erzeugt je Export eine Zusatzzeile. Als `DEV-001`/`DEV-002` dokumentiert (OS-Festlegung); der Paritätslauf bestätigt die K-40-Annahme **nicht**. Entscheidung: `in_tables = 0` für die robots.txt interner Hosts (Referenzverhalten) oder Festlegung beibehalten.
**→ Entschieden 14.10.2026 (K-54): Parität, keine Tabellen-Sichtbarkeit; DEV-001/-002 entfallen (M1-P31).** Status: **erledigt durch M1-P31** (DEV-001/-002 gestrichen).

**B-02 · Verhalten bei nicht erreichbarer robots.txt (CRAWL-100, Q30 beantwortet)** – Die Referenz crawlt URLs eines Hosts, dessen robots.txt nicht erreichbar ist (`http://localhost:1/`, Verbindung abgelehnt), trotzdem und meldet `Connection Timeout`/`No Response`; das entspricht der OpenSpider-Policy „Alles erlauben“ (`robots.unreachable_policy = allow_all`), nicht der Voreinstellung „Host sperren“. Das Paritätsprofil nutzt `allow_all`. Entscheidung: Voreinstellung ändern oder bewusst abweichen (CRAWL-100 nennt das Risiko selbst).
**→ Entschieden 14.10.2026 (K-55): bewusste OS-Festlegung bleibt (RFC 9309, Abschnitt 2.3.1.4); das Paritätsprofil nutzt die dafür vorgesehene Option `allow_all`, kein DEV-Eintrag.** Status: **erledigt** (Profil und Kommentar in `parity.osconfig` auf K-55 angepasst, M1-N18).

**B-03 · „Respect Self Referencing Meta Refresh“ (TECH-068)** – Die Referenz (Free-Standard) wertet Selbst-Refresh-Seiten nicht als nicht indexierbar; OpenSpider-Voreinstellung `true`. Das Paritätsprofil setzt `false`. Entscheidung: Voreinstellung für Parität anpassen oder beibehalten.
**→ Entschieden 14.10.2026 (K-57): Standard `false` wie die Referenz; Selbst-Refresh wird als Weiterleitung auf sich selbst geführt (N-07; M1-P31).** Status: **erledigt durch M1-P31** (Kommentar in `parity.osconfig` auf K-57 angepasst, M1-N18).

**B-04 · Statusklassen und Populationen** – Die Tabs Canonicals/Links/Hreflang und der Nenner von `% of Total` verwenden Status 200 (TECH-011/-071/-130, ONPAGE-015); die Referenz zählt alle 2xx (Fixture `/s201`): `DEV-021`, `DEV-032`. Entscheidung: auf 2xx erweitern oder Festlegung bestätigen.
**→ Entschieden 14.10.2026 (K-56): 2xx; DEV-021/-032 entfallen (Canonicals/Links: M1-N16; Hreflang und `% of Total`: M1-P31).** Status: **erledigt durch M1-N16 und M1-P31**; die verbleibenden Zähler (PDF ohne Canonical, Selbstlink-Seite) stehen als DEV-035/-036 (K-71).

**B-05 · Wirkungsbereich von `issues.ignore_non_indexable` (CRAWL-089)** – Die Referenz schließt nicht indexierbare URLs auch in Canonicals › Missing und Links › Pages Without Internal Outlinks aus (parity-response: 9 gegen 13 bzw. 3 gegen 7; parity-technical: 86 gegen 94 bzw. 1 gegen 2); CRAWL-089 begrenzt den Wirkungsbereich abschließend auf Element-Tabs und Low Content Pages: `DEV-022`.
**→ Entschieden 14.10.2026 (K-56): Wirkungsbereich um TECH-015 und TECH-133 erweitert; DEV-022 entfällt (M1-N16).** Status: **erledigt durch M1-N16** (DEV-022 gestrichen, M1-N18).

### Spec-Annahmen, die die Referenz widerlegt oder bestätigt

**→ Entschieden 14.10.2026:** A-4, A-23 und Crawl Depth folgen der Referenz (K-58, M1-P31; DEV-019/-020/-015 entfallen; Status: erledigt durch M1-P31). A-4 vergleicht seit M1-P31 mit der Kollation `OS_TEXT` (K-70, Annahme A-4b). Die Lücken TECH-130 (`Inlinks`) und TECH-051/TECH-071 (Indexability-Spalten) gehen nach M2 (`docs/plan/M2-backlog.md`).

- **widerlegt, A-4** (Duplikate exakt, Groß-/Kleinschreibung sensitiv, ONPAGE-005): Titel/Meta Description, die sich nur in der Schreibweise unterscheiden, zählt die Referenz als Duplikat (`parity-onpage` `dup-case.html`; Crawl Overview Page Titles › Duplicate 10 statt 9, Meta Description › Duplicate 10 statt 9) – `DEV-019`.
- **widerlegt, A-23** (Underscores nur im Pfad, ONPAGE-072): `?sort_by=x` zählt in der Referenz mit (URL › Underscores 2 statt 1) – `DEV-020`.
- **widerlegt, Crawl Depth Zeile 0** (OUT-025/OUT-080: „die Referenz beginnt bei 1“): Der Crawl Overview der Referenz beginnt bei Tiefe **0**; die Population dort sind interne HTML-Seiten mit 2xx, OpenSpider zählt alle internen URLs – `DEV-015`.
- **widerlegt, TECH-130** („`Inlinks` steht nicht in der Referenz-Spaltenliste des Links-Tabs“): Der Export der Referenz enthält `Inlinks` im Tab Links; OpenSpider blendet die Spalte im Export aus (Mapping ignoriert sie, Befund für die Spec).
- **Spec-Lücke, TECH-051/TECH-071:** Die Referenz exportiert `Indexability` und `Indexability Status` auch im Directives- und im Hreflang-Tab; die Spaltenlisten der Spec nennen sie nicht (Mapping ignoriert sie mit Grund).
- **bestätigt, K-02:** `Transferred (Bytes)` und `Total Transferred (Bytes)` stimmen in allen fünf Sites zellgleich überein (kein Header-Anteil in `Transferred`); `Size (Bytes)` ebenso – ONPAGE-012 ist `verified`.
- **bestätigt, K-44/K-45 (soweit in den Paritäts-Sites enthalten):** Response-Codes-Status/Indexability der Fixtures stimmen bis auf N-07 und `DEV-005` überein; `dir/none.html` = `Non-Indexable`/`noindex`, `canon/none.html` = `Indexable` wie in der Referenz (N12). `ext-noindex.html` gehört nicht zu den fünf Paritäts-Sites und ist daher nicht gegen die Referenz prüfbar. Externe Redirect-Ketten/-Schleifen sind leer (K-44), die Referenz hat in den Sites keine.
- **bestätigt, K-50:** Der Dateiname `links_all_inlinks.csv` wird im Runner über `OsExport::file_candidates` gefunden; der Name der Referenzdatei (`all_inlinks.csv`) steht im Manifest (`sf_file`).
- **bestätigt, N09 (Hreflang-Mehrfachspalten):** Die Spaltenfolge `HTML hreflang n`, `HTML hreflang n URL` (`ColumnSql::MultiPaired`) deckt sich mit der Referenz; `hreflang_all` stimmt zellgleich überein (1696 von 1696 Zellen). Dass os-export die URL-Spalten als Text statt `ColumnKind::Url` behandelt, beeinträchtigt die Parität nicht – kein Meldebedarf.
- **Referenzformat (K-53):** Die Referenz schreibt nummerierte Spalten nur bei vorhandenen Daten, Dezimalkomma (`91,670`) und im Crawl Overview ein Blockformat mit `%`-Zeichen und Dezimalkomma; sie schreibt `Last Modified`, `Content Type`, `Size (Bytes)` und `Transferred (Bytes)` mit anderen Namen als die Spec-Spalten (Mappings).
- **Nicht im M1-Umfang:** Der Export `Redirects to Error(s)` (OUT-084, P1) und der Bulk-Export `All Error Inlinks` unter dem Pfad „Response Codes:All Error Inlinks“ (OUT-067) – letzterer wird über `Response Codes:Internal & External:All Error Inlinks` verglichen (97,6 %).

## A4 Performance

**Status A4: nicht erfüllt (PERF-002, Abschnittsgrenzen).** Alle übrigen Kriterien sind erfüllt. Quellen: `docs/plan/M1-acceptance/loadtest/README.md` und `docs/plan/M1-acceptance/benchmarks/README.md` (M1-N21, Commit `88fd424` = origin/main nach Welle 7, Apple M2, 8 Kerne, 16 GB RAM, 15.10.2026, jeder Block nach `wait_quiet.sh 2` unter `caffeinate -dimsu`; hier nur gelesen, nicht geändert). Die Messung N14 (vor Welle 7, Commit `43caf19`) liegt als `n14-*` im Ordner `loadtest/`.

### Großläufe und Benchmarks (M1-N21)

| Kriterium | Grenze | Messwert | Ergebnis |
|---|---|---|---|
| PERF-001 large: Spitzen-RSS bei 1 Mio. URLs x 30 Links | < 1,5 GB | 471,1 MB (N14: 720,8 MB) | erfüllt |
| PERF-001 large: RSS(1 Mio.) / RSS(100 k) | < 2 | 1,134 (471,1 MB / 415,4 MB) | erfüllt |
| PERF-001: Cache-Grenze über die PRAGMA-Werte (K-16) | Balanced 256 MiB; MaxSpeed <= min(25 % RAM, 4 GiB) | Balanced 268 435 456 B; MaxSpeed 4 GiB (= 25 % von 16 GiB) | erfüllt |
| PERF-001 large: längster Stillstand (K-73) | <= 30 s | 6,1 s | erfüllt |
| PERF-002 (K-72): Mittelwert abgerufene URLs ÷ Crawl-Dauer | >= 500 URLs/s | 2 098,8 URLs/s (N14: 133,1) | erfüllt |
| **PERF-002 (K-72): jeder der zehn 10-%-Abschnitte zwischen 70 % und 130 % des Mittelwerts** | 70 % bis 130 % | **149, 186, 173, 164, 139, 116, 100, 75, 72, 68 %** (3 118, 3 904, 3 635, 3 440, 2 926, 2 429, 2 106, 1 577, 1 515, 1 421 URLs/s); zweiter Lauf Balanced 138, 172, 160, 157, 127, 109, 97, 86, 79, 71 % | **nicht erfüllt** (Abschnitte 1 bis 5 und 10) |
| PERF-003 large: Spitzen-RSS bei SlowSink 100 ms (K-52), 1 Mio. URLs (K-73) | < 500 MB | 468,1 MB (N14: 707,6 MB) | erfüllt |
| PERF-003 large: Crawl endet regulär | `queue_empty`, 1 Mio. URLs | ja | erfüllt |
| PERF-003 large: längster Stillstand (K-73) | <= 30 s | 4,3 s (N14: 31,3 s) | erfüllt |
| PERF-005 large: 10 harte Abbrüche, danach `integrity_check` | ok | 10 von 10 ok | erfüllt |
| PERF-005 large: URL-Menge und Status = Referenz, Dubletten in `urls` | identisch, 0 | identisch (100 001 URLs), 0 Dubletten | erfüllt |
| PERF-005 large: wiederholte Requests je Abbruch (K-49) | <= 2 264 (8 + 256 + 2 000) | `[2264 x 9, 2263]`, Maximum 2 264 | erfüllt (an der Grenze) |
| CRAWL-124: MinMemory-RSS mindestens 20 % unter Balanced (1 Mio. Seiten, alle drei Profile) | <= 80 % | -53,7 % (217,4 MB gegen 469,9 MB); MaxSpeed 5 437 MB | erfüllt |
| CRAWL-126 Pausieren / Fortsetzen mit 1 Mio. Warteschlangeneinträgen | < 3 s | 10,9 ms / 648 ms | erfüllt |
| CRAWL-127 Öffnen von 1 Mio. URLs / Speichern unter | < 3 s / < 60 s | 1 ms / 4,8 s | erfüllt |
| OUT-001 Export 1 Mio. x 60 Spalten | Speicherzuwachs <= 300 MB | 1 MiB | erfüllt |
| Benchmark-Ignores (18 Tests in 12 Dateien, `grep -rn 'ignore = "Benchmark' crates --include='*.rs'` ohne `crates/os-parity/tests/data`) | alle Zusicherungen | **18 von 18 bestanden** | erfüllt |
| `cargo test --workspace --exclude openspider-desktop --locked`, clippy `-D warnings`, `cargo fmt --all --check` | grün | 2 037 bestanden, 0 fehlgeschlagen, 36 ignoriert; clippy und fmt ohne Befund | erfüllt |

**PERF-002 (Nachbesserungsauftrag in `loadtest/README.md`, keine Codeänderung in N21):** Der Mittelwert ist mit 2 099 URLs/s weit über der Grenze, aber der Durchsatz fällt im 1-Mio.-Crawl stetig (Faktor 2,7 zwischen Abschnitt 2 und 10; in zwei unabhängigen Läufen gleich). Bis 500 000 Seiten gelten die Abschnittsgrenzen (96 % bis 126 % bei 500 000, 85 % bis 114 % bei 250 000). Eingrenzung: Writer 87 % der Laufzeit (Commit 248,5 s, Aufbau 131,9 s), Checkpoints ohne Einfluss, die Cache-Größe ist nicht der Hebel (MinMemory 64 MiB gleich schnell, MaxSpeed 4 GiB langsamer und steiler), das reine Store-Benchmark `writer_1m_uniform_segments` bleibt flach (98 % bis 103 %). Die Ursache ist nicht gemessen. Nachbesserung: `os-engine` (`writer_thread.rs`, Warteschlange `engine_q_*`) und `os-store` (`writer.rs`); Prüfung: `os-loadtest run --scenario perf-001 --size large` (rund 8 Minuten) mit leerem `crawler_violations`.

Zur Einordnung (N21, kein Kriterium): Gegenüber N14 ist der 1-Mio.-Crawl von 125 Minuten auf 8 Minuten gefallen (133 auf 2 099 URLs/s), die RSS-Spitze von 721 auf 471 MB (PERF-001) bzw. von 708 auf 468 MB (PERF-003). Der Lasttest bewertet `--profile max-speed` seit N19 nicht mehr mit den PERF-001-RSS-Grenzen (K-16); bei 1 Mio. Seiten belegt MaxSpeed 5 437 MB und ist 9,3 % langsamer als Balanced, MinMemory 3,6 % schneller (alle drei Läufe wegen der PERF-002-Abschnitte mit Exit 1, die Abschnittsregel gilt für das Standardprofil).

### Klein-Varianten (10 000 URLs), grün in jeder CI

Job `loadtest-small` auf allen drei Plattformen (PERF-001/003/005/009 und die Server-Messungen), CI-Lauf des N18-Branches ([%(run)s](%(url)s)). Berichte: `docs/plan/M1-acceptance/ci-loadtest-small/loadtest-small-<plattform>.json` (in diesem Paket durch die Berichte des N18-Laufs ersetzt).

| Plattform | PERF-001 small: Spitzen-RSS | URLs/s | PERF-003 small: Spitzen-RSS | URLs/s | Ergebnis |
|---|---:|---:|---:|---:|---|
| macos-arm64 | 114,3 MiB | 4.560,9 | 109,6 MiB | 3.163,3 | grün |
| macos-x64 | 99,3 MiB | 931,4 | 98,9 MiB | 994,9 | grün |
| windows-x64 | 102,8 MiB | 1.973,2 | 99,1 MiB | 1.783,2 | grün |

**PERF-003 (K-52):** „100 ms je Batch“ gilt wörtlich (`SlowSink` wartet 100 ms je Schreibaufruf der Writer-Stufe, auch bei Gruppen-Commit); Kriterium bleibt RSS < 500 MB und reguläres Crawl-Ende. In der Klein-Variante liegt die Spitze bei rund 99 bis 110 MiB und der Crawl endet regulär. Das Drosseln selbst belegt `crates/os-engine/src/writer_thread.rs` › `full_channel_blocks_the_sender_until_the_writer_catches_up` (Tag PERF-003).

**Windows-Quote (Fable-Gate 4b):** `crawler_urls_per_s` in perf-001 small, windows-x64 gegen macos-arm64 (Job `loadtest-small`):

| Lauf | macos-arm64 | macos-x64 | windows-x64 | Quote windows/arm64 |
|---|---:|---:|---:|---:|
| [38507699280](https://github.com/domdewom2/openspider/actions/runs/38507699280) (main, vor P30) | 2.058,4 | 199,0 | 80,9 | 3,9 % |
| [38527465880](https://github.com/domdewom2/openspider/actions/runs/38527465880) (main, Merge N13) | 3.460,2 | 1.317,2 | 1.706,3 | 49,3 % |
| [38536936966](https://github.com/domdewom2/openspider/actions/runs/38536936966) (main, Plan 4c) | 3.416,6 | 1.440,6 | 1.965,3 | 57,5 % |
| [38544701689](https://github.com/domdewom2/openspider/actions/runs/38544701689) (main, Merge N15) | 4.193,1 | 2.082,0 | 1.962,1 | 46,8 % |
| [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445) (Branch m1/m1-p29) | 3.320,9 | 1.199,8 | 2.022,9 | 60,9 % |
| [%(run)s](%(url)s) (Branch m1/m1-n18) | 4.560,9 | 931,4 | 1.973,2 | 43,3 % |

**Befund an den Orchestrator (kein eigener Fix, unverändert seit P29):** Die Quote liegt in drei von fünf Läufen seit P30 unter 50 % (49,3 %, 46,8 %, 43,3 %). Windows selbst bleibt stabil bei 1,7 bis 2,0 k URLs/s (94 bis 212 % von macos-x64); die Schwankung der Quote kommt vom schnellen, stark streuenden macos-arm64-Runner (3,3 bis 4,6 k URLs/s). Der vor P30 gemessene Wert (80,9 URLs/s) ist durch P30 erledigt.

## A5 CLI

- **Exit-Codes:** je Code ein grüner Integrationstest (`crates/os-cli/tests`): 0 `cli_basics::the_binary_runs_without_a_gui_session_and_returns_an_exit_code`; 1 `cli_load::exit_code_1_when_a_running_crawl_is_resumed_within_the_heartbeat_window`; 2 `cli_basics::invalid_names_are_rejected_before_any_request_in_under_a_second` (und die Aufruffehler-Tests in `cli_config.rs`); 3 `cli_basics::exit_code_3_when_the_crawl_cannot_be_written`; 4 `cli_basics::exit_code_4_when_an_export_fails_but_the_crawl_succeeded`; 130 `cli_load::the_first_sigint_pauses_and_saves_with_exit_code_130_and_resume_completes_the_crawl` (Spec-Tag OUT-204).
- **`out-basic-site`-E2E-Exporte byte-identisch (nach Normalisierung) auf allen drei Plattformen:** Die Golden-Vergleiche (`tests/golden/out-basic-site`, `out-cli`) laufen im Job `build` von `cargo test --workspace` auf macos-arm64, macos-x64 und windows-x64 und sind dort grün (main-Lauf 38544701689 und P29-Lauf).
- **Namensvalidierung ohne Netzzugriff:** `cli_basics::invalid_names_are_rejected_before_any_request_in_under_a_second` und `exit_code_3_when_the_crawl_cannot_be_written` (`site.total_requests() == 0`).

## A6 Crawl-Datei

CI-Job-Paar `crawlfile-create` / `crawlfile-crossplatform` (neu in P29, `.github/workflows/ci.yml`): Jede Plattform (macos-arm64, macos-x64, windows-x64) crawlt dieselbe Fixture-Site (`parity-technical`, 116 URLs, Profil `parity.osconfig`), speichert die Crawl-Datei (`--save-crawl` → `crawl.ospider`) und exportiert `Internal:All` (`internal_all.csv`) sowie `Links:All Inlinks` (`links_all_inlinks.csv`). Danach lädt jede Plattform die Crawl-Dateien der **beiden anderen** (`--load-crawl`), exportiert erneut und vergleicht mit `cmp` byte-gleich mit den Exporten der Ursprungsplattform. Ergebnis im P29-Lauf [38553901445](https://github.com/domdewom2/openspider/actions/runs/38553901445): alle sechs Richtungen grün – macos-arm64 → macos-x64, macos-arm64 → windows-x64, macos-x64 → macos-arm64, macos-x64 → windows-x64, windows-x64 → macos-arm64, windows-x64 → macos-x64 (Ausgabe „Crawl-Datei von <Ursprung> auf <Ziel>: Exporte byte-gleich“). Lokal (macOS arm64) ebenso: Der Export aus der geladenen Kopie ist byte-gleich zum Export direkt nach dem Crawl.

- **Determinismus:** `crates/os-engine/tests/lifecycle_determinism.rs` › `two_crawls_with_a_fixed_clock_a_seed_and_one_thread_are_byte_identical` (zwei Crawls mit fester Uhr, festem Seed und einem Thread liefern nach dem abschließenden Checkpoint byte-identische `.ospider`-Dateien; läuft in der CI auf allen drei Plattformen).
- **SIGINT → Exit 130 → `--load-crawl <id> --resume` vervollständigt:** `crates/os-cli/tests/cli_load.rs` › `the_first_sigint_pauses_and_saves_with_exit_code_130_and_resume_completes_the_crawl` (unter Windows über `GenerateConsoleCtrlEvent`, `cfg(windows)` in derselben Datei) sowie `resume_continues_a_crawl_that_stopped_at_the_url_limit`.

## A7 MCP

Alle Tests laufen in der CI auf drei Plattformen (`crates/os-mcp/tests`):
- **SDK-Client, initialize, tools/list, tools/call der fünf Werkzeuge:** `server_protocol::standard_client_can_run_the_five_basic_requests`, `server_process::rmcp_client_talks_to_the_binary_over_stdio`; Standard nur `read`: `server_protocol::tools_list_without_switches_and_with_read_only_has_only_read_tools`.
- **`crawl_overview` = Golden:** `tools_read::crawl_overview_equals_the_golden_of_the_overview_report` (und `…_has_exactly_the_numbers_of_the_crawl_overview_report`).
- **`query_urls`-Seitenvereinigung = Tab-Export ohne Dubletten:** `tools_read::internal_all_paged_with_limit_5_equals_the_tab_export_without_duplicates`, `server_protocol::paging_default_is_50_and_pages_union_to_the_full_list_without_duplicates`.
- **Injection-Fixture:** `tools_injection::injected_instructions_appear_only_inside_the_marked_data_block`, `tools_injection::no_tool_fetches_urls_that_are_only_mentioned_in_page_text` (kein Abruf von `/geheim`).
- **stdout nur Protokolldaten:** `server_process::stdout_carries_only_json_rpc_and_logs_go_to_stderr`.

## A8 Clean-Room

Stand M1-N18 (mechanische Prüfung):

- **`sf/`-Dateien nur in `tests/golden/parity/*/sf/`:** `git ls-files | grep -E "(^|/)sf/"` liefert 39 Dateien, alle `tests/golden/parity/<site>/sf/*.csv` (0 Treffer außerhalb); `tests/parity/fixtures_check.rs` (`cargo test -p os-testsite --test parity_fixtures`) prüft je Site: `sf/` enthält genau die `sf_file`-Dateien der Manifest-Einträge mit `status = present` (vorhanden, nicht leer, `.csv`, keine Unterordner), `pending`/`unavailable` haben keine Datei.
- **Referenzdateien unverändert:** `git diff bea719f HEAD -- tests/golden/parity/*/sf` ist leer (`bea719f` = Merge der Referenzexporte; der Branch `parity/sf-reference` ist nach dem Merge vom Remote entfernt). Kein Umbenennen, kein Neukodieren; Zeilenenden normalisiert `.gitattributes`.
- **Runner verwirft Beschreibungsspalten:** `crates/os-parity` › `csvio::tests::description_columns_are_dropped_on_read`, `diff_engine::reference_description_texts_never_reach_the_report`, `reference_format::the_block_format_becomes_a_table_without_descriptions` (Spalten `Total URLs Description` im Crawl Overview werden gar nicht erst gelesen); jedes Mapping führt `drop_sf_columns = ["Description", "How To Fix"]`. In den 39 Referenzexporten kommen keine solchen Spalten vor. Der neue Paritätsbericht (`report.md`/`report.json`) enthält nur Zahlen, Spaltennamen und die OpenSpider-eigenen Begründungstexte aus `deviations.toml`.
- **Referenz-Issue-Texte im Repo:** `grep -rniE 'Screaming ?Frog|SEO Spider|Learn More' core` (Issue-Katalog `core/issues/catalog.toml`, Texte `core/issues/texts/{de,en}.toml`): **0 Treffer** (Stand 86bace1 plus N18). Treffer außerhalb von `core/` sind keine Referenz-Issue-Texte, sondern Produktnamen in Quellenangaben und Kopfzeilen der Spec (`docs/spec/*`), in Planungsdokumenten (`docs/plan/*`), im Modulkommentar `crates/os-parity/src/diff.rs` und der Eintrag `learn more` in der Standardliste nicht beschreibender Ankertexte (TECH-136, `crates/os-model/src/config.rs`, `docs/spec/schemas/osconfig.schema.json`); das ist eine eigene Festlegung, kein Issue-Text.
- **Textstichprobe (Fable):** Die Stichprobenliste des Gates vom 14.10.2026 (10 Katalogtexte, Seed 20261014) bleibt die Grundlage; Fables Ersatzprüfung war ohne Befund. M1-N18 ändert keinen Katalogtext.

Stichprobe für die Textprüfung (10 Katalogtexte, fester Seed 20261014; Quellen `core/issues/texts/{de,en}.toml`, Schlüssel `issues.<id>.what|why|fix`):

- `issues.canonicals-canonicalised`
- `issues.canonicals-multiple-conflicting`
- `issues.content-exact-duplicates`
- `issues.directives-nofollow`
- `issues.h1-duplicate`
- `issues.h1-multiple`
- `issues.h2-duplicate`
- `issues.hreflang-non-200-hreflang-urls`
- `issues.response-codes-external-no-response`
- `issues.response-codes-external-server-error-5xx`

## A9 Statusbericht

`docs/plan/M1-status.json` (Stand 5f65ffc, jüngster grüner main-Lauf nach Welle 7c [38868822232](https://github.com/domdewom2/openspider/actions/runs/38868822232), Artefakt `spec-coverage` unverändert übernommen; erzeugt im CI-Job `spec-coverage`, Befehle in A2): 242 IDs – implemented 237, verified 5, deferred 0, **open 0**; Zusatz-IDs (`extra`, nicht gewertet) 7 (mit PERF-002, 31 Tests); Warnungen 0; ignorierte Tests ohne gültigen Grund 0. Der Lauf `--strict` endete mit Exit 0, der Job `spec-coverage` ist grün. Die Datei nennt Pakete bis M1-N22 und M1-N26 (als Eigentümer von PERF-001, PERF-003, PERF-005 und der Zusatz-ID PERF-002; M1-N26 liefert keine Tests). Den Paritätsbericht liest der Job aus seinem eigenen Lauf (`target/parity/report.json`); die eingecheckten Berichte in `docs/plan/M1-acceptance/parity/` sind unverändert der Stand von M1-N18.

Statusregeln 5.4: `implemented` (>= 1 getaggter Test `ok`), `verified` (zusätzlich Paritätsbeleg aus `tests/parity/evidence.toml`), `deferred` (alle getaggten Tests ignoriert mit `M2:`/`M3:`/`Klärung K-xx`), `open`. Die fünf `verified`-IDs stehen in A2 (ONPAGE-012, ONPAGE-120, ONPAGE-130, TECH-010, TECH-011); alle Belege nennen Site, Export, Zellzahl, Übereinstimmung und die angewandten Abweichungen. Sitemaps- und Hreflang-Filter mit Referenzeinschränkung SF Free bleiben `implemented` (K-53). Zu ONPAGE-060 gehört seit M1-N18 der Test `chains_ending_in_a_loop_count_but_self_redirects_do_not` (K-66). PERF-002 steht in `extra`, nicht in A2/A9 (K-72).
