# Alpha-Gate M2-α: Befundliste (M2-A01)

Stand 21.10.2026, Branch `m2/a01` (Basis `origin/main` d9cc198). Quelle: die E2E-A-Alpha-Suite `apps/desktop/e2e/alpha/**` gegen die echte
`os-app-bridge` und das Vite-Frontend, dazu die Kommandozeile `openspider` und `os-testsite ui/fx-ui-basic`. Befunde werden nur gemeldet, nicht
außerhalb von `owns` behoben (Auftrag, M2.md 2a C10). Jeder Befund, an dem ein Alpha-Kriterium scheitert, hat einen Test mit
`test.fixme(…, 'Befund A01-nn …')` in der genannten Datei; `grep -rn "Befund A01-" apps/desktop/e2e/alpha` liefert die Zuordnung.

**Plattform:** Alle Befunde sind auf macOS arm64 lokal (Chromium und WebKit von Playwright) beobachtet. Das Verhalten liegt in Kern und
Frontend und ist nicht plattformabhängig, soweit nicht anders angegeben; die CI-Läufe auf macOS-WebKit, Windows-Chromium und Linux stehen im
Bericht `docs/plan/M2-reports/a01.md`.

**Schwere:** Blocker = ein Alpha-Kriterium ist ohne Behebung nicht erfüllbar und der Hauptpfad ist unbenutzbar; Hoch = Kriterium oder
Hauptfunktion scheitert; Mittel = Funktion verfälscht oder unzuverlässig, Umgehung möglich; Niedrig = Darstellung, Hinweis, Komfort.

**Reproduktion ohne Browser:** Die Bridge spricht WebSocket (`{"id":n,"cmd":"…","args":{…}}`, Anmeldung `{"cmd":"__subscribe","window":"main"}`).
`cargo build -p os-app --features e2e-bridge --bin os-app-bridge -p os-testsite`, `os-testsite ui/fx-ui-basic` starten (gibt die Basis-URL aus),
`os-app-bridge --home <leerer Ordner>` starten (gibt `{"port":…,"token":…}` aus). Befehle unten sind in dieser Form gemeint.

## Übersicht

| ID | Titel | Schwere | Spec | Alpha-Kriterium | Test (fixme) |
|---|---|---|---|---|---|
| A01-01 | Tabs Internal und Content lassen sich nach einem Crawl nicht öffnen (`view.open` → Store-Fehler) | Blocker | UI-004, UI-007, OUT-020 | 2, 4 | `table.spec.ts` |
| A01-02 | Auswahl in der Haupttabelle erreicht das Detailfenster nie | Hoch | UI-030, UI-012 | 3 | `detail-sidebar.spec.ts` |
| A01-03 | Klick in Overview/Issues navigiert die Tabelle nicht | Hoch | UI-006, UI-061, UI-062 | 3 | `detail-sidebar.spec.ts` |
| A01-12 | Image Details und Resources: Oberfläche fragt dem Kern unbekannte Spalten an | Hoch | UI-030, ONPAGE-172 | 3 | `detail-sidebar.spec.ts` |
| A01-14 | Export der Ansicht: kein Dialog mit Formatwahl, kein XLSX aus der Tabelle | Hoch | OUT-006, OUT-020, UI-015 | 4 | `export.spec.ts` |
| A01-15 | XLSX-Blattname der UI-Exporte weicht von der CLI ab („Export“) | Mittel | OUT-001, OUT-007 | 4 | `export.spec.ts` |
| A01-04 | `config.set` ohne offenen Crawl wirkt nicht auf `crawl.start` | Mittel | OUT-220, CRAWL-126 | 1 (Umgehung im Test) | Hilfsfunktion `useProfile` |
| A01-06 | Nach `crawl.stop` zeigt die Statusleiste „Completed“ und 100 % bei Rest > 0 | Mittel | UI-013 | 1 | `crawl-control.spec.ts` |
| A01-08 | Tab-Leiste überlappt die Schaltfläche „Configure tabs“ | Mittel | UI-001, UI-004 | 5 | `appearance.spec.ts` |
| A01-09 | Sortier-Geste bleibt bei laufendem Crawl oft wirkungslos | Mittel | UI-010 | 2 | Kommentar in `table.spec.ts` |
| A01-10 | Beim Einsortieren rutscht die markierte Zeile um 1–2 Positionen | Mittel | UI-018 | 2 | `table.spec.ts` |
| A01-16 | Klick auf eine Zeile der laufenden, sortierten Tabelle: Fehlerbanner, Auswahl weg | Mittel | UI-018, UI-142 | 2 | `table.spec.ts` |
| A01-20 | Filterzähler M bleibt nach gedrosseltem Crawl hinter den Zeilen zurück | Mittel | UI-018, UI-006 | 2 | `table.spec.ts` |
| A01-05 | Kein Bedienelement für `crawl.stop` | Niedrig (Rückfrage) | UI-003, CRAWL-126 | 1 | Kommentar in `crawl-control.spec.ts` |
| A01-07 | Live-Aktualisierung: Platzhalter „…“ (Flackern), Zähler N > M | Niedrig | UI-142, UI-018 | 2 | Tests tolerieren es |
| A01-11 | HTTP Headers ohne Extraktion: „not available yet“ statt Voraussetzung | Niedrig | UI-030, UI-016 | 3 | `detail-sidebar.spec.ts` |
| A01-13 | Issue-Namen mit unaufgelösten Platzhaltern | Niedrig | UI-062, UI-064 | 3 | `detail-sidebar.spec.ts` |
| A01-17 | Seitenleiste: Mausklick auf „Spelling & Grammar“ gelegentlich abgefangen (WebKit) | Niedrig | UI-060, UI-001 | 3 | `detail-sidebar.spec.ts` (nur WebKit) |
| A01-22 | Bulk-/Report-Menü: Beschriftungen abgeschnitten, Schlüssel ohne Übersetzung | Niedrig | UI-002, UI-121 | 4, 7 (Warnungen) | – |
| A01-18 | Statusleiste: „Unavailable menu actions (n)“ stößt an „Version“ | Niedrig | UI-013 | 5 | – |
| A01-19 | Gespeicherte Tab-Leiste (Reihenfolge, Sichtbarkeit) geht gelegentlich verloren | Mittel | UI-004 [P1] | 2 | `table.spec.ts` |
| A01-21 | Tabelle bleibt nach Tabwechsel direkt nach Crawl-Ende gelegentlich leer | Mittel | UI-142, UI-004 | 2 | Wiederholung im Test |

## Befunde

### A01-01 Internal und Content lassen sich nicht öffnen (Blocker)

- **Datei:** Kern: `crates/os-analyze/src/views/session/mod.rs` (Zeilenabfrage der Ansicht, bindet nur `built.plan.params`; vermutlich fehlt hier
  `tabs::content::bind_analysis_params` wie in `views.rs` `query_view_in`), Spalten `crates/os-analyze/src/tabs/content.rs`
  (`p1_column`, `analysis_columns`; benannte Parameter `:readability_enabled`, `:near_ready`, `:spelling_enabled`, `:grammar_enabled`).
  Anfrager: `apps/desktop/src/features/table/controller.ts` Zeile 49–51 (`view.open` mit **allen** Spalten aus `catalog.columns`).
- **Reproduktion:** `crawl.start` auf die Fixture, nach „Completed“ `view.open` mit
  `{spec:{kind:"tab",tab:"Internal",filter:"All",search:null,sort:[]},columns:["Address","Flesch Reading Ease Score"]}` →
  `Store: Wrong number of parameters passed to query. Got 0, needed 1`. Betroffen sind die sieben Spalten Flesch Reading Ease Score,
  Readability, Closest Similarity Match, No. Near Duplicates, Spelling Errors, Grammar Errors, Language; alle übrigen 31 Tabs sind in
  Ordnung (Schleife über `catalog.tabs` mit allen Spalten: nur Internal und Content scheitern). In der App: Crawl starten, Standard-Tab
  „Internal“ zeigt „Showing: 0 of 25“ und die Meldung „Store“ statt der 25 Zeilen.
- **Spec-ID:** UI-004 (Tab zeigt Tabelle), UI-007, OUT-020; Alpha-Kriterium 2 und 4.
- **Auswirkung:** Der Start-Tab ist nach jedem Crawl unbenutzbar, der Export „Internal:All“ aus der UI ebenfalls. Die Alpha-Suite weicht auf
  Response Codes, Page Titles und External aus und führt den Export „Internal:All“ über den Kernbefehl `export.view` (funktioniert, die Standardspalten
  enthalten die sieben Spalten nicht).
- **Test:** `table.spec.ts` „the Internal tab lists all 25 internal URLs …“ (fixme).

### A01-02 Auswahl erreicht das Detailfenster nie (Hoch)

- **Datei:** `apps/desktop/src/features/detail/frame/selection.ts` (`setDetailSelection`, „top-table owner: publish URL ids“) – in der App ruft
  es **nichts** auf (`grep -rn setDetailSelection apps/desktop/src` trifft nur Tests). Zuständig wäre `features/table/controller.ts`
  (`setSelection`) bzw. `UpperTable.tsx`.
- **Reproduktion:** Crawl, Tab „Response Codes“, Zeile anklicken (Zähler zeigt „1 selected“), im Detailfenster bleibt in allen Reitern
  „Select a row in the table above to see its details.“
- **Spec-ID:** UI-030 (Detailfenster zeigt die Auswahl), UI-012 (Detailfenster folgt der Fokuszeile), UI-145; Kriterium 3.
- **Test:** `detail-sidebar.spec.ts` „selecting a row in the main table loads its URL Details“ (fixme). Die Detail-Reiter selbst prüft ein aktiver
  Test, der die Auswahl über `setDetailSelection` veröffentlicht: URL Details, Inlinks (20), Outlinks (25) und HTTP Headers laufen gegen den Kern.

### A01-03 Overview und Issues navigieren nicht (Hoch)

- **Datei:** `apps/desktop/src/features/sidebar/frame/state.ts` (`navigate` setzt `target` und sendet `os:table-navigate`); in `features/table/**`
  liest beides niemand.
- **Reproduktion:** Seitenleiste „Issues“, auf „Response Codes: Internal Client Error (4XX)“ klicken: die Erklärung erscheint, der Tab
  bleibt (hier Response Codes) und der Filter bleibt „All (27)“; aus einem anderen Tab heraus wechselt auch der Tab nicht.
- **Spec-ID:** UI-006 (Klick in Overview/Issues setzt Tab und Filter), UI-061, UI-062; Kriterium 3.
- **Test:** `detail-sidebar.spec.ts` „a click on an issue or an Overview row navigates the table …“ (fixme); aktiv geprüft ist, dass die
  Seitenleiste `{kind:"tab",tab:"Response Codes",filter:"Internal Client Error (4XX)"}` veröffentlicht.

### A01-12 Image Details und Resources scheitern an unbekannten Spalten (Hoch)

- **Datei:** Oberfläche `apps/desktop/src/features/detail/images/**` und `…/resources/**` (angefragte Spaltenlisten) gegen die Ansichtskataloge des
  Kerns (`crates/os-analyze/src/details*`, `view.open` Art `images` bzw. `resources`).
- **Reproduktion:** Detailauswahl der Startseite (über `setDetailSelection`, siehe A01-02), Reiter „Image Details“: `view.open` mit
  `spec.kind="images"` und Spalte „Alt Text Length“ → `InvalidInput err-unknown-column {"column":"Alt Text Length"}`; Reiter „Resources“ mit
  Spalte „Content Type“ → `err-unknown-column`. Anzeige in beiden Reitern: „The input is invalid.“ (die Fixture hat ein Bild `pixel.svg`).
- **Spec-ID:** UI-030, ONPAGE-172 (Image Details); Kriterium 3 (Image Details, Resources).
- **Test:** `detail-sidebar.spec.ts` „Image Details list …“ und „Resources list …“ (fixme).

### A01-14 Export der Ansicht ohne Format-Dialog (Hoch)

- **Datei:** `apps/desktop/src/features/table/UpperTable.tsx` (eigener CSV-Dialog „Destination CSV file“); `features/export/model.ts`
  (`registerExportContext`) wird weder von Tabelle noch Detailfenster noch Seitenleiste aufgerufen (`PanelActions.tsx` Zeile 20 vermerkt es als offen).
- **Reproduktion:** Crawl, Tab „Response Codes“, Schaltfläche „Export“ der Tabelle: Dialog mit Textfeld für den Dateipfad, nur CSV, kein
  Formatwahl-Dialog, kein vorgeschlagener Dateiname, kein „Ordner öffnen“; Mod+E und die große Schaltfläche links bleiben deaktiviert.
- **Spec-ID:** OUT-006 (Exportdialog und Formatwahl), OUT-020, UI-015; Kriterium 4 („Export der Ansicht als CSV und XLSX“).
- **Umgehung in der Suite:** CSV der Ansicht über den Tabellendialog (identisch zur CLI, siehe Bericht); XLSX der Ansicht über den Kernbefehl
  `export.view`; Bulk-Export und Report über den Dialog aus U16 (`export.bulk.*`, `export.report.*`, `export.multi`), der funktioniert.
- **Test:** `export.spec.ts` „the view export of the table offers CSV and Excel in the export dialog“ (fixme).

### A01-15 XLSX-Blattname der UI-Exporte (Mittel)

- **Datei:** `crates/os-app/src/commands/export.rs` (`writer()` Zeile ~160 ruft `detail_writer(…, "Export")`; auch der Bulk-Export aus dem Dialog schreibt „Export“).
- **Reproduktion:** `export.view` (oder Bulk-Export „Links:All Inlinks“ im Dialog, Format „Excel Workbook“) und `openspider --load-crawl … --export-tabs
  "Internal:All" --export-format xlsx` auf dieselbe Crawl-Datei: gleiche Zellen, Blatt „Export“ gegenüber „Internal All“ bzw. „Links All Inlinks“.
  CSV ist Byte für Byte gleich.
- **Spec-ID:** OUT-001 (UI = CLI, byte-identisch außer Zeitstempeln), OUT-007 (Blattnamen aus „Tab Filter“ bzw. Bulk-Name); Kriterium 4.
- **Test:** `export.spec.ts` „the XLSX sheet name of a view export equals …“ (fixme).

### A01-04 `config.set` ohne Crawl wirkt nicht auf `crawl.start` (Mittel)

- **Datei:** `crates/os-app/src/commands/config.rs` (Entwurf `Drafts` je Fenster) gegenüber `crates/os-app/src/session/control.rs` `config()` (nimmt Sitzung
  oder Standardprofil, **nicht** den Entwurf).
- **Reproduktion:** Bridge ohne Crawl: `config.set` mit `speed.threads=1`, `speed.max_urls_per_sec=4`, dann `crawl.start` auf die Fixture: der Crawl
  endet nach 0,6 s (Standardwerte). `openspider --crawl … --set speed.max_urls_per_sec=4 --set speed.threads=1` braucht 7,3 s. Mit
  `profile.save` + `profile.setDefault` und anschließendem `crawl.start` wirkt die Drosselung (9 s).
- **Spec-ID:** OUT-220 (Profil/Konfiguration wirkt auf den Crawl), CRAWL-126; betrifft den Konfigurationsdialog vor dem ersten Crawl (U13).
- **Umgehung in der Suite:** `alpha.useProfile` / `slowCrawl` speichern ein Profil und setzen es als Standard.

### A01-06 Zustand nach `crawl.stop` (Mittel)

- **Datei:** Statusleiste `apps/desktop/src/features/statusbar/Statusbar.tsx` bzw. Fortschrittsmeldung im Kern (`crates/os-app/src/session/feed.rs`).
- **Reproduktion:** gedrosselter Crawl, nach 11 von 27 URLs `crawl.stop`: Statusleiste „Completed“, „Done 11 of 27 (100 %)“, „Remaining: 16“,
  Start und Löschen aktiv.
- **Spec-ID:** UI-013 („100 % erst, wenn die Warteschlange leer ist“), Zustandstexte; Kriterium 1.
- **Test:** `crawl-control.spec.ts` „a stopped crawl never reports 100 % …“ (fixme). Der Stopp selbst wirkt (aktiver Test).

### A01-08 Tab-Leiste überlappt „Configure tabs“ (Mittel)

- **Datei:** `apps/desktop/src/features/toptabs/TopTabs.tsx`, `toptabs.css`.
- **Reproduktion:** Fenster 1 024 × 640 (auch 1 280 × 800), Crawl, `layoutReport`: Tab „Pagination“ überlappt die Schaltfläche „Configure tabs“ um 18 × 24 px,
  „Directives“ um 4 × 24 px. Playwright meldet für einen Mausklick auf die Schaltfläche „<div role="tab">Pagination</div> … intercepts pointer
  events“; die Schaltfläche ist nur per Tastatur (Fokus + Enter) zu öffnen. Eine vorhandene Fixture-Seite (`e2e/table/table.spec.ts`) umgeht das
  ebenfalls mit Fokus + Enter.
- **Spec-ID:** UI-001 („keine Überlappung bei 1 024 × 640“), UI-004; Kriterium 5. Chromium und WebKit.
- **Test:** `appearance.spec.ts` „at 1024 x 640 no interactive elements overlap“ (fixme). Aktiv: alle Elemente liegen im Fenster, die Seitenleiste
  startet eingeklappt, die Hauptbereiche überlappen bei 1 280 × 800 nicht.

### A01-09 Sortier-Geste bei laufendem Crawl wirkungslos (Mittel)

- **Datei:** `apps/desktop/src/grid/DataGrid.tsx` (`sort`, Zeile ~250) / `features/table/UpperTable.tsx` (`onSortGesture` → `tabs.query`).
- **Reproduktion:** Tab „Response Codes“ vor dem Start wählen, gedrosselten Crawl starten, bei ≥ 16 Zeilen auf den Spaltenkopf „Address“ klicken
  oder mit dem Fokus per Pfeil nach oben in die Kopfzeile + Enter: In mehreren von je fünf bis sechs Läufen (Tastatur häufiger als Klick) ändert sich weder
  `useTabs.queries["Response Codes"].sort` noch die Tabelle; auch nach Pause. Nach Crawl-Ende funktioniert dieselbe Geste immer.
- **Spec-ID:** UI-010; Kriterium 2 („Sortierung“). **Umgehung:** `useTabs.getState().query({sort})` im Live-Test; Sortierung nach Ende per Kopfklick geprüft.

### A01-10 Markierte Zeile rutscht beim Einsortieren (Mittel)

- **Datei:** `apps/desktop/src/data/ViewDataSource.ts` (`onViewChanged`, Anker auf Fokuszeile, Zeilen 115–131) bzw. `grid/DataGrid.tsx`.
- **Reproduktion:** gedrosselter Crawl (2 URL/s), Tab „Response Codes“ nach Address aufsteigend, Zeile 5 markieren; während neue URLs oberhalb
  einsortiert werden, steht die markierte Zeile in etwa 3 von 5 Läufen 1–2 Zeilen tiefer als anfangs (länger als 150 ms stabil gemessen,
  `relative` 4 → 5 oder 6). Die URL bleibt dieselbe.
- **Spec-ID:** UI-018 („Fokuszeile und Scrollposition springen nicht“); Kriterium 2.
- **Test:** `table.spec.ts` „the selected row keeps its screen position …“ (fixme).

### A01-16 Klick auf eine Zeile der laufenden, sortierten Tabelle (Mittel)

- **Datei:** `apps/desktop/src/features/table/controller.ts` (`fail`, `tick`), `data/ViewDataSource.ts`, `grid/DataGrid.tsx` (`selectRow`).
- **Reproduktion:** wie A01-10; in etwa 30 % der Läufe (häufiger unter paralleler Last) erscheint in der Tabelle „An unexpected error occurred.“, „1 selected“
  verschwindet und die Auswahl ist weg. Das Protokoll zeigt viele `view.rows` → `Stale` (`err-view-stale`, wird still verworfen); die Ursache des
  unbekannten Fehlers ist nicht eingegrenzt.
- **Spec-ID:** UI-018 (stabile Auswahl), UI-142 (veraltete Antworten verwerfen, keine Rücksprung-Effekte); Kriterium 2.
- **Test:** `table.spec.ts` „the selected URL stays selected …“ (fixme).

### A01-20 Filterzähler M bleibt zurück (Mittel)

- **Datei:** `apps/desktop/src/features/table/controller.ts` (`tick`: Zähler nur bei `tick.countsVersion > counts.version`, kein erneutes Laden beim Tabwechsel).
- **Reproduktion:** gedrosselter Crawl (2 URL/s) mit geöffnetem Tab „Response Codes“ bis „Completed“: „Showing: 27 of 25“ bzw. „27 of 26“ bleibt
  stehen (auch nach Wechsel zu einem anderen Tab und zurück), in etwa 2 von 3 Läufen; in einzelnen Läufen fehlen nach „Completed“ auch die letzten ein bis zwei
  Zeilen in der Tabelle selbst (25 statt 27, nach Address sortiert). Bei schnellem, ungedrosseltem Crawl stimmt der Zähler.
- **Spec-ID:** UI-018 („Filterzähler … spätestens 1 s nach Abschluss einer URL aktualisiert“), UI-006 (Anzahl stimmt mit den Zeilen); Kriterium 2.
- **Test:** `table.spec.ts` „after a throttled crawl the counter shows the final N of M“ (fixme).

### A01-05 Kein Bedienelement für Stopp (Niedrig, Rückfrage)

- **Datei:** `apps/desktop/src/features/header/Header.tsx` (Start, Pause/Fortsetzen, Löschen; `crawl.stop` ist nur in `ipc/crawl.ts` angebunden).
- **Hinweis:** UI-003 nennt keine Stopp-Schaltfläche; das Alpha-Kriterium 1 nennt „Stopp“. Offen: Soll „Löschen“ bzw. Menü „Crawl“ Stopp leisten, oder fehlt ein
  Element? Die Suite prüft `crawl.stop` über den Kernbefehl (aktiver Test, grün).

### A01-07 Flackern und Zählerabstand bei Live-Aktualisierung (Niedrig)

- **Datei:** `apps/desktop/src/data/ViewDataSource.ts`, `grid/DataGrid.tsx`.
- **Beobachtung:** Bei jeder Versionsänderung der Ansicht zeigen alle sichtbaren Zeilen kurz „…“ (Platzhalter) und die Auswahl ist für Augenblicke nicht
  markiert (`aria-selected` fehlt); „Showing: N of M“ zeigt vorübergehend N > M (z. B. „19 of 18“, „27 of 25“; innerhalb der 1 s der Spec erlaubt).
- **Spec-ID:** UI-142 („keine sichtbaren Flacker-/Rücksprung-Effekte“), UI-018. Die Tests werten nur Momentaufnahmen, die 150 ms lang gleich sind.

### A01-11 HTTP Headers ohne Extraktion (Niedrig)

- **Datei:** `apps/desktop/src/features/detail/headers/**`; Kern liefert `Unsupported` mit `FEATURE_DISABLED`/Sprungziel `extraction.http_headers`
  (`detail.headers`, Meldung `err-detail-unavailable`).
- **Reproduktion:** Standardprofil, Crawl, Detailauswahl, Reiter „HTTP Headers“: „This function is not available yet.“ Mit `extraction.http_headers=true`
  im Standardprofil erscheinen Request- und Response-Header (aktiver Test grün).
- **Spec-ID:** UI-030, UI-016 (Leerzustand mit Voraussetzung und Sprung zur Einstellung). **Test:** `detail-sidebar.spec.ts` (fixme).

### A01-13 Platzhalter in Issue-Namen (Niedrig)

- **Datei:** Kern `crates/os-analyze/src/issues*` / `sidebar.issues` (Name aus `labelTemplate`), Anzeige `features/sidebar/issues/Issues.tsx`.
- **Reproduktion:** Seitenleiste „Issues“: „Page Titles: Below {title.min_chars} Characters“, „Meta Description: Below {meta_description.min_chars} Characters“.
- **Spec-ID:** UI-062, UI-064. **Test:** `detail-sidebar.spec.ts` „Issue names show the configured thresholds …“ (fixme).

### A01-22 Bulk-/Report-Menü: Beschriftungen und Übersetzungen (Niedrig)

- **Datei:** `apps/desktop/src/features/export/ExportHost.tsx` (`labelKey` = Katalogname), `ui/menu`, i18n-Dateien.
- **Beobachtung:** Das Menümodell zeigt „Accessibility:WCAG 2.0 A“ als „0 A“ (Fluent liest „WCAG 2.0 A“ als Nachricht mit Attribut) und
  mehrere hundert Konsolenwarnungen „i18n: Schlüssel ohne Übersetzung: …“ für Katalognamen. Warnungen sind keine Fehler, Kriterium 7 bleibt erfüllt.
- **Spec-ID:** UI-002, UI-121.

### A01-17 Seitenleisten-Reiter in WebKit (Niedrig)

- **Datei:** `apps/desktop/src/features/sidebar/frame/SidebarFrame.tsx` (Reiterleiste „Crawl summary“, sieben Reiter im 360-px-Streifen), `sidebar.css`.
- **Beobachtung:** In WebKit meldet Playwright für einen Mausklick auf den letzten Reiter „Spelling & Grammar“ gelegentlich
  „<div role="tablist"> intercepts pointer events“ (macOS-CI-Lauf 39540400349 beim ersten Versuch, beim zweiten grün; lokal einmal beim Gang durch die
  Oberfläche). Der Reiter liegt am Rand der scrollbaren Leiste; per Tastatur (Fokus + Enter) ist er immer erreichbar.
- **Spec-ID:** UI-060 (Seitenleiste), UI-001 (keine verdeckten Bedienelemente). **Test:** `detail-sidebar.spec.ts` „every sidebar tab can be selected with the mouse“ (fixme nur WebKit).

### A01-18 Statusleiste (Niedrig)

- **Datei:** `apps/desktop/src/features/help/MenuAvailability.tsx`, Fußzeile in `App.tsx`.
- **Beobachtung:** „▸ Unavailable menu actions (70)“ steht unmittelbar vor „Version 0.1.0“ ohne Abstand (Screenshot bei 1 024 und 1 280 px).
- **Spec-ID:** UI-013 (Anordnung der Statusleiste), UI-001.

### A01-19 Gespeicherte Tab-Leiste geht gelegentlich verloren (Mittel)

- **Datei:** `apps/desktop/src/state/tabs.ts` (`reorder`, `show`, `layout`, `reset`, `load`; Speichern über `saveTableState` → `state/uistate.ts`), Menü
  `features/toptabs/TopTabs.tsx`.
- **Beobachtung (lokal, Chromium und WebKit, je 1 von etwa 6–12 Läufen, unter Last häufiger):** (a) Nach Umordnen (Images vor Internal) und anschließendem
  Ausblenden von Hreflang über das Menü „Configure tabs“ steht in `ui-state.json` die Standardreihenfolge (`topTabs.order[0] = "Internal"`), obwohl die Oberfläche
  „Images“ zuerst zeigte; (b) nach Ausblenden, Neuladen ist der Haken „Hreflang“ im Menü wieder gesetzt, obwohl `hidden = ["Hreflang"]` gespeichert war; (c) nach
  „Reset tabs“ fehlt der Tab „Hreflang“ in der Tab-Leiste gelegentlich noch (Linux-CI-Lauf 39540400349, Chromium, beim Wiederholen grün). Verdacht: verlorene
  Aktualisierung zwischen `layout()`/`show()` (Menü rechnet mit seiner Reihenfolge) und `load()`; nicht eingegrenzt.
- **Spec-ID:** UI-004 [P1] (Reihenfolge und Sichtbarkeit bleiben nach Neustart erhalten, „Tabs zurücksetzen“).
- **Test:** `table.spec.ts` „reordering and then hiding a tab keeps both changes …“ (fixme); die beiden einzeln geprüften Abläufe (nur Umordnen, nur Ausblenden)
  sind aktiv und tragen `// spec: UI-004 [P1]`, gelegentliches Rot wird von der CI-Wiederholung aufgefangen.

### A01-21 Leere Tabelle nach Tabwechsel direkt nach dem Crawl-Ende (Mittel)

- **Datei:** `apps/desktop/src/features/table/controller.ts` (`open`, `ensureRange`), `data/ViewDataSource.ts`.
- **Beobachtung:** In einzelnen Läufen (zwei von etwa dreißig, unter paralleler Last) bleibt der gewählte Tab (z. B. „Page Titles“, „Response Codes“)
  unmittelbar nach „Completed“ bei „Showing: 0 of 22“ bzw. „0 of 27“ und lädt keine Zeilen nach (5 bzw. 15 s beobachtet, kein Banner). Die Suite weicht über
  `alpha.openTable` aus (Wechsel über einen anderen Tab und zurück, Annotation `table-reload`).
- **Spec-ID:** UI-142, UI-004 (Tab-Wechsel zeigt die erste Bildschirmseite).

## Offene Fragen an den Planer

1. **Gültiger Ignore-Grund:** `os-parity coverage` (M2.md 2 Regel 5) kennt für `test.fixme` nur Gründe mit Präfix `M3:`, `M4:`, `M5:`, `P2:` oder
   `Klärung K2-nn`. Die hier gesetzten Gründe beginnen auftragsgemäß mit „Befund A01-nn“. Entweder vergibt der Planer je Befundgruppe eine K2-Nummer
   (Ersetzung per `sed` über `Befund A01-` möglich) oder die Prüfung nimmt „Befund A01-“ auf.
2. **Stopp (A01-05):** Soll es ein Bedienelement geben?
3. **Fremdprozess-Crawl:** Die App öffnet ihn über „Crawls verwalten…“ (Doppelklick); eine automatische Anzeige ohne Öffnen gibt es nicht. Kriterium 6
   („erscheint live“) ist so erfüllt (grün); falls automatische Anzeige gemeint ist, fehlt sie.
