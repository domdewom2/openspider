# B3-UI – Funde aus den Beta-Tests (beta.3)

Stand: 05.11.2026. Branch `m3/ui3`, Basis `origin/main` `ebeaf768`. Lokaler Worktree `openspider-wt/c-ui3`, Bauen und
Testen ausschließlich auf dem Build-Server (Worktree `openspider-wt/m3-ui3`, eigenes `target/`). Kein Push. Modell:
Claude Opus 5.5.

Reihenfolge nach `docs/plan/beta3.md` §3. Jeder Fund mit Ursache, Fix, Dateien und Test.

## 1. Befunde-Seitenleiste: Namen auf Englisch

- **Ursache:** Weder Katalog noch `.ftl`: Der Kern liefert Tab und Filter jedes Befunds als kanonische englische Namen
  (`sidebar.issues`, `IssueRow.tab`/`filter`, Konvention 05 „Spalten- und Filterschlüssel“). Übersicht und Dashboard
  übersetzen sie über den Kanon-Katalog (`i18n/canon`), die Befunde-Liste und die Überschrift der Erklärung setzten sie
  fest im Code roh zusammen (`` `${row.tab}: ${row.filter}` ``). Alle Übersetzungen waren vorhanden (Prüfung: jeder der
  291 Katalogeinträge hat de-Labels für Tab und Filter).
- **Fix:** `issueLabel(t, row, config)` in `features/sidebar/issues/Issues.tsx` (Tab über `tabLabel`, Filter über
  `filterLabel` mit aufgelöster Schwelle, A01-13); genutzt von Befunde-Liste, Erklärungsüberschrift
  (`IssueExplanation` erhält `title`) und Dashboard „Top-Probleme“ (dort wurden Schwellen-Platzhalter bisher nicht
  aufgelöst). Die Suche läuft jetzt im Client auf dem angezeigten und dem kanonischen Namen; der Kern bekommt
  `search: null`, weil er nur die englischen Namen kennt (sonst fände „Seitentitel“ nichts). Navigation nutzt weiter den
  kanonischen Filter.
- **Sprach-Audit:** `language-audit.test.ts` prüft zusätzlich, dass jeder Eintrag aus `core/issues/catalog.toml` und
  `catalog.d/*.toml` ein deutsches Tab- und Filter-Label hat (kein Rückfall auf den Kernnamen). Neu
  `language-audit.render.test.tsx`: rendert Seitenleiste (Übersicht, Befunde mit Erklärung) und Dashboard in Deutsch mit
  der Seitenleisten-Fixture und schlägt fehl, wenn ein übersetzbarer Kernname roh im Text steht. Gegenprobe: mit dem
  alten `Issues.tsx` schlägt der Render-Audit fehl.
- **Dateien:** `features/sidebar/issues/{Issues,IssueExplanation}.tsx`, `features/dashboard/Dashboard.tsx`,
  `i18n/language-audit.test.ts`, `i18n/language-audit.render.test.tsx`, `features/sidebar/frame/sidebar.test.tsx`.
- **Tests:** `sidebar.test.tsx` „shows issue names, explanation heading and search in German …“ (`// spec: UI-062`,
  `UI-121`), beide Audit-Tests (`// spec: UI-121`). **K2-63:** Zwei englische Erwartungen ändern sich von der
  Fixture-Schreibweise `(4xx)`/`(3xx)` auf die Kanon-Anzeige `(4XX)`/`(3XX)` (wie Übersicht und die Alpha-E2E
  `CLIENT_ERROR`), und die Erwartung „Kern erhält `search: "DUPLICATE"`“ wird zu `search: null` mit gleichem
  Trefferergebnis im Client (Begründung oben; die deutsche Suche ist zusätzlich geprüft).

## 2. „Kein Crawl geöffnet.“ ohne Innenabstand

- **Ursache:** Ohne geöffneten Crawl antwortet `view.open`/`catalog.columns` mit `NoCrawl`. `UpperTable` zeigte die
  Meldung als nacktes `<div role="alert">` zwischen Werkzeugzeile und Tabelle (kein Abstand) und darunter zusätzlich
  den Leerzustand `NO_MATCH` („Keine Zeile passt …“), obwohl UI-016 für „noch kein Crawl“ `NO_DATA_YET` vorsieht (Beleg:
  Snapshot in `docs/plan/M2-reports/int-18/webkit-preferences-error.txt`).
- **Fix:** `NoCrawl` ergibt den Leerzustand `NO_DATA_YET` (Primitive `EmptyState`, Innenabstand `--os-space-24`) ohne
  Fehlerzeile. Andere Fehler über der Tabelle als `Banner`-Primitive in `.u10-message` (Abstand
  `--os-space-4`/`--os-space-8` wie die Werkzeugzeile); die Zeile „Daten veraltet“ bekommt denselben Abstand.
- **Dateien:** `features/table/{UpperTable.tsx,table.css,table.test.tsx}`.
- **Test:** `table.test.tsx` „shows the padded no-data state instead of a bare “No crawl is open” line …“
  (`// spec: UI-016`).

## 3. Start-URL leer nach dem Öffnen eines gespeicherten Crawls

- **Spec:** UI-003, CRAWL-001 und CRAWL-127 sagen nichts dazu, was das URL-Feld nach dem Öffnen zeigt.
- **Vorschlag OS-Festlegung (UI-003):** „Öffnet der Nutzer einen gespeicherten Crawl (Datei → Öffnen, Zuletzt geöffnet,
  Crawl-Verwaltung, Wiederöffnen nach Update), zeigt das URL-Feld die Start-URL dieses Crawls; Modus folgt dem Crawl.
  Im Listen- und SERP-Modus bleibt das Feld unverändert. Die Übernahme geschieht nur beim Wechsel des Crawls, Eingaben
  im Feld werden durch Live-Aktualisierungen nicht überschrieben; die Start-URL kommt erst mit einem Start in die
  Vorschlagsliste.“ So umgesetzt.
- **Ursache:** Die Kopfleiste kannte die Start-URL nur aus der eigenen Eingabe; der Status eines geöffneten Crawls
  (`CrawlStatus`) trug sie nicht.
- **Fix:** `CrawlStatus.start_url` (optional, `skip_serializing_if`, also additiv) aus `crawl_meta.start_url` in
  `session::feed::refresh`, erhalten in `settled`; ts-rs neu erzeugt (`os-app-manifest`). `Header.tsx` übernimmt die
  URL bei neuem `crawlId` (bzw. wenn sie mit einem späteren Status eintrifft). Die Lösung über den Status deckt alle
  Öffnungswege ab, auch das Öffnen im Kern nach einem Update (`update/pending.rs`).
- **Dateien:** `crates/os-app/src/{dto/events.rs,session/feed.rs}`, `crates/os-app/tests/session_feed.rs`,
  `apps/desktop/src/ipc/gen/CrawlStatus.ts`, `features/header/{Header.tsx,Header.test.tsx}`.
- **Tests:** Rust `session_feed::opened_crawl_status_carries_its_start_url_for_the_header` (`// spec: UI-003`), Vitest
  `Header.test.tsx` „shows the start URL of a reopened crawl …“ (`// spec: UI-003`).

## 4. Fenstertitel der Visualisierung in der falschen Sprache

- **Ursache:** Tauri übernimmt `document.title` nicht in den nativen Fenstertitel. Das Visualisierungsfenster wurde in
  `src-tauri/src/desktop.rs` mit dem fest deutschen Titel „OpenSpider – Visualisierung“ angelegt und nie umbenannt;
  das Einstellungsfenster erhielt den Titel nur beim Anlegen (nach einem Sprachwechsel veraltet).
- **Fix:** `app/windowTitle.ts` (`useWindowTitle`) setzt `document.title` und in Tauri `getCurrentWindow().setTitle()`
  bei jedem Wechsel von Titel bzw. Sprache; genutzt in Haupt-, Einstellungs- und Visualisierungsfenster (dort auch vor
  der Artwahl: „Visualisierung – OpenSpider“). Die Hülle legt das Fenster neutral mit „OpenSpider“ an. Berechtigung
  `core:window:allow-set-title` bestand bereits.
- **Dateien:** `app/windowTitle.ts`, `App.tsx`, `features/settings/window/SettingsWindow.tsx`,
  `features/viz/VizWindow.tsx`, `features/viz/title.test.tsx`, `src-tauri/src/desktop.rs`.
- **Tests:** `features/viz/title.test.tsx` (Tauri-Fenster gemockt; de/en, Sprachwechsel, Einstellungsfenster;
  `// spec: UI-121`, `UI-090`).

## 5. Update-Dialog: Knöpfe brechen zweizeilig um

- **Ursache:** Bis zu vier Knöpfe (Installieren jetzt, beim Beenden, Herunterladen, Schließen) standen in einer
  Fußzeile mit `margin-left:auto` in einem 520 px breiten Dialog; die langen Beschriftungen brachen um.
- **Fix:** Aktionen in voller Breite untereinander (`features/update/update.css`, `white-space:nowrap`), primäre Aktion
  „Jetzt installieren und neu starten“ zuerst, dann „Beim Beenden installieren“, „Im Hintergrund herunterladen“,
  „Schließen“. Texte unverändert (sie sind die Spec-Texte aus OUT-251).
- **Spec:** OUT-251 nennt nur die Beschriftungen, kein Layout; nichts nachzuziehen. Vorschlag (optional) für OUT-251:
  „Die Aktionen des Update-Dialogs sind in allen Sprachen einzeilig (Prüfung bei 1024×640).“
- **Dateien:** `features/update/{UpdateHost.tsx,update.css}`, `e2e/update/update.spec.ts`.
- **Test:** Playwright `update.spec.ts` „de/en: update actions stay on one line at the minimum window size“ – je Knopf
  eine Zeilenbox (`Range.getClientRects`), kein Überlauf, Inhaltshöhe unter zwei Zeilenhöhen, gestapelt
  (`// spec: OUT-251`, `UI-121`).

## 6. „Hilfe → Nach Updates suchen“ öffnete nach über 17 Stunden keinen Dialog

**Ergebnis: Die Ursache ließ sich nicht sicher nachweisen** (kein Log und kein Zustand der betroffenen Instanz). Der
Pfad Menü → `runAction("help.updates")` → `checkUpdates()` setzt `open: true` synchron, bevor irgendetwas abgefragt
wird; ein hängender Kern hätte also einen Dialog mit „Suche nach Updates …“ gezeigt, keinen fehlenden Dialog.
Untersucht und bewertet:

| Kandidat | Befund | Maßnahme |
|---|---|---|
| Dialog liegt im Hauptfenster; Menüleiste (macOS) ist auch bei minimiertem/verstecktem Fenster bedienbar | **wahrscheinlichste Erklärung** für „kein Dialog“: er öffnet sich unsichtbar im minimierten Fenster; nach dem Neustart ist das Fenster sichtbar | Menüaktion holt das Hauptfenster zuerst nach vorn (`isMinimized` → `unminimize`, `show`, `setFocus`); Berechtigung `core:window:allow-unminimize` ergänzt |
| Kern: `run_check` wartet auf die Sperre `gate`, die ein Hintergrund-Download für seine ganze Dauer hält (Zeitgrenze 1 h; nach einem Ruhezustand kann die Verbindung bis dahin hängen) | echte Wartestelle (Dialog bliebe auf „Suche …“) | Bei `Downloading`/`Ready` antwortet `run_check` sofort mit dem Zustand, ohne die Sperre |
| Frontend: Prüf-Promise ohne Zeitgrenze, kein Schutz gegen eine noch laufende ältere Prüfung | endloses „Suche …“ möglich, späte Antwort einer alten Prüfung überschreibt eine neue | Zeitgrenze 20 s mit eigener Meldung und „Erneut suchen“; Generationszähler, ältere Prüfung wird abgebrochen und ihre Antwort verworfen; Status-Abfragen überschreiben eine laufende Prüfung nicht |
| Polling-Flag `busy` in `UpdateHost`: hängt eine `update.status`-Antwort (z. B. nach dem Ruhezustand), bleibt es gesetzt und das Polling steht für immer | echter Fehler, betrifft aber nur die Statusanzeige | Status-Abfrage mit Zeitgrenze 10 s; nach Fokus/Sichtbarkeit sofort neu abfragen |
| Aktionswächter (`menuContextReason`) | blockt nur Aktionen mit Tastaturkürzel bei offenem Modal; `help.updates` hat keins | – |
| Natives Menü (Tauri 2.12.3 `MenuChannels`, Schlüssel = Menü-ID) | Handler je ID werden bei jedem Neuaufbau ersetzt; alte Einträge werden nie geschlossen, daher entfernt ihr `Drop` den aktuellen Handler nicht – kein Defekt gefunden (siehe offene Punkte) | – |
| Dialogzustand nach dem Schließen | `open` wird über `onOpenChange` zurückgesetzt, `checkUpdates` setzt `queued` zurück; Test „reopens the dialog after it was closed while a check was still running“ | – |

- **Dateien:** `features/update/{state.ts,UpdateHost.tsx,UpdateHost.test.tsx,i18n/de.ftl,i18n/en.ftl}`,
  `src-tauri/capabilities/default.json`, `crates/os-app/src/commands/update.rs`,
  `crates/os-app/tests/update_install_flow.rs`. Neue Schlüssel `update-check-timeout`, `update-retry` (de/en).
- **Tests (Fake-Timer, hängende Promises):** `UpdateHost.test.tsx` „always shows a state for a manual check …“,
  „reopens the dialog after it was closed …“, „keeps polling after a status request got no answer“, „brings a minimized
  main window to the front …“ (`// spec: OUT-250`, `OUT-251`, `UI-002 [M3]`); Rust
  `update_install_flow::manual_check_answers_at_once_while_a_download_holds_the_gate`. Gegenprobe: drei der vier neuen
  Vitest-Fälle schlagen mit dem alten Code fehl; der Wiederöffnen-Fall sichert bestehendes Verhalten ab.

## 7. Low-Punkte aus den Backlogs

| Punkt | Commit | Test |
|---|---|---|
| V02 „Legende der Zusatzkanten nutzt feste Standardfarben statt der Token-Farben“: die Seitenleisten-Legende erhält die Themenfarben, die Zeichnung, Kurzlegende und Exporte schon nutzen (2D und 3D; Test nur 2D, 3D braucht WebGL) | `ui: Legende der Zusatzkanten …` | `v02.test.tsx` „zeigt Inlinks in der Seitenleisten-Legende in der Themenfarbe …“ (`// spec: UI-097`, `UI-120`) |

Nicht angefasst (Risiko oder kein sinnvoller Test ohne WebGL): 3D `resize()` setzt den Beschriftungs-Cache nicht zurück,
`onPointerLeave` bricht den Treffertest nicht ab, Szenenwechsel bei Kontextverlust (V02); V01-Backlog: CSV-Einstellungen,
Esc im Diagramm, Pfeiltasten-Navigation (Verhaltensentscheidungen). V01 „Balken `role="group"`“ ist bereits umgesetzt.

## Commits

| Commit | Inhalt |
|---|---|
| `aa95712d` | ui: Befunde-Seitenleiste in der UI-Sprache, Sprach-Audit (Fund 1) |
| `5aa4919f` | ui: Leerzustand ohne geöffneten Crawl (Fund 2) |
| `4cd452a2` | ui: Fenstertitel in der UI-Sprache (Fund 4) |
| `2ab68de4` | ui: Legendenfarben der Zusatzkanten (V02-Backlog, Punkt 7) |
| `fedec433` | ui: Update-Dialog, Aktionen einzeilig (Fund 5) |
| `85fa6f72` | app: Start-URL nach dem Öffnen (Fund 3) |
| `f2e134c4` | app: „Nach Updates suchen“ robust (Fund 6) |
| (dieser) | docs: Bericht |

## Testergebnisse (Server)

Alle Läufe im Server-Worktree `m3-ui3` auf dem Endstand der Commits.

- `npm run -w apps/desktop typecheck`: ok. `npm run -w apps/desktop test`: CSS-Lint ok, **Vitest 116 Dateien,
  1001 Tests bestanden**, 0 fehlgeschlagen. `check:static` ok (30 Tests), `gen-ipc --check` ok.
- **Playwright** (eigene Konfiguration außerhalb des Repos mit Vite-Port 1437 und `reuseExistingServer: false`, damit
  kein fremder Dev-Server wiederverwendet wird; `--workers=1`, Engines nacheinander, echte Bridge `os-app-bridge`):
  `update/update.spec.ts`, `sidebar/sidebar.spec.ts`, `header/header.spec.ts`, `statusbar/statusbar.spec.ts`,
  `i18n/*` (dialogs, exports, help, language, text-fit), `viz/viz.spec.ts`, `viz/v02.spec.ts`,
  `alpha/detail-sidebar.spec.ts`, `alpha/crawl-control.spec.ts`, `beta/overview.spec.ts`, `sidebar-chart/live.spec.ts`,
  `polish/layout.spec.ts` – **Chromium 87/87, WebKit 87/87 bestanden**. Hinweis: Ein Vorlauf ohne gebaute Bridge
  (Rückfall auf den Mock-Transport) scheiterte nur an `header.spec.ts` „starts, pauses, resumes and clears a saved crawl
  through the bridge contract“ (der Mock kennt einen Befehl nicht, Hinweis „This function is not available yet.“);
  mit Bridge grün. Nicht auf `main` gegengeprüft; der Fehler liegt im Mock-Rückfall, nicht in den geänderten Pfaden.
- **Rust:** `cargo fmt --all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; gezielt
  `cargo test -p os-app --test session_feed --test update_install_flow --test dto_exports` ok (21 Tests).
  `cargo test --workspace`: **4354 bestanden, 0 fehlgeschlagen, 51 ignoriert**.
- **Plattenplatz:** Der erste Workspace-Lauf brach mit „No space left on device“ ab (eigenes `target/` nach Clippy,
  Bridge- und Testbau 114 GB, daneben `m3-rel3` 119 GB; Server kurz bei 96 %). Eigenes `target/` sofort gelöscht,
  Lauf auf frischem `target/` wiederholt (Ergebnis oben). Falls ein paralleler Lauf (Release-Paket, B3-QUEUE) um
  diese Zeit mit Plattenfehler endete, liegt es an diesem Engpass. Am Ende `target/`, Testartefakte und Logs gelöscht;
  der Server-Worktree belegt nur noch Quellen und `node_modules`.

## Zusätzlich geänderte Dateien (K2-92)

Das Paket hat keine eigene Owns-Liste; alle Dateien gehören Bereichen früherer Pakete. Jeweils minimal:

| Datei | Bereich | Änderung |
|---|---|---|
| `crates/os-app/src/dto/events.rs` | Kern/IPC (F05) | optionales Feld `CrawlStatus.start_url` (additiv, `skip_serializing_if`) |
| `crates/os-app/src/session/feed.rs` | Sitzung (N03/K04) | Feld befüllen bzw. in `settled` erhalten |
| `crates/os-app/src/commands/update.rs` | Updater (U02) | Zustandsprüfung vor der Sperre |
| `crates/os-app/tests/{session_feed,update_install_flow}.rs` | Tests | je ein neuer Test |
| `apps/desktop/src/ipc/gen/CrawlStatus.ts` | generiert | `os-app-manifest` |
| `apps/desktop/src-tauri/src/desktop.rs` | Hülle | neutraler Anfangstitel des Visualisierungsfensters |
| `apps/desktop/src-tauri/capabilities/default.json` | Hülle | `core:window:allow-unminimize` |
| `apps/desktop/src/App.tsx`, `features/settings/window/SettingsWindow.tsx` | Rahmen | Titel über `useWindowTitle` |
| `features/table/*`, `features/header/*`, `features/dashboard/Dashboard.tsx`, `features/sidebar/*` | U10, U03, U35, U12 | siehe Fund 1–3 |
| `features/viz/{VizWindow,Sidebar,GraphView,Graph3DView}.tsx`, `v02.test.tsx` | V02 | Titel; Legendenfarben |

Kein Manifest, kein Lockfile, keine neue Abhängigkeit, kein `cargo update`.

## Spec-Fragen und offene Punkte

1. **UI-003** – OS-Festlegung „Start-URL nach dem Öffnen“ (Text oben, Fund 3) bitte bestätigen und in die Spec
   übernehmen.
2. **OUT-251** – optionales Kriterium „Aktionen einzeilig“ (Fund 5).
3. **Export der Seitenleisten-Tabellen** (Kopieren/Exportieren in Übersicht und Befunde) nutzt weiter die kanonischen
   englischen Namen, die Spaltenköpfe aber die UI-Sprache (bestehendes Verhalten, `overviewRows`/`issueRows`). Soll das
   der Exportoption „Spaltennamen in UI-Sprache“ folgen?
4. **Vergleichsmodus** (`ComparisonNumbers`): Tab-Namen und die Spaltenköpfe „Current/Previous/Change“ erscheinen
   unübersetzt; nicht Teil dieses Pakets (Bereich Vergleich), Kandidat für den Render-Audit.
5. **Kanon-Lücke:** Filter `Flash` (Internal/External, Harness-Katalog) hat kein deutsches Label.
6. **Natives Menü, Langläufer:** Jeder Neuaufbau (u. a. bei jedem Fokuswechsel zwischen Tabelle, Feld und Dialog) legt
   alle Menüeinträge neu an, geschlossen wird nur das oberste Menü. Die alten Einträge bleiben als Ressourcen im
   Speicher (Wachstum über viele Stunden). Vorsicht bei einer Bereinigung: Tauri entfernt beim `Drop` eines Eintrags
   den Handler seiner ID – und die ID ist beim Nachfolger dieselbe; ein naives Schließen der alten Einträge würde das
   Menü stumm schalten. Vorschlag: Einträge nur bei geänderter Beschriftung/Aktivierung neu anlegen
   (`setText`/`setEnabled`) oder eindeutige IDs je Aufbau.
7. **Fund 6** bleibt offen, bis er erneut auftritt; dann bitte prüfen, ob das Hauptfenster minimiert oder ausgeblendet
   war, und das App-Log der Instanz sichern.
