# Beta-Abnahme M2-β: Befundliste (M2-A02)

Stand 28.10.2026, 11:30. Build: `main` 572e14db, Universal-DMG aus Lauf 40624897076 (siehe `build.md`). Rückmeldung des Nutzers
(Sprachnotiz und vier Screenshots), übertragen vom Orchestrator; keine Fixes in diesem Paket (K2-75).

**Plattform:** macOS (Apple Silicon), Fenster ≈ 1 546 × 1 030 und ≈ 2 000 × 1 060, Sprache Deutsch (Gegenprobe Englisch), Theme dunkel
(Gegenprobe hell). Crawl einer eigenen Site des Nutzers (≈ 3 300 URLs, robots.txt mit `Crawl-delay: 1`), Modus Spider, Bereich „Diese
Subdomain“, 5 Threads, Auto-Tempo an.

**Schwere** (wie alpha): Blocker = Hauptpfad unbenutzbar; Hoch = Kriterium oder Hauptfunktion scheitert; Mittel = Funktion verfälscht
oder unzuverlässig, Umgehung möglich; Niedrig = Darstellung, Hinweis, Komfort.

**Screenshots:** Die Bilder des Nutzers zeigen Inhalte seiner eigenen Site und liegen deshalb nicht im öffentlichen Repo; sie sind
unten beschrieben und beim Orchestrator lokal abgelegt.

## Übersicht

| ID | Titel | Schwere | Ort | Screenshot | Spec |
|---|---|---|---|---|---|
| A02-01 | Haupt-Tab-Leiste: zweizeilige Beschriftungen unten abgeschnitten | Mittel | Haupt-Tab-Leiste | Hauptfenster (beide) | UI-004 |
| A02-02 | Detail-Tab-Leiste: „Chrome Console“ oben abgeschnitten, zweizeilige Labels | Mittel | Detail-Tab-Leiste | Hauptfenster 1 546 px | UI-030 |
| A02-03 | Seitenleisten-Tabs ohne Überlaufsteuerung | Mittel | Seitenleiste | Hauptfenster 1 546 px | UI-060 |
| A02-04 | Sprachmischung bei Deutsch („All (214)“, englische Tab- und Spaltennamen neben deutschen) | Mittel | ganze Oberfläche | Hauptfenster (beide) | UI-121 |
| A02-05 | Leere linke Spalte mit zweitem „Exportieren“-Knopf | Mittel | Hauptfenster links | Hauptfenster (beide) | UI-001, UI-015 |
| A02-06 | Uneinheitliche Bedienelemente der Tab-Konfiguration | Niedrig | Seitenleiste, Detailfenster | Hauptfenster (beide) | UI-004, UI-060 |
| A02-07 | Übersicht flackert während des Crawls zwischen „wird geladen“ und Inhalt | Hoch | Seitenleiste → Übersicht | – | UI-018, UI-061 |
| A02-08 | Übersicht bleibt nach Pause → Fortsetzen auf „wird geladen“ stehen | Hoch | Seitenleiste → Übersicht | – | UI-018, UI-061, CRAWL-126 |
| A02-09 | Detail-Tabs zeigen während des Crawls nur „…“ statt Werten | Hoch | Detailfenster (Inlinks, Outlinks, Image Details, Ressourcen) | Hauptfenster 2 000 px | UI-030, UI-032, UI-033, UI-035, UI-036, UI-145 |
| A02-10 | Tabellen reagieren während des Crawls träge (Tab External lädt lange) | Mittel | obere Tabelle | – | UI-143 |
| A02-11 | Pause: Knopf wechselt verzögert auf „Fortsetzen“, Crawl kommt spät zur Ruhe | Mittel | Kopfleiste | – | UI-003, CRAWL-126 |
| A02-12 | Begrenzung der Crawl-Rate (Crawl-delay) ist nicht sichtbar – 1,1 URL/s wirkt wie ein Fehler | Mittel | Statusleiste, Konfiguration → Geschwindigkeit | Hauptfenster 1 546 px | CRAWL-145, UI-013 |
| A02-13 | Sprach- und Theme-Wechsel wirken erst nach Neustart | Hoch | Einstellungen | – | UI-120, UI-121 |
| A02-14 | Menü „OpenSpider beenden“ ohne Wirkung | Hoch | App-Menü | – | UI-001, UI-002 |
| A02-15 | Pausierter Crawl nach Neustart nicht wieder auffindbar | Hoch | Datei → Zuletzt geöffnet, Startzustand | – | CRAWL-126, UI-124 |
| A02-16 | MCP-Dialog, Werkzeuge: Spaltenköpfe brechen buchstabenweise um, Liste leer | Mittel | MCP-Dialog → Werkzeuge | MCP-Dialog | UI-129 |
| A02-17 | Mehrfach-Export (Bulk) unverständlich, Export scheint nicht zu entstehen | Mittel | Menü Bulk-Export / Mehrfach-Export | – | OUT-066, UI-002 |
| A02-18 | „Crawl-Analyse starten“ ohne sichtbare Rückmeldung | Mittel | Menü Crawl-Analyse | – | UI-080, UI-082 |
| A02-19 | Segmente: JSON-Feld klein und unglücklich neben der Beschriftung | Niedrig | Konfiguration → Segmente | Konfiguration | UI-068 |
| A02-20 | Leerzustände der Seitenleiste ohne Innenabstand („Crawl öffnen“ klebt am Rand) | Niedrig | Seitenleiste | – | UI-016, UI-060 |
| A02-21 | JavaScript-Rendering nicht wählbar, ohne Erklärung | Niedrig | Konfiguration → Spider → Rendering | – | UI-016 (keine eigene Spec-ID – Rückfrage an den Planer) |
| A02-22 | Wunsch: Update-Prüfung beim Start und Update-Download | – (Wunsch, bleibt M3) | App | – | OUT-250, OUT-251 (Plan: M3) |

## Befunde

### A02-01 Haupt-Tab-Leiste: zweizeilige Beschriftungen unten abgeschnitten (Mittel)
- **Ort:** Haupt-Tab-Leiste über der oberen Tabelle.
- **Reproduktion:** App starten, Crawl laden, Fenster ≈ 1 546 px oder 2 000 px breit, Deutsch.
- **Beobachtung:** „Response Codes“, „Page Titles“, „Meta Description“, „Meta Keywords“, „Structured Data“, „Custom Search“ umbrechen
  zweizeilig, die zweite Zeile ist unten angeschnitten (Nutzerpunkt 1 vom 27.10. besteht weiter).
- **Erwartung:** Einzeilige Beschriftung mit Kürzung und Tooltip oder eine Leistenhöhe, die beide Zeilen vollständig zeigt; Überlauf
  über das »-Menü.
- **Screenshot:** Hauptfenster 1 546 px und 2 000 px (Beschreibung oben).
- **Spec:** UI-004.

### A02-02 Detail-Tab-Leiste: „Chrome Console“ oben abgeschnitten, zweizeilige Labels (Mittel)
- **Ort:** Detail-Tab-Leiste unter der oberen Tabelle.
- **Reproduktion:** Fenster ≈ 1 546 px, Zeile wählen.
- **Beobachtung:** „Chrome Console“ ist oben abgeschnitten (nur „Console“ lesbar); URL Details, Image Details, SERP Snippet, Gerenderte
  Seite, HTTP Headers, Strukturierte Daten, Lighthouse Details, Rechtschreibung und Grammatik, N-Gramme umbrechen zweizeilig
  (Nutzerpunkt 2 besteht weiter).
- **Erwartung:** wie A02-01.
- **Spec:** UI-030.

### A02-03 Seitenleisten-Tabs ohne Überlaufsteuerung (Mittel)
- **Ort:** Tab-Leiste der rechten Seitenleiste.
- **Reproduktion:** Fenster ≈ 1 546 px.
- **Beobachtung:** Nach „Segmente“ enden die Tabs ohne Hinweis; Antwortzeiten, API, Rechtschreibung & Grammatik sind nicht erreichbar,
  kein »-Menü (Nutzerpunkt 3 besteht weiter).
- **Erwartung:** Überlaufmenü wie in der Haupt-Tab-Leiste.
- **Spec:** UI-060.

### A02-04 Sprachmischung bei Deutsch (Mittel)
- **Ort:** ganze Oberfläche.
- **Beobachtung:** Filter „All (214)“, Spalten „Address/Content/Status Code/Indexability“, Haupt-Tabs englisch, Detail-Tabs teils deutsch
  („Gerenderte Seite“, „Strukturierte Daten“), teils englisch („Image Details“, „HTTP Headers“); Seitenleiste „Internal/External“
  neben „Zusammenfassung“ (Nutzerpunkt 4 besteht weiter).
- **Erwartung:** Einheitliche Regel nach UI-121/U33 (welche Fachbegriffe bewusst englisch bleiben, festlegen und konsequent anwenden).
- **Spec:** UI-121.

### A02-05 Leere linke Spalte mit zweitem „Exportieren“-Knopf (Mittel)
- **Ort:** Hauptfenster, linker Rand neben der Tabelle.
- **Beobachtung:** Schmale leere Spalte (≈ 75–95 px), darin ein einzelner „Exportieren“-Knopf auf halber Höhe; ein zweiter
  „Exportieren“ steht in der Werkzeugleiste über der Tabelle (Nutzerpunkt 5 besteht weiter).
- **Erwartung:** Ein Export-Einstieg an einem festen Ort, keine verlorene Fläche.
- **Spec:** UI-001, UI-015.

### A02-06 Uneinheitliche Bedienelemente der Tab-Konfiguration (Niedrig)
- **Ort:** Seitenleiste („▸ Tabs anpassen“ als Aufklapptext) gegenüber Detailfenster („Detail-Tabs konfigurieren“, „Tabs zurücksetzen“ als
  Knöpfe) und Haupt-Tab-Leiste (Symbol rechts).
- **Erwartung:** Ein Muster für alle drei Leisten (Nutzerpunkt 6).
- **Spec:** UI-004, UI-060.

### A02-07 Übersicht flackert während des Crawls (Hoch)
- **Ort:** Seitenleiste → Übersicht.
- **Reproduktion:** Crawl starten, Übersicht geöffnet lassen.
- **Beobachtung:** Die Tabelle wechselt fortlaufend zwischen „wird geladen“ und Inhalt („es blinkt, man wird wahnsinnig“).
- **Erwartung:** Gebündelte Aktualisierung ohne Leerzustand dazwischen; alte Werte bleiben stehen, bis neue da sind.
- **Spec:** UI-018 (Aktualisierungen gebündelt, Anzeige stabil), UI-061.

### A02-08 Übersicht bleibt nach Pause → Fortsetzen auf „wird geladen“ stehen (Hoch)
- **Ort:** Seitenleiste → Übersicht.
- **Reproduktion:** Crawl starten, pausieren, fortsetzen.
- **Beobachtung:** Statusleiste zählt weiter („Erledigt …“), die Übersicht zeigt dauerhaft „wird geladen“ und aktualisiert sich nicht mehr.
  Nach erneuter Pause erscheinen die Werte nach kurzer Wartezeit wieder.
- **Erwartung:** Live-Aktualisierung läuft nach dem Fortsetzen weiter.
- **Spec:** UI-018 (Pause/Fortsetzen-Variante), UI-061, CRAWL-126.

### A02-09 Detail-Tabs zeigen während des Crawls nur „…“ (Hoch)
- **Ort:** Detailfenster: Inlinks, Outlinks, Image Details, Ressourcen.
- **Reproduktion:** Crawl läuft, Zeile in Internal (oder Security) wählen, Detail-Tab Inlinks öffnen.
- **Beobachtung:** Spaltenköpfe und Zeilenanzahl erscheinen, alle Zellen zeigen dauerhaft „…“; erst nach Pause/Stopp werden die Werte
  geladen. SERP Snippet funktioniert während des Crawls.
- **Erwartung:** Detailinhalte auch während des Crawls (Ladezeitgrenzen UI-145).
- **Screenshot:** Hauptfenster 2 000 px, Detail-Tab Inlinks mit „…“-Zellen.
- **Spec:** UI-030, UI-032, UI-033, UI-035, UI-036, UI-145.

### A02-10 Tabellen reagieren während des Crawls träge (Mittel)
- **Ort:** obere Tabelle, Tab External.
- **Beobachtung:** Wechsel auf External dauert während des Crawls deutlich, nach dem Stopp sofort. Die Crawl-Rate lag nur bei ≈ 1 URL/s.
- **Erwartung:** Tabellenwechsel innerhalb der Grenzen aus UI-143 auch unter Crawl-Last.
- **Spec:** UI-143.

### A02-11 Pause: verzögerte Rückmeldung (Mittel)
- **Ort:** Kopfleiste, Knopf Pause.
- **Beobachtung:** Nach Klick auf Pause dauert es einen Augenblick, bis „Fortsetzen“ erscheint; Risiko von Mehrfachklicks. Der Crawl
  kommt erst spät zur Ruhe. Der Nutzer vermisst ein „Stoppen“ (Kopfleiste bietet Start/Pause/Leeren).
- **Erwartung:** Sofortiger Zustandswechsel des Knopfes („Pausiere …“, deaktiviert) und Pause ≤ 3 s (CRAWL-126). Ob ein eigener
  Stopp-Knopf fehlt, ist nach UI-003 zu bewerten (Start/Pause/Löschen) – Rückfrage an den Planer.
- **Spec:** UI-003, CRAWL-126.

### A02-12 Begrenzung der Crawl-Rate ist nicht sichtbar (Mittel)
- **Ort:** Statusleiste, Konfiguration → Geschwindigkeit.
- **Beobachtung:** 1,1 URL/s bei 5 Threads, nur 2 aktiv; der Nutzer hält das für einen Fehler. Ursache laut Orchestrator: Die robots.txt
  der Site setzt `Crawl-delay: 1`, die OS-Voreinstellung beachtet ihn (CRAWL-145) – das Verhalten ist spec-konform, aber nirgends erklärt.
- **Erwartung:** Statusleiste bzw. Geschwindigkeitsseite zeigen die wirksame Begrenzung („Crawl-delay 1 s (robots.txt) – angewandt“) mit
  Hinweis auf die Option; der robots.txt-Hinweis nach CRAWL-145 ist in der Oberfläche auffindbar.
- **Spec:** CRAWL-145, UI-013.

### A02-13 Sprach- und Theme-Wechsel wirken erst nach Neustart (Hoch)
- **Ort:** Einstellungen → Sprache / Erscheinungsbild.
- **Reproduktion:** Sprache auf Englisch: Kopfleiste („Diese Subdomain“, „Fortsetzen“, „Leeren“) und Detail-Tabs bleiben deutsch. Theme
  auf hell: nur der Einstellungsdialog wird hell, das Hauptfenster erst nach Neustart.
- **Erwartung:** Wechsel ohne Neustart in ≤ 200 ms auf alle offenen Fenster (UI-120), Sprachwechsel ohne Neustart (UI-121).
- **Spec:** UI-120, UI-121.

### A02-14 Menü „OpenSpider beenden“ ohne Wirkung (Hoch)
- **Ort:** macOS-App-Menü → Beenden.
- **Beobachtung:** Klick bewirkt nichts; nur das Schließen des Fensters (X) beendet die App.
- **Erwartung:** Beenden folgt der Schließen-Festlegung aus UI-001 (Rückfrage bei laufendem Crawl, sonst Beenden).
- **Spec:** UI-001, UI-002.

### A02-15 Pausierter Crawl nach Neustart nicht wieder auffindbar (Hoch)
- **Ort:** Datei → Zuletzt geöffnet, Crawl-Verwaltung, Startzustand der Seitenleiste.
- **Reproduktion:** Crawl pausieren, App beenden, neu starten.
- **Beobachtung:** Der Crawl erscheint nicht unter „Zuletzt geöffnet“ und ist nicht aufrufbar.
- **Erwartung:** Fortsetzen nach App-Neustart (CRAWL-126); der Crawl steht in „Zuletzt geöffnet“ und in der Crawl-Verwaltung.
- **Spec:** CRAWL-126, UI-124.

### A02-16 MCP-Dialog, Werkzeuge: Spaltenköpfe brechen buchstabenweise um (Mittel)
- **Ort:** MCP-Dialog → Werkzeuge.
- **Beobachtung:** Die Spalten „Freigegeben“, „Werkzeug“, „Klasse“, „Parameter (JSON-Schema)“ sind so schmal, dass die Köpfe Buchstabe
  für Buchstabe umbrechen; die Werkzeugliste ist leer, obwohl die Klasse „Lesen“ freigegeben ist.
- **Erwartung:** Lesbare Spaltenbreiten; Werkzeuge der freigegebenen Klassen werden gelistet (oder ein Leerzustand erklärt, warum nicht).
- **Screenshot:** MCP-Dialog, Reiter Werkzeuge.
- **Spec:** UI-129.

### A02-17 Mehrfach-Export (Bulk) unverständlich (Mittel)
- **Ort:** Menü Bulk-Export / Mehrfach-Export, Exportoptionen.
- **Beobachtung:** Der Nutzer weiß nicht, was einzutragen ist („Vorlage?“); nach Speichern entsteht scheinbar kein Export. Einzelexport
  (Excel) der Ansicht funktioniert und sieht gut aus.
- **Erwartung:** Geführter Ablauf (Auswahl → Zielordner → Export), klare Rückmeldung über erzeugte Dateien. Genaue Reproduktion beim
  nächsten Durchgang erfragen (Rückfrage an den Nutzer).
- **Spec:** OUT-066, UI-002.

### A02-18 „Crawl-Analyse starten“ ohne sichtbare Rückmeldung (Mittel)
- **Ort:** Menü Crawl-Analyse.
- **Beobachtung:** Nach Klick passiert sichtbar nichts.
- **Erwartung:** Fortschritt bzw. Analyse-Status sichtbar (UI-082), danach gefüllte analyseabhängige Filter.
- **Spec:** UI-080, UI-082.

### A02-19 Segmente: JSON-Feld klein und unglücklich platziert (Niedrig)
- **Ort:** Konfiguration → Segmente.
- **Beobachtung:** Das Textfeld „Segmentliste als JSON“ ist ein kleines Kästchen rechts oberhalb seiner Beschriftung, darunter volle
  Breite „JSON anzeigen“/„Übernehmen“, „Datei importieren“ als Standard-Dateiauswahl in Englisch („Choose File“, „no file selected“).
- **Erwartung:** Volle Breite, Beschriftung über dem Feld, lokalisierte Dateiauswahl.
- **Screenshot:** Konfigurationsdialog, Seite Segmente.
- **Spec:** UI-068.

### A02-20 Leerzustände der Seitenleiste ohne Innenabstand (Niedrig)
- **Ort:** Seitenleiste → Übersicht/Befunde/Seitenstruktur ohne geladenen Crawl.
- **Beobachtung:** „Crawl öffnen“ klebt am Rand.
- **Erwartung:** Innenabstand wie im Rest der Seitenleiste.
- **Spec:** UI-016, UI-060.

### A02-21 JavaScript-Rendering nicht wählbar, ohne Erklärung (Niedrig)
- **Ort:** Konfiguration → Spider → Rendering.
- **Beobachtung:** JavaScript-Rendering lässt sich nicht auswählen; der Nutzer vermutet, dass erst ein Chromium-Pfad nötig ist.
- **Erwartung:** Hinweis im Dialog, warum die Option gesperrt ist und was zu tun ist (Chromium-Pfad; Bündelung ist M3, K2-09).
- **Spec:** UI-016; keine eigene Spec-ID – Rückfrage an den Planer.

### A02-22 Wunsch: Update-Prüfung beim Start (Wunsch)
- **Ort:** App.
- **Beobachtung/Wunsch:** Beim Start prüfen, ob eine neuere Version vorliegt, und das Update herunterladen bzw. installieren.
- **Einordnung:** Spezifiziert als OUT-250 (Update-Prüfung) und OUT-251 (Herunterladen und Installieren), geplant für M3.
  **Nutzerentscheidung 28.10.2026:** bleibt in M3, kein Vorziehen.

## Geprüft ohne Befund
- Export der Ansicht als Excel: Datei vollständig und gut lesbar.
- SERP Snippet während und nach dem Crawl.
- Konfigurationsdialog: Extraktion (JSON-LD), Limits, Bereich, robots.txt, URL-Umschreibung, Einschließen/Ausschließen, User-Agent,
  HTTP-Header, API-Zugänge – Werte bleiben gespeichert.
- Crawl-Baum öffnet in einem eigenen Fenster – spec-konform (UI-090: alle Visualisierungen in eigenem, nicht-modalem Fenster).
- MCP-Dialog grundsätzlich (Server, Zugang, Clients, Protokoll) ohne Auffälligkeit außer A02-16.

## Nicht geprüft
Gerenderte Seite, Chrome Console Log, Cookies, Quelltext (Speicherung des HTML in der Konfiguration nicht aktiviert), Vergleichsmodus,
Fenster 1 024 × 640, Crawl-Verwaltung im Detail.
