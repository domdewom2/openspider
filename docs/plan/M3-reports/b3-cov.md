# Bericht B3-COV: spec-coverage-m2 – Fehlschläge immer rot, Merge-Extraktion der M2-Abdeckung

Branch `m3/cov` (Basis main `f6d98ccf`), Worktree `~/Projects/openspider-wt/m3-cov`, Server-Worktree `m3-cov`.
Nachweis auf origin über den Zweig `diag/cov` (Stand `5f990a6c`, bleibt stehen). `m3/cov` nicht nach origin gepusht,
nichts gemergt.

## Ergebnis in Kürze

- **A – Fehlschläge immer rot.** Rust-Tests, vitest, E2E-A und Laufzeitbudgets geben ihren Exit jetzt immer weiter, nicht
  nur im Gate. Der Abdeckungsschritt bleibt außerhalb des Gates weich, meldet einen Exit ≠ 0 aber per `::warning`.
- **Neuer Befund: Auch der Rust-Testschritt war seit Langem rot, unbemerkt.** Im Lauf 42037881032 (B3-MACIMG-Nachweis,
  als grün gemeldet) scheiterten 59 Rust-Tests in 16 Testzielen (`os-render`, `os-engine`, `os-analyze`, `os-app`,
  `os-cli`, `os-export`, `os-mcp`), alle mit Chromium „No usable sandbox“. Gleiche Ursache wie bei den E2E-Jobs in
  B3-MACIMG: Dem `evidence`-Job fehlte die AppArmor-Freigabe für User-Namespaces. Ich habe den Schritt aus `ui-e2e.yml`
  übernommen. Danach liefen 4349 Rust-Tests ohne Fehlschlag (vorher 4290 bestanden, 59 gescheitert).
- **B – Merge-Extraktion.** Der Abdeckungsschritt läuft auf diesem Stand durch und schreibt `M2-status.json`.
  Die Extraktion liefert 124 echte Paket-IDs; vorher waren es 126 Treffer, davon 14 Pseudo-IDs (`m2`, `main`, `N00` …).
  12 Pakete fehlten früher, darunter N24–N31, die vier FIX-Pakete und CI-E2E. `os-parity` behandelt unbekannte
  `--merged`-IDs jetzt als Warnung.
- **Nachweis:** Lauf **42056676906** (`workflow_dispatch` auf `diag/cov`, Stand `5f990a6c`): alle 7 Jobs grün,
  0 Fehlschläge in Rust, vitest und allen fünf E2E-Jobs, `M2-status.json` erzeugt. Das Ergebnis ist `passed: false`
  mit 2 blockierenden IDs, 25 offenen Anhang-B-Einträgen und 13 ungültigen Ignores. Der Schritt gibt dafür
  `::warning`; Details im Abschnitt „Was M2-status.json jetzt aussagt“.

## Änderungen

| Commit | Inhalt |
|---|---|
| `24cdc150` | `parity:` `coverage/m2.rs` – unbekannte `--merged`-IDs wählen kein Paket aus, landen als Warnung „unbekanntes gemergtes M2-Paket ignoriert: <id>“ in `warnings` und im neuen Feld `unknown_merged` der Statusdatei; vorher `ParityError::Invalid` und kein Bericht. Test `unknown_merged_ids_warn_and_select_no_package` gegen die Fixture `tests/data/coverage_m2`. Die bisherige Assertion `run(...).is_err()` für `typo` im Test `development_defaults_to_wave_zero_and_requires_only_selected_packages` ist entfallen, weil sie genau das alte Verhalten prüfte; die neue Erwartung steht im neuen Test. README zu `--merged` angepasst. |
| `5f990a6c` | `ci:` `spec-coverage-m2.yml` – siehe unten |
| `d2285cd9` | `parity:` doppelte Ausgabe der Warnung entfernt (die Zusammenfassung gibt Warnungen schon aus) |
| (dieser Bericht) | `docs:` |

Änderungen in `spec-coverage-m2.yml` (`5f990a6c`):

| Stelle | vorher | nachher |
|---|---|---|
| `evidence`, neu vor `npm ci` | – | `Chromium-Sandbox erlauben (Linux)`: `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` (wie `ui-e2e.yml`) |
| `evidence` „Rust-Tests und echte Ergebnisdatei“ (Z. 89) | `exit` nur bei `M2_GATE` | `exit "$status"` immer; vitest (`!cancelled()`) und Upload (`always()`) laufen trotzdem |
| `evidence` „vitest-JUnit“ (Z. 97) | `exit` nur bei `M2_GATE` | `exit "$status"` immer |
| `e2e` „E2E-A mit JUnit“ (Z. 207–210) | `::warning` + `exit` nur bei `M2_GATE` | `exit "$status"` immer (die rote Schrittmarkierung ersetzt die Warnung) |
| `e2e` „E2E-A Laufzeitbudgets“ (Z. 214, 226–229) | `if: matrix.perf`; `exit` nur bei `M2_GATE` | `if: ${{ !cancelled() && matrix.perf }}`, damit die Budgets auch nach roten Haupttests laufen und `playwright-m2-perf.xml` nicht fehlt; `exit "$status"` immer |
| `spec-coverage-m2` „Spec-Abdeckung und Anhang B“ | Einzeiler-Regex `m2: Merge (?:M2-)?([A-Za-z0-9]+)\b` | Python-Schnipsel mit Abgleich gegen `docs/plan/M2-packages.json`, siehe unten; Zusammenfassung „Gemergte M2-Pakete laut main: N; andere Merge-Zweige: M“ |
| ebenda, nach dem `os-parity`-Aufruf | stumm außerhalb des Gates | `::warning` bei Abdeckungs-Exit ≠ 0 und bei unvollständigen Matrix-Belegen; im Gate weiterhin `exit` |
| Upload `spec-coverage-m2` | – | zusätzlich `main-merged-m2.txt` (Zuordnung ID ← Merge-Titel, unbekannte Zweige) |

Unverändert (K2-63): keine Tests, Assertions, Retries, Budgets oder Zeitlimits; `needs: [evidence, e2e]` mit
`!cancelled()` und `continue-on-error` der Downloads außerhalb des Gates bleiben, der Abdeckungsjob läuft also auch nach
roten Testjobs und wertet die vorhandenen Belege aus.

### Extraktionsregel

`m2: Merge [m2/][M2-]<Zweig>` (Groß-/Kleinschreibung egal). `<Zweig>` ist eine Folge von `[A-Za-z0-9]+`-Teilen mit
Bindestrich. Es zählt der **längste Bindestrich-Präfix**, der eine Paket-ID aus `M2-packages.json` ist (mit oder ohne
`M2-`). Übergeben wird die volle ID (`M2-U27`). Andere Zweige werden auf stderr protokolliert („kein M2-Paket, nicht
übergeben: …“), in `main-merged-m2.txt` festgehalten und nicht an `os-parity` übergeben.

Fables Vorschlag `(?:m2/)?(?:M2-)?([A-Za-z]{1,2}\d{2}[A-Za-z]?)\b` hätte 5 echte Pakete verloren: `M2-FIX-LIVE`,
`M2-FIX-ABOUT`, `M2-FIX-WAL`, `M2-FIX-TX` (gemergt als `m2/fix-…`) und `M2-CI-E2E` (gemergt als `m2/ci-e2e-limit`).
Ich habe deshalb den Präfix-Abgleich gewählt; Fables zweiter Punkt, Filtern gegen `M2-packages.json`, ist umgesetzt.

## Beleg der extrahierten IDs

Grundlage ist `git log --format=%s origin/main` auf `f6d98ccf`: 156 Titel der Form `m2: Merge …`.

| | vorher (alte Regex) | nachher |
|---|---|---|
| Treffer | 126 | 124 Paket-IDs + 23 protokollierte Zweige |
| davon echte Paket-IDs | 112 | 124 |
| Pseudo-IDs (an `os-parity` übergeben) | 14: `N00, Plan, app, base, fix, int, j01, k2, m2, main, neutral, origin, plan, w01` | 0 |
| Ergebnis in CI | Abbruch `unbekanntes gemergtes M2-Paket: N00`, kein `M2-status.json` (Lauf 42037881032) | Bericht geschrieben, `unknown_merged: []` |

**Nur nachher (12):** `M2-CI-E2E, M2-FIX-ABOUT, M2-FIX-LIVE, M2-FIX-TX, M2-FIX-WAL, M2-N24, M2-N25, M2-N26, M2-N27,
M2-N28, M2-N30, M2-N31`. Die alte Regex fing bei „m2: Merge m2/n31“ nur `m2`. **Nur vorher:** keine.

Gegenprobe gegen die Merge-Titel: Jede der 150 Paket-IDs wurde in allen 156 Titeln als eigenes Wort gesucht. Im Text
erwähnt, aber nicht als gemergter Zweig erkannt, sind nur drei:

- `M2-N06` und `M2-Q02` („m2: Merge base-4r (Fable N06/Q02 freigegeben)“)
- `M2-N15F` („m2: merge int-21 into n15f“, „m2: Merge N20 (Fable N15F approved …)“)

Diese Pakete kamen über Integrationszweige auf main. Ich zähle sie nicht, siehe Offene Punkte.

Die 124 IDs stehen vollständig in `main-merged-m2.txt` im Artefakt des Nachweislaufs. Die 23 protokollierten Zweige
(identisch im CI-Log von Lauf 42056676906):

| Zweig | Merge-Titel |
|---|---|
| `N00-SPEC-TAGS` | m2: Merge M2-N00-SPEC-TAGS (Welle 1) |
| `Plan-Nachtrag` | m2: Merge Plan-Nachtrag K2-74 und K2-75 |
| `app-icon` | m2: Merge app-icon (Tauri-Icon-Satz des Nutzers) |
| `base-4r` | m2: Merge base-4r (Fable N06/Q02 freigegeben) |
| `fix-auth-e2e-linux` | m2: Merge fix-auth-e2e-linux (Login-Endpunkt für den Auth-E2E-Test) |
| `fix-browser-ci` | m2: Merge fix-browser-ci (CI-stabile Chromium-Tests) |
| `fix-int15`, `fix-int15b`, `fix-int16` | Integrationskorrekturen |
| `fix-n02`, `fix-n02b-e2e` | Korrekturen zu N02/N02b (die Pakete selbst sind gezählt) |
| `fix-stop` | m2: Merge m2/fix-stop – laut Plan Teil von `M2-FIXE-PORT`, kein eigenes Paket |
| `int-10`, `int-12`, `int-16`, `int-21` | Integrationsrunden |
| `j01`, `w01` | kein Eintrag in `M2-packages.json` |
| `k2-82`, `plan-k275a`, `neutral-ver` | Klärungs-/Planzweige |
| `main`, `origin` | Merge von main in Integrationszweige |

Grenzfälle durch die Präfixregel: `A03-plan` → `M2-A03` (die alte Regex lieferte ebenfalls `A03`), `U31-MAC` → `M2-U31`,
`ci-e2e-limit` → `M2-CI-E2E`. Alle drei haben 0 `spec_ids` und 0 `extra_spec_ids`; die Zuordnung ändert an der Abdeckung
nichts.

### Zu „N00“ (M2-N00-SPEC-TAGS)

Befund, keine Entscheidung:

- `M2-N00` steht nicht in `docs/plan/M2-packages.json`. Ohne Eintrag hat N00 keine `spec_ids`, keine `owns` und keine
  Welle. Als Besitzerpaket kann `os-parity` es also gar nicht zählen; es kann nichts auslösen.
- Inhalt laut `docs/plan/M2-reports/n00-spec-tags.md` (Merge `ffcbb676`): ein Kommentar-Tag `// spec: OUT-233`/`OUT-234`
  in `crates/os-store/tests/schema_v3_tables.rs` und der Bericht. Beide IDs gehören `M2-F02`, das gemergt und gezählt ist.
  Der Bericht sagt selbst: „der Tag verschiebt ihren Status in `os-parity coverage` nicht“.
- Daher: nicht zählen. Die Extraktion protokolliert den Zweig, `os-parity` würde eine trotzdem übergebene ID `N00` nur
  noch als Warnung führen. Soll N00 als Planpaket geführt werden, müsste der Planer es in `M2-packages.json` nachtragen.
  Fachlich ist das nicht nötig.
- Verwechslungsgefahr: `M3-N00` / `B3-N00` („Restpunkte M2“, gemergt 05.11.) ist ein anderes Paket und wird mit
  `m3:`/`merge:` gemergt, nicht mit `m2: Merge`.

## Nachweislauf 42056676906

`gh workflow run spec-coverage-m2.yml --ref diag/cov` (Stand `5f990a6c`, `gate` = false). Ein mitlaufender
`linux-core`-Push-Lauf auf `diag/cov` (42056665241) wurde abgebrochen.

| Job | Ergebnis | Belege |
|---|---|---|
| evidence | success (37 min) | Rust 4349 passed, 0 failed, 50 ignored (Lauf 42037881032: 4290 / **59 failed** / 50); vitest 1011 Tests, 0 Fehler |
| e2e (macos-webkit-1) | success (29 min) | 263 Tests, 0 failures; Budgets `playwright-m2-perf.xml` 2 Tests, 0 failures |
| e2e (macos-webkit-2) | success (31 min) | 272 Tests, 0 failures, 13 skipped |
| e2e (windows-chromium) | success (48 min) | 537 Tests, 0 failures, 10 skipped |
| e2e (linux-chromium) | success (38 min) | 537 Tests, 0 failures, 10 skipped |
| e2e (linux-webkit) | success (42 min) | 537 Tests, 0 failures, 13 skipped |
| spec-coverage-m2 | success (1 min) | 5 × `playwright-m2.xml` + 1 × `playwright-m2-perf.xml` eingelesen (kein „unvollständig“); 124 Pakete übergeben, 23 Zweige protokolliert; **`target/M2-status.json` erzeugt** und im Artefakt `spec-coverage-m2`; `::warning` „os-parity coverage Exit 1 (Gate: false)“ |

Weil alle Testschritte jetzt hart sind, ist der grüne Lauf ein echter Beleg: kein Rust-, vitest-, E2E- oder Budget-Test
ist gescheitert.

### Was M2-status.json jetzt aussagt (Modus `development`)

| Feld | Wert |
|---|---|
| `summary` | 590 IDs: 588 implemented, 2 open, 0 verified, 0 deferred |
| Zusatz-IDs | 75, alle implemented |
| `required_packages` | 127 (124 gemergt + Welle 0: V02, S02, G0) |
| `unknown_merged` | `[]` |
| `failed_tests` | 0 |
| `blocking_specs` | 2: `AGENT-041`, `AGENT-042` (Paket `M2-M02`, gemergt; „kein Test mit `// spec:`-Tag gefunden“) |
| `blocking_appendix_b` | 25 von 137 (ID, Paket)-Einträgen, fehlender Marker `[M2]` (z. B. CRAWL-126 / M2-FIX-WAL, CRAWL-130 / M2-N25, N28; ONPAGE-142 / M2-D02, D19, N08; PERF-008 / M2-Q03) |
| `ignored_without_valid_reason` | 13: 10 ohne gültigen M2-Grund (`e2e/perf/n23-soak-diagnostics.bench.ts` 3×, `e2e/perf/perf.bench.ts:202`, `e2e/polish/tabbars.spec.ts:73`, `e2e/viz/perf3d.spec.ts:26` ohne Grund, `e2e/viz/v02.spec.ts` 3× WebGL, `os-app/tests/render_provision.rs:392` „Manuell: OUT-260“); 3 `M2:`-Altgründe gemergter Besitzer (`tab_internal.rs:588`, `tab_pagination_m2.rs:305`, `tab_response_codes.rs:622`) |
| `passed` | **false** |

Das ist der erste vollständige M2-Status seit Oktober. Bisher brach der Bericht vorher ab, oder ihm fehlten die
JUnit-Belege. Inhaltlich ist er weitgehend grün: Kein Test scheitert, 588/590 IDs sind implementiert. Offen sind
Nachweispflichten: Tags für AGENT-041/042, `[M2]`-Marker für Anhang B, gültige Ignore-Gründe. Behoben habe ich davon
nichts, das sind Fachpakete (K2-63); Bewertung im Fable-Gate M2 bzw. B3-Planung. Im Gate würde zusätzlich jede ID
über alle Wellen verlangt.

## Server-Ergebnisse (Worktree `m3-cov`, Stand `5f990a6c` bzw. `d2285cd9`)

| Prüfung | Ergebnis |
|---|---|
| `df -h /` vor dem Workspace-Test | 247 GB frei |
| `actionlint` 1.7.12 + shellcheck 0.11.0 auf `spec-coverage-m2.yml` | 0 Befunde |
| `cargo fmt --all --check` | grün (auch nach `d2285cd9`) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | grün (Exit 0); nach `d2285cd9` erneut `-p os-parity --all-targets`: grün |
| `cargo test -p os-parity` | 190 passed, 0 failed (nach `d2285cd9` erneut 190 / 0); neuer Test `unknown_merged_ids_warn_and_select_no_package` ok |
| `cargo test --workspace --locked` (über `~/ws-slot.sh`) | **4370 passed, 0 failed, 51 ignored** (Summe per awk über alle `test result`-Zeilen, Stand `5f990a6c`; `d2285cd9` entfernt nur eine `println!`-Zeile) |
| Workflow-Schritt lokal und auf dem Server nachgestellt (Python-Schnipsel mit den echten Merge-Titeln von `f6d98ccf`) | 124 IDs, 23 protokollierte Zweige; `os-parity coverage --merged <124 IDs>,N00 --strict` schreibt `target/M2-status.json` mit `unknown_merged: ["N00"]` und gibt die Warnung aus |

Server-`target/` des Worktrees `m3-cov` ist am Ende gelöscht.

## Fremde Dateien (K2-92)

| Datei | Besitz laut Plan | Änderung | Begründung |
|---|---|---|---|
| `.github/workflows/spec-coverage-m2.yml` | `M2-Q01` (M2, abgeschlossen) | siehe Tabelle oben | B3-COV-Auftrag (Fable-Folgepunkte A/B zu B3-MACIMG); keine Spec-ID (CI) |
| `crates/os-parity/src/coverage/m2.rs` | `M2-Q01`; `M3-Q01` besitzt `coverage/**` „ohne m2.rs-Verhalten zu ändern“ | unbekannte `--merged`-IDs: Warnung + `unknown_merged` statt Fehler | Auftrag B; **ändert m2.rs-Verhalten** – bei der Integration mit einem laufenden M3-Q01-Stand abgleichen |
| `crates/os-parity/tests/coverage_m2.rs` | `M2-Q01` | ein neuer Test, eine Assertion des alten Verhaltens entfernt | Auftrag B; keine Spec-ID (Dateikopf: „M2-Q01 ist Infrastruktur ohne eigene Spec-IDs“) |
| `crates/os-parity/src/coverage/README.md` | `M2-Q01` / `M3-Q01` | Absatz zu `--merged` | Dokumentation des neuen Verhaltens |
| `docs/plan/M3-reports/b3-cov.md` | neu | dieser Bericht | – |

## Offene Punkte

1. **Spec-ID für den neuen Test:** Es gibt keine passende Spec-ID. `os-parity coverage` ist Infrastruktur, so steht es im
   Kopf von `coverage_m2.rs`. Der Test trägt deshalb nur einen Kommentar „B3-COV: keine Spec-ID“. Bitte bestätigen, dass
   das die CLAUDE.md-Regel erfüllt.
2. **Über Integrationszweige gemergte Pakete werden nicht erkannt.** C1 liest nur `m2: Merge`-Titel. Nicht gezählt
   werden `M2-N06`, `M2-Q02` (via `base-4r`), `M2-N15F` (via `int-21`/N20) und `M2-FIXE-PORT` (inkl. `fix-stop`).
   Ohne eigenen `m2: Merge`-Titel sind außerdem `R01, U04, U11, N01, N02a, K05, A01, E05, I05, X03, T01, U13, U14,
   U15, U21, U22, U24, U25, Q04` und Welle 0 `V02, S02, G0`. Ob und über welche Integrationsrunde diese gemergt sind, habe
   ich nicht einzeln geprüft. Ihre offenen IDs bleiben Warnungen statt Blockern; im Gate gilt ohnehin alles. Ob C1
   sie einschließen soll (z. B. Alias-Liste im Plan oder Feld `merged_via`), muss der Planer entscheiden. Ich habe das
   nicht geraten.
3. **`M2-status.json` meldet `passed: false`** (2 IDs ohne Tag, 25 Anhang-B-Marker, 13 Ignore-Gründe). Das sind
   Fachnacharbeiten, keine CI-Fehler. Vorschlag: als B3-/M3-Nacharbeit einplanen. Fable entscheidet, ob das Gate M2
   (M2-Q04) diesen Stand braucht.
4. **Folge von A für main:** Ab dem Merge macht jeder echte Rust-, vitest-, E2E- oder Budget-Fehlschlag den Workflow
   rot. Flaky-Tests (macOS-WebKit, `n04-live` unter Linux-WebKit aus B3-MACIMG) können ihn künftig rot färben. Das ist
   gewollt, kann nach dem Merge aber zunächst auffallen.
5. `diag/cov` bleibt auf origin stehen (Stand `5f990a6c`, ohne `d2285cd9` und diesen Bericht). Löschen nach der Abnahme.
