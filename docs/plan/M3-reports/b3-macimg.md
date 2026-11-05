# Bericht B3-MACIMG: macOS-WebKit-E2E in spec-coverage-m2

Branch `m3/macimg` (Basis main `6723077d`), Worktree `~/Projects/openspider-wt/m3-macimg`, Server-Worktree `m3-macimg`.
Diagnose auf origin über die Zweige `diag/macos-image` und `diag/macos-image-2`. Nichts nach main gemergt, `m3/macimg`
nicht gepusht, keine Produkt- oder Testdatei geändert.

## Ergebnis in Kürze

- **Das Runner-Image 20261104.0514 hat die 19 Fehlschläge nicht verursacht.** Derselbe Job hatte genau dieselben 19
  Fehlschläge schon auf Image 20261028.0489 (Lauf 41989485643 auf 032c4345: `19 failed`, `503 passed (47.2m)`). Der Job
  war trotzdem grün, weil der Schritt „E2E-A mit JUnit“ den Playwright-Exit nur im manuellen Gate weitergibt
  (`if [[ "$M2_GATE" == "true" ]]; then exit "$status"; fi`).
- **Ursache:** Das `e2e`-Job in `spec-coverage-m2.yml` war gegenüber `ui-e2e.yml` veraltet. Es fehlten drei Schritte:
  1. `cargo build -p os-testsite -p os-cli -p os-parity --bins`: 12 Tests scheitern mit `spawn …/target/debug/os-testsite ENOENT`,
     **auf allen vier Plattformen** (Windows 12, Linux je 13 Fehlschläge in Lauf 41989485643, alle grün gemeldet).
  2. `defaults write org.webkit.Playwright AppleKeyboardUIMode -int 2` (macOS, N17): 7 Tab-Reihenfolge-Tests in `a11y/`
     scheitern, weil WebKit ohne diese Einstellung native Buttons beim Tab überspringt.
  3. `sysctl kernel.apparmor_restrict_unprivileged_userns=0` (Linux): `auth/bridge.spec.ts:26` scheitert unter Linux
     (der 13. Linux-Fehlschlag).
- **Der rote Lauf war das 60-min-Job-Limit, nicht die Tests.** Ein einziger macOS-WebKit-Job brauchte 47–54 min Testzeit
  plus ≈ 6 min Bau. Das kippte unabhängig vom Image: Versuch 1 von Lauf 41999934314 lief noch auf **20261028.0489** und
  wurde ebenfalls nach 60 min abgebrochen (Testlauf nach 54 min unvollständig); Versuch 2 auf 20261104.0514 brauchte
  53,9 min Testzeit und lief im Post-Schritt ins Limit. `ui-e2e.yml` hatte dieses Problem schon früher und teilt macOS
  deshalb in zwei Shards.
- **Fix (nur Workflows):** `spec-coverage-m2.yml` übernimmt die fehlenden Schritte aus `ui-e2e.yml` und teilt macOS-WebKit
  in zwei Shards (Laufzeitbudgets UI-146 im ersten Shard mit eigener JUnit-Datei). Dazu Robustheit in beiden Workflows:
  Schritt-Limit 50 min unter dem Job-Limit 60 min, Log, Report, Traces und `test-results/` immer als Artefakt.
- **Nachweis:** Lauf **42037881032** (spec-coverage-m2 auf `diag/macos-image`): alle Jobs grün, beide macOS-Shards auf
  **20261104.0514.1**, **0 Fehlschläge** in allen fünf E2E-Jobs (vorher 12–19 je Job), macOS 29 bzw. 41 min Jobdauer.
  Die 19 Specs zusätzlich isoliert ohne Wiederholung: 19/19 grün auf 20261104.0514.1 (Lauf 42039968386, Probe 1) und
  20261028.0489.1 (Läufe 42037878395, 42039968386 Proben 2 und 3).

## Fehlertexte (gekürzt, Lauf 41999934314 Versuch 2, Image 20261104.0514.1; identisch in 41989485643 auf 20261028)

Die Fehlertexte standen vollständig im Job-Log (Playwright-Zusammenfassung um 13:17:17, Abbruch erst um 13:17:26 im
Post-Schritt). Es fehlte nur der Artefakt-Upload von Report und Traces, den der Job nicht vorsah.

| Tests | Fehlertext |
|---|---|
| `beta/` control:6, delay:7 (2×), detail-during-crawl:12, n04-live:17, overview:8, preferences:6, quit:6, resume:10, services:37; `mcp/bridge.spec.ts:131`; `sidebar-chart/live.spec.ts:40` | `Error: spawn /Users/runner/work/openspider/openspider/target/debug/os-testsite ENOENT` (nach 0,4–0,9 s) |
| `a11y/contrast.spec.ts:62` (4 Varianten) | `control must be reachable in the sequential keyboard order` – `getByRole('button', { name: 'Green' })` Expected: focused, Received: inactive (`a11y/fixture.ts:66`) |
| `a11y/grid.spec.ts:4` | `expect(locator).toBeFocused()` – `getByRole('button', { name: 'Before grid' })` Received: inactive |
| `a11y/keyboard.spec.ts:129` | `every enabled sequential control must be reachable without a focus trap` – 2 558 Zeilen fehlende Stopps („All (3)“, „HTML (3)“ …) |
| `a11y/keyboard.spec.ts:137` | dieselbe Meldung, ein fehlender Stopp (`id 3`, ohne Namen) |
| nur Linux: `auth/bridge.spec.ts:26` | Chromium-Sandbox des Login-Fensters ohne User-Namespaces (Freigabe fehlte; in ui-e2e seit Längerem gesetzt) |

## Belege zur Ursache

| Lauf / Job | Workflow | Image macOS | Ergebnis |
|---|---|---|---|
| 41989485643 / 127118027596 | spec-coverage-m2, 032c4345 | 20261028.0489.1 | 19 failed, 503 passed (47,2 min), Job „success“ |
| 41989485643 / Windows, Linux-WebKit, Linux-Chromium | spec-coverage-m2 | – | 12 / 13 / 13 failed (os-testsite ENOENT, Linux zusätzlich auth/bridge), Jobs „success“ |
| 41999934314 Versuch 1 / 127152299417 | spec-coverage-m2, c4f5305d | **20261028.0489.1** | nach 60 min abgebrochen, dieselben Fehlschläge bis Test 366 sichtbar, JUnit fehlte |
| 41999934314 Versuch 2 / 127239675657 | spec-coverage-m2, c4f5305d | 20261104.0514.1 | 19 failed, 503 passed (53,9 min), Abbruch im Post-Schritt bei 60:15 min |
| 41999934548 macos-webkit-1/2 | ui-e2e, c4f5305d | 20261028.0489.1 | grün (mit Fixture-Werkzeugen und AppleKeyboardUIMode, zwei Shards) |

Der Vergleich ui-e2e ↔ spec-coverage-m2 auf demselben Commit und demselben Image (Versuch 1) zeigt: Unterschied ist die
Workflow-Konfiguration, nicht das Image. WebKit (27.2, playwright 1.64.1) und Produktcode waren gleich.

## Fix

Commits auf `m3/macimg`:

| Commit | Inhalt |
|---|---|
| `8a99e213` | `ci:` spec-coverage-m2 `e2e` wie ui-e2e: Fixture-Werkzeuge bauen, WebKit-Tastaturnavigation (macOS), Chromium-Sandbox (Linux), macOS-WebKit in zwei Shards (`--project=webkit --shard=n/2`), Laufzeitbudgets im ersten Shard mit `--no-deps` und `playwright-m2-perf.xml`; Warnung `::warning` bei Playwright-Exit ≠ 0; Report/Traces/`test-results`/`e2e.log` immer als Artefakt `playwright-report-m2-<name>`; Auswertung erwartet 5 × `playwright-m2.xml` + 1 × `playwright-m2-perf.xml` |
| `d289356f` | `ci:` ui-e2e: Schritt-Limits für E2E-A und Laufzeitbudgets, `e2e.log` im Artefakt |
| `6ede975e` | `ci:` Reporter-Argument quoten (shellcheck SC2054) |
| `c0ba7754` | `ci:` Schritt-Limit 50 statt 45 min (Windows und Linux-WebKit brauchten im Nachweis 38 min Testzeit) |
| (dieser Bericht) | `docs:` |

Nicht geändert (K2-63): keine Tests, keine Assertions, keine Budgets, kein Überspringen auf macOS, Job-Limit bleibt 60 min,
Wiederholungen bleiben bei 1 in CI. Der Gate-Mechanismus (Exit nur im Gate) bleibt; neu ist nur die sichtbare Warnung.

### Zum Schritt-Limit

Das Job-Limit bleibt 60 min. Neu ist ein Schritt-Limit von 50 min für den Playwright-Lauf, damit bei Überschreitung noch
Zeit für die `if: always()`-Uploads bleibt (Fehlertexte stehen dann in `test-results/*/error-context.md`, im Nachweis
geprüft: Abschnitt „Error details“ ist enthalten). Gemessene Testzeiten: spec-coverage 21–38 min (Lauf 42037881032),
ui-e2e 24–34 min (Lauf 41999934548). Einrichtung bis 10 min (Windows). 45 min wären für Windows/Linux-WebKit mit 38 min
zu knapp gewesen, deshalb 50.

### Zum Line-Reporter

Ich habe `list` beibehalten (Zeilen je Test mit Dauer, gleich wie ui-e2e). Die Fehlertexte stehen bei regulärem Ende in
der Zusammenfassung des Logs, bei Schritt-Abbruch in den hochgeladenen `error-context.md` und Traces. Der Diagnose-Workflow
nutzte `line` (Fehler sofort).

## Nachweisläufe

| Lauf | Zweig / Workflow | Image | Ergebnis | Dauer |
|---|---|---|---|---|
| 42037878395 | `diag/macos-image`, diag-macimg (19 Specs, `--retries=0 --trace=on`) | 20261028.0489.1 | 19 passed (5,6 min) | Job 13 min |
| 42039968386 Probe 1 | `diag/macos-image-2`, diag-macimg | **20261104.0514.1** | 19 passed (5,2 min) | Job 12 min |
| 42039968386 Proben 2, 3 | ebenso | 20261028.0489.1 | 19 passed (5,5 / 5,3 min) | Job 13 / 11 min |
| **42037881032** | `diag/macos-image`, spec-coverage-m2 (`workflow_dispatch`, Stand `6ede975e`) | **20261104.0514.1** (beide Shards) | alle 7 Jobs success, 0 failed | siehe unten |
| 42045967024 | `diag/macos-image`, ui-e2e (`workflow_dispatch`, Stand `c0ba7754`) | Shard 2: **20261104.0514.1**, Shard 1: 20261028.0489.1 | macOS 259 + 263 (+2 Budgets) passed, Windows 527, Linux-Chromium 526 (1 flaky); Linux-WebKit 1 failed (`n04-live:17`, siehe Offene Punkte) | macOS 23 / 35 min, Windows 53 min |

Lauf 42037881032 im Einzelnen:

| Job | Image | Ergebnis | Testzeit | Jobdauer |
|---|---|---|---|---|
| e2e (macos-webkit-1) | 20261104.0514.1 | 262 passed, 1 flaky; Budgets 2 passed | 21,0 min | 29 min |
| e2e (macos-webkit-2) | 20261104.0514.1 | 258 passed, 1 flaky, 13 skipped | 34,0 min | 41 min |
| e2e (windows-chromium) | 20261103.298.1 | 527 passed, 10 skipped | 38,2 min | 49 min |
| e2e (linux-chromium) | 20261101.360.1 | 527 passed, 10 skipped | 34,7 min | 38 min |
| e2e (linux-webkit) | 20261101.360.1 | 524 passed, 13 skipped | 38,3 min | 42 min |
| evidence | – | success | – | 29 min |
| spec-coverage-m2 | – | success, 5 + 1 JUnit-Dateien eingelesen | – | 1 min |

Die Zahl bestandener Tests stieg gegenüber Lauf 41989485643 (macOS 503, Windows 513, Linux 509/512) auf macOS 524 (inkl. 2
Budget-Tests und 2 flaky), Windows 527, Linux 524/527, weil die bisher nach < 1 s gescheiterten Specs jetzt wirklich laufen.

## Server-Prüfungen

Keine Rust- oder App-Änderung, daher kein cargo/npm-Lauf nötig. Auf dem Server (Worktree `m3-macimg`, kein `target/`
angelegt):

- `actionlint` 1.7.12 mit shellcheck 0.11.0 auf `spec-coverage-m2.yml`, `ui-e2e.yml` und `diag-macimg.yml`: 0 Befunde
  (nach `6ede975e`; vorher 1 Befund SC2054).
- Über alle Workflows meldet actionlint vorbestehende Befunde in fremden Dateien, nicht angefasst: `ci.yml:219` (SC2034),
  `ci.yml:334` (SC2012, info), `ui-e2e-b.yml:55` (SC1050/1072/1073 – PowerShell-Block, Fehlalarm), `ui-nightly.yml:53`
  (SC2016, info).

## Fremde Dateien (K2-92)

| Datei | Warum |
|---|---|
| `.github/workflows/ui-e2e.yml` | Auftrag Punkt 3: Robustheit (Schritt-Limit, `e2e.log` im Artefakt), da ui-e2e dasselbe Image bekommt |
| `.github/workflows/diag-macimg.yml` | nur auf `diag/macos-image*`, temporärer Diagnose-Workflow, nicht auf `m3/macimg` |

Kernauftrag: `.github/workflows/spec-coverage-m2.yml`, dieser Bericht.

## Diagnose-Zweige auf origin (stehen gelassen)

- `diag/macos-image` (`dc5e0fd4`): Stand `m3/macimg` ohne Bericht + `diag-macimg.yml`.
- `diag/macos-image-2`: wie oben, Diagnose-Workflow mit drei Proben.

Beim Pushen startete jeweils `linux-core` (Pfadfilter `**`); diese Läufe (42037878087, 42039968435, 42045952669) habe ich
abgebrochen, da sie nichts zum Auftrag beitragen. Löschen der Zweige entscheidet der Orchestrator.

## Offene Punkte

1. **`docs/plan/beta3.md`, Abschnitt 6** nennt das Image als Ursache; das ist widerlegt (siehe oben). Korrektur durch den
   Orchestrator.
2. **spec-coverage-m2 zeigt E2E-Fehlschläge nicht als rot.** Die Fehlschläge liefen seit Einführung des Jobs unbemerkt,
   die JUnit-Belege für die M2-Abdeckung enthielten also falsche Fehlschläge. Jetzt gibt es eine Warnung; ob der Job bei
   Fehlschlägen auch außerhalb des Gates rot werden soll, ist eine Entscheidung (K2-67/Q01).
3. **Abdeckungsschritt scheitert auf main schon vorher:** `os-parity: ungültige Eingabe: unbekanntes gemergtes M2-Paket: N00`
   (auch in Lauf 41989485643 auf main). Die Merge-Regex im Workflow macht aus „m2: Merge M2-N00-SPEC-TAGS“ die ID `N00`.
   Job bleibt grün, weil nicht im Gate. Nicht Teil dieses Pakets.
4. **Flaky im Nachweis** (je einmal, Wiederholung grün):
   - `sidebar-chart/live.spec.ts:40` (macos-webkit-2): ein `config.get` fiel in das Fenster „kein IPC bei Auswahl“
     (Zeile 71, `added` = `["config.get"]`). Diese Spec lief in spec-coverage bisher nie wirklich; in ui-e2e grün. Kandidat
     für M2-N32 bzw. ein Flake-Paket (Hintergrundabruf vs. Auswahl).
   - `compare/bridge.spec.ts:44` (macos-webkit-1): Spaltenkopf „Title 1 (Previous)“ nach Tab-Wechsel nicht innerhalb 5 s
     sichtbar. In ui-e2e 41999934548 grün (32 s).
   - `beta/n04-live.spec.ts:17` in ui-e2e Linux-WebKit (Lauf 42045967024, Image 20261025.353.1, beide Versuche):
     `getByRole('grid', { name: 'Inlinks' })` nicht innerhalb 1 000 ms gefunden (Zeile 72 ff., Zeitmessung nach Tab-Wechsel).
     Das ist der bekannte WebKit-Latenzpunkt `n04-live:74` aus B3-FLAKES (→ M2-N32), unabhängig von macOS und von diesem
     Paket (geändert wurden in ui-e2e nur Schritt-Limits und der Log-Upload). In spec-coverage 42037881032 war dieselbe
     Spec auf Linux-WebKit grün.
5. **Shard-Balance:** Shard 2 ist mit 34 min deutlich länger als Shard 1 (21 min + Budgets). Bei weiterem Wachstum wäre
   ein dritter Shard oder bessere Verteilung sinnvoller als ein höheres Job-Limit.
6. Der Matrix-Eintrag `macos-webkit` heißt jetzt `macos-webkit-1/-2` (Jobnamen `e2e (macos-webkit-1)` usw., Artefakt
   `playwright-junit-macos-webkit-1/-2`). Kein Ruleset verlangt diese Checks (Ruleset nur deletion/non_fast_forward).
