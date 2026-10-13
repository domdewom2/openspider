# M2 – Oberflächenkonzept (Desktop-UI)

Stand: 2026-10-13 (Entwurf v1) · Planer/Architekt: Opus · Status: Grundlage für den späteren M2-Paketplan (`M2.md`/`M2-packages.json`), noch keine verbindliche Paketliste.

Quellen: `docs/ARCHITECTURE.md`, `docs/spec/README.md` (Meilensteinzuordnung), `docs/spec/05-ui.md` (**führend für Oberfläche**), `docs/spec/07-performance.md` (PERF-004), `docs/spec/10-agent-interface.md` (MCP-Menü, AGENT-043), Tab-Definitionen in `02`/`03`, Export/Vergleich/Verwaltung/MCP in `06` (OUT-006, -020, -021, -140 ff., -222, -228, -285), Schnittstellen aus `docs/plan/M1.md` Abschnitt 3 (Code-Stand `5b4065b`: P00a gemergt, `apps/desktop` ist ein Gerüst mit einem einzigen Command `app_version`).

Clean-Room: Alle Texte, Layouts und Wireframes sind eigenständig formuliert. Übernommen werden nur branchenübliche Layout-Konventionen eines Desktop-Crawlers (Tabelle oben, Detailbereich unten, Zusammenfassung rechts), wie sie 05 ohnehin festlegt. Es wurden keine Screenshots, Ressourcen oder Texte des Referenzprodukts verwendet.

Konventionen dieses Dokuments:
- **[Entscheidung]** = Vorschlag des Architekten, gilt für die M2-Feinplanung, solange Fable nicht widerspricht.
- **[Kernbedarf]** = muss in Rust (außerhalb `apps/desktop`) neu entstehen oder erweitert werden.
- **[Offen]** = Klärung nötig (Sammlung in Abschnitt 10).
- `Mod` = Cmd (macOS) bzw. Strg (Windows).

---

## Inhalt

1. [Informationsarchitektur](#1-informationsarchitektur)
2. [Frontend-Architektur](#2-frontend-architektur)
3. [Tabellen-Engine](#3-tabellen-engine)
4. [Tauri-Command-Katalog](#4-tauri-command-katalog)
5. [Design-System](#5-design-system)
6. [Wireframes](#6-wireframes)
7. [Teststrategie UI](#7-teststrategie-ui)
8. [Paketschnitt M2-UI](#8-paketschnitt-m2-ui-vorschlag)
9. [Risiken](#9-risiken)
10. [Offene Fragen](#10-offene-fragen)

---

## 1 Informationsarchitektur

### 1.1 Fenster und Ebenen

| Ebene | Inhalt | Art | UI-IDs |
|---|---|---|---|
| **Hauptfenster** | Menüleiste, Kopfleiste, obere Tabelle mit Top-Tabs, Detailfenster unten, Seitenleiste rechts, Statusleiste | natives Fenster, eine WebView | UI-001 … UI-021, UI-030 … UI-082, UI-110 … UI-114 |
| **Einstellungen** | App-Einstellungen (`settings.toml`) | eigenes Fenster, nicht modal (macOS-Konvention Cmd+,; Windows Datei → Einstellungen) | UI-120 … UI-128, OUT-222, OUT-229 |
| **Konfiguration** | Crawl-Konfiguration (`.osconfig`) inkl. Profile, Segmente, Crawl-Analyse | modaler In-App-Dialog (große Fläche, Seitennavigation links) | 01 C–O, UI-068, OUT-220, OUT-224 |
| **Crawl-Verwaltung** | Liste aller Crawls, Projekte, Aktionen | modaler In-App-Dialog | OUT-228, OUT-229, UI-124 |
| **Exporte** | Speichern-Dialog (nativ) + vorgeschalteter Optionsdialog (Format, „alle Spalten“, Arbeitsmappe) | nativer Dateidialog + kleiner In-App-Dialog | OUT-006, OUT-007, OUT-020, OUT-021, UI-015 |
| **Vergleich** | Auswahl Current/Previous, URL-Mapping, Change-Detection-Optionen; danach **Modus des Hauptfensters** (kein eigenes Fenster) | In-App-Dialog + Hauptfenster-Modus | OUT-140 … OUT-153, TECH-300 ff. |
| **MCP** | Server, Werkzeuge, Klassen, Token, Client-Verbindungen, Aufrufprotokoll | In-App-Dialog mit Reitern | UI-129, OUT-285, AGENT-002, -003, -043 |
| **Visualisierungen** | Graphen, Bäume, Wortwolken | je eigenes, nicht modales Fenster (≥ 4 gleichzeitig) | UI-090 … UI-100 |
| **Kleine Dialoge** | Crawl-Pfad, Spalten-Konfigurator, Tabs konfigurieren, Rückfragen, Startfehler, Debug-Info, Tastaturkürzel, Über | In-App-Dialog | UI-009, UI-019, UI-050, UI-112, UI-127, UI-113 |

**[Entscheidung]** In-App-Dialoge statt nativer Zusatzfenster überall dort, wo der Dialog den Zustand des Hauptfensters braucht (Auswahl, aktiver Crawl, Spalten). Eigene Fenster nur für Einstellungen (Plattformkonvention, nicht modal) und Visualisierungen (Spec verlangt nicht-modale Mehrfachfenster). Damit gibt es nur zwei sekundäre WebView-Typen (`settings`, `viz`), die ihren Zustand ausschließlich über den Kern beziehen.

### 1.2 Hauptfenster

```
Menüleiste (nativ)                                                     UI-002
Kopfleiste: URL/Liste · Bereich · Start/Pause/Fortsetzen/Leeren · Fortschritt   UI-003
┌───────────────── linke Spalte ─────────────────┬── Seitenleiste ──┐
│ Top-Tab-Leiste (+ Überlauf »)                   │ Tabs: Overview,  │  UI-004, UI-060
│ Werkzeugzeile: Filter ▾ · Export · Segment-Badge│ Issues, Site     │  UI-006, UI-015, UI-020
│   · Zähler „N von M“ · Suche · Liste/Baum (P2)  │ Structure, Seg-  │  UI-011, UI-014
│ Hinweisleisten (Analyse veraltet, Speicher)     │ ments, Response  │  UI-082, CRAWL-129
│ Obere Tabelle (virtualisiert)                   │ Times, API, …    │  UI-007, UI-140
├──────────── Trenner (UI-001) ───────────────────┤                  │
│ Detail-Tab-Leiste                               │                  │  UI-030
│ Werkzeugzeile Detail: Filter · Suche · Export   │                  │
│ Detailinhalt (Tabelle / Code / Vorschau)        │                  │  UI-031 … UI-050
└─────────────────────────────────────────────────┴──────────────────┘
Statusleiste: Modus · Zustand · Raten · Erledigt · Rest · Threads · Speicher · Datei · MCP   UI-013
```

Regionen und ihre Zuständigkeit im Frontend (Feature-Ordner, Abschnitt 8):

| Region | Feature | Datenquelle | Landmark (Screenreader) |
|---|---|---|---|
| Kopfleiste | `features/header` | `crawl.*`, Tick-Fortschritt | `banner` |
| Top-Tabs + Werkzeugzeile | `features/toptabs` | `catalog.tabs`, `catalog.filterCounts` | `navigation` „Datenansichten“ |
| Obere Tabelle | `features/table` + `grid` | `view.*` | `main` |
| Detailfenster | `features/detail/*` | `detail.*`, `view.*` (Detail-Views) | `region` „Details“ |
| Seitenleiste | `features/sidebar/*` | `sidebar.*` | `complementary` |
| Statusleiste | `features/statusbar` | Tick | `contentinfo` |

**Modi des Hauptfensters:** Spider, Liste, (SERP, APIs als Varianten der Kopfleiste), Vergleich. Der Vergleichsmodus tauscht nur Inhalte (Overview-Spalten Current/Previous/Change/…, Unterfilter Added/New/Removed/Missing, Tab „Change Detection“, Vergleichsfilter in Inlinks/Outlinks), nicht das Layout (05 Übersicht: „Layout muss einen zusätzlichen Modus tragen können“).

### 1.3 Top-Tabs, Filter, Suche, Tabelle

- **Top-Tabs (UI-004):** Reihenfolge und Sichtbarkeit kommen aus `catalog.tabs` (Kern, `tabs::all()`), ergänzt um die nutzerbezogene Anordnung aus `ui-state.json`. Tabs, die erst ab M4/M5 existieren, liefert der Kern in M2 nicht aus – die UI kennt keine Tab-Liste im Code.
- **Filter-Dropdown (UI-006):** Combobox mit „Name (Anzahl)“, Kennzeichen für `NeedsCrawlAnalysis` (Anzahl „–“), `NeedsConfig`/`NeedsRendering` (Hinweissymbol) und Issue-Klassifizierung (Typsymbol + Prioritätsfarbe). Zahlen aus `catalog.filterCounts` (Version per Tick).
- **Suche (UI-011):** einfaches Feld (Teilstring auf `Address`, Enter löst aus, Esc bricht ab) – P0. Erweiterter Modus (Regelbaum, Textsyntax, Regex, Spaltenwahl, Verlauf) – P1. **[Entscheidung]** Parser und Auswertung der Textsyntax liegen im Kern (`search.parse`), damit UI, Export (OUT-020) und MCP dieselbe Grammatik nutzen; die UI rendert nur den Regelbaum.
- **Tabelle (UI-007, UI-140):** virtualisiert, `Row` + `Address` angeheftet, typisierte Zellen, Spaltenkopf mit Sortierpfeil + Rang, „+“-Schnellmenü rechts (UI-008), Kontextmenüs (UI-110), Kopieren (UI-012).
- **Zähler (UI-014):** „Angezeigt: N von M“, „(Suche aktiv)“, „K markiert“.

### 1.4 Detailfenster und Seitenleiste

- Detail-Tabs in Reihenfolge UI-030; nur der sichtbare Tab lädt (Lazy Loading, UI-145), Auswahlwechsel entprellt 100 ms. Tabellarische Detail-Tabs nutzen dieselbe Grid-Komponente und denselben View-Mechanismus wie die obere Tabelle (Detail-Views, z. B. 1 Mio. Inlinks).
- Nicht-tabellarische Detail-Tabs: URL Details (Name/Value-Tabelle), SERP Snippet (Vorschau + Messwerte, Pixelwerte aus dem Kern), Rendered Page (Kachelbild über eigenes URI-Schema), View Source (Code-Ansicht mit Diff), Structured Data (Baum + Meldungen), Accessibility (zwei Tabellen), Duplicate Details (Liste + Textvergleich).
- Seitenleiste (UI-060): Overview (Baum mit auf-/zuklappbaren Tab-Blöcken), Issues (Liste + Erklärungsbereich UI-065), Site Structure, Segments, Response Times, API, Spelling & Grammar (P2), Semantic Search (M4, nicht in M2). Jede zählende Zeile ist ein Navigationsziel: Klick setzt Tab + Filter (bzw. Suche) der oberen Tabelle.

### 1.5 Dialoge (Inhaltsstruktur)

| Dialog | Gliederung | Besonderheiten |
|---|---|---|
| Konfiguration | Seitennavigation links (Spider: Crawl, Extraktion, Limits, Rendering, Erweitert, Voreinstellungen · robots.txt · URL-Umschreibung · Einschließen/Ausschließen · Geschwindigkeit · User-Agent · HTTP-Header · Benutzerdefiniert (Suche, Extraktion, JavaScript, Linkpositionen) · API-Zugänge · Authentifizierung · Segmente · Crawl-Analyse · Profile); Suchfeld über der Navigation | Validierung ausschließlich im Kern (`config.validate`), Fehler je TOML-Pfad am Feld; Regex-Tester inline (Include/Exclude, Umschreibung); Sprungziele für UI-016-Aktionen (`openConfig({anchor})`) |
| Einstellungen | Allgemein (Sprache, Benachrichtigungen) · Oberfläche (Theme, Akzent, Zoom, Zeilenhöhe, Zebra, Live-Intervall, Zurücksetzen UI-021/125) · Speicher (Ort, Leistungsprofil, RAM-Grenze) · Crawls (Aufbewahrung) · Exporte (Ordner, Standardformat) · Netzwerk (Proxy, Zertifikate) · Konten · Externe Werkzeuge (UI-117) · MCP (Verweis auf MCP-Dialog) | Änderungen wirken sofort; neustartpflichtige Einträge markiert (OUT-222) |
| Crawl-Verwaltung | Projektbaum links, Crawl-Tabelle rechts (Spalten OUT-228), Werkzeugleiste, Filter „Anzeigen“, Suche | Mehrfachauswahl bis 1 000, „Zum Vergleich wählen“ bei genau 2, gesperrte Einträge ausgegraut |
| Export | Optionsdialog (Format, „Alle Spalten“, „In einer Arbeitsmappe“) → nativer Speichern-Dialog → Fortschritt als Hintergrundoperation mit Abbrechen → Meldung „Ordner öffnen“ | Bulk-Export/Berichte/Sitemaps werden aus dem Kern-Katalog (`export.catalog`) als Menüs erzeugt |
| Vergleich | Zwei Crawl-Auswahlen (Current/Previous, Tausch-Schaltfläche), URL-Mapping-Regeln mit Test-Reiter, Change-Detection-Elemente | danach Vergleichsmodus im Hauptfenster |
| MCP | Reiter Server (Start/Stopp, Port, Auto-Start, Status) · Werkzeuge (Klassen-Schalter, Einzelschalter, Markdown herunterladen) · Zugang (Token anzeigen/kopieren/neu) · Clients (Claude Desktop/Code, Codex: Diff-Vorschau + Schreiben mit Sicherung) · Protokoll (letzte Aufrufe) | Token nie im Klartext im DOM, solange nicht ausdrücklich „Anzeigen“ gewählt; Protokoll ohne Geheimnisse (AGENT-043) |

### 1.6 Tastaturnavigation

Grundprinzip: Jede Region ist ein **Fokusbereich**; F6/Shift+F6 springt zwischen Tabelle → Detailfenster → Seitenleiste (UI-113, Reihenfolge laut Spec), Tab/Shift+Tab bewegt sich innerhalb und zwischen Bedienelementen in DOM-Reihenfolge (Kopfleiste → Top-Tabs → Werkzeugzeile → Tabelle → …).

| Kontext | Tasten |
|---|---|
| Tab-Leisten (oben/unten/rechts) | Roving Tabindex: ←/→ wechselt den Tab, Pos1/Ende, Shift+F10 Kontextmenü (UI-112) |
| Tabelle | ↑/↓ Fokuszeile, Bild↑/↓, Pos1/Ende (Zeile), Mod+Pos1/Ende (erste/letzte Zeile), Shift+Pfeile Bereich erweitern, Mod+Leertaste Zeile umschalten, ←/→ Zellfokus, Mod+A, Mod+C, Shift+F10/Kontextmenütaste, Enter = Detailfenster fokussieren (OS-Festlegung), Entf mit Rückfrage |
| Global | Kürzelregister UI-113 (`ui/shortcuts.ts`): Mod+1…9, Mod+Alt+←/→, Mod+F, Mod+L, Mod+E, Mod+J, Mod+Shift+J, Mod+R, Mod+Shift+A, Mod+Enter, Mod+P … |
| Laufende Operation | Esc bricht die fokussierte bzw. jüngste abbrechbare Operation ab (UI-141) |
| Dialoge | Fokusfalle, Esc schließt, Enter = Standardaktion, Fokus kehrt zum Auslöser zurück |
| Link-Navigation | Alt+←/→ (macOS Cmd+[ / Cmd+]) über einen Verlauf der Auswahl (UI-032) |

Kontextregel UI-113: Hat ein Texteingabefeld den Fokus, wirken global nur Esc, Enter, Mod+F, F6/Shift+F6 – technisch über einen zentralen `keydown`-Dispatcher, der `event.target` prüft und Kürzel nach `scope` (`global`, `table`, `field`, `dialog`) filtert.

---

## 2 Frontend-Architektur

### 2.1 Schichten

```
React-Komponenten (features/*, ui/*)            ← nur Darstellung + Interaktion
   │ Hooks (useView, useDetail, useSidebar, useT, useShortcut …)
   ▼
Client-Zustand (Zustand-Stores)   Server-Daten (TanStack Query)   Grid-Datenquelle (ViewDataSource)
   │                                     │                               │
   └──────────────── ipc/ (typisierte Command-Wrapper, Fehlerumsetzung, Event-Bus) ───────────────┘
                                         │  Tauri invoke / Channel
                                         ▼
src-tauri: dünne Command-Hüllen (je Domäne ein internes Tauri-Plugin)
                                         ▼
os-app [Kernbedarf, Vorschlag]: Session, ViewRegistry, Coalescer, DTOs, Operationen – Tauri-frei, mit cargo test prüfbar
                                         ▼
os-analyze · os-store · os-engine · os-export · os-mcp · os-model
```

**[Entscheidung]** Eine **Tauri-freie Service-Schicht** (`crates/os-app`, neues Crate – Zustimmung des Orchestrators nötig, [Offen] F-1) hält Sitzungszustand, View-Verwaltung, Ereignisbündelung und DTOs. `apps/desktop/src-tauri` registriert nur Command-Hüllen. Begründung: (1) Rust-Integrationstests der gesamten UI-Logik ohne WebKit/WebView2-Build; (2) die E2E-Bridge (Abschnitt 7.3) und ein späterer In-Prozess-MCP-HTTP-Server nutzen dieselbe Schicht; (3) `openspider-desktop` bleibt der einzige Ort mit Tauri-Abhängigkeiten. Fallback ohne neues Crate: Modul `apps/desktop/src-tauri/src/service/` mit `#[cfg(test)]`-Tests (schwerer zu testen, CI baut dann immer Tauri).

### 2.2 Zustandsverwaltung

| Art | Werkzeug | Beispiele | Regel |
|---|---|---|---|
| UI-Zustand (lokal, persistent) | **Zustand** (MIT) – mehrere kleine Stores | Layout/Trenner, Tab-Reihenfolge, Spaltenlayouts je Tabelle, Filter je Tab, Suchverlauf, Theme-Wahl | Persistenz über `uistate.save` (debounced 1 s, atomar im Kern geschrieben, UI-125); Schema versioniert, Migration in TS, Validierung mit **zod** (MIT) |
| Sitzungszustand (vom Kern gespiegelt) | Zustand-Store `session` | Crawl-Zustand, Fortschritt, Modus, Analyse-Status, MCP-Status, Speicherstufe | ausschließlich durch Tick-Ereignisse geschrieben; Komponenten lesen mit Selektoren (keine Re-Render-Stürme) |
| Abfragedaten (Request/Response) | **TanStack Query** (MIT) | Filterzähler, Overview, Issues, URL Details, SERP, Header, Konfiguration, Crawl-Liste | Query-Keys enthalten die Kern-Version (`countsVersion`, `issuesVersion`, `viewVersion`) → Invalidierung durch Ticks ohne Polling; `AbortSignal` → `op.cancel` |
| Tabellenzeilen | eigene **ViewDataSource** (keine React-State) | Blöcke à 200 Zeilen, LRU ≤ 20 000 Zeilen (UI-144) | siehe 2.4 |
| Auswahl | Store `selection` je Tabelle | `{ mode: "ids", ids, focus }` oder `{ mode: "all", viewId, version, except }` | Auswahl hängt an **UrlId**, nicht an Zeilenindex – bleibt bei Live-Einsortierung stabil (UI-018) |

Nicht verwendet: Redux (zu viel Zeremonie für reine Spiegelung), globale React-Contexts für häufig wechselnde Werte (Re-Render der ganzen App bei jedem Tick).

### 2.3 Datenzugriff über Tauri-Commands

Grundregeln:
1. **Alle Commands sind `async`** und führen Datenbankarbeit in `spawn_blocking` bzw. einem eigenen Lese-Pool aus. Synchrone Tauri-Commands laufen auf dem Haupt-Thread und sind verboten (PERF-004 „keine Datenbankabfrage im UI-Thread“; CI-Lint: `grep` auf `#[tauri::command]` ohne `async`).
2. **Server-seitig:** Sortierung, Filter, Suche, Segmentfilter, Zählung (UI-141). Die UI sortiert, filtert oder misst nie selbst (auch keine Pixelbreiten, UI-038).
3. **Paging:** Die UI adressiert Zeilen per **Offset auf einer materialisierten View mit Versionsstempel** (wahlfreier Zugriff für Scrollbalken-Sprünge, UI-140/142). Der undurchsichtige **Cursor** aus M1 (`ViewQuery.cursor`, AGENT-005) bleibt für MCP/CLI; beide setzen auf derselben Materialisierung auf (Cursor = kodierter View-Schlüssel + Offset). **[Kernbedarf]** M1 liefert nur Cursor-Paging (`query_view`), siehe 4.4.
4. **Fenstergröße** 200 Zeilen (UI-142), Projektion = sichtbare Spalten + angeheftete; Vorabladen 2 Bildschirmseiten ober-/unterhalb (UI-140).
5. **Versionen:** Jede Antwort trägt `version`; Antworten mit älterer Version als der zuletzt bekannten werden verworfen (UI-142, kein Flackern).
6. **Abbruch:** Operationen > 300 ms laufen als **Op** mit `opId`; Fortschritt kommt per Tick, Abbruch per `op.cancel(opId)` (Kern: `CancelToken` + `rusqlite::InterruptHandle`). Während eine neue View-Version gebaut wird, liefert der Kern weiter die alte (Doppelpuffer) → Abbruch stellt die alte Ansicht sofort wieder her (≤ 100 ms, UI-141).
7. **Große Datenmengen verlassen den Kern nicht:** Kopieren von Mod+A über 5 Mio. Zeilen, Exporte und Bulk-Aktionen werden im Kern ausgeführt (`view.copy` schreibt selbst in die Zwischenablage, Exporte streamen in Dateien).
8. **Nutzlast:** spaltenweise kompaktes JSON (Arrays je Spalte, `null` für leer). **[Offen]** F-6: UI-142 verlangt Kompression ab 64 KB – bei In-Prozess-IPC ohne Nutzen; Vorschlag: Kriterium durch die Nutzlastgrenze (≤ 150 KB) als erfüllt werten. Falls Messungen > 30 ms p95 zeigen, Umstieg auf Binärantwort (`tauri::ipc::Response`, MessagePack) ohne API-Änderung im Frontend.

### 2.4 Grid-Datenquelle (ViewDataSource)

```
class ViewDataSource {
  viewId, version, total, columns
  blocks: LRU<blockIndex, RowBlock>        // 200 Zeilen je Block, max. 100 Blöcke = 20 000 Zeilen
  pending: Map<blockIndex, Promise>        // Dedupe paralleler Anfragen
  getCell(col, row) → Zellwert | Loading   // synchron für das Canvas; Loading = Platzhalter (UI-140)
  ensureRange(first, last)                 // vom Grid bei Scroll aufgerufen; lädt fehlende Blöcke + Puffer
  onViewChanged(version, total)            // aus Tick: Blöcke als veraltet markieren, sichtbare zuerst neu laden
  locate(urlId) → rowIndex                 // view.locate, für Anker/Tabwechsel/Doppelklick-Navigation
}
```

- Live-Updates (UI-018): Beim Versionswechsel lädt die Datenquelle zuerst den sichtbaren Bereich nach; der **Anker** (Fokuszeile, sonst oberste sichtbare Zeile) wird per `view.locate` neu bestimmt und die Scrollposition so korrigiert, dass er an derselben Bildschirmposition bleibt. **Tail-Modus:** unsortiert + ganz unten gescrollt → folgt neuen Zeilen; manuelles Hochscrollen beendet ihn.
- Platzhalterzeilen statt leerer Fläche bei Sprüngen (≤ 150 ms bis Inhalt, UI-140).
- Die Datenquelle ist reines TypeScript ohne React und wird isoliert mit vitest getestet (Abschnitt 7.1).

### 2.5 Event-Kanäle

**[Entscheidung]** Ein **einziger Ereigniskanal je Fenster**: Beim Start ruft jedes Fenster `events.subscribe(channel)` mit einem Tauri-`Channel<UiEvent>` auf. Der Kern bündelt **alle** Änderungen (Fortschritt, View-Versionen, Zähler, Issues, Analyse, Operationen, MCP-Status, Hinweise) in einem **Coalescer** und sendet pro Takt höchstens **eine** Nachricht `Tick`:

- Taktabstand = eingestelltes Intervall (Standard 500 ms, 250–5 000 ms, UI-018), nie unter 100 ms → **≤ 10 Nachrichten/s über alle Ereignisarten** (PERF-004, UI-018 „über alle Ansichten und Ereignisarten“) per Konstruktion.
- Fenster unsichtbar/minimiert (`visibilitychange` → `events.setVisibility(false)`) → ≤ 1 Hz (UI-143).
- Ohne Änderungen kein Tick (Leerlauf-CPU, UI-143).
- Seltene Ereignisse (Startfehler, Crawl-Ende, Plattenplatz knapp, Op beendet) werden nicht sofort gesendet, sondern im nächsten Tick mitgeliefert (Latenz ≤ 1 Takt; für Startfehler-Dialog „≤ 2 s nach Timeout“, UI-019, ausreichend). Zustandsänderungen der Kopfleiste (Start/Pause) erzwingen einen vorgezogenen Tick, sofern der letzte ≥ 100 ms zurückliegt.
- Fortschrittsanzeige der Statusleiste aktualisiert höchstens 4×/s (UI-013); die UI drosselt die Darstellung, der Kern liefert die Werte.

Quelle der Änderungen im Kern: **[Kernbedarf]** ein Änderungsfeed (Abschnitt 4.4, K-2). Er muss auch Crawls erfassen, die ein anderer Prozess schreibt (per MCP-STDIO gestarteter Crawl, AGENT-020 „erscheint live in der App“) – daher bevorzugt als Sequenzzähler in der Crawl-Datei statt nur als In-Prozess-Hook.

### 2.6 Fehlerbehandlung

| Klasse | Erkennung | Darstellung |
|---|---|---|
| Validierung | `CmdError.code = InvalidInput` mit `field` (TOML-Pfad bzw. Feldname) | Inline am Feld (rot + Text), Aktion deaktiviert |
| Veraltet/abgebrochen | `Stale`, `Cancelled` | still verworfen, nie angezeigt |
| Erwartbare Laufzeitfehler | `Io`, `NotFound`, `Busy`, `Unsupported`, `Permission` | Banner/Toast nicht modal, mit Aktion („Ordner öffnen“, „Erneut versuchen“) |
| Startfehler Crawl | Hinweis-Ereignis `notice` mit Code (`start_dns`, `start_tls`, `start_robots_blocked`, `start_offsite_redirect` …) | modaler Dialog UI-019 mit Ursache und ggf. Hinweis auf Einstellung |
| Unerwarteter Kernfehler | `Internal` | modaler Dialog „Log öffnen“ / „Diagnose kopieren“ (UI-019, UI-127) |
| Frontend-Fehler | React-Error-Boundary je Region (Tabelle, Detail, Seitenleiste, Dialog) + `unhandledrejection` | Region zeigt Ersatzfläche „Bereich neu laden“; Fehler geht per `diag.logFrontend` ins Kern-Log |
| Leer-/Voraussetzungszustände | vom Kern gelieferter Zustandscode | UI-016-Fläche mit `data-empty-state="<CODE>"`, Text + Aktion |

Texte von Fehlern stammen nie aus dem Kern als Prosa, sondern als `messageKey` + Argumente (i18n). `detail` (technische Ursache, englisch) erscheint nur ausklappbar und im Diagnose-Text.

### 2.7 Internationalisierung (de/en)

- **[Entscheidung]** **Fluent** (UI-121 erlaubt Fluent oder ICU): Frontend `@fluent/bundle` + `@fluent/langneg` (Apache-2.0) mit eigenem dünnen Hook `useT()`; Rust-Seite bei Bedarf `fluent-bundle` (Apache-2.0/MIT). Ressourcen je Feature: `src/features/<f>/i18n/{de,en}.ftl` (keine gemeinsame Riesendatei → keine Merge-Konflikte zwischen Paketen), per `import.meta.glob` gebündelt.
- **Kanonische Schlüssel:** Tab-, Filter- und Spaltennamen sind englische Kanon-Schlüssel (05 Konventionen). Übersetzungsschlüssel: `tab-<slug>`, `filter-<tab>-<slug>`, `column-<slug>`. Fehlt eine Übersetzung, zeigt die UI den **englischen Kanon-Namen** (nie einen Schlüsselnamen, UI-002/UI-121).
- **Filter mit Schwellen:** Der Kern liefert `labelTemplate` (z. B. `Over {title.max_chars} Characters`) und `params` (`{"title.max_chars": 65}`); die Übersetzung erhält dieselben Parameter.
- **Issue-Texte** (UI-065, AGENT-013 verlangt Textgleichheit App ↔ MCP): kommen **aus dem Kern** (`sidebar.issueExplain(id, lang)`), werden in der UI nicht dupliziert. [Offen] F-4 zur Textstruktur (what/why/fix vs. description/how_to_fix).
- **Zahlen/Datum:** `Intl.NumberFormat`/`Intl.DateTimeFormat` nach UI-Sprache, Instanzen gecacht (de `1.234,5`, en `1,234.5`, UI-007). Exporte bleiben sprachunabhängig (Kern).
- **Sprachwechsel ohne Neustart:** Bundles neu laden, Stores bleiben; die native Menüleiste wird aus TS neu aufgebaut (2.9), dadurch entfällt auch dort der Neustart.
- **CI:** Schlüsselvollständigkeit de/en je Feature; Platzhalter-Prüfung; Screenshot-Test Deutsch bei 1 024×640 auf abgeschnittene Texte (UI-121).
- Systemsprache: Tauri liefert die OS-Locale (`plugin-os`), `de-*` → de, sonst en.

### 2.8 Theming (hell/dunkel/System)

- Design-Tokens als **CSS Custom Properties** auf `:root[data-theme="light|dark"]`, erzeugt aus einer einzigen Quelle `src/ui/tokens/tokens.ts` (Werte Abschnitt 5). Dieselbe Quelle speist das **Canvas-Theme** des Grids und den **Kontrast-Unit-Test** (UI-120).
- „System“: Tauri-Fenster-Theme `null` + `onThemeChanged` bzw. `prefers-color-scheme` → Umschalten ≤ 1 s ohne Neustart. Umschalten in allen offenen Fenstern über ein Kern-Ereignis `settingsChanged` (≤ 200 ms, UI-120).
- Akzentfarbe: Ableitungen (Hover, Auswahlhintergrund aktiv/inaktiv, Fokusring) per `color-mix(in oklch, …)`; Kontrastprüfung im Farbwähler, bei < 3:1 Warnung + zusätzlicher 1-px-Rahmen für Markierungen (UI-120).
- **Windows-Kontrastdesigns:** `@media (forced-colors: active)` → Systemfarben in CSS; das Canvas-Grid erhält ein eigenes Theme aus Systemfarben (Canvas folgt `forced-colors` nicht automatisch).
- UI-Zoom (80–150 %): **[Entscheidung]** über den WebView-Zoom (`Webview.setZoom`), nicht über eigene Skalierung – Schrift, Abstände, Zeilenhöhe und Canvas (über `devicePixelRatio`) skalieren gemeinsam. [Spike] Prüfen, dass WKWebView bei Zoom `devicePixelRatio` korrekt meldet; sonst Grid-Theme mit Zoomfaktor multiplizieren.

### 2.9 Native Menüleiste und Kürzel

- **[Entscheidung]** Menü wird **in TypeScript** über `@tauri-apps/api/menu` aufgebaut (`Menu.new` + `setAsAppMenu`/`setAsWindowMenu`), aus dem einzigen Kürzelregister `src/ui/shortcuts.ts` (UI-113) und den Menübeiträgen der Features (`features/<f>/menu.ts`). Damit sind Menüanzeige, Hilfe-Dialog und Tastaturbehandlung aus einer Quelle (CI-Test UI-113). Aktivierungszustand („Crawl-Analyse starten“ ohne Daten deaktiviert, UI-002) folgt Selektoren auf den Stores.
- Bulk-Export-, Berichte- und Sitemaps-Untermenüs werden aus `export.catalog` erzeugt.
- **macOS:** App-Menü (Über, Einstellungen… Cmd+,, Dienste, Ausblenden, Beenden). [Offen] F-5: WKWebView braucht ein Menü „Bearbeiten“ mit den vordefinierten Einträgen, damit Cmd+C/V/X/Z/A in Textfeldern funktionieren – das widerspricht der exakten Menüreihenfolge aus UI-002.
- **Windows:** Fenstermenü mit Mnemonics (`&Datei`), F10/Alt aktiviert die Leiste, Einstellungen unter Datei. [Spike] Menü-Accelerators auf Windows dürfen Eingabefeldern Strg+C/V/A nicht entziehen; Edit-Kürzel daher nur als Beschriftung, Ausführung im JS-Dispatcher.

### 2.10 Barrierefreiheit (Basis für UI-126)

- **Tastatur:** vollständig (1.6), sichtbarer Fokusring in beiden Themes (Token `focus`, ≥ 3:1).
- **Screenreader:** Landmarks (1.2), Tab-Leisten als `tablist`, Filter als `combobox`, Dialoge mit `aria-modal`, Titel und Beschreibung. Primitive aus **React Aria Components** (Apache-2.0), die Tastatur, Fokus und ARIA für Menüs, Dialoge, Tabs, Combobox, Tooltip, Listbox, Tree korrekt umsetzen.
- **Grid:** Glide erzeugt parallel zum Canvas einen versteckten `role="grid"`-Baum mit `aria-rowcount` (Gesamtzahl + Kopfzeile) und `aria-rowindex` für die sichtbaren Zeilen (im Quelltext vorhanden, [Spike] mit VoiceOver/NVDA prüfen); Zellwerte über `getAccessibilityString` (Statuscode als „404 Client Error“, nicht nur Farbe).
- **Live-Regionen:** `aria-live="polite"` nur für seltene Zustandswechsel (Crawl gestartet/pausiert/fertig, Export fertig, Analyse fertig), nicht für jeden Tick.
- **Nie nur Farbe:** Statuscodes mit Text, Issue-Typen mit Formsymbol (Kreis/Dreieck/Raute), Diff mit +/−-Randmarke.
- **Kontrast:** Token-Unit-Test (≥ 4,5:1 Text, ≥ 3:1 Symbole/Grafik/Fokus).
- `prefers-reduced-motion`: keine Animationen außer Fortschrittsbalken.

---

## 3 Tabellen-Engine

### 3.1 Anforderungen (aus PERF-004, UI-140, UI-141, UI-007 … UI-012, UI-110, UI-126)

| # | Anforderung | Quelle |
|---|---|---|
| A1 | 5 Mio. Zeilen, Scrollbalken repräsentiert die volle Zeilenzahl, Sprung zur letzten Zeile ohne Präzisionsverlust (über Browser-Höhengrenze ~33,5 Mio. px) | UI-140 |
| A2 | ≥ 55 fps, kein Frame > 50 ms bei Dauerscrollen; < 300 DOM-Zeilen | UI-140 |
| A3 | Spaltenvirtualisierung über 65 Spalten, ≥ 55 fps horizontal | UI-140 |
| A4 | Asynchrones Nachladen mit Platzhaltern, Sprung ≤ 150 ms | UI-140, UI-142 |
| A5 | Angeheftete Spalte, Spaltenbreite ziehen/Autobreite, Spalten verschieben, Kopf-Kontextmenü, „+“-Menü | UI-007, UI-008 |
| A6 | Mehrfachauswahl (Shift/Mod), Kopieren TSV mit Kopfzeile, Mod+A inkl. nicht geladener Zeilen | UI-012 |
| A7 | Kontextmenüs auf Zeilen und Kopf, < 100 ms | UI-110 |
| A8 | Eigene Zellarten (Statuscode farbig + Text, Bytes mit Tooltip, Segment-Label, Stift-Symbol), Tooltips | UI-007, UI-020, UI-037 |
| A9 | ARIA-Grid mit `aria-rowcount`/`aria-rowindex` | UI-126 |
| A10 | HiDPI/Zoom korrekt | UI-001, UI-120 |
| A11 | Lizenz Apache-2.0-kompatibel | CLAUDE.md |

### 3.2 Bewertung

| Kriterium | **Glide Data Grid** | **TanStack Virtual + eigenes Canvas** | **AG Grid Community** |
|---|---|---|---|
| Lizenz | MIT ✔ (Abhängigkeiten lodash, marked, react-number-format, canvas-hypertxt, @linaria/react – MIT) | MIT ✔ | MIT ✔ – aber Zwischenablage/Bereichsauswahl, Kontextmenü, Server-Side Row Model nur in der kommerziellen Enterprise-Ausgabe |
| Rendering | Canvas, nur sichtbare Zellen | Canvas (eigen) | DOM-Zeilen und -Zellen |
| A1 > 33,5 Mio. px | im aktuellen Entwicklungszweig (6.0.4-alpha, Okt. 2025) **hybrides Scrollen** jenseits der Browsergrenze (Konstante `BROWSER_MAX_DIV_HEIGHT`, Fortschritt-Mapping); stabile 6.0.3 (Feb. 2024) ohne → **Spike** | frei gestaltbar (eigener Scrollbalken), Aufwand hoch | eingebaute Höhenstreckung für große Zeilenzahlen ✔ |
| A2/A3 Leistung 65 Spalten | sehr gut (Canvas, Spalten- und Zeilenvirtualisierung eingebaut) | sehr gut, wenn gut gebaut | mäßig: DOM-Last bei 65 Spalten × ~40 Zeilen, Spaltenvirtualisierung vorhanden; 55 fps auf Referenzhardware (B) fraglich |
| A4 async | ✔ `getCellContent` + Loading-Zellen, `onVisibleRegionChanged` | eigen | ✔ Infinite Row Model (Community) |
| A5 Spalten | ✔ Resize, Move, `freezeColumns`, Header-Menü, `rightElement` für „+“ | alles eigen | ✔ |
| A6 Kopieren | ✔ `getCellsForSelection` (auch asynchron), eigener `onCopy`-Pfad möglich | eigen | ✘ in Community nur rudimentär (Enterprise-Feature) |
| A7 Kontextmenüs | Ereignisse ✔ (`onCellContextMenu`, `onHeaderContextMenu`), Menü selbst eigen | eigen | ✘ Kontextmenü Enterprise |
| A8 eigene Zellen | ✔ `customRenderers`, `drawHeader`, `getRowThemeOverride` | eigen | ✔ React-Zellrenderer (DOM, teuer) |
| A9 ARIA | ✔ versteckter `role="grid"`-Baum mit `aria-rowcount`/`aria-rowindex` | komplett eigen (hoher Aufwand) | ✔ sehr gut |
| A10 HiDPI | ✔ `devicePixelRatio` | eigen | ✔ (DOM) |
| React 19 | Peer-Range erst im Alpha-Zweig (`… 18.x \|\| 19.x`), stabil nur bis 18 | ✔ | ✔ |
| Wartung | langsame Releases; stabile Linie seit Feb. 2024 unverändert | bei uns | aktiv, aber Community-Funktionsumfang bewusst begrenzt |
| Bündelgröße | mittel | klein | groß |
| Aufwand für uns | gering–mittel (Adapter, Menüs, Zwischenablage) | **sehr hoch** (Auswahl, Fokus, Spalten-Drag, IME, A11y, Tooltips, Textmessung) | mittel, aber Nachbau der Enterprise-Funktionen nötig |

Weitere geprüfte Alternativen (kurz): RevoGrid (MIT, Web Components, DOM) – A2/A3 bei 65 Spalten fraglich; SlickGrid (MIT, DOM) – veraltetes Modell, A11y schwach; Handsontable – **Lizenz nicht freizügig**, ausgeschlossen; Perspective (Apache-2.0) – Analyse-Engine mit eigener Datenhaltung, passt nicht zum Kern-zentrierten Modell.

### 3.3 Empfehlung

**[Entscheidung] Glide Data Grid** (wie in `docs/ARCHITECTURE.md` vorgesehen), **hinter einem eigenen Adapter** `src/grid/` mit schmaler Schnittstelle:

```ts
interface DataGridProps {
  source: ViewDataSource;                 // 2.4 – einzige Datenquelle
  columns: ColumnState[];                 // Sichtbarkeit, Reihenfolge, Breite, Anheftung, Sortierung (aus Store)
  selection: SelectionModel;              // UrlId-basiert
  onSelectionChange, onSortChange, onColumnsChange, onContextMenu, onActivate;
  rowHeight: number; theme: GridTheme; ariaLabel: string;
}
```

Begründung:
1. Einziger Kandidat, der Canvas-Leistung, Spaltenvirtualisierung, Spaltenbedienung, Kopier-Hooks und ein ARIA-Grid **zusammen** unter MIT bietet.
2. AG Grid Community verlangt den Nachbau genau der Teile, die Enterprise vorbehalten sind (Zwischenablage, Kontextmenü), und erreicht die Bildratenziele mit DOM-Zellen bei 65 Spalten auf Referenzhardware (B) voraussichtlich nicht.
3. Ein eigenes Canvas-Grid ist die bessere *Rückfalloption*, aber kein Startpunkt: A11y, Fokus, Auswahl, IME und Spalten-Drag kosten mehrere Paketwellen.

Auflagen (Welle 0, Spike S-1, Abbruchkriterien):
- **Version:** Alpha-Zweig `6.0.4-alpha24` (oder neuer) **exakt gepinnt**, weil nur dieser React 19 und das hybride Scrollen enthält. Spike misst auf beiden Plattform-WebViews (WKWebView, WebView2) mit 5 Mio. Zeilen × 65 Spalten: Sprung ans Ende, Dauerscrollen ≥ 55 fps, horizontales Scrollen, Platzhalter ≤ 150 ms.
- **Fallback A** (falls hybrides Scrollen nicht trägt): **Fenster-Rebasing** im Adapter – Glide sieht höchstens ~1,4 Mio. Zeilen (unter der Höhengrenze), der Adapter führt einen eigenen vertikalen Scrollbalken über die Gesamtzahl und verschiebt die Basis bei Annäherung an den Rand unsichtbar.
- **Fallback B** (falls Glide grundsätzlich scheitert, z. B. A11y oder React 19): eigenes Canvas-Grid hinter derselben `DataGridProps`-Schnittstelle (eigenes Paket, Welle 2).
- **Kopieren:** Glide-eigene Kopierfunktion aus; Mod+C ruft `view.copy(viewId, selection, columns)` – der Kern erzeugt TSV (Kopfzeile, sichtbare Spalten in Anzeigereihenfolge, Tab/Zeilenumbruch → Leerzeichen, Zeilenende `\n`/`\r\n` je Plattform) und schreibt selbst in die Zwischenablage (`tauri-plugin-clipboard-manager`, Rust-Seite). Größenwarnung > 100 000 Zeilen vorher in der UI (UI-012).
- **Kontextmenüs:** HTML-Menüs (React Aria `Menu`) statt nativer Popups – deaktivierte Einträge brauchen Tooltip-Begründungen (UI-110), und HTML-Menüs sind in E2E und Komponenten-Tests prüfbar. Öffnen < 100 ms, weil Zählungen nicht synchron berechnet werden.
- **Spalten „Row“:** echte Datenspalte (Position in der View + 1), nicht Glides Zeilenmarker – damit Export/Kopie konsistent sind.

### 3.4 Weitere Ansichtskomponenten (Entscheidungen)

| Zweck | Wahl | Lizenz | Begründung |
|---|---|---|---|
| Code-Ansicht View Source (bis 20 MB, Syntax, Suche, Diff-Markierung) | CodeMirror 6 (`@codemirror/*`), nur lesend | MIT | Viewport-Rendering, inkrementelles Highlighting; Diff-**Berechnung** im Kern (`similar`, Apache-2.0), CM zeigt nur Dekorationen |
| Baumgraphen (UI-091, UI-093) | d3-hierarchy + Canvas | ISC | Layout trivial, Canvas statt SVG-DOM (UI-146) |
| Kraftgerichtete Graphen (UI-092, UI-094) | graphology + sigma.js (WebGL), Layout d3-force bzw. ForceAtlas2 im Web Worker | MIT | 10 000 Knoten ≥ 30 fps; [Offen] F-9 Abweichung von „D3/Cytoscape“ in ARCHITECTURE |
| 3D (UI-098) | three.js / 3d-force-graph | MIT | WebGL |
| Wortwolken (UI-099/100) | d3-cloud | BSD-3-Clause | Standard |
| Kleine Diagramme (Site Structure, Response Times, Nutzungsstatistik) | eigene SVG-Komponenten | – | ≤ 12 Balken; keine Bibliothek nötig |
| UI-Primitive | React Aria Components | Apache-2.0 | A11y/Tastatur/i18n ausgereift |
| Icons | Lucide | ISC | siehe 5.5 |

---

## 4 Tauri-Command-Katalog

### 4.1 Konventionen

- **Domänen als interne Tauri-Plugins** (`tauri::plugin::Builder::new("<domain>")`): Aufruf `plugin:<domain>|<command>`. Vorteil: jede Domäne hat eine eigene Datei (`src-tauri/src/commands/<domain>.rs`), `lib.rs` registriert nur die Plugin-Liste – klare Besitzgrenzen zwischen Paketen. Im Katalog unten als `<domain>.<command>` geschrieben.
- **Typen:** Rust-DTOs in `os-app::dto` mit `serde(rename_all = "camelCase")` und **ts-rs** (MIT) → je Typ eine Datei `apps/desktop/src/ipc/gen/<Type>.ts` (keine Sammeldatei → keine Merge-Konflikte). Command-Wrapper in TS handgeschrieben je Domäne (`src/ipc/<domain>.ts`); ein Vertragstest vergleicht das vom Kern erzeugte Manifest (`commands.json`: Name, Argumenttypen, Rückgabetyp) mit den Wrappern. (Alternative `tauri-specta` erzeugt beides, war für Tauri v2 lange nur als RC verfügbar – im Spike S-4 neu bewerten.)
- `UrlId` als JSON-Zahl (< 2^53 garantiert), `CrawlId` als String (UUID v7), Zeitstempel ISO 8601 UTC, Bytes/Dauern als Rohzahlen (Formatierung in der UI).
- Jeder Command liefert `Result<T, CmdError>`; lange Operationen liefern sofort `OpHandle` und melden sich per Tick.

### 4.2 Fehlerformat

```rust
#[derive(Serialize, TS, thiserror::Error, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    pub code: ErrorCode,
    pub message_key: String,                 // i18n-Schlüssel, z. B. "err-export-dir-not-writable"
    pub args: BTreeMap<String, serde_json::Value>, // z. B. {"path": "/Users/…/Exports"}
    pub field: Option<String>,               // bei InvalidInput: TOML-Pfad bzw. Feldname
    pub detail: Option<String>,              // technische Ursache (englisch, redigiert, OUT-243)
    pub retryable: bool,
}
#[derive(Serialize, TS)]
pub enum ErrorCode { InvalidInput, NotFound, Stale, Cancelled, Busy, Timeout, Io, Permission,
                     Unsupported, NoCrawl, CrawlRunning, Store, Engine, Internal }
```
```ts
export type ErrorCode = "InvalidInput" | "NotFound" | "Stale" | "Cancelled" | "Busy" | "Timeout" | "Io"
  | "Permission" | "Unsupported" | "NoCrawl" | "CrawlRunning" | "Store" | "Engine" | "Internal";
export interface CmdError { code: ErrorCode; messageKey: string; args: Record<string, unknown>;
  field?: string; detail?: string; retryable: boolean }
```
Abbildung aus M1-Fehlern: `ViewError`/`NameError` → `InvalidInput` (mit Vorschlägen in `args.suggestions`), `StoreError` (Datei fremd/neuer) → `Store` mit eigenem Schlüssel, `EngineError` → `Engine`, SQLite-Interrupt → `Cancelled`, Versionskonflikt → `Stale`. Panics im Command werden abgefangen → `Internal` + Log.

### 4.3 Kern-DTOs (Auszug, verbindlich für die Feinplanung)

```rust
// Views ------------------------------------------------------------------
pub enum ViewSpec {                                   // #[serde(tag = "kind")]
    Tab { tab: String, filter: String, search: Option<SearchSpec>, sort: Vec<SortKey>,
          segment: Option<SegmentFilter>, compare: Option<CompareClass> },
    Inlinks  { url_ids: Vec<UrlId>, filter: LinkFilter, search: Option<String>, sort: Vec<SortKey>, compare: Option<CompareClass> },
    Outlinks { url_ids: Vec<UrlId>, filter: LinkFilter, search: Option<String>, sort: Vec<SortKey>, compare: Option<CompareClass> },
    Resources { url_ids: Vec<UrlId>, filter: ResourceFilter, search: Option<String>, sort: Vec<SortKey> },
    Images { url_ids: Vec<UrlId>, search: Option<String>, sort: Vec<SortKey> },
    Detail { detail: DetailTable /* Headers, Cookies, ConsoleLog, Spelling, A11yRules, A11yNodes, Duplicates, NGrams … */,
             url_ids: Vec<UrlId>, params: serde_json::Value, sort: Vec<SortKey> },
}
pub struct SortKey { pub column: String, pub dir: SortDir /* Asc | Desc */ }
pub enum SearchSpec { Simple { text: String }, Rules { tree: RuleNode, case_sensitive: bool } }
pub struct ViewHandle { pub view_id: String, pub version: u64, pub total: Option<u64> /* None = wird berechnet */,
                        pub columns: Vec<ColumnMeta>, pub state: ViewState /* Ready | Building */, pub op: Option<OpHandle>,
                        pub empty_state: Option<EmptyState> }
pub struct ColumnMeta { pub key: String /* Kanon-Name */, pub kind: ColumnKind, pub default_visible: bool,
                        pub default_width: u16, pub align: Align, pub sortable: bool, pub searchable: SearchOps,
                        pub extension: bool /* [OS-Erweiterung] */, pub needs: Option<EmptyState> }
pub struct RowWindow { pub view_id: String, pub version: u64, pub offset: u64,
                       pub row_keys: Vec<i64> /* UrlId bzw. Detailzeilen-ID */, pub columns: Vec<String>,
                       pub cells: Vec<Vec<serde_json::Value>> /* spaltenweise; null = leer */ }
pub struct EmptyState { pub code: String /* UI-016: NO_SELECTION … NOT_APPLICABLE */, pub arg: Option<String>,
                        pub action: Option<EmptyAction> /* OpenSetting{anchor} | StartAnalysis | ClearSearch | ConnectApi{api} */ }
pub enum Selection { Ids { ids: Vec<UrlId> }, All { view_id: String, version: u64, except: Vec<UrlId> } }

// Kataloge -----------------------------------------------------------------
pub struct TabInfo { pub id: String, pub default_order: u16, pub filters: Vec<FilterInfo>, pub detail_capable: bool }
pub struct FilterInfo { pub id: String, pub label_template: String, pub params: BTreeMap<String, serde_json::Value>,
                        pub count: Option<u64>, pub pct: Option<f64>, pub state: FilterStateDto,
                        pub issue: Option<IssueBadge> /* Typ + Priorität */, pub analysis_step: Option<String> }

// Ereignisse ---------------------------------------------------------------
pub struct Tick { pub seq: u64, pub at: String,
                  pub crawl: Option<CrawlStatus>,          // Zustand, Fortschritt (CRAWL-130), Speicherstufe, Dateigröße
                  pub analysis: Option<AnalysisStatus>,    // Teilschritt, Prozent, veraltete Schritte
                  pub views: Vec<ViewChanged>,             // { view_id, version, total }
                  pub counts_version: u64, pub issues_version: u64, pub sidebar_version: u64,
                  pub ops: Vec<OpStatus>,                  // { op_id, label_key, done, total, state, result?, error? }
                  pub mcp: Option<McpStatus>,              // Off | Ready{port} | Connected{clients}
                  pub notices: Vec<Notice> }               // { level, code, args } – Startfehler, Plattenplatz, Crawl-Ende …
pub struct OpHandle { pub op_id: String, pub label_key: String, pub cancellable: bool }
```

```ts
// Generiert (Auszug) – Spiegel der Rust-Typen
export type ViewSpec =
  | { kind: "tab"; tab: string; filter: string; search?: SearchSpec; sort: SortKey[]; segment?: SegmentFilter; compare?: CompareClass }
  | { kind: "inlinks" | "outlinks"; urlIds: number[]; filter: LinkFilter; search?: string; sort: SortKey[]; compare?: CompareClass }
  | { kind: "resources"; urlIds: number[]; filter: ResourceFilter; search?: string; sort: SortKey[] }
  | { kind: "images"; urlIds: number[]; search?: string; sort: SortKey[] }
  | { kind: "detail"; detail: DetailTable; urlIds: number[]; params: unknown; sort: SortKey[] };
export interface RowWindow { viewId: string; version: number; offset: number; rowKeys: number[];
  columns: string[]; cells: (string | number | boolean | null)[][] }
export interface Tick { seq: number; at: string; crawl?: CrawlStatus; analysis?: AnalysisStatus;
  views: { viewId: string; version: number; total: number | null }[]; countsVersion: number;
  issuesVersion: number; sidebarVersion: number; ops: OpStatus[]; mcp?: McpStatus; notices: Notice[] }
```

### 4.4 Katalog

Spalte „Quelle“: **M1** = existiert laut M1 3.6 (Funktion in Klammern), **K-n** = Kernbedarf aus 4.5, **neu** = reine Service-Schicht (`os-app`) über vorhandenen Funktionen.

**app / events / settings / uistate / diag**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `app.info` | – | `AppInfo { version, build, platform, arch, paths }` | UI-127 | M1 (`paths::app_paths`) |
| `events.subscribe` | `Channel<UiEvent>` | `SubscriptionId` | UI-018, UI-143 | K-2, neu |
| `events.setVisibility` | `visible: bool` | – | UI-143 (≤ 1 Hz) | neu |
| `events.setInterval` | `ms: u32` (250–5000) | – | UI-018 | neu |
| `settings.get` / `settings.set` | – / `SettingsPatch` | `AppSettings` | OUT-222, UI-120 … 124 | K-7 |
| `uistate.load` / `uistate.save` | – / `UiStateJson` (versioniert) | `UiStateJson` | UI-125, UI-021 | neu (atomar + `.bak`) |
| `diag.debugInfo` | – | `DebugInfo` | UI-127 | neu |
| `diag.bundle` | `dest: PathBuf` | `OpHandle` | UI-127 | neu (Redaktion OUT-243) |
| `diag.logFrontend` | `level, message, context` | – | UI-019 | neu |
| `diag.openLogs` / `diag.usageStats` | – / `range` | – / `UsageStats` | UI-002, OUT-245 | neu / K-12 |

**crawl / analysis**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `crawl.validateStartUrl` | `input: String` | `{ normalized } \| CmdError(field)` | UI-003 | M1 (`os_url::normalize::parse_start_url`) |
| `crawl.parseList` | `{ text } \| { path }` | `ListPreview { valid, invalid, duplicates, sample }` | UI-003, CRAWL-027 | M1 (`list_input`) |
| `crawl.start` | `StartRequest { source: Spider{url} \| List{items,label}, scope, profile?: String }` | `CrawlRef { crawlId, file }` | UI-003 | M1 (`start_crawl`) + K-3 |
| `crawl.pause` / `crawl.resume` / `crawl.stop` | – | – | UI-003, CRAWL-126 | M1 (`CrawlHandle::pause/stop`, `resume_crawl`) |
| `crawl.clear` | – | – | UI-003 („Leeren“ nur Ansicht) | neu |
| `crawl.open` | `{ crawlId } \| { path }` | `CrawlSummaryDto` | CRAWL-127 | M1 (`CrawlFile::open`, Migration) |
| `crawl.saveAs` | `dest` | `OpHandle` | CRAWL-127 | M1 (`save_as`) |
| `crawl.recrawl` / `crawl.remove` | `Selection` | `OpHandle` | UI-114, CRAWL-131 | M1 (`recrawl_urls`, `remove_urls`) |
| `crawl.addUrls` | `urls: Vec<String>` | `OpHandle` | CRAWL-131 [P1] | K-3 |
| `crawl.path` | `urlId` | `CrawlPathDto[]` | UI-050, OUT-023 | K-11 |
| `analysis.start` | `steps?: Vec<String>` | `OpHandle` | UI-080, CRAWL-141 | M1 (`analysis::run`/`ensure_analysis`) |
| `analysis.status` | – | `AnalysisStatus { steps: [{step, state, stale}] }` | UI-082 | M1 (`analysis_steps`), K-5 |

**catalog / view / search**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `catalog.tabs` | – | `TabInfo[]` (ohne Zähler) | UI-004, UI-006 | M1 (`tabs::all()`) + K-4 (Metadaten) |
| `catalog.columns` | `tab` | `ColumnMeta[]` inkl. dynamischer Spalten | UI-008 | M1 (`ColumnDef`) + K-4 |
| `catalog.filterCounts` | `segment?: SegmentFilter`, `compare?` | `{ version, filters: FilterInfo[] }` | UI-006, UI-014, UI-061 | M1 (`filter_counts`) + K-1 (inkrementell) |
| `view.open` | `ViewSpec`, `columns: Vec<String>` | `ViewHandle` | UI-142 | K-1 |
| `view.update` | `viewId`, `ViewSpec` | `ViewHandle` (neue Version, ggf. `Building` + `OpHandle`) | UI-010, UI-011, UI-141 | K-1 |
| `view.rows` | `viewId, version, offset, limit (≤ 500), columns` | `RowWindow` | UI-140, UI-142 | K-1 |
| `view.locate` | `viewId, urlId` | `Option<u64>` (Zeilenindex) | UI-004, UI-018, UI-031 | K-1 |
| `view.copy` | `viewId, Selection, columns, withHeader` | `CopyResult { rows, bytes }` (Zwischenablage im Kern) | UI-012 | K-1, neu |
| `view.close` | `viewId` | – | UI-142 (Freigabe; zusätzlich 60 s Leerlauf im Kern) | K-1 |
| `search.parse` | `text` | `RuleNode \| CmdError(position)` | UI-011 [P1] | K-6 |
| `search.validateRegex` | `pattern` | `Ok \| CmdError(position)` | UI-011, UI-068, OUT-146 | M1 (`RuleSet::compile`) |
| `op.cancel` | `opId` | – | UI-141 | neu (`CancelToken`, Interrupt) |

**detail** (nur nicht-tabellarische; tabellarische laufen über `view.*` mit `ViewSpec::Inlinks/Outlinks/Resources/Images/Detail`)

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `detail.urlDetails` | `urlId` | `NameValue[]` (Reihenfolge UI-031) | UI-031 | M1 (`url_by_address`, Spalten Internal) |
| `detail.serp` | `urlId, device` | `SerpData { title, description, host, breadcrumb, favicon?, edited? }` | UI-037 | K-8 |
| `detail.serpMeasure` | `title, description, device, boldTerms, prefix` | `SerpMeasure { rows: [{chars, px, maxPx, remaining, truncated, cutAt}] }` | UI-037, UI-038 (< 16 ms) | M1 (`pixel::width_px`) + K-8 (`truncate_at`) |
| `detail.serpSaveEdit` / `detail.serpReset` | `urlId, title?, description?` | – | UI-037 | K-8 |
| `detail.headers` | `urlId` | `{ statusLine, request: Header[], response: Header[] }` | UI-042 | M1 (`queries::headers`) |
| `detail.sourceOpen` | `urlId, which: Original\|Rendered, mode: Html\|Text, pretty` | `SourceDoc { docId, lines, bytes }` | UI-041 | K-9 (`page_html`) |
| `detail.sourceLines` | `docId, from, count` | `String[]` | UI-041 (20 MB virtualisiert) | K-9 |
| `detail.sourceDiff` | `urlId, mode` | `OpHandle` → Ergebnis `DiffHunks { added, removed, hunks }` | UI-041 | K-9 |
| `detail.screenshot` | `urlId` | `{ url: "ospider://shot/<crawl>/<urlId>", width, height, tiles }` | UI-039 | K-9, URI-Schema |
| `detail.imagePreview` | `urlId` | `{ url: "ospider://img/…", width, height, stored }` | UI-035 | K-9 |
| `detail.structuredData` | `urlId` | `SdTree` | UI-045 | K-10 |
| `detail.duplicates` | `urlId, kind: Exact\|Near` | `DupPartner[]` + `textOf(urlId)` | UI-044 | K-10 |
| `detail.lighthouse` | `urlId, device` | `LighthouseDto` | UI-046 (P2) | K-10 |

**sidebar**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `sidebar.overview` | `segment?, compare?` | `OverviewDto { summary: Row[], tabs: [{tab, filters: FilterInfo[]}] }` | UI-061, OUT-142 | M1 (`filter_counts`) + K-1 |
| `sidebar.issues` | `type?, priority?, search?, segment?, compare?` | `IssueRow[]` (inkl. Segmentanteile) | UI-062, OUT-143 | M1 (`issues_from_counts`) |
| `sidebar.issueExplain` | `issueId, lang` | `IssueText { what, why, fix, rule, learnMoreUrl }` | UI-065, AGENT-013 | M1 (`issues::texts`) – [Offen] F-4 |
| `sidebar.siteStructure` | `view: Indexability\|ResponseCodes\|CrawlDepth, segment?` | `{ paths: PathNode[], depths: DepthRow[] }` | UI-066 | K-5 (Aggregate) |
| `sidebar.responseTimes` | `scope: All\|InternalHtml` | `{ buckets: Bucket[], noResponse }` | UI-069 | K-5 |
| `sidebar.segments` | `view` | `SegmentRow[]` | UI-067 | K-5, CRAWL-134 |
| `sidebar.apiStatus` | – | `ApiRow[]` | UI-070 | K-13 |

**config / profile / segment**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `config.get` | – | `CrawlConfigDto` (+ `schemaVersion`) | 01 C–O | M1 (`CrawlFile::config`, `CrawlConfig`) |
| `config.validate` | `CrawlConfigDto` | `ValidationIssue[] { path, messageKey, args }` | ONPAGE-002, CRAWL-132 | M1 (P13-Validierung) |
| `config.set` | `CrawlConfigDto` | `{ invalidatedSteps: String[] }` | ONPAGE-002, UI-082 | M1 + K-5 |
| `config.schema` | – | JSON-Schema | – | M1 (P13 `schemars`) |
| `config.diff` | `base: Defaults\|Profile{name}\|Crawl{id}` | `DiffRow[]` | OUT-224 (P2) | neu |
| `config.testRules` | `kind: IncludeExclude\|Rewrite\|Mapping, rules, input` | `RuleTrace` | CRAWL-109 ff., OUT-146 | M1 (`RuleSet`) + K-3 |
| `profile.list/load/save/delete/setDefault` | Name/Pfad | – | OUT-220 | K-7 |
| `segment.get/set/validate/previewCounts` | `SegmentDef[]` | Zähler je Segment | UI-068, CRAWL-134 | K-14 |

**crawls (Verwaltung)**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `crawls.list` | `CrawlListQuery { show, search, sort }` | `CrawlListing[]` (≤ 1 000, aus Index) | OUT-228 | M1 (`registry::list_crawls`) + K-12 (Index) |
| `crawls.rename/move/lock/unlock/duplicate/delete` | `ids, …` | `OpHandle` bzw. – | OUT-228 | K-12 |
| `crawls.import/export` | `path(s)` | `OpHandle` | OUT-228, CRAWL-127 | M1 (`save_as`) + K-12 |
| `crawls.reveal` / `crawls.copyIds` | `ids` | – | OUT-228 | neu |
| `projects.create/rename/lock` | … | – | OUT-228 | K-12 |

**export**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `export.catalog` | – | `{ bulk: Group[], reports: Group[], sitemaps: … }` | UI-002, OUT-030 ff. | M1 (`catalog::list`) |
| `export.suggestName` | `ExportTarget` | `String` (OUT-008/OUT-020) | OUT-006 | M1 (`naming::file_name`) |
| `export.view` | `viewId, version, path, format, allColumns` | `OpHandle` | OUT-020 | M1 (`export_tab`) + K-1 (Snapshot der View) |
| `export.detail` | `ViewSpec \| DetailKind, Selection, path, format` | `OpHandle` | OUT-021 | M1 (`export_detail`) |
| `export.bulk` / `export.report` | `name, dir, format, singleWorkbook` | `OpHandle` | OUT-030 ff., OUT-080 ff., OUT-007 | M1 (`export_bulk`, `save_report`) |
| `export.sitemap` | `SitemapOptions, dir` | `OpHandle` | OUT-120 ff. | M1 (`generate_sitemap`) |
| `export.issues` / `export.sidebar` | `which, path, format` | `OpHandle` | UI-060, UI-062 | K-11 |

**compare**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `compare.open` | `current, previous, mapping: MappingRule[], elements` | `CompareSession` | OUT-140, OUT-146 | K-15 |
| `compare.swap` | – | `CompareSession` | OUT-141 | K-15 |
| `compare.testMapping` | `rules, input` | `RuleTrace` | OUT-146 | K-15 |
| `compare.close` | – | – | – | K-15 |
| (Overview/Issues/Views) | `compare`-Parameter der obigen Commands | Vergleichsspalten, Unterfilter | OUT-142 … 149 | K-15 |

**viz**

| Command | Eingabe | Ausgabe | Spec | Quelle |
|---|---|---|---|---|
| `viz.open` | `kind, rootUrlId?` | öffnet Fenster, `SnapshotId` | UI-090 | neu |
| `viz.graph` | `snapshotId, kind, root, budget, metric` | `GraphDto { nodes, edges, truncated }` | UI-091 … 097 | K-16 |
| `viz.words` | `snapshotId, kind: Anchors\|Body, urlId` | `WordFreq[]` | UI-099, UI-100 | K-16 |
| `viz.exportCsv` | `snapshotId, dir` | `OpHandle` (nodes.csv, edges.csv) | UI-090 | K-16 |
| `viz.focusInMain` | `urlId` | – (Hauptfenster nach vorn, Auswahl setzen) | UI-090 | neu |

**mcp** (UI-129, OUT-285, AGENT-002, -003, -043)

| Command | Eingabe | Ausgabe | Quelle |
|---|---|---|---|
| `mcp.status` | – | `McpStatus` | K-17 |
| `mcp.start` / `mcp.stop` | `port?` | – | K-17 (HTTP-Transport in-process) |
| `mcp.tools` | – | `ToolInfo[] { name, class, enabled, description, schema }` | M1 (`tools::all()`) |
| `mcp.setToolEnabled` / `mcp.setClasses` | `name, enabled` / `classes` | – | K-17 |
| `mcp.token` / `mcp.regenerateToken` | `reveal: bool` | `{ masked, value? }` | K-17 (Schlüsselbund) |
| `mcp.setAutostart` | `bool` | – | K-17 |
| `mcp.saveToolsMarkdown` | `path` | – | K-17 |
| `mcp.clientConfigPreview` | `client: ClaudeDesktop\|ClaudeCode\|Codex` | `{ path, before, after, diff }` | K-17 |
| `mcp.clientConfigWrite` | `client` | `{ backupPath }` | K-17 |
| `mcp.callLog` | `offset, limit` | `CallLogRow[]` (ohne Geheimnisse) | K-17 |

**api** (Integrationen 04 A–J, ohne KI) – Grundgerüst; Einzelheiten gehören in den Paketplan der Integrationen:
`api.connect(kind)` (OAuth im Systembrowser mit Loopback-Rückruf), `api.disconnect(kind)`, `api.settings.get/set(kind)`, `api.status()` (→ `sidebar.apiStatus`), `api.errors(kind, offset, limit)`.

### 4.5 Was M1 liefert und was M2 im Kern ergänzen muss

**Aus M1 direkt nutzbar** (laut M1 3.3–3.6): `CrawlFile` (open/meta/config/save_as/integrity_check/effective_state), `queries::{url_by_address, inlinks, outlinks, headers, redirect_chain}`, `registry::list_crawls`, `paths::app_paths`, `tabs::all()` mit `TabDef/ColumnDef/FilterDef`, `views::{query_view, stream_view, filter_counts, resolve_name}`, `analysis::{run, mark_stale, ensure_analysis, run_after_crawl, CancelToken}`, `issues::{catalog, texts, issues_from_counts}`, `os-engine::{start_crawl, resume_crawl, CrawlHandle(progress/watch/pause/stop/set_threads), recrawl_urls, remove_urls, list_input}`, `os-export::{export_tab, export_detail, export_bulk, save_report, generate_sitemap, catalog, naming}`, `os_parse::pixel::width_px`, `os_url::normalize::parse_start_url`, `RuleSet`, `os-mcp::tools::all()`, `Progress`, `FilterState`.

**Prüfpunkte bei der M1-Abnahme** (sonst Nacharbeit vor M2-Welle 1):
- `ColumnDef` enthält Spaltenart und Standardsichtbarkeit; `FilterDef.id` nutzt Platzhalter-Vorlagen (für i18n nötig, vorhanden).
- `filter_counts` liefert `FilterState` je Filter (für Leerzustände) – vorhanden.
- `issues::texts` – Struktur passt nicht zu UI-065 (F-4).
- `Progress` enthält kein „Durchschnitt URL/s ohne Pausen“, keine Speicher-/Dateigrößenfelder (UI-013) → K-3.

**[Kernbedarf] M2** (Kurzbeschreibung; Feinplanung im M2-Kernplan):

| ID | Bedarf | Warum | Ort (Vorschlag) |
|---|---|---|---|
| K-1 | **View-Sitzungen mit wahlfreiem Zugriff:** Filterzugehörigkeit als Bitmaps (RoaringBitmap, `roaring`, MIT/Apache) je (Tab, Filter), persistiert in der Crawl-Datei (Migration `0002`) und inkrementell gepflegt; materialisierte Sortierpermutationen (`Vec<u32>` je Sortierspalte, Richtungswechsel = Rückwärtslesen, UI-141 ≤ 200 ms); Schnittmenge Filter × Suche × Segment × Sortierung als Positionsliste; `rows(offset, limit)` per PK-Abfrage; Versionsstempel; Doppelpuffer; LRU-Speicherbudget und 60-s-Freigabe; Detail-Views (1 Mio. Inlinks) über materialisierte Link-ID-Listen; Cursor für MCP/CLI auf derselben Basis | M1 hat nur Cursor-Paging und rechnet Filter per SQL je Abfrage – UI-140/141/142 (5 Mio. Zeilen, Filterwechsel ≤ 200 ms, Sprung ≤ 150 ms) sind so nicht erreichbar | `os-analyze::views::session` + `os-store` (Migration) |
| K-2 | **Änderungsfeed:** `urls.change_seq` + `crawl_meta.change_seq` (Writer erhöht je Batch), Abfrage „geändert seit Seq“; dient Coalescer, Bitmap-Pflege und Crawls fremder Prozesse | Live-Aktualisierung UI-018 ohne Vollscan; AGENT-020 „MCP-Crawl erscheint live“ | `os-store` (Writer, Migration), `os-app` (Coalescer) |
| K-3 | **Sitzungs-/Lebenszyklus-Erweiterungen:** Durchschnittsrate ohne Pausen, Speicherstufe (CRAWL-129), Dateigröße, aktive Threads im Status; `crawl.addUrls` (CRAWL-131 [P1]); Analyse-Hook wie CLI | UI-013, UI-114 | `os-engine`, `os-app` |
| K-4 | **Spalten-/Tab-Metadaten für die UI:** Standardbreite, Ausrichtung, sortierbar, Operatorenmenge, Erweiterungs-Kennzeichen, dynamische Spalten (Custom Extraction/Search, Segment, API-Metriken) | UI-007, UI-008, UI-011 | `os-analyze::tabs` |
| K-5 | **Vorberechnete Aggregate:** Site Structure (Pfade, Tiefen × Indexierbarkeit/Statusklassen), Response-Time-Buckets, Segment-Kennzahlen; Teilschritt-Invalidierung „veraltet“ | UI-066/067/069 (≤ 1 s bei 5 Mio., ≤ 200 ms View-Wechsel), UI-082 | `os-analyze` |
| K-6 | **Suchgrammatik + Regelauswertung:** Textsyntax ↔ Regelbaum, Operatoren aus UI-011, `fancy-regex` mit Schrittlimit, Gesamtabbruch nach 2 s, parallele Adress-Teilstringsuche (≤ 3 s bei 5 Mio.) | UI-011, UI-141; gleiche Semantik für Export (OUT-020) und MCP | `os-analyze::search` |
| K-7 | **App-Einstellungen und Profile:** `AppSettings` (`settings.toml`, OUT-222) inkl. Exportordner-Auflösung, Live-Intervall, Theme, Sprache, Aufbewahrung; Profilverwaltung (OUT-220) | Einstellungen-Fenster, OUT-222 | `os-model::settings`, `os-store` |
| K-8 | **SERP:** Kürzungsposition aus der Pixel-Engine (`truncate_at`), Geräteprofile (ONPAGE-194), gespeicherte Bearbeitungen je URL (neue Tabelle) | UI-037, UI-038 | `os-parse::pixel`, `os-store` |
| K-9 | **Quelltext/Screenshot/Bild:** Zugriff auf `page_html` (+ gerendertes HTML), Zeilenindex für virtualisiertes Lesen, Diff (Zeilen/Wörter) als Op; Screenshot-Kacheln und Bildvorschau über URI-Schema `ospider://` | UI-035, UI-039, UI-041 | `os-store`, `os-app`, `src-tauri` (Protokoll) |
| K-10 | **Detaildaten der P1-Tabs:** Strukturierte Daten, Duplikate, Cookies, Console Log, Rechtschreibung, Accessibility, Lighthouse – entstehen mit den jeweiligen Fachpaketen (01 G, 02/03 P1, 04 I); UI braucht je eine Lesefunktion | UI-040 … UI-048 | Fachcrates |
| K-11 | **Crawl-Pfad, Seitenleisten-/Issues-Export** | UI-050, UI-060, UI-062 | `os-store::queries`, `os-export` |
| K-12 | **Crawl-Index und Verwaltung:** separater Metadaten-Index (1 000 Crawls < 500 ms), Projekte, Sperren, Duplizieren, Import/Export, Aufbewahrung, Nutzungsstatistik | OUT-228/229, OUT-245 | `os-store::registry` |
| K-13 | **API-Status** je Datenquelle (Anforderungen, Empfangen, Fehler, Kontingent) | UI-070 | `os-integrations` |
| K-14 | **Segmente:** Regeln, Validierung, Neuzuordnung ohne Neu-Crawl, Vorschauzähler | UI-020, UI-068, CRAWL-134 | `os-analyze` |
| K-15 | **Vergleich:** Zuordnung mit URL-Mapping, Klassifikation Added/New/Removed/Missing je Filter (Bitmaps aus K-1 je Crawl), Change Detection | OUT-140 … 153, TECH-300 | `os-analyze::compare` |
| K-16 | **Graphdaten:** Baum (erster Entdeckungsweg), Verzeichnisbaum, Kanten mit Knotenbudget, Wortfrequenzen; Schnappschuss je Fenster | UI-090 … 100 | `os-analyze::graph` |
| K-17 | **MCP in der App:** Streamable-HTTP-Server im App-Prozess mit Token/Origin-Prüfung, Sitzungszähler für die Statusanzeige, Werkzeug-/Klassenfreigaben persistent, Client-Konfigurationen schreiben (Diff + Sicherung), Aufrufprotokoll | UI-129, OUT-285, AGENT-002/-003/-043 | `os-mcp` |
| K-18 | **Sortier-Kollation:** sprachbewusst, Groß/Klein-unabhängig (UI-010), identisch für UI, Export und MCP (AGENT-011) – Vorschlag ICU4X `icu_collator` (Unicode-3.0, in `deny.toml` erlaubt) | UI-010, AGENT-011 | `os-analyze` |
| K-19 | **Generator `gen-ui-large`:** deterministische Crawl-Datei mit 5 Mio. Zeilen, ~65 Spalten, ~20 Links/Seite, Ausreißer (1 Mio. Inlinks, 15-MB-HTML), ≤ 10 min | UI-148, PERF-004 | `os-loadtest` (bin) |

---

## 5 Design-System

### 5.1 Typografie

**[Entscheidung]** Systemschriften, keine gebündelten Webfonts (nativer Eindruck, keine Lizenzfrage, kleinere App):

| Rolle | macOS | Windows | Größe / Zeilenhöhe |
|---|---|---|---|
| UI-Text | `-apple-system` (SF Pro) | `"Segoe UI Variable Text", "Segoe UI"` | 13 px / 18 px |
| Tabelle kompakt / normal | wie UI | wie UI | 12 px in 22-px-Zeile / 13 px in 26-px-Zeile (UI-120) |
| Statusleiste, Hinweise, Badges | wie UI | wie UI | 11 px / 14 px |
| Abschnittstitel (Seitenleiste, Dialogseiten) | wie UI, 600 | wie UI, 600 | 13 px |
| Dialogtitel | wie UI, 600 | wie UI, 600 | 17 px |
| Monospace (Quelltext, Header, Regex) | `ui-monospace, "SF Mono", Menlo` | `Consolas, "Cascadia Mono"` | 12 px / 17 px |

Zahlen in Tabellen und Statusleiste mit `font-variant-numeric: tabular-nums`; rechtsbündig (UI-007). Das Canvas-Grid nutzt dieselben Schriftstapel (`system-ui` funktioniert in WKWebView und WebView2 auch im Canvas).

### 5.2 Farben und Tokens

Semantische Tokens (Auszug; vollständige Liste in `src/ui/tokens/tokens.ts`). Kontrastwerte berechnet gegen Tabellenhintergrund `surface` / Zebra-Zeile `zebra` / Auswahl `selection`; der Unit-Test aus UI-120 prüft sie automatisch.

| Token | Hell | Dunkel | Verwendung | Kontrast hell (surface/zebra/sel) | dunkel |
|---|---|---|---|---|---|
| `bg.app` | `#EEF0F3` | `#131416` | Fensterhintergrund, Leisten | – | – |
| `bg.surface` | `#FFFFFF` | `#1B1D20` | Tabellen, Panels | – | – |
| `bg.zebra` | `#F6F7F9` | `#212429` | alternierende Zeilen | – | – |
| `bg.selection` | `#D3EBDD` | `#1F4A35` | markierte Zeilen (aus Akzent abgeleitet) | – | – |
| `text` | `#1B1F24` | `#E7E9EC` | Fließtext | 16,6 / 15,5 / 13,2 | 13,9 / 12,8 / 8,3 |
| `text.muted` | `#59626D` | `#A3ABB5` | Nebentexte | 6,2 / 5,8 / 4,9 | 7,3 / 6,7 / 4,3* |
| `text.disabled` | `#8B939C` | `#6C747E` | deaktiviert (WCAG-Ausnahme) | 3,1 | 3,6 |
| `accent` (OpenSpider-Grün) | `#17784A` | `#4CC38A` | Akzent, Fortschritt, aktive Tabs | 5,5 / 5,1 / 4,4 | 7,6 / 7,0 / 4,5 |
| `focus` | `#1F62C4` | `#6AA8FF` | Fokusring 2 px | 5,8 | 7,0 |
| `link` | `#1F5FBF` | `#7DB3FF` | Verweise | 6,1 | 7,9 |

\* `text.muted` auf Auswahl dunkel 4,3 → in markierten Zeilen wird `text` statt `text.muted` verwendet (Regel im Grid-Theme).

**Statusfarben** (UI-007: 2xx neutral, 3xx gelb, 4xx/5xx rot, 0 grau – Farbe nie allein, Code bleibt als Text sichtbar):

| Token | Hell | Dunkel | Darstellung in der Zelle |
|---|---|---|---|
| `status.2xx` | = `text` | = `text` | nur Zahl |
| `status.3xx` | `#A86A00` (4,4 / 4,1 / 3,5) | `#E0A53A` (7,7) | 3-px-Farbmarke links + Zahl in Textfarbe |
| `status.4xx` | `#C42B2B` (5,6 / 5,3 / 4,5) | `#F2716B` (5,9) | Marke + Zahl |
| `status.5xx` | `#C42B2B` | `#F2716B` | Marke (doppelt) + Zahl – 5xx unterscheidbar ohne Farbe |
| `status.none` | `#7B848E` (3,8 / 3,5 / 3,0) | `#8C949E` (5,5) | Marke + „0“/leer + Statustext |

**Issues** (UI-062): Typ über Form **und** Farbe, Priorität über Text **und** Farbe.

| Token | Hell | Dunkel | Form/Label |
|---|---|---|---|
| `issue.type.issue` | `#C42B2B` | `#F2716B` | gefüllter Kreis mit „!“ |
| `issue.type.warning` | `#A86A00` | `#E0A53A` | Dreieck mit „!“ |
| `issue.type.opportunity` | `#1F62C4` | `#6AA8FF` | Raute ⚖ (UI-062) |
| `issue.prio.high` | `#C42B2B` | `#F2716B` | Label „High“/„Hoch“ |
| `issue.prio.medium` | `#B8530F` (4,9) | `#F08F4A` (7,0) | Label „Medium“/„Mittel“ |
| `issue.prio.low` | `#6B737D` (4,8) | `#9AA2AC` (6,6) | Label „Low“/„Niedrig“ |

**Diff** (UI-041/UI-044): `diff.add` `#1C7C3A`/`#56C773` mit Hintergrund 12 % Deckkraft und Randmarke „+“; `diff.del` `#C42B2B`/`#F2716B` mit „−“.

**Paletten:** 8 Akzentfarben (vorgeprüft ≥ 3:1 gegen `bg.app` und `bg.surface` in beiden Themes) und 12 Segmentfarben (UI-068; Textfarbe auf Label automatisch Schwarz/Weiß für ≥ 4,5:1, UI-020). Statusfarben der Site-Structure-Balken = `status.*` (UI-066).

### 5.3 Dichte, Abstände, Größen

| Größe | Wert | Verwendung |
|---|---|---|
| Raster | 4 px (Stufen 2, 4, 6, 8, 12, 16, 24, 32) | alle Abstände |
| Tabellenzeile | 22 px kompakt (Standard für Datenansichten) / 26 px normal | UI-120 |
| Spaltenkopf | 26 px kompakt / 30 px normal | |
| Zellinnenabstand | 6 px horizontal | |
| Werkzeugzeilen | 32 px Höhe, Steuerelemente 24 px | Filter, Suche, Export |
| Eingabefelder/Schaltflächen in Dialogen | 28 px | |
| Tabs | 28 px (oben/unten/rechts) | |
| Statusleiste | 24 px | |
| Trenner | 4 px sichtbar, 8 px Greiffläche; eingeklappt 6-px-Griff | UI-001 |
| Radius | 4 px Steuerelemente, 8 px Dialoge, 0 px Tabellen | |

**[Entscheidung]** Standard-Zeilenhöhe **kompakt** für Datenansichten (obere Tabelle, Detailtabellen); „normal“ wählbar (UI-120). Hinweis: UI-120 nennt keinen Standard – [Offen] F-12 (Vorschlag kompakt).

### 5.4 Komponentenbestand (Design-System `src/ui/`)

Button (primär/sekundär/Werkzeug), IconButton, TextField (mit Inline-Fehler), SearchField (Lade-/Abbruchzustand), Select/Combobox (Filter-Dropdown mit Zähler und Badges), Tabs (mit Überlauf-Dropdown und Drag-Reihenfolge), SplitPane (Mindestmaße, Einklappen, Doppelklick-Reset), Dialog (plattformgerechte Fußleiste), Menu/ContextMenu (deaktiviert mit Begründung), Tooltip (300 ms), Toast/Banner, EmptyState (`data-empty-state`), ProgressBar (determiniert/indeterminiert), Badge (Segment, Issue-Typ, Priorität), Tree (Overview, Site Structure, Projektbaum), KeyHint (Kürzel plattformgerecht), PropertyTable (Name/Value), ColorPicker (mit Kontrastwarnung).

### 5.5 Icons

**[Entscheidung]** **Lucide** (`lucide-react`, ISC): einheitlicher Strichstil, baumschüttelbar, große Abdeckung. Eigene SVG-Komponenten nur für fachliche Symbole mit fester Bedeutung (Issue-Typen Kreis/Dreieck/Raute, Statusmarken, Analyse-erforderlich-Kennzeichen), damit sie im Canvas identisch gezeichnet werden können (Pfaddaten geteilt). Größen 16 px (Werkzeug), 14 px (Tabelle/Badges), 12 px (Statusleiste). App-Icon unverändert aus dem Gerüst.

### 5.6 Plattform-Konventionen macOS vs. Windows

| Thema | macOS | Windows | Umsetzung |
|---|---|---|---|
| Menüleiste | global, App-Menü links (Über, Einstellungen… ⌘,, Dienste, Ausblenden, Beenden); „Bearbeiten“-Menü nötig für Textfelder (F-5) | Fenstermenü mit Mnemonics, Einstellungen unter Datei (Strg+,), Beenden unter Datei | Menüaufbau aus TS, plattformabhängige Beiträge |
| Kürzel-Modifikator | ⌘ | Strg | `Mod` im Register, Auflösung zur Laufzeit |
| Kürzelanzeige | Symbole `⌃⌥⇧⌘` in Apple-Reihenfolge | `Strg+Alt+Umschalt+X` (de) / `Ctrl+Alt+Shift+X` (en) | `KeyHint` aus Register (UI-113) |
| Zurück/Vor | ⌘[ / ⌘] | Alt+← / Alt+→ | Register |
| Fensterrahmen | native Dekoration, Ampel links | native Dekoration, Schaltflächen rechts | `decorations: true` in beiden; Overlay-Titelleiste erst nach 1.0 (P2) |
| Mindestgröße | 1 024 × 640 | 1 024 × 640 | `minWidth/minHeight` in `tauri.conf.json` (UI-001) |
| Dialog-Fußleiste | Abbrechen links von OK, OK rechts | OK, dann Abbrechen (rechtsbündig) | `DialogFooter` ordnet je Plattform |
| Kontextmenü auslösen | Rechtsklick, Ctrl+Klick | Rechtsklick, Shift+F10, Kontextmenütaste | Grid + Dispatcher |
| Scrollbalken | Overlay-Stil | klassisch | Grid-Theme; eigener Balken (Fallback A) in beiden Stilen |
| Schriften | SF Pro, SF Mono | Segoe UI Variable, Consolas | 5.1 |
| Benachrichtigungen | Mitteilungszentrale | Toasts | `tauri-plugin-notification` (UI-019) |
| Datei-Dialoge | nativ | nativ | `tauri-plugin-dialog` |
| Hauptfenster schließen | Plattformüblich bliebe App aktiv – bei laufendem Crawl Rückfrage, dann beenden (F-11) | Rückfrage bei laufendem Crawl, dann beenden | |
| Kontrastmodus | „Kontrast erhöhen“ (`prefers-contrast`) | Kontrastdesigns (`forced-colors`) | 2.8 |
| Einzelinstanz | – | – | `tauri-plugin-single-instance` (OUT-232), Datei-Öffnen an laufende Instanz |

---

## 6 Wireframes

Die sechs wichtigsten Ansichten, ASCII, logische Pixel nicht maßstäblich. Annotationen rechts nennen die UI-IDs.

### W1 Hauptfenster – Spider-Modus, Crawl läuft

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ ●●●  OpenSpider — shop.beispiel.test                                                                             │
├──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ [https://shop.beispiel.test/                ▾] [Nur diese Subdomain ▾]  [❚❚ Pause] [Leeren]   ▓▓▓▓▓▓▓░░░░  64 %  │ UI-003
├───────────────────────────────────────────────────────────────────────────────────┬──────────────────────────────┤
│ Internal│External│Security│Response Codes│URL│Page Titles│Meta Description│H1│…  » │ Overview│Issues│Site Str.│ » │ UI-004/060
│ [Filter: HTML (12.480) ▾] [⤓ Export]  Angezeigt 12.480 von 18.902   [🔍 Adresse enthält…    ][⌄] │ ▾ Zusammenfassung │ UI-006/014/011
│ ┌─────┬───────────────────────────────────┬────────────┬───────────┬──────────────┬──────┐ [+] │   URLs gefunden  18.902 │ UI-008
│ │ Row │ Address                         📌│ Content    │Status Code│ Indexability │Title…│     │   URLs gecrawlt  12.480 │
│ ├─────┼───────────────────────────────────┼────────────┼───────────┼──────────────┼──────┤     │   …                     │
│ │   1 │ https://shop.beispiel.test/       │ text/html  │      200  │ Indexable    │ Start│     │ ▾ Internal              │
│ │   2 │ https://shop.beispiel.test/schuhe/│ text/html  │ ▌    301  │ Non-Indexable│      │     │   All      18.200 100 % │ UI-061
│ │▓▓3▓▓│▓https://shop.beispiel.test/alt▓▓▓▓│▓text/html▓▓│▓▌▌▓▓▓404▓▓│▓Non-Indexable│▓▓▓▓▓▓│     │   HTML     12.480 68,6 %│
│ │   4 │ https://shop.beispiel.test/kontakt│ text/html  │      200  │ Indexable    │ Kont…│     │ ▸ External              │
│ │ ░░░ │ ░░░░░░░░░░░░░ (lädt) ░░░░░░░░░░░░│            │           │              │      │     │ ▾ Page Titles           │
│ └─────┴───────────────────────────────────┴────────────┴───────────┴──────────────┴──────┘     │   ⊘ Missing      14     │ UI-062 Symbole
├═══════════════════════════════════════════════════════════════════════════ (Trenner) ═══════════┤   △ Duplicate    122     │
│ URL Details│Inlinks│Outlinks│Image Details│Resources│SERP Snippet│View Source│HTTP Headers│ »   │   ◇ Over 60 Ch.  301     │
│ [Alle ▾] [Links|Anchors]                 3 Zeilen    [🔍 Suchen…        ] [⤓ Export]           │ ▸ Canonicals  ⓘ Analyse │ UI-006 „–“
│ ┌──────────┬──────────────────────────────┬──────────────────────────────┬────────┬─────┬────┐ │                          │
│ │ Type     │ From                         │ To                           │ Anchor │Follow│Code│ │                          │ UI-032
│ │ Hyperlink│ https://shop.beispiel.test/  │ https://shop.beispiel.test/alt│ Alt   │ true │404 │ │                          │
│ └──────────┴──────────────────────────────┴──────────────────────────────┴────────┴─────┴────┘ │                          │
├───────────────────────────────────────────────────────────────────────────────────┴──────────────────────────────┤
│ Spider · Läuft · Aktuell 212 URL/s · Ø 187 URL/s · 12.480 von 19.400 (64 %) · Rest 0:32 · Threads 5 ·             │ UI-013
│ Speicher 612 MB / 4 GB · Datei 1,2 GB · MCP: bereit (Port 51234)                                                   │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 ▓ = Auswahl/Fokuszeile   ▌ = Statusmarke (Farbe + Code als Text)   ░ = Platzhalterzeile   📌 = angeheftete Spalte
```

### W2 Seitenleiste Issues mit Erklärung

```
┌─ Overview │ Issues │ Site Structure │ Segments │ » ─────────────────────┐
│ Typ [Alle ▾]  Priorität [Alle ▾]  [🔍 Issues filtern…    ]  [⤓]       │ UI-062
│ ⓘ Weitere Issues nach Crawl-Analyse verfügbar   [Analyse starten]      │ UI-062, UI-016
│ ┌───────────────────────────────────┬──────┬────────┬───────┬────────┐ │
│ │ Issue                             │ Typ  │Priorit.│ URLs  │ % Ges. │ │
│ ├───────────────────────────────────┼──────┼────────┼───────┼────────┤ │
│ │ ● Response Codes: Client Error 4XX│ Issue│ Hoch   │     3 │   0,0 %│ │
│ │▓● Page Titles: Missing ▓▓▓▓▓▓▓▓▓▓▓│▓Issue│▓Hoch▓▓▓│▓▓▓14▓│▓▓0,1 %▓│ │
│ │ △ Directives: Noindex             │ Warn.│ Hoch   │    41 │   0,3 %│ │
│ │ ◇ Page Titles: Over 60 Characters │ Chance│ Mittel│   301 │   2,4 %│ │
│ │   ███████░░░░ Segmentanteile (Blog 60 % · Shop 40 %)               │ │ UI-062 Segmentleiste
│ └───────────────────────────────────┴──────┴────────┴───────┴────────┘ │
│ ── Page Titles: Missing ──────────────────────────────────────────────  │ UI-065
│ Was bedeutet das?  <eigener Text aus dem Kern>                          │
│ Warum ist es relevant?  <eigener Text>                                  │
│ Wie beheben?  <eigener Text>                                            │
│ Typ/Priorität: Issue · Hoch (Begründung: Pflichtelement fehlt; Signal   │
│ wird falsch ausgewertet)                       [Mehr erfahren ↗]        │
└─────────────────────────────────────────────────────────────────────────┘
 Klick auf Zeile → obere Tabelle: Tab „Page Titles“, Filter „Missing“; Doppelklick zusätzlich Fokus Tabelle
```

### W3 Konfiguration (Dialog)

```
┌─ Crawl-Konfiguration ─────────────────────────────────────────────────────────────────── [Profil: Standard ▾] ─┐
│ [🔍 Einstellung suchen…   ] │  Spider › Limits                                                                │
│ ▾ Spider                    │                                                                                 │
│     Crawl                   │  Max. URLs              [ 5.000.000 ]                                           │ CRAWL-066
│     Extraktion              │  Max. Crawl-Tiefe       [           ]  ☐ unbegrenzt                              │ CRAWL-067
│   ▸ Limits ◂                │  Max. Weiterleitungen   [ 10 ]                                                  │
│     Rendering               │  Max. Seitengröße (KB)  [ 51.200 ]   ⚠ Wert muss ≥ 1 sein                        │ Feldfehler (config.validate)
│     Erweitert               │                                                                                 │
│     Voreinstellungen        │  ── Abhängig ─────────────────────────────────────────────                      │
│   robots.txt                │  ⓘ Änderungen an Schwellwerten wirken ohne Neu-Crawl; betroffene                │ ONPAGE-002
│   URL-Umschreibung          │     Analyse-Teilschritte werden als veraltet markiert.                         │ UI-082
│   Einschließen/Ausschließen │                                                                                 │
│   Geschwindigkeit           │                                                                                 │
│   User-Agent · HTTP-Header  │                                                                                 │
│ ▸ Benutzerdefiniert         │                                                                                 │
│   API-Zugänge               │                                                                                 │
│   Authentifizierung         │                                                                                 │
│   Segmente                  │                                                                                 │
│   Crawl-Analyse             │                                                                                 │
├─────────────────────────────┴─────────────────────────────────────────────────────────────────────────────────┤
│ [Abweichungen anzeigen (3)]                         macOS: [Abbrechen] [OK]    Windows: [OK] [Abbrechen]       │ OUT-224, 5.6
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### W4 Crawl-Verwaltung

```
┌─ Crawls verwalten ───────────────────────────────────────────────────────────────────────────────────────────┐
│ [Öffnen] [Umbenennen] [Verschieben ▾] [Duplizieren] [Exportieren] [Importieren…] [🔒] [Löschen]  [Zum Vergleich]│ OUT-228
│ Anzeigen [Crawls ▾]                                                           [🔍 Name, Projekt, Start-URL… ] │
│ ┌───────────────┬──────────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ ▾ Alle (214)  │ Project │ Name          │ Mode  │ Start URL            │ Started    │ URLs   │ Size │Status │ │
│ │   Kunde A (12)│─────────┼───────────────┼───────┼──────────────────────┼────────────┼────────┼──────┼───────│ │
│ │ 🔒 Kunde B (3) │▓Kunde A▓│▓Relaunch neu▓▓│▓Spider│▓https://a.test/▓▓▓▓▓▓│▓13.10. 09:12│▓48.112▓│▓1,1 GB│Completed│ │
│ │   Ohne Projekt│▓Kunde A▓│▓Relaunch alt▓▓│▓Spider│▓https://a.test/▓▓▓▓▓▓│▓01.10. 14:03│▓47.900▓│▓1,0 GB│Completed│ │
│ │               │ –       │ Liste Q4      │ Liste │ –                    │ 12.10. 18:40│  1.200 │ 30 MB│Paused │ │
│ │               │ –       │ shop.test     │ Spider│ https://shop.test/   │ 11.10. 07:55│  9.004 │ 0,3 GB│Incompl.│ │
│ └───────────────┴──────────────────────────────────────────────────────────────────────────────────────────┘ │
│ 2 markiert · zusammen 2,1 GB                                                                [Schließen]         │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
 „Zum Vergleich“ nur bei genau 2 markierten aktiv; gesperrte Einträge: Löschen/Umbenennen ausgegraut
```

### W5 Vergleichsmodus (Hauptfenster)

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ Vergleich:  Current [Relaunch neu · 13.10. ▾]  ⇄  Previous [Relaunch alt · 01.10. ▾]   [URL-Mapping (2)] [Beenden]│ OUT-140/141/146
├───────────────────────────────────────────────────────────────────────────────────┬──────────────────────────────┤
│ Internal│…│Page Titles│…│Change Detection                                     » │ Overview (Vergleich)          │ TECH-300
│ [Filter: Duplicate ▾] [Vergleich: Removed (5) ▾] [⤓]   Angezeigt 5 von 122        │ Name      Cur Prev Chg + N − M│ OUT-142
│ ┌─────┬───────────────────────────────┬─────────────────────┬────────────────────┐ │ ▾ Page Titles                 │
│ │ Row │ Address                       │ Title 1 (Current)   │ Title 1 (Previous) │ │  Duplicate 122 130 −8 2 1 9 2 │
│ │   1 │ https://a.test/schuhe/        │ Schuhe kaufen       │ Shop                │ │  Missing    14  10 +4 3 2 1 0 │
│ │   … │                               │                     │                     │ │ ▾ Response Codes               │
│ └─────┴───────────────────────────────┴─────────────────────┴────────────────────┘ │  Client 4XX  3   4 −1 0 0 1 0 │
├═══════════════════════════════════════════════════════════════════════════════════┤  (+ Added, N New, − Removed,  │
│ Inlinks  [Vergleich: Alle | Hinzugefügt | Entfernt | Unverändert]                  │   M Missing; Zahl anklickbar)  │ OUT-149
│ …                                                                                  │                               │
├───────────────────────────────────────────────────────────────────────────────────┴──────────────────────────────┤
│ Vergleich · Current 48.112 URLs · Previous 47.900 URLs · MCP: aus                                                  │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### W6 MCP-Dialog

```
┌─ MCP ─────────────────────────────────────────────────────────────────────────────────────────┐
│ Server │ Werkzeuge │ Zugang │ Clients │ Protokoll                                              │ UI-129, AGENT-043
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ Werkzeugklassen                                                                                │ AGENT-003
│   ☑ read  (immer)    ☐ crawl  Crawls starten/stoppen    ☐ write  Exporte, Profile             │
│   ☐ cost  kostenpflichtige APIs – Ausführung erst nach Bestätigung                             │
│ ┌──────────────────┬───────┬─────┬────────────────────────────────────────────────────────┐   │
│ │ Werkzeug         │Klasse │ An  │ Beschreibung                                            │   │ OUT-285
│ │ list_crawls      │ read  │ ☑   │ Gespeicherte Crawls auflisten                           │   │
│ │ query_urls       │ read  │ ☑   │ Zeilen eines Tabs/Filters seitenweise                   │   │
│ │ start_crawl      │ crawl │ ☐ ⓘ │ Klasse „crawl“ nicht freigegeben                        │   │
│ └──────────────────┴───────┴─────┴────────────────────────────────────────────────────────┘   │
│ [Werkzeugbeschreibung als Markdown speichern…]                                                │
│ ── Reiter „Clients“ (Ausschnitt) ──────────────────────────────────────────────────────────── │
│ Claude Desktop   Konfiguration: ~/…/claude_desktop_config.json   [Vorschau & verbinden…]       │
│   Vorschau:  + "openspider": { "command": "/Applications/…/openspider", "args": ["mcp"] }      │ Diff + Sicherung
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ Status: MCP: verbunden (1 Client) · Port 51234 · ☐ Beim App-Start automatisch starten  [Stopp] │ OUT-285
└───────────────────────────────────────────────────────────────────────────────────────────────┘
```

Weitere Ansichten (Einstellungen, Exportdialog, SERP Snippet, View Source Diff, Visualisierungsfenster) werden im jeweiligen Paket als Wireframe nachgereicht; ihr Aufbau ist in 1.4/1.5 beschrieben.

---

## 7 Teststrategie UI

### 7.1 Komponenten- und Einheitentests (vitest, jsdom)

- **Werkzeuge:** vitest + Testing Library (vorhanden), `@tauri-apps/api/mocks` (`mockIPC`, `mockWindows`) für Commands/Ereignisse, `vitest-canvas-mock` (MIT) nur für Grid-Smoke-Tests.
- **Fixtures aus dem Kern:** Rust-Tests in `os-app` serialisieren DTO-Beispiele aus echten Fixture-Crawls (`fx-ui-basic`, `out-basic-site`) nach `apps/desktop/src/ipc/__fixtures__/*.json`; TS-Tests nutzen genau diese Dateien → Vertrag zwischen Kern und UI ist doppelt geprüft (Typen per ts-rs, Inhalte per Fixture).
- **Reine Logik ohne DOM:** `ViewDataSource` (Blockbildung, LRU 20 000, Versionsverwerfen, Anker-Korrektur, Tail-Modus), Auswahlmodell (UrlId-basiert, „alle außer“), Kürzel-Dispatcher (Kontextregeln UI-113), Layout-Mathematik der Trenner (UI-001 Prüfwerte bei 1 440 × 900), Persistenz-Migration `ui-state.json` (ungültig → `.bak` + Standard, Version n−1, UI-125), Zahlen-/Datumsformate de/en (UI-007), Kürzelanzeige je Plattform.
- **Statische Prüfungen (CI, Build schlägt fehl):** i18n-Vollständigkeit de/en + Platzhalter (UI-121, UI-065); Kürzelregister ↔ Hilfe-Dialog ↔ Menü, Kollision mit reservierten Systemkürzeln (UI-113); Voraussetzungsregister-Abdeckung (UI-016, Daten aus dem Kern); Token-Kontrasttest beider Themes (UI-120); kein `measureText` für Pixelspalten (UI-038); kein synchroner `#[tauri::command]` (2.3); npm-Lizenzprüfung gegen dieselbe Positivliste wie `deny.toml`.

### 7.2 Rust-Tests der Service-Schicht

`cargo test -p os-app` (ohne Tauri): Commands als normale async-Funktionen gegen Fixture-Crawls – View-Versionen, Offsets, Abbruch (Interrupt ≤ 100 ms), Coalescer (≤ 10 Ticks/s über 60 s bei 200 URL/s mit `ManualClock`, ≤ 1 Hz unsichtbar), Fehlerabbildung, Zwischenablage-TSV-Format (Zeilenenden je Plattform).

### 7.3 End-to-End

Problem: Tauris offizieller WebDriver-Weg (`tauri-driver`) unterstützt Windows (WebView2) und Linux, **nicht macOS** (für WKWebView gibt es keinen WebDriver-Treiber). 05 verlangt „UI-E2E-Tests per WebDriver gegen die Tauri-App“ – [Offen] F-7.

**[Entscheidung]** Drei Stufen:

| Stufe | Werkzeug | Plattformen | Inhalt |
|---|---|---|---|
| E2E-A „Frontend + echter Kern“ | **Playwright** (Apache-2.0) gegen das Vite-Frontend; IPC über eine **E2E-Bridge** (nur mit Cargo-Feature `e2e-bridge`, Loopback + Zufallstoken, ersetzt `invoke`/`Channel` durch WebSocket zu `os-app`) | macOS (Engine **WebKit** ≈ WKWebView), Windows (Chromium ≈ WebView2) | Großteil der Spec-E2E: Layout/Trenner, Tabs, Filter, Suche, Sortierung, Leerzustände (`data-empty-state`), Live-Aktualisierung, Kontextmenüs, Tastatursuite UI-113, axe-Läufe, Screenshots |
| E2E-B „echte App“ | **tauri-driver** (WebDriver) | Windows | native Menüs, Zwischenablage, Datei-Dialoge (über Testmodus mit vorgegebenem Pfad), Mehrfachfenster (Visualisierungen, Einstellungen), Benachrichtigungen, Fenstergröße/HiDPI 100/125/150/200 % |
| E2E-C „macOS nativ“ | Smoke über Bedienungshilfen-API (`osascript`/AX) + manueller VoiceOver-Plan | macOS | Menüreihenfolge, Kürzel im App-Menü, Fenster öffnen/schließen, Theme-Wechsel des Systems |

Testdaten: `os-testsite`-Fixtures aus 05 (`fx-ui-*`) und 06 (`out-*`); Uhr fixiert, Ports zufällig.

### 7.4 Performance-Benchmarks (PERF-004, UI-140 … UI-148)

| Messung | Ort | Verfahren | Budget |
|---|---|---|---|
| `view.rows` 200 × 20 p95, Filterwechsel, Sortieren num./Text 1/5 Mio., Suche, Regex, Richtungsumkehr | Kern (`criterion`) gegen `gen-ui-large` | ohne UI | UI-141, UI-142 |
| IPC-Rundreise und Nutzlast | E2E-A | Zeitstempel im Wrapper | ≤ 30 ms p95, ≤ 150 KB |
| Scrollen 10 s, Sprung ans Ende, horizontal 65 Spalten | E2E-A/B | In-Page-Frame-Recorder (`requestAnimationFrame`-Abstände, `PerformanceObserver` „long-animation-frame“ wo verfügbar); auf Windows zusätzlich CDP-Trace über `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port` | ≥ 55 fps, kein Frame > 50 ms; Platzhalter ≤ 150 ms |
| Tabwechsel bis erste Zeilen | E2E-A | Markierungen `performance.mark` | ≤ 200 ms (UI-141) bzw. < 300 ms (PERF-004) |
| Crawl-Last 200 URL/s | E2E-A + Fixture-Server | Klick→Reaktion p95, Ereigniszähler, CPU des UI-Prozesses | ≤ 100 ms, ≤ 10 Ereignisse/s, ≤ 15 % (UI-143) |
| Speicher | E2E-B (Win) / E2E-A (mac) | RSS der WebView-Prozesse (WebContent bzw. msedgewebview2) über 30-min-Soak mit Zufallsaktionen | ≤ 400 MB, kein Wachstum > 10 % (UI-144) |
| Kaltstart, Öffnen 5 Mio. | E2E-B + Kern-Instrumentierung | Zeitstempel Start → bedienbar | UI-147 |
| Visualisierung 10 000 Knoten | E2E-A | Frame-Recorder | UI-146 |

Ablauf (UI-148): nightly auf macOS arm64 und Windows x64, Bericht als CI-Artefakt (`target/loadtest/ui-report.json`, Format wie PERF-009), Historie, Fehlschlag bei Budgetüberschreitung oder > +10 % gegenüber gleitendem Median der letzten 7 Läufe. Kleinvarianten (100 000 Zeilen) in jeder CI.

### 7.5 Visuelle Regression

- Playwright `toHaveScreenshot` in E2E-A gegen eine **Harness-Seite** (`src/harness/`, nur im Dev-Build) mit deterministischen Fixture-Daten (mockIPC) – je Ansicht/Komponente × Theme (hell/dunkel/System) × Sprache (de/en) × Dichte.
- Baselines **je Plattform** (Systemschriften unterscheiden sich), Toleranz pixelbasiert klein (z. B. 0,1 %), Canvas-Grid eingeschlossen.
- Pflichtszenen aus der Spec: Hauptfenster, SERP Snippet, View Source Diff, eine Visualisierung (UI-120); Dialoge in Deutsch bei 1 024 × 640 (UI-121); Layout-Kollisionstest (Bounding-Box-Prüfung, kein Pixelvergleich) bei 1 024 × 640 und vier Skalierungen (UI-001).
- Baseline-Änderungen nur mit Begründung im Commit (wie Golden-Regel M1 2.6).

### 7.6 Barrierefreiheit

`@axe-core/playwright` (MPL-2.0, zulässig) auf allen Hauptansichten ohne critical/serious (UI-126); Tastatursuite deckt jedes Bedienelement per Tab ab; manueller Prüfplan VoiceOver (macOS) und NVDA/Narrator (Windows) je Meilenstein-Abnahme.

---

## 8 Paketschnitt M2-UI (Vorschlag)

### 8.1 Verzeichnisstruktur und Besitzgrenzen

```
apps/desktop/src/
  app/                 Shell, Fenster-Einstieg (main, settings, viz), Provider, Error-Boundaries     → U00, danach U01
  ipc/                 client.ts, errors.ts, events.ts (Event-Bus), <domain>.ts Wrapper, gen/ (ts-rs), __fixtures__/
  state/               Zustand-Stores: layout, tabs, columns, selection, session, prefs              → je Store ein Besitzer
  data/                ViewDataSource, Query-Hooks
  grid/                Glide-Adapter, Zellrenderer, Spaltenkopf, Zwischenablage-Anbindung
  ui/                  tokens/, primitives/, icons/, shortcuts.ts (Register UI-113), menu/ (Aufbau aus Beiträgen)
  i18n/                Loader, useT, Formatierer (Ressourcen liegen je Feature)
  features/<f>/        header, toptabs, table, detail/<tab>, sidebar/<tab>, statusbar, config, settings,
                       crawls, export, compare, viz, mcp, integrations, help  (+ je Feature i18n/, menu.ts)
  harness/             Test-Harness für visuelle Tests (nur Dev)
apps/desktop/src-tauri/src/
  lib.rs               Plugin-Registrierung (U00/K00, danach eingefroren)
  commands/<domain>.rs je Domäne ein internes Plugin → Besitzer = Paket der Domäne
  protocol.rs          URI-Schema ospider:// (Screenshots, Bilder)
crates/os-app/         Service-Schicht (falls F-1 genehmigt)
```

Regeln (analog M1 2/2a): Ein Paket ändert nur Dateien aus seinem `owns`. `package.json`, `package-lock.json`, `tauri.conf.json`, `capabilities/*`, `Cargo.toml`/`Cargo.lock`, `src-tauri/src/lib.rs`, `src/ipc/client.ts`, `src/ui/shortcuts.ts` und `src/app/` gehören **Welle 0** und sind danach eingefroren; Bedarf wird als „Schnittstellenbedarf“ gemeldet und zwischen den Wellen gebündelt. Übersetzungen, Menübeiträge und Command-Wrapper liegen je Feature/Domäne, damit parallele Pakete nicht dieselben Dateien berühren.

### 8.2 Wellen

Die Kernpakete (K-1 … K-19) werden im M2-Kernplan geschnitten; hier nur die Abhängigkeiten der UI-Pakete. Modellwahl wie M1: Welle 0 Opus, übrige Sonnet.

**Welle 0 – Fundament (sequenziell, Opus)**
- **M2-U00 Fundament Frontend:** Abhängigkeiten (Glide gepinnt, React Aria, Zustand, TanStack Query, zod, Fluent, Lucide, CodeMirror, Playwright, axe), Ordnerstruktur, Tokens + Kontrasttest, i18n-Infrastruktur, IPC-Client + Fehlerumsetzung + Event-Bus, Store-Gerüste, Kürzelregister (alle Zeilen aus UI-113 mit Aktions-IDs) + Dispatcher, Menü-Aufbau aus Beiträgen, Harness, CI-Jobs (typecheck, vitest, Lizenzprüfung npm, statische Checks 7.1), CSP setzen (`tauri.conf.json`, externe Quellen verboten, UI-065).
- **M2-K00 Verträge Kern↔UI:** `os-app`-Gerüst, alle DTOs aus 4.3/4.4 mit ts-rs, Command-Stubs je Domäne (liefern `Unsupported`), Plugin-Registrierung, E2E-Bridge-Gerüst, `commands.json`-Manifest + Vertragstest.
- **Spikes (Teil von U00/K00, Ergebnis vor Welle 1):** S-1 Glide 5 Mio. × 65 auf WKWebView/WebView2 (Scroll, fps, A11y-Baum, React 19); S-2 Zoom/DPR; S-3 Windows-Menü-Accelerators vs. Eingabefelder, macOS-Bearbeiten-Menü; S-4 ts-rs vs. tauri-specta; S-5 Playwright-WebKit mit Bridge auf macOS.

**Welle 1 – Gerüst und Querschnitt (parallel)**
| Paket | Inhalt | Spec | owns (Auszug) | braucht |
|---|---|---|---|---|
| U01 Shell & Layout | SplitPanes, Ein-/Ausblenden, Mindestmaße, Persistenz `ui-state.json`, Fenstergeometrie | UI-001, UI-125, UI-021 (Zustandsteil) | `features/shell/**`, `state/layout.ts` | K00 |
| U02 Grid & Datenquelle | Glide-Adapter, Zellarten, Kopf (Sortierpfeil/Rang), Platzhalter, Auswahlmodell, Kopieren-Anbindung, Spaltenbreiten | UI-007, UI-012, UI-140 (Client), UI-142 (Client) | `grid/**`, `data/viewDataSource.ts`, `state/selection.ts` | K00, K-1 (Stub reicht mit Fixture-Daten) |
| U03 Kopfleiste & Statusleiste | URL-Feld (Vorschläge, Validierung), Modus/Bereich, Zustandsautomat Start/Pause, Fortschritt, Statusleistenfelder, Startfehler-Dialoge, Benachrichtigungen | UI-003, UI-013, UI-019 | `features/header/**`, `features/statusbar/**`, `state/session.ts` | K-2, K-3 |
| U04 Menü & Kürzel | alle Menüs UI-002 (Aktivierungslogik), Hilfe → Tastaturkürzel, Über, Debug-Info, Logs | UI-002, UI-113, UI-127 | `features/help/**`, `ui/menu/**` | U00 |
| U05 Design-System-Komponenten | Primitive aus 5.4, EmptyState-Fläche, Dialog-Fußleiste je Plattform | UI-016 (Darstellung), UI-126 (Basis) | `ui/primitives/**` | U00 |

**Welle 2 – Hauptansichten (parallel)**
| Paket | Inhalt | Spec |
|---|---|---|
| U10 Top-Tabs & Tabelle | Tab-Leiste mit Überlauf, Filter-Dropdown, einfache Suche, Sortierung, „+“-Spaltenmenü, Zähler, Export-Schaltfläche, Live-Aktualisierung (Anker, Tail), Kontextmenü Zeilen (P0-Gruppen), Erneut crawlen/Entfernen | UI-004 (P0), UI-006, UI-008 (P0), UI-010, UI-011 (P0), UI-014, UI-015, UI-018, UI-110 (P0), UI-114 |
| U11 Detailfenster Basis | Rahmen, Lazy Loading, Entprellung, URL Details, Inlinks, Outlinks (inkl. Link Crawlability), Resources, Image Details, HTTP Headers, Cookies, Crawl-Pfad, Detail-Kontextmenü | UI-030 … 036, UI-042, UI-043, UI-050, UI-111 |
| U12 Seitenleiste Basis | Overview, Issues, Erklärungen, Navigation in die Tabelle | UI-060 … 065 |
| U13 Konfigurationsdialog | Seiten aus 1.5, Validierung über Kern, Regex-Tester, Profile, Sprungziele für Leerzustände | 01 C–O (UI-Teile), OUT-220, UI-016-Aktionen |
| U14 Einstellungen | Fenster, Theme/Akzent/Zoom/Dichte, Sprache, Speicher, Aufbewahrung, Exportordner, externe Werkzeuge, Benachrichtigungen | UI-120 … 124, OUT-222, OUT-229, UI-117 |
| U15 Crawl-Verwaltung | Dialog, Projekte, Aktionen, Import/Export, Öffnen/Speichern unter, Zuletzt geöffnet | OUT-228, CRAWL-127 (UI-Teile) |
| U16 Exporte | Optionsdialog + nativer Dialog, Export der Ansicht/Details/Seitenleiste, Bulk-/Berichte-/Sitemap-Menüs, Fortschritt/Abbruch, „Ordner öffnen“ | OUT-006, OUT-007, OUT-020, OUT-021, UI-015 |

**Welle 3 – Fachansichten und Modi (parallel)**
| Paket | Inhalt | Spec |
|---|---|---|
| U20 Erweiterte Tabellenbedienung | Regelbaum + Textsyntax + Verlauf, Spalten-Konfigurator + Vorlagen, Tabs konfigurieren (Drag, Ausblenden), Tab-Kontextmenüs, Mehrfachsortierung, Segmentfilter/-spalte, „Spalten aller Tabellen zurücksetzen“ | UI-004 [P1], UI-008 [P1], UI-009, UI-011 [P1], UI-020, UI-021, UI-110 [P1], UI-112 |
| U21 Fach-Detailtabs | SERP Snippet, View Source + Diff, Rendered Page, Console Log, Structured Data, Duplicate Details, Spelling & Grammar, Accessibility Details (+ P2: Lighthouse, N-Grams nach Kapazität) | UI-037, UI-039 … 041, UI-044 … 048 |
| U22 Seitenleiste Fach | Site Structure, Segments + Segment-Dialog, Response Times, API-Tab, Analyse-Status/Veraltet-Hinweise | UI-066 … 070, UI-068, UI-080, UI-082 |
| U23 Vergleichsmodus | Auswahl, Tausch, URL-Mapping mit Test, Vergleichsspalten/-filter in Overview/Issues/Tabellen, Change Detection, Inlinks/Outlinks-Vergleich | OUT-140 … 153, TECH-300 ff. |
| U24 Visualisierungen | Fensterrahmen, Bäume, kraftgerichtete Graphen, 3D, Wortwolken, Exporte | UI-090 … 100 |
| U25 MCP | Menü, Dialog (5 Reiter), Statusleistenfeld, Client-Verbindungen | UI-129, OUT-285, AGENT-002/-003/-043 |
| U26 Integrationen-UI | Verbindungsdialoge GA4/GSC/PSI/Majestic/Ahrefs/Moz, Custom Search/Extraction/JS-Konfiguration, API-Zahnräder | 04 A–J (UI-Teile), UI-070 |

**Welle 4 – Abnahmereife**
| Paket | Inhalt | Spec |
|---|---|---|
| U30 Barrierefreiheit | axe-Läufe, Tastatur-Vollabdeckung, Screenreader-Prüfplan, Kontrastmodus | UI-126 |
| U31 Performance | Benchmarks 7.4, Soak-Test, nightly-Job, Budgets | PERF-004, UI-140 … 148 |
| U32 E2E- und Visual-Suite | Tastatursuite je Plattform, Baselines, Layout-Kollision, Menü-Snapshots | UI-001, UI-002, UI-113, UI-120, UI-121 |
| U33 Sprache | Vollständigkeit de/en, sprachliche Durchsicht, Kürzung bei 1 024 × 640 | UI-121 |

Kritischer Pfad: K00 → K-1 (Views) → U02 → U10. K-1 ist das größte Kernrisiko und sollte in Welle 0/1 des Kernplans beginnen; U02 kann bis dahin gegen Fixture-Daten (mockIPC) entwickeln.

---

## 9 Risiken

| Risiko | Auswirkung | Gegenmaßnahme |
|---|---|---|
| Glide nur als Alpha mit React 19/hybridem Scrollen; langsame Wartung | Grid-Fehler, Upgrade-Stau | Exakt pinnen, Adapter mit schmaler Schnittstelle, Spike S-1 mit Abbruchkriterien, Fallback A (Rebasing) und B (eigenes Canvas-Grid) |
| K-1 (View-Materialisierung) unterschätzt | PERF-004/UI-141 verfehlt | früh starten, Kern-Benchmarks ohne UI zuerst, Speicherbudget festlegen |
| Kein WebDriver auf macOS | Spec-Satz nicht wörtlich erfüllbar | Stufenmodell 7.3, Klärung F-7 |
| Native Menüs fangen Edit-Kürzel ab (Windows) bzw. fehlendes Bearbeiten-Menü (macOS) | Kopieren/Einfügen in Feldern kaputt | Spike S-3, F-5 |
| WebView-Unterschiede (WebKit vs. Chromium) bei Canvas, Fokus, `color-mix`, Zoom | Plattformfehler | E2E-A auf beiden Engines, Spike S-2 |
| Speicherbudget WebView 400 MB bei 5 Mio. Zeilen | UI-144 verfehlt | LRU 20 000 Zeilen, keine Volllisten im JS, Auswahl als „alle außer“ |
| Parallele Pakete kollidieren auf Querschnittsdateien | Merge-Konflikte | Besitzregeln 8.1, Ressourcen/Menüs/Wrapper je Feature, Welle-0-Einfrieren |

---

## 10 Offene Fragen

| # | Frage | Vorschlag |
|---|---|---|
| F-1 | Neues Crate `os-app` (Tauri-freie Service-Schicht) – Zustimmung Orchestrator (CLAUDE.md: neue Crates nur nach Absprache) | ja; sonst Modul in `src-tauri` |
| F-2 | Glide-Alpha-Version als Abhängigkeit akzeptabel? | ja, gepinnt, mit Spike-Gate |
| F-3 | **Widerspruch Sortier-Gleichstand:** ONPAGE-001 „Address aufsteigend“ vs. UI-010 „Ursprungsreihenfolge“ | Fachspec (02) führt bei Inhaltsfragen; Export/MCP-Gleichheit spricht für eine einzige Regel → Entscheidung Fable |
| F-4 | **Issue-Texte:** UI-065 verlangt drei Texte (what/why/fix) als i18n-Ressourcen `issues.<id>.*`; M1 plant `IssueTexts { description, how_to_fix }`; 06 nennt Pflege in `core/issues/catalog.toml` und Exportspalten `Description`/`How To Fix`, UI-065 zusätzlich `Why It Matters` | eine Quelle im Kern mit drei Texten; Exportspalten nach UI-065 vereinheitlichen |
| F-5 | macOS braucht ein „Bearbeiten“-Menü (und konventionell „Fenster“) – Abweichung von der exakten Reihenfolge in UI-002 | `[Abweichung]` nur macOS: „Bearbeiten“ nach „Datei“, „Fenster“ vor „Hilfe“ |
| F-6 | UI-142 „komprimiert ab 64 KB“ bei In-Prozess-IPC sinnlos | Kriterium durch Nutzlastgrenze ersetzen |
| F-7 | 05 „UI-E2E per WebDriver“ – auf macOS nicht verfügbar | Stufenmodell 7.3 in 05 aufnehmen |
| F-8 | Änderungsfeed als Schema-Migration (`change_seq`) statt Engine-Hook – Zustimmung für Migration `0002` | ja (deckt auch Crawls fremder Prozesse ab) |
| F-9 | Visualisierung mit sigma.js/graphology statt Cytoscape (ARCHITECTURE nennt D3/Cytoscape) | sigma.js wegen WebGL/10 000 Knoten; ARCHITECTURE anpassen |
| F-10 | Sprachbewusste Sortierung im Kern (ICU4X-Kollator) muss auch für Exporte und MCP gelten – betrifft M1-Exporte (Bytevergleich?) | Kollation einheitlich im Kern, M1-Goldens prüfen |
| F-11 | Verhalten beim Schließen des Hauptfensters (macOS) mit/ohne laufenden Crawl | Rückfrage bei laufendem Crawl, danach App beenden |
| F-12 | Standard-Zeilenhöhe (UI-120 nennt keinen Standard) | kompakt (22 px) |
| F-13 | Sollen Konfigurationsänderungen während eines laufenden Crawls erlaubt sein (welche Felder)? | nur Schwellwerte/Voreinstellungen und Threads; Rest erst nach Pause – in 01 klären |
| F-14 | Darf die App einen per MCP-STDIO (fremder Prozess) laufenden Crawl nur lesend live anzeigen, oder auch pausieren? | nur lesend + Hinweis; Steuerung nur über den startenden Prozess |
