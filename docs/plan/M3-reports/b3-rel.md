# Bericht B3-REL: Release-Ablauf für den wöchentlichen Rhythmus

Branch `m3/rel3` (Basis origin/main `ebeaf768`), Worktree `c-rel3`, Server `m3-rel3`. Grundlage:
`docs/release/release-rhythmus.md`, `docs/plan/beta3.md` §5/§6, Spec `docs/spec/06-output-ops.md` (OUT-251, OUT-255,
OUT-261). Alle acht Punkte sind umgesetzt bzw. geprüft; nichts wurde weggelassen. Nichts gepusht, kein Workflow
gestartet, kein Release, Tag oder Deploy.

## Umgesetzt

### 1 Release-Notes stehen vor dem Bauen fest (OUT-251)

- Neu: `docs/release/notes/<Version>.md` ist die einzige Quelle für `notes` in `latest.json`/`latest-beta.json`
  und für den Release-Text. Angelegt für `1.0.0-beta.0`, `-beta.1`, `-beta.2` (wortgleich mit den CHANGELOG-Abschnitten,
  also dem, was tatsächlich im Update-Dialog stand). `1.0.0-beta.3.md` schreibt der Nutzer/Orchestrator am Mittwoch.
- `scripts/release/notes.mjs` baut auf dem Bestand auf (`releaseNotes`, `releaseBody`, `update-notes.md` bleiben):
  - `checkNotes`/`notes.mjs --check <Version>`: Dokument vorhanden, nicht leer, keine Überschriften `#`/`##`
    (würden CHANGELOG-Abschnitt und Release-Text zerteilen; `###` erlaubt), keine `TODO`/`STUB`/`Entwurfshinweis`,
    CHANGELOG-Eintrag vorhanden und wortgleich. Meldungen nennen Datei und Abhilfe, ohne Stacktrace, in Actions als
    `::error title=Release-Notes::…` und in der Zusammenfassung.
  - `notes.mjs --changelog <Version> --date <JJJJ-MM-TT>` schreibt den CHANGELOG-Abschnitt aus dem Dokument direkt
    unter `[Unreleased]`; ein vorhandener gleicher Abschnitt bleibt, ein abweichender wird nie überschrieben.
  - Der Standardaufruf im `assemble`-Job (`notes.mjs <Version> update-notes.md --body …`) liest jetzt das Dokument
    (mit derselben Prüfung) statt nur den CHANGELOG.
- `.github/workflows/release.yml`, Job `prepare`: neuer Schritt „Release-Notes vor dem Bauen prüfen (OUT-251)“ nach
  „Versionsgleichstand und Tag prüfen“. `licenses`, `macos`, `windows` hängen direkt, `linux-core` über
  `[prepare, licenses]` von `prepare` ab, also startet ohne gültige Notes kein Bau-Job.
- Warum CHANGELOG wortgleich statt abgeschafft: Der CHANGELOG bleibt die Versionsgeschichte im Repo, darf aber nicht
  vom Update-Dialog abweichen (bei beta.2 wich der von Hand geänderte Release-Text vom Manifest ab). Das Dokument darf
  am Mittwoch als reiner Doku-Commit auf `main` landen (startet nach Punkt 4 keine schwere CI), der CHANGELOG-Abschnitt
  entsteht auf dem Release-Branch mit einem Befehl.
- Tests: `tests/release/orchestration.test.mjs` (alle eingecheckten Notes gültig und gleich dem CHANGELOG; Gate-Schritt
  des Workflows mit fehlendem, leerem, abweichendem Dokument und fehlendem CHANGELOG; `--changelog` und danach `--check`;
  Einfügen unter `[Unreleased]`, Idempotenz, kein Überschreiben), jeweils `// spec: OUT-251 [M3]`.

### 2 Entwurfshinweise nur in die Actions-Zusammenfassung

- `release.yml`, `assemble`: „unsigniert“, „Installations-Smoke“ und „TODO: Offene M2-Paritätspakete“ gehen nach
  `$GITHUB_STEP_SUMMARY` unter „**Entwurfshinweise (nicht Teil des Release-Texts):**“ und zusätzlich nach
  `release-reports/entwurfshinweise.md` (Actions-Artefakt). `release-notes.md` = Download-Kasten + Notes, sonst nichts.
- `draft`: bricht vor `gh release create` ab, wenn `release-notes.md` `Entwurfshinweis`, `TODO` oder `STUB` enthält.
- Tests: Release-Text exakt gleich `releaseBody(…)`, Hinweise in Summary und Bericht, Fall „unsigniert“ getrennt;
  Draft-Guard lehnt einen Text mit Hinweisen ab; D01-Schritt schreibt nicht in den Release-Text.

### 3 Feedback-Link im Download-Kasten

- `notes.mjs`, `feedbackLink`: Version mit Vorabanteil (`-beta.N`, `-rc.N`) → `[openspider.de/beta](https://openspider.de/beta/)`,
  stabil → `[GitHub Discussions](https://github.com/<owner/name>/discussions)`, für das echte Repo also
  `https://github.com/wojcikdominik/openspider/discussions`. Die Fixture des veröffentlichten beta.2-Kastens bleibt
  bytegleich. Test `// spec: OUT-261 [M3]`.

### 4 CI-Pfadfilter

- Befund vorher: `ci.yml`, `linux-core.yml`, `parity.yml` ignorierten schon `**/*.md` (K2-67), aber nicht
  `website/**` (HTML, CSS, PHP, `check-site.mjs`) und nicht die Nicht-Markdown-Dateien unter `docs/` (Messdaten,
  JSON, PNG der Berichte). `licenses.yml` lief bei jedem Push auf `main` und jedem PR.
- `ci.yml`, `linux-core.yml`, `parity.yml`: zusätzlich `!docs/**`, `!website/**`, `!learnings/**`; wieder enthalten
  sind `docs/spec/**`, `docs/issues/**`, `docs/plan/M2.md` und neu ausdrücklich `docs/plan/*.json`, `docs/plan/*.toml`
  (gelesen von `os-analyze`-, `os-export`-, `os-model`-, `os-parity`-Tests und der M1-Coverage in `ci.yml`).
  Danach `tests/**`, am Ende `!assets/**`. `licenses.yml`: gleicher Filter für push/pull_request; `workflow_call` aus
  `release.yml` kennt keine Pfadfilter und läuft immer.
- `spec-coverage-m2.yml`, `ui-e2e.yml`, `integrations.yml`, `cloud-setup.yml` haben Positivlisten ohne Website/Doku
  und bleiben unverändert; `docs/spec/**` bleibt Auslöser der Spec-Abdeckung. `neutral.yml` bleibt absichtlich
  ungefiltert (billig, muss Markdown prüfen). Tag-Pushes (`v*` in `ci.yml`) werten Pfadfilter auf GitHub nicht aus.
- Pflicht-Checks: `gh api repos/wojcikdominik/openspider/branches/main/protection` → 404 „Branch not protected“.
  Das aktive Ruleset „OpenSpider“ (id 25470279) hat nur `deletion` und `non_fast_forward`, keine Status-Checks.
  Es kann also kein Pflicht-Check hängen bleiben; ein Ersatz-Job ist nicht nötig. Falls später Required Checks
  kommen, braucht jeder gefilterte Workflow einen gleichnamigen leichten Ersatz-Job.
- Neu `.github/workflows/website.yml` (ubuntu, 10 min): `node website/check-site.mjs`, `php -l` und die beiden
  PHP-Testskripte der Endpunkte, für `website/**`, `tests/fixtures/crash/**` und sich selbst. Vorher lief
  `check-site` in keinem Workflow.
- Neu `.github/workflows/release-scripts.yml` (ubuntu, 10 min): `node --test scripts/release/*.test.mjs
  scripts/release/licenses/generate.test.mjs tests/release/orchestration.test.mjs` für Release-Skripte,
  `docs/release/notes/**`, `CHANGELOG.md` und die Release-/Lizenz-Workflows. Vorher liefen die Vertragstests nur im
  `prepare`-Job des Release-Laufs; jetzt fällt ein ungültiges Notes-Dokument schon am Mittwoch beim Commit auf.
- Test: `orchestration.test.mjs` wertet die Filter mit GitHub-Semantik (Reihenfolge, letztes passendes Muster gewinnt,
  `*`/`**`) für typische Pfade aus: ruhig bleiben z. B. `README.md`, `CHANGELOG.md`, `website/…`, `docs/plan/M3-reports/…`,
  `docs/release/notes/…`, `learnings/…`; auslösen weiter `docs/spec/…`, `docs/issues/…`, `docs/plan/M2-packages.json`,
  `docs/plan/M1-filters.toml`, `tests/fixtures/…/README.md`, Code. Dazu Pins und Aufrufe der beiden leichten Läufe.

### 5 Lizenztexte robust

- `scripts/release/licenses/generate.mjs`: `fetchWithRetry` – 3 Versuche je Quelle, Timeout 30 s je Versuch
  (umfasst den Body), Backoff 2 s, 4 s; wiederholt werden Timeouts, Netzfehler und HTTP 408/429/5xx, andere HTTP-Fehler
  (z. B. 404) brechen sofort ab. Jede Wiederholung wird mit Grund protokolliert.
- Zwischenspeicher: `OPENSPIDER_LICENSE_CACHE=<Ordner>` legt nachgeladene Texte unter ihrer gepinnten SHA-256 ab und
  verwendet sie nur bei passender Prüfsumme (beschädigter Eintrag → neu laden und ersetzen; fremde Bytes werden nie
  gespeichert und scheitern wie bisher an `verifiedSourceText`). `licenses.yml`, `release-macos.yml` und
  `release-windows.yml` halten den Ordner per `actions/cache` (Schlüssel aus `scripts/release/licenses/*-sources.json`).
- Tests in `generate.test.mjs` (`// spec: OUT-273 [M3]`): simulierte hängende Verbindung, die nur der Timeout beendet
  (zweimal Timeout, dann Erfolg; dreimal Timeout → klare Meldung), 503/429/404/502, Cache-Treffer ohne Netz,
  beschädigter Eintrag, fremde Bytes.

### 6 CHANGELOG und Kompatibilitäts-Fixtures

- `CHANGELOG.md`: Abschnitte `1.0.0-beta.2` und `1.0.0-beta.1` wortgleich aus `origin/rel/beta2` (dort `beta.1`
  identisch zu `origin/rel/beta1`). Keine anderen Dateien der Release-Branches.
- `tests/fixtures/migrations/releases/1.0.0-beta.1/` und `…/1.0.0-beta.2/`: per `gh release download v<v> --pattern
  'compat_*'` in je einen leeren Ordner unter dem Scratchpad geladen, gegen das `SHA256SUMS` des Releases geprüft (OK),
  entpackt (keine Links, keine Pfade außerhalb) und unverändert übernommen. Kein Archiv im Repo: `compat_archive`
  (os-cli, os-app) erwartet entpackte Ordner mit `crawl.ospider` und `internal_all.csv`, wie
  `archive-compat-fixture.mjs` sie erzeugt. Größe je Ordner ~450 KiB (`crawl.ospider` 408 KiB, ≤ 1 MiB laut README),
  zusammen 904 KiB. `.gitignore`/`.gitattributes` behandeln `crawl.ospider` bereits richtig (eingecheckt, binär).
- `tests/fixtures/migrations/README.md`: Satz „keine erfundenen beta.1/beta.2-Archive“ durch die Herkunft ersetzt.

### 7 Website-Beta-Seiten

- `website/site/beta/index.html`, `website/site/en/beta/index.html`: Kopf mit zwei Schaltflächen „Für macOS laden“/
  „Für Windows laden“ (en: „Download for macOS/Windows“) auf `releases/latest/download/OpenSpider-macOS.dmg` und
  `…/OpenSpider-Windows-Setup.exe`, wie die Download-Seite. Abschnitt „Herunterladen“ verlinkt dieselben Dateien;
  MSI und Linux-CLI über die neueste Version auf GitHub. CSS unverändert (`.actions` bricht bereits um), daher keine
  neue `site.css?v=`-Kennung. Nicht deployt.

### 8 Hotfix-Versionsschema (nur Bericht, ein Test)

Siehe Abschnitt „Ergebnis Hotfix-Schema“.

## Trockenlauf durch den Orchestrator (nach dem Merge)

Voraussetzung: Release-Branch wie bei beta.1/beta.2 (`rel/beta3`; die Deployment-Regeln des Environments `release`
müssen ihn zulassen, R01/R02 binden es).

1. Auf `rel/beta3` (von `main`): `node scripts/release/set-version.mjs 1.0.0-beta.3`, `cargo metadata --locked
   --format-version 1 > /dev/null`, `node scripts/release/check-version.mjs`.
2. Optionaler Negativtest vorher: `gh workflow run release.yml --ref rel/beta3 -f release_tag=v1.0.0-beta.3 -f dry_run=true`
   ohne Notes-Dokument. Erwartet: `prepare` scheitert im Schritt „Release-Notes vor dem Bauen prüfen (OUT-251)“ mit
   Annotation „Release-Notes für 1.0.0-beta.3 fehlen: docs/release/notes/1.0.0-beta.3.md anlegen und committen, bevor
   gebaut wird …“; `licenses`, `macos`, `windows`, `linux-core` starten nicht.
3. Notes: `docs/release/notes/1.0.0-beta.3.md` schreiben (oder vom `main` übernehmen),
   `node scripts/release/notes.mjs --changelog 1.0.0-beta.3 --date 2026-11-12`,
   `node scripts/release/notes.mjs --check 1.0.0-beta.3`, committen, Branch pushen.
4. `gh workflow run release.yml --ref rel/beta3 -f release_tag=v1.0.0-beta.3 -f dry_run=true`.

Erwartung im Lauf:

- `prepare`: Schritt „Release-Notes vor dem Bauen prüfen (OUT-251)“ grün mit „Release-Notes 1.0.0-beta.3:
  docs/release/notes/1.0.0-beta.3.md vorhanden und gleich dem CHANGELOG-Eintrag“; Vertragstests grün.
- `licenses`/`macos`/`windows`: Schritt „Zwischenspeicher für nachgeladene Lizenztexte“ beim ersten Lauf „Cache not
  found“, danach gespeichert; bei Netzproblemen Zeilen „…; retry 2/3 in 2000 ms“ statt Abbruch.
- Zusammenfassung des Laufs (`assemble`): Updater-Status, dann „**Entwurfshinweise (nicht Teil des Release-Texts):**“
  mit „- Installations-Smoke: …“, „- TODO: Offene M2-Paritätspakete …“ und, falls nicht alle vier Ziele signiert
  sind, „- unsigniert: …“.
- Artefakt `release-reports`: `release-notes.md` = „## Download“-Kasten mit `OpenSpider_1.0.0-beta.3_universal.dmg`
  und `OpenSpider_1.0.0-beta.3_x64-setup.exe`, Feedback-Link `openspider.de/beta`, dann „## Neu in dieser Version“
  mit genau dem Notes-Dokument, kein „Entwurfshinweise“, keine Linie danach. `update-notes.md` bytegleich mit dem
  Dokument; `entwurfshinweise.md` mit den Hinweisen. In `release-assets/latest.json` und `latest-beta.json` ist
  `notes` gleich dem Dokument.
- `draft` läuft bei `dry_run=true` nicht. Der Draft-Guard (Hinweise im Release-Text → Abbruch) ist im Test abgedeckt.
- Pfadfilter danach am besten mit einem reinen Doku-Commit auf `main` beobachten: nur `neutral`, ggf. `Website` oder
  `Release scripts` starten, kein `CI`/`linux-core`/`parity`/`Third-party licenses`.

## Testergebnisse (Server `m3-rel3`)

| Prüfung | Ergebnis |
|---|---|
| `node --test scripts/release/*.test.mjs scripts/release/licenses/generate.test.mjs tests/release/orchestration.test.mjs` | 35/35 grün (vorher 25 + 7 Generator-Tests; neu 3 Generator-, 6 Orchestrierungs-Tests, 3 bestehende erweitert). `zip` aus `~/b3rel-tools` im PATH, siehe Offene Punkte |
| `node --test tests/fixtures/migrations/node/archive.test.mjs` | grün |
| `actionlint` 1.7.12 `-shellcheck=` über alle `.github/workflows/*.yml` | 0 Befunde (1.7.7 kennt YAML-Anker und `macos-15-intel` noch nicht, daher die neuere Version) |
| `node website/check-site.mjs` | OK: 20 Seiten |
| `php -l` + `website/api/tests/{feedback,crash}_test.php` (PHP 8.5, entpackt) | OK: 122 bzw. 88 Prüfungen |
| `scripts/check-neutral.sh` | grün |
| `cargo test -p os-cli --test compat_archive` | 3/3 grün, mit den neuen Release-Ordnern |
| `cargo test -p os-app --test compat_archive --test update_security` | 4/4 und 5/5 grün (neu: `hotfix_versions_order_between_weekly_betas_without_downgrade`) |
| `cargo fmt --all --check` | grün |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | grün |
| `cargo test --locked -j 4 --workspace` | 3982 bestanden, 1 fehlgeschlagen, 48 ignoriert (533 Testbinaries). Fehlschlag: `os-store` `crawl_files::damaged_file_is_detected_and_repaired_into_a_new_copy` (1362 statt ≥ 1900 gerettete URLs) bei Serverlast ~38 parallel zur B3-QUEUE-Messreihe; einzeln danach 3/3 grün. `crates/os-store` ist in diesem Paket unverändert; Test nicht gelockert (K2-63), als neuer Flake gemeldet. |
| `apps/` | nicht geändert, daher kein `typecheck`/`test` der App |

Server-Platz: eigenes `target/` des Worktrees `m3-rel3` nach dem Lauf gelöscht (vorher 120 GB).

## Ergebnis Hotfix-Schema

Frage: Ordnet der Updater `1.0.0-beta.3.1` über beta.3 und unter beta.4 ein, und akzeptieren `set-version`,
`check-version` und die MSI-Abbildung das Schema?

| Stelle | `1.0.0-beta.3.1` | Nächste Beta-Nummer als Hotfix (`1.0.0-beta.4`) |
|---|---|---|
| Update-Kern `os-app` (`UpdateManifest::is_newer_than`, SemVer-Vorrang `cmp_precedence`) | ja: beta.3 < beta.3.1 < beta.4 (neuer Test) | ja |
| Tauri-Updater (`tauri-plugin-updater` 2.14.1, `restore_update`, Standardvergleich `release.version > current_version`) | ja (SemVer-Ordnung, gleiche Reihenfolge; Quelle im Server-Registry gelesen) | ja |
| Stabiler Endpunkt (`validate(false)`) | Vorabversion abgelehnt, bleibt am Beta-Endpunkt | gleich |
| `scripts/release/set-version.mjs` | **nein**, Muster `x.y.z(-beta.N\|-rc.N)?` | ja |
| `check-version.mjs` | prüft nur Gleichstand, kein Format | ja |
| `notes.mjs`, `manifest.mjs`, `assets.mjs`, `archive-compat-fixture.mjs` | **nein**, gleiches Muster | ja |
| MSI (`apps/desktop/src-tauri/windows/build-config.ps1`, K3-07) | **nein**: Muster lehnt ab; Abbildung `patch*1000 + Beta-Nummer` ergibt beta.3 → `1.0.3`, beta.4 → `1.0.4`, kein Wert dazwischen. Ein viertes Feld hilft nicht, Windows Installer vergleicht nur die ersten drei Felder der ProductVersion. | ja (`1.0.4`) |
| NSIS-Versionsinfo | nicht geprüft (Tauri leitet sie ab; ohne Windows-Build nicht belegbar) | wie jede Beta |
| Homebrew/winget (`publish-packages.yml`) | nur stabile `vX.Y.Z`, für Betas ohne Belang | gleich |

Ergebnis: Der Vergleich im eigenen Code (und im Tauri-Updater) trägt `beta.3.1`, die Release-Werkzeuge und die
MSI-Abbildung nicht. Empfehlung: **Hotfix = nächste Beta-Nummer** (z. B. Freitag-Hotfix `beta.4`, das folgende
Donnerstags-Release `beta.5`). Funktioniert heute ohne Codeänderung auf allen Stufen; der Rhythmus behält „ein
Release pro Woche“, nur die Nummer verschiebt sich.

Wenn `beta.N.M` gewünscht ist, wären nötig: Muster in sechs Skripten erweitern, MSI-Abbildung ändern, z. B. Stufe
`10·N + M` (beta bis 49, bis 9 Hotfixes; veröffentlichte beta.1/beta.2 = `1.0.1`/`1.0.2` bleiben darunter, beta.3 →
`1.0.30`), RC-Bereich neu legen (≤ `1.0.998`, stable bleibt `1.0.999`), Tests in `build-config.tests.ps1` und
`version.test.mjs`; das ist eine K3-07-Entscheidung. Ungeeignet ist `1.0.0-beta.3-hotfix.1`: `3-hotfix` ist
alphanumerisch und steht nach SemVer über jeder Zahl, also über beta.4.

Test: `crates/os-app/tests/update_security.rs`, `hotfix_versions_order_between_weekly_betas_without_downgrade`
(`// spec: OUT-252`, `// spec: OUT-254`): volle Ordnung beta.2 < beta.3 < beta.3.1 < beta.3.2 < beta.4 < beta.10 <
rc.1 < 1.0.0, `+hotfix` ist nicht neuer, Vorabversion am stabilen Endpunkt abgelehnt. `release-rhythmus.md` nennt das
Ergebnis; die Entscheidung bleibt beim Nutzer/Orchestrator.

## Fremde Dateien (K2-92)

| Ort | Datei/Verzeichnis | Herkunft | Umgang |
|---|---|---|---|
| `~/Projects/openspider` (Haupt-Checkout, Mac) | – | `git status` beim Start sauber | nicht angefasst |
| `~/Projects/openspider-wt/c-rel3` (Mac) | – | nur eigene Änderungen, alle committet | – |
| `~/Projects/openspider-wt/m3-rel3` (Server) | `.claude-active`, `target/` | `srv-run.sh` bzw. eigene Builds | nicht committet; `target/` (120 GB) am Ende gelöscht |
| Server `~/b3rel-tools/` | `root/usr/bin/zip` (Ubuntu-Paket `zip` 3.0, per `apt-get download` + `dpkg -x`), `actionlint12/actionlint` 1.7.12 (GitHub-Release rhysd/actionlint, Prüfsumme OK; 1.7.7 wieder gelöscht), `php/` (Ubuntu `php8.5-cli` + Bibliotheken, entpackt) | eigene Prüfwerkzeuge ohne root, je eigener leerer Ordner | bleibt für den Orchestrator liegen (klein), darf gelöscht werden |
| Server `~/b3rel-ws-test.log` | Log des Workspace-Tests | eigener Lauf | darf gelöscht werden |
| Mac-Scratchpad | `compat-dl-1.0.0-beta.{1,2}/` (heruntergeladene Archive, `SHA256SUMS`, entpackte Kopie) | eigener Download in leere Ordner | außerhalb des Repos |
| Server allgemein | B3-QUEUE-Messreihe (`~/perf/q/series.sh`, `openspider-q1`) lief parallel | anderes Paket | nicht angefasst; eigene Läufe mit `nice -n 10`, `-j 4`, ein Workspace-Lauf |

## Spec-Fragen

1. **Sprache der Notes.** Der Download-Kasten ist zweisprachig, die Notes (und damit der Update-Dialog, auch für
   englische Oberfläche) bisher nur Deutsch. Sollen `docs/release/notes/<Version>.md` zweisprachig sein oder je
   Sprache eine Datei (`notes` in `latest.json` ist ein einziges Feld)? Umgesetzt: unverändert eine Datei, Sprache frei.
2. **RC und Feedback-Link.** Umgesetzt: jede Version mit `-` (auch `-rc.N`) verlinkt `/beta/`, nur `x.y.z` die
   Discussions. Falls RCs schon auf Discussions zeigen sollen, ist `feedbackLink` die einzige Stelle.
3. **Spec-Text geändert (knapp):** OUT-251 neues OS-Festlegungs-Kriterium „Notes vor dem Bau“ (Dokument, Gleichheit
   mit CHANGELOG, Abbruch vor dem Bau); OUT-261 Release-Text-Kriterium um Feedback-Link je Kanal und „Entwurfshinweise
   nur in der Actions-Zusammenfassung“ ergänzt. Keine neue Spec-ID; Herkunft `docs/plan/beta3.md` §5 und dieser Auftrag,
   nicht als Nutzerentscheidung gekennzeichnet. Bitte bei der Abnahme bestätigen.
4. Der Lizenzgenerator hat keine eigene Produkt-ID; die neuen Tests tragen `OUT-273` (Drittlizenzen der CLI-Archive),
   wie die bestehenden L01-Tests in `orchestration.test.mjs`. Die CI-Pfadfilter sind Projektbetrieb (K2-67), ohne Spec-ID.

## Offene Punkte

- **Hotfix-Schema entscheiden** (Empfehlung: nächste Beta-Nummer, siehe oben).
- **Server ohne `zip`:** `orchestration.test.mjs` („Workflow packt L01-Hinweise …“) braucht `zip` und schlug auf dem
  Server schon vor meinen Änderungen fehl (`zip: command not found`); mit `~/b3rel-tools/root/usr/bin` im PATH grün.
  Auf GitHub-Runnern ist `zip` vorhanden. Empfehlung: `zip` auf dem Server per apt installieren (braucht root).
- **PHP-Version:** `website.yml` läuft auf `ubuntu-latest` mit dem vorinstallierten PHP (8.3); lokal geprüft habe ich
  mit PHP 8.5 (beide Testskripte OK). Der erste Lauf nach dem Merge zeigt es.
- **shellcheck** ist auf dem Server nicht vorhanden; `actionlint` lief mit `-shellcheck=` (ohne Shell-Analyse). Die
  geänderten Shell-Blöcke sind über die Tests ausgeführt.
- **Environment `release`:** Für den Trockenlauf von `rel/beta3` müssen dessen Deployment-Regeln den Branch zulassen
  (wie zuvor `rel/beta1`, `rel/beta2`); nicht geprüft (nur lesend, keine Einstellungen abgefragt).
- **Neuer Flake** `os-store` `damaged_file_is_detected_and_repaired_into_a_new_copy` (siehe Testergebnisse) für die
  Flake-Liste in `docs/plan/beta3.md` §6.
- `CHANGELOG.md` `[Unreleased]` enthält noch den alten Satz „Frühe Beta … in Vorbereitung“; nicht angefasst
  (Auftrag: nur die beiden Abschnitte). `notes.mjs --changelog` lässt `[Unreleased]` unverändert.
- Pfadfilter wirken erst ab dem Merge-Commit; der Merge selbst ändert Workflows und Code und startet die volle CI.
- Falls später Required Checks auf `main` eingerichtet werden, brauchen gefilterte Workflows gleichnamige leichte
  Ersatz-Jobs, sonst bleiben Doku-PRs hängen.
