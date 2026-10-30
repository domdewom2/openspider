# M3-Integration int-b – vorgezogene Beta-Pakete

Stand: 30.10.2026; Branch `m3/int-b`, Basis `c107e900` (int-a plus
R02-Korrektur `84535dc6` und R01-Korrektur `c107e900`). Kein Push, kein Dispatch,
kein Tag und keine Veröffentlichung durch diese Integration.

## Merges und Konfliktlösungen

| Reihenfolge | Paket | Merge-Commit | Auflösung |
|---|---|---|---|
| 1 | U02 | `23d635be` | `docs/release/README.md`: präzise F01-Regeln für Schlüssel, Endpunkt-/Repo-Overrides erhalten; gemeinsame Beta-Variable und automatische Windows-Installerwahl aus U02 ergänzt. Anschließend veraltete Stub-/Platzhalterbeschreibungen auf den integrierten Stand gebracht. |
| 2 | L01 | `09a2c2ba` | Zusätzlicher Konflikt in `release-windows.yml`: `OPENSPIDER_REQUIRE_NOTICES=1` ausschließlich im Tauri-Release-Schritt ergänzt, korrigierte R02-Fallbacks `channel || 'beta'` und `offline_webview || 'false'` erhalten. |
| 3 | R04 | `e866d51e` | Zusätzlicher Konflikt in `windows/build-config.tests.ps1`: R02-Fallbacktests erhalten, R04-Positivfall beta.0 und Negativfall rc.0 kombiniert; beta.0 nicht mehr gleichzeitig als ungültig geprüft. |

Alle drei Merges sind eigene Merge-Commits mit dem verlangten Titel
`m3: Merge <paket> (Fable-Korrekturen, Beta-Vorzug)` und Codex-Trailer.

## Integrationsauflagen und Gegenprüfung R04/R01/R02

- `OPENSPIDER_UPDATE_BETA` ist die Build-Kanalvariable in Tauri (`build.rs`),
  Update-Policy und allen drei Release-Workflows. `1` bedeutet Beta, `0` stabil.
  Der Linux-CLI-Job erhält den aus der vorbereiteten Version abgeleiteten Kanal.
  R02s Beta-Fallback bei fehlenden Ereigniseingaben bleibt erhalten. Repo-weites
  Grep der beiden abgelösten Variablennamen: **0 Treffer**, auch kein alter
  Variablenname mehr in der negativen Testassertion. R01 validiert die Workflow-
  Eingabe weiterhin separat; die Konfiguration wird aus der gemeinsamen Beta-
  Build-Variable erzeugt.
- `release.yml` ruft `licenses.yml` per `workflow_call` auf. Linux-Build und
  Assemble warten auf das erfolgreiche Lizenz-Gate. Beide laden das Artefakt
  `third-party-notices` und verlangen eine nicht leere `THIRD_PARTY_NOTICES.txt`.
  Tatsächliche Pack-/Entpack-Fixtures prüfen Linux, Windows und macOS
  aarch64/x86_64/universal einschließlich Lizenztext. Signierte Updater-Archive
  bleiben bytegleich. Native Workflows setzen `OPENSPIDER_REQUIRE_NOTICES=1`
  ausschließlich im jeweiligen Tauri-Release-Build, nicht in Tests/Frontend-Builds.
- R04 übergibt R01 `version`, `channel`, `run_tests: true`; R02 erhält `channel`
  (R02 besitzt keine Eingaben `version`/`run_tests`). R01 prüft die vorbereitete
  Version ohne Lockfile-Mutation. R02 behält die korrigierten leeren Eingaben.
- Artefaktvertrag: `openspider-macos-{aarch64,x86_64,universal}`,
  `openspider-windows-x86_64`, `openspider-linux-x86_64`; R04 lädt
  `openspider-*`. R01s `artifact_names`-Output bleibt eine JSON-Liste tatsächlich
  vorhandener Artefakte; R04 sammelt die Dateien über das Artefaktmuster und
  benötigt keine zweite, abweichende Namensliste. R01 liefert die eigenständigen
  `openspider_<version>_darwin-<arch>.tar.gz` sowie eindeutig benannte
  `OpenSpider_<arch>.app.tar.gz[.sig]`. R02 liefert genau einen NSIS-/MSI-Installer
  und `openspider.exe`; Assemble erzeugt daraus das Windows-CLI-ZIP.
- Permissions: Standard `contents: read`; R01-Aufruf zusätzlich `actions: read`
  für die Artefaktliste, R02-Aufruf `id-token: write` für Azure/OIDC. Nur Draft
  erhält `contents: write`. Native Signierjobs, macOS-Universal-Smoke und Draft
  verwenden Environment
  `release`; macOS wurde entsprechend ergänzt. Der Universal-Smoke benötigt
  dieselbe Secret-Konfiguration für die korrekte Bewertung fehlender Artefakte.
  Das Lizenz-Gate benötigt keine Secrets.
- Vorhandener Release-Vertragstest ausgeführt und auf alle externen Actions in
  `release*.yml` und `licenses.yml` erweitert: ausschließlich 40-stellige SHA-Refs.
- Dem Desktop fehlte ein npm-Script `lint`; es ruft jetzt die bestehenden
  `check:static`-Prüfungen auf, ohne eine zusätzliche Lint-Konfiguration einzuführen.

## Prüfungen

Schwere lokale Befehle laufen mit `timeout 3600s nice -n 10`;
Cargo mit `CARGO_INCREMENTAL=0`, `-j 6 --locked` (außer `cargo fmt`, das diese
Build-Optionen nicht unterstützt). Dev-/Test-Debug-Infos sind in der Umgebung
abgeschaltet. Workspace-Tests verwenden `OPENSPIDER_SECRET_STORE=memory` und
`OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome`.
Logs liegen unter `/tmp/int-b-logs/`.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | Grün, Exit 0. |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets -j 6 --locked -- -D warnings` | Grün, Exit 0, 2m37s. Linux-Ausnahme `openspider-desktop` ausdrücklich verwendet. |
| `cargo test --workspace --exclude openspider-desktop -j 6 --locked` | Erster Lauf: 3.057 bestanden, 1 Portkonflikt in `os-parity --test run_site` (`Address already in use` vor Serverstart), 29 ignoriert; dort abgebrochen. Gezielte Gegenprobe **23/23 grün**, ohne Code-/Teständerung. Zweiter Gesamtlauf: 2.570 bestanden, 1 Fehler, 28 ignoriert; Abbruch in `os-loadtest --test engine_adapter`, RSS des kurzlebigen `list`-Kindes nur 12.288 Bytes. Gezielte Loadtest-Gegenprobe: **7 bestanden / 1 Fehler**, diesmal 266.240 Bytes statt mindestens 1 MB. Dritter Gesamtlauf mit `--no-fail-fast`: **1.895 bestanden / 2 U02-Fehler / 20 ignoriert**, 288 abgeschlossene Testprogramme; danach wegen akutem Plattenengpass gestoppt (Exit 130) und lokales `target/` gelöscht. **Kein vollständig grüner Gesamtlauf; übrige Tests nicht erreicht.** |
| `cargo test -p os-app --test update_install_flow -j 6 --locked` | **5 bestanden / 2 Fehler**, Exit 101; 5,62 s nach frischem Build (12m00s). Beide U02-Testnamen reproduziert; Details unten. |
| `OPENSPIDER_UPDATE_BETA=1 cargo test -p os-app --test update_security build_beta_variable -j 6 --locked` | **1/1 grün**, 3 andere Tests bewusst gefiltert. Endpunkt und Beta-Manifestpolicy gemeinsam geprüft; Build 2m33s, Test <0,01 s. |
| `cargo check -p openspider-desktop -j 6 --locked` | **Nicht möglich, Exit 101:** bekannter Fremdfehler E0063 in `tauri-plugin-deep-link 2.6.2` gegen `rust-ini 0.21.3`; `ParseOption` fehlen `enabled_indented_mutiline_value` und `enabled_preserve_key_leading_whitespace` (`src/lib.rs:539`). Schon im U02-Bericht belegt. Kein Dependency-/Registry-Patch und kein Lockfile-Update. |
| Desktop `typecheck && test && lint && build`, ohne `OPENSPIDER_REQUIRE_NOTICES` | Grün, Exit 0: Typecheck; **91 Dateien / 858 Tests** (187,88 s); Lint (`check:static`, IPC/Lizenzen/Tokens, zusätzlich **2 Dateien / 29 Tests**); Produktionsbuild (3.978 Module, 1,35 s). Nur Vite-Hinweis auf Chunks >500 kB. |
| Update-E2E Chromium + WebKit, `--workers=2` | **14/14 grün**, je 7 pro Engine; 57,6 s. Axe: keine critical/serious-Verstöße. |
| `node --test scripts/release/*.test.mjs scripts/release/licenses/generate.test.mjs tests/release/*.test.mjs` | **23/23 grün**, 0 Fehler/Skips; inklusive vorhandener Release-/Pin-Prüfung, Manifest-/Archiv-Fixtures und Lizenzgenerator-Tests. `zip` aus bereits vorhandenem `/tmp/r04-zip-tools/unpacked/usr/bin` im Test-PATH. |
| `python3 -m unittest discover -s tests/release -p 'test_*.py' -v` | **3/3 grün**, 10,350 s. |
| actionlint (`/tmp/l01-tools/actionlint -shellcheck= .github/workflows/*.yml`) | **18 Workflows grün**, keine Diagnosen; ShellCheck als separate optionale Integration deaktiviert. |
| YAML (PyYAML `safe_load`, alle Workflows) | **18/18 grün**. |
| `bash scripts/check-neutral.sh` | Grün, keine Fundstellen. |
| `git diff --check` | Grün. |

Die ersten beiden Workspace-Läufe bestanden sämtliche 27 Update-Rusttests
(Build-Konfiguration, Crawl-Sperren, Installation, Netzwerk, Sicherheit,
Commands). Im dritten Gesamtlauf scheitern zwei davon; siehe U02-Befund unten.
Die Gesamtzählung
verwendet jeweils das äußere Testprogramm-Ergebnis; ein intern erneut gestarteter
Proxy-Kindprozess wird nicht doppelt gezählt.

Platte: zu Beginn 101 GB frei. `df` wurde zunächst alle 30 s, bei knapper Platte
alle 10 s erfasst. Nach deutlichem Platzverbrauch durch parallele Builds wurde
zuerst das lokale `target/debug/incremental` entfernt. Anschließend wurden nur
bereits abgeschlossene Testprogramme aus diesem Worktree gelöscht (zunächst
195 Programme / 26,88 GiB; später bei <30 GiB automatisch). Die Automatik wartet
auf den Beginn des nächsten Testprogramms und lässt laufende/künftige Programme
sowie Bibliotheken bestehen. Insgesamt wurden so **288 abgeschlossene
Testprogramme / 36,36 GiB** freigegeben. Erfasstes Minimum: **4,94 GiB** frei
(5.179.680 KiB). Wegen des anhaltenden Engpasses wurde der dritte Gesamtlauf
gezielt gestoppt und das restliche `target/` vollständig gelöscht; das war kein
regulärer Testabschluss. Für die zwei kleinen Gegenproben wurde anschließend
neu gebaut. Auch dieses `target/` und die eigenen E2E-Ergebnisse sind am Ende
wieder gelöscht; danach `df -h .`: **22 G frei (94 % belegt)**. Keine fremden
Worktree-Artefakte gelöscht, keine fremden Prozesse beendet. Logs und
Ressourcenzählung bleiben unter `/tmp/int-b-logs/`.

Die geplanten separaten Restpaket-Prüfungen wurden bei erneut weniger als
25 GiB freiem Plattenplatz nicht gestartet. Der fehlende vollständige
Workspace-Nachweis bleibt ausdrücklich offen.

## Neuer U02-Integrationsbefund

Im dritten Gesamtlauf meldet `crates/os-app/tests/update_install_flow.rs`
**5 bestanden / 2 fehlgeschlagen**:

- `queued_after_crawl_installs_on_completion` (Zeile 233): fünfsekündiges Warten
  auf den Installer endet mit `Elapsed(())`.
- `install_error_does_not_block_quit_or_leave_reopen_ticket` (Zeile 312):
  Installer-Aufrufe 0 statt 1. Der Pfad vor dem Backend-Installer (u.a.
  Checkpoint/Ticket) ist weiter zu untersuchen; die Assertion belegt keinen
  tatsächlichen Aufruf des fehlschlagenden Mock-Installers.

Beide Tests waren in den ersten zwei Gesamtläufen grün. Der dritte Lauf fiel in
starke parallele Build-/Plattenlast. Das ist Kontext, kein Nachweis der Ursache.
Keine Erwartungen abgesenkt und kein Test ignoriert. Die gezielte Gegenprobe
nach dem Plattennotstopp reproduziert **beide Testnamen, 5/7 grün**. Beim
Quit-Test scheitert sie nun bereits an Zeile 302: `on_quit()` liefert `false`
(„Install-Fehler blockiert Quit nicht“); die abschließende Installerzählung wird
nicht erreicht. Der Timeout des zweiten Tests bleibt unverändert. Das ist ein
reproduzierbarer U02-Befund und benötigt eine eigene Korrektur mit erneutem
Review; die konkrete Ursache ist noch nicht nachgewiesen.

## Befunde außerhalb der integrierten Pakete

1. `crates/os-parity/tests/run_site.rs`: `free_port()` bindet an Port 0 und gibt
   den Port vor `run_sites()` wieder frei. Zwischen Auswahl und Serverstart kann
   ein anderer Test/Prozess den Port belegen. Erster Gesamtlauf scheitert deshalb
   vor der Markdown-Assertion; unveränderte gesamte Suite danach 23/23 grün.
2. `crates/os-loadtest/tests/engine_adapter.rs:316`: RSS-Test startet `os-loadtest
   list`, also einen sofort endenden Prozess. Linux misst in `rss.rs` `VmHWM`
   zyklisch alle 25 ms. Der zweite Gesamtlauf erfasste nur 12 KiB aus der frühen
   Prozessphase und verfehlte die unveränderte Untergrenze von 1 MB. Im ersten
   Gesamtlauf war die Suite vollständig grün; die gezielte Gegenprobe scheitert
   erneut mit 266.240 Bytes (7/8 Tests grün). Eine robuste Fixture muss den
   messbaren Kindprozess über mehrere Abtastintervalle am Leben halten (z.B.
   kleine lokale Crawl-Fixture), ohne die RSS-Grenzen abzusenken.

Keine Änderungen an diesen fremden Tests oder der RSS-Implementierung. M3.md §2
übernimmt M2.md §2a C10: „Braucht ein Fix eine fremde Datei, bricht das Paket ab
und meldet Datei, Diff-Auszug und Grund.“ Die Ausnahmen für Stub-Ablösung,
additive Schnittstellen und geänderte fachliche Erwartungen treffen hier nicht
zu. Der Integrationsauftrag umfasst die freigegebenen M3-Pakete und ihre
Verknüpfung; die bestehenden M2-Test-Fixtures benötigen ein eigenes Fixpaket.
Die übrigen abgeschlossenen Prüfungen sind oben dokumentiert.
Insbesondere wurden die späteren Testprogramme für Parser, Rendering-Crate,
Scheduler (einschließlich `reregister_update`), Store und Testsite in keinem
Gesamtlauf erreicht; dafür liegt in dieser Integration kein Testnachweis vor.

## Offene native Nachweise / Übergabe

Der Linux-Desktopcheck bleibt durch die bestehende Fremdabhängigkeit blockiert
(siehe Prüftabelle und `u02.md`). Die Behebung gehört in ein gesondertes
Dependency-Paket; dieser Lauf liefert keinen vollständigen Desktop-Kompiliernachweis.

1. **macOS-Dispatch:** nach Übernahme/Push durch den Orchestrator
   `release-macos.yml` mit `channel=beta`, `run_tests=true`, `arch=all` und zur
   vorbereiteten Workspace-Version passendem `version` starten. Environment
   `release` muss Branch/Tag erlauben und Apple-Konfiguration bereitstellen.
   R01s Bash-3.2-Korrektur, beide nativen Testarchitekturen, drei DMGs,
   Universal-Smoke, Signierung/Notarisierung, vollständige Lizenzressourcen und
   Updater-Signaturen belegen; Run-ID/URL/Head-SHA ergänzen.
2. **Windows-Dispatch:** `release-windows.yml` mit `channel=beta`,
   `offline_webview=false`; Environment `release`/Azure-OIDC sowie Updater-Schlüssel
   bereitstellen. R02s Fallbacktests einschließlich beta.0/rc.0, NSIS/MSI,
   Authenticode, stille Installation/Deinstallation und Bundle-Lizenztexte nativ
   belegen; Run-ID/URL/Head-SHA ergänzen. Offline-WebView-Variante bleibt ein
   zusätzlicher nativer Nachweis.
3. **R04-Trockenlauf:** Version/CHANGELOG nach dem R04-Prozess vorbereiten
   (aktueller Workspace: 0.1.0), auf dem vom Orchestrator gepushten vorbereiteten
   Branch `release.yml` mit passendem `release_tag` und `dry_run=true` starten.
   Kein Tag/Entwurf nötig. L01-Gate, reale Plattform-Artefakte, alle fünf
   CLI-Archive mit vollständigen Lizenztexten, SHA256SUMS, Release-Notes sowie
   bei vollständigen Signierschlüsseln beide Manifeste mit vier Zielschlüsseln
   prüfen. Ohne vollständige Schlüssel muss der Unsigned-Status sichtbar sein;
   dieser ersetzt keinen Signaturnachweis. Kein GitHub-Release darf entstehen.
4. R03-/D01-Hooks bleiben ausdrücklich sichtbare Stubs gemäß R04-Paketstand;
   plattformübergreifende Artefakt-Smokes, Migrationsarchiv und offene M2-Parität
   vor einer Veröffentlichung separat abnehmen. Eine Veröffentlichung bleibt
   an das Nutzer-Go gebunden.

## Abschluss und nächster Schritt

Alle drei freigegebenen Pakete sind lokal gemergt; die Integrationsauflagen an
Release-/Lizenz-Gate, Kanal, Artefakte, Permissions und Environment sind umgesetzt.
Kein Push. Arbeitsverzeichnis nach dem Bericht-Commit sauber; `target/` fehlt.

**PAKET OFFEN:** Zuerst U02 für die zwei reproduzierten Installations-Testfehler
wieder öffnen und die M2-Port-/RSS-Fixtures im zuständigen Paket stabilisieren.
Danach vollständigen Pflichtlauf ohne Plattenkonkurrenz wiederholen (einschließlich
der nicht erreichten Pakete und Scheduler-Regression). Erst mit grünem Ergebnis
die oben beschriebenen nativen Dispatches und den R04-Trockenlauf durchführen.
Der bekannte Linux-Desktop-Dependencyfehler bleibt separat zu beheben.
