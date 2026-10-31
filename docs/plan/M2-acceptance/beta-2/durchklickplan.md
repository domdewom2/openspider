# Zweite Nutzer-Runde M2-A03: Durchklickplan

Stand 31.10.2026. App aus `main` 322a2328 (Build siehe `build.md`). Dauer ≤ 30 min. Keine Fixes in dieser Runde (K2-75).

**So den Plan benutzen:** Durchgeklickt wird in der Reihenfolge von Abschnitt 3 („Ablauf in 30 Minuten“). Die Tabellen in
Abschnitt 1 und 2 sind das Nachschlagewerk dazu: Jeder Schritt im Ablauf nennt die IDs, die er prüft. Auffälligkeiten kurz notieren
(wo, was, was erwartet, Fenstergröße, Sprache, Theme, ggf. Screenshot) – der Orchestrator überträgt sie nach `befunde.md` (A03-nn).

**Ausgangslage:** Sprache Deutsch, Theme dunkel, Fenster normal (≈ 1 500 × 1 000), dazu einmal 1 024 × 640.

## 1. Gegenprobe A02

Status nach Berichten und Git-Verlauf (`docs/plan/M2-reports/*.md`, Merges in `main`):
**behoben** = laut Paketbericht umgesetzt und mit Test belegt, in `main` gemergt; **offen** = bewusst nicht umgesetzt;
**unklar** = kein Beleg. „Zwischentest bestätigt“ = vom Nutzer am 29.10. mit der DMG aus `06c52c9d` bereits gesehen
(`beta/zwischentest-2026-10-29.md`). Der Planer bewertet die Gegenprobe; hier nur prüfen und notieren.

Zusammenfassung: **21 behoben, 1 offen (M3), 0 unklar.** Wo der Beleg schmal ist, steht es in der Spalte Status.

| ID | Befund (kurz) | Status | Prüfen: klicken → erwarten |
|---|---|---|---|
| A02-01 | Haupt-Tabs zweizeilig, unten gekappt | behoben (U34 `72cec8f1`, `7b80f68d`; Zeilenbox N12 `c39af984`); Zwischentest bestätigt | Gruppen „Inhalte ▾“, „Technik ▾“, „Links ▾“ öffnen, mehrere Tabs wählen, auch bei 1 024 × 640 → jede Tab-Beschriftung einzeilig und vollständig; ein zu langer Name endet mit „…“ und zeigt beim Darüberfahren den vollen Namen. |
| A02-02 | Detail-Tabs: Konsolen-Tab oben gekappt, zweizeilig | behoben (U34 `72cec8f1`; N12 `c39af984`); Zwischentest bestätigt | Zeile wählen → Detail-Tab-Leiste: alle Beschriftungen einzeilig, „Chrome-Konsolenprotokoll“ ganz lesbar (ggf. über das Überlaufmenü rechts). |
| A02-03 | Seitenleisten-Tabs ohne Überlauf | behoben (U34 `72cec8f1`, `e361627e`); Zwischentest bestätigt | Seitenleiste schmal ziehen → Überlaufknopf rechts; „Antwortzeiten“, „API“, „Rechtschreibung …“ über das Menü erreichbar. |
| A02-04 | Sprachmischung bei Deutsch | behoben (U34 `e78cc988`, Rest U33 `185fbec3`) | Deutsch: Tab-, Filter- und Spaltennamen, Seitenleiste, URL-Details, Menü Bulk-Export/Berichte durchsehen → deutsch, außer bewusst englischen Fachbegriffen (Liste `beta/begriffe.md`, z. B. „Canonicals“). Englische Reste notieren. |
| A02-05 | Leere linke Spalte, zweites „Exportieren“ | behoben (U34 `6d7df192`); Zwischentest bestätigt | Hauptfenster → kein leerer Streifen links der Tabelle; genau ein „Exportieren ▾“ über der Tabelle. |
| A02-06 | Tab-Konfiguration uneinheitlich | behoben (U34 `72cec8f1`, `e361627e`) | In allen drei Leisten Überlaufknopf bzw. Rechtsklick auf einen Tab → jeweils „Tabs konfigurieren…“ und „Tabs zurücksetzen“, gleiches Muster überall. |
| A02-07 | Übersicht flackert im Crawl | behoben (N03 `4c312c64`) | Während des Crawls Seitenleiste „Übersicht“ offen lassen (1 min) → Zahlen laufen hoch, kein Wechsel auf „wird geladen“ zwischendurch. |
| A02-08 | Übersicht hängt nach Pause → Fortsetzen | behoben (N03 `4c312c64`); Zwischentest bestätigt | Pausieren, Fortsetzen → Übersicht und Statuskarte zählen weiter. |
| A02-09 | Detail-Tabs zeigen im Crawl nur „…“ | behoben (N03 `4c312c64`; Nachbesserung N04 `d0ca97de` nach Zwischentest Z-01) | Während des Crawls Zeile in „Intern“ wählen → „Eingehende Links“, „Ausgehende Links“, „Bilddetails“, „Ressourcen“ füllen sich in wenigen Sekunden; zurück auf „Eingehende Links“ → Werte noch da. |
| A02-10 | Tabellen träge im Crawl (Tab Extern) | behoben laut N03/N04 (Ursache: belegte Leseplätze, N04 `d0ca97de`); nur bis ≈ 5 000 URLs belegt, Grenzen für sehr große Crawls nicht nachgewiesen | Während des Crawls „URLs → Extern“ und zurück „Intern“ → Tabelle erscheint ohne spürbare Wartezeit (≈ 1 s). |
| A02-11 | Pause reagiert verzögert, kein Stoppen | behoben (N03 `4c312c64`, schneller N18 `0110a736`); Zwischentest bestätigt | Pausieren → Knopf sofort „Pausiere …“ und gesperrt, nach ≤ 3 s „Fortsetzen“. Es gibt jetzt „Stoppen“ (Schritt 6 im Ablauf). |
| A02-12 | Crawl-delay-Begrenzung unsichtbar | behoben (N03 `4c312c64`; Voreinstellung jetzt „nicht beachten“, K2-76) | Eigene Site mit `Crawl-delay` in der robots.txt → Statusleiste nennt „Crawl-delay … in robots.txt – nicht angewandt“ bzw. „angewandt“, wenn in Konfiguration → robots.txt „Crawl-delay aus der robots.txt beachten“ an ist. Text verständlich? |
| A02-13 | Sprache/Theme erst nach Neustart | behoben (N03 `4c312c64`; alle Fenster ≤ 200 ms N18 `a617eac7`); Zwischentest bestätigt | Einstellungen offen lassen, Sprache Englisch ↔ Deutsch und Theme Hell ↔ Dunkel → Hauptfenster und Einstellungsfenster wechseln sofort, auch Kopfleiste und Detail-Tabs. |
| A02-14 | „OpenSpider beenden“ ohne Wirkung | behoben (N03 `4c312c64`); Zwischentest bestätigt | Bei laufendem/pausiertem Crawl App-Menü → „OpenSpider beenden“ (oder ⌘Q) → Rückfrage mit „Pausieren und beenden“, danach ist die App zu. |
| A02-15 | Pausierter Crawl nach Neustart weg | behoben (N03 `4c312c64`; Anzeige N04 `d0ca97de`) | Nach Neustart Datei → „Zuletzt geöffnet“ → Crawl steht mit Domain und Datum da, öffnet, „Fortsetzen“ möglich. |
| A02-16 | MCP → Werkzeuge: Köpfe buchstabenweise, Liste leer | behoben (Daten N03 `4c312c64`; Spalten N04 `d0ca97de` nach Z-05; U35 `0a5853dc`) | Menü MCP → Dialog → Reiter „Werkzeuge“ → lesbare Spaltenköpfe, Liste mit Werkzeugen (≈ 16), auch bei gestopptem Server. |
| A02-17 | Mehrfach-Export ohne Auswahl/Export | behoben (U34 `28068f63`); Zwischentest bestätigt | „Exportieren ▾“ → „Mehrfachexport…“ → Ausgabe, Auswahlbaum links/„Ausgewählt (n)“ rechts, Vorlagen; zwei Positionen wählen, Ordner wählen, „Exportieren…“ → Liste der erzeugten Dateien. |
| A02-18 | „Crawl-Analyse starten“ ohne Rückmeldung | behoben (N03 `4c312c64`) | Nach Crawl-Ende Menü Crawl-Analyse → „Crawl-Analyse starten“ → Statusleiste „Analyse läuft“, danach „Analyse abgeschlossen“. |
| A02-19 | Segmente: JSON-Feld klein, Dateiauswahl englisch | behoben (U34 `ee274cde`) | Konfiguration → Segmente → Feld in voller Breite unter der Beschriftung; „Datei wählen…“ / „Keine Datei ausgewählt“ deutsch. |
| A02-20 | Leerzustände der Seitenleiste ohne Innenabstand | behoben (U35 `0a5853dc`, „Dialoge/Leerzustände vereinheitlicht“); Beleg nur allgemein, kein eigener Test zu A02-20 genannt | Ohne geladenen Crawl (nach Neustart oder „Löschen“) Seitenleiste „Übersicht“, „Befunde“, „Seitenstruktur“ → Text und Knopf mit Abstand zum Rand. |
| A02-21 | JavaScript-Rendering gesperrt ohne Erklärung | behoben (U34 `ee274cde`; Spec-ID-Frage bleibt beim Planer) | Konfiguration → Spider: Seitendarstellung → Hinweis, warum gesperrt (Crawl offen → „Löschen“ und neuen Crawl beginnen) und dass ein Chromium-Pfad nötig ist. |
| A02-22 | Wunsch Update-Prüfung | offen → M3 (OUT-250/251, Nutzerentscheidung 28.10., `dae57875`) | Nicht prüfen. |

**Zwischentest 29.10. (Z-01 … Z-07)** – zur Einordnung, gleiche Gegenprobe im Ablauf:

| ID | Befund (kurz) | Status | Prüfen |
|---|---|---|---|
| Z-01 | Detail-Tabs langsam, Werte beim Zurückwechseln weg | behoben (N04 `d0ca97de`) | wie A02-09 |
| Z-02 | Filter „HTML“ im Crawl zeigt „Noch keine Daten“ | behoben (N04 `d0ca97de`) | Während des Crawls Filter-Chip „HTML (…)“ → Zeilen erscheinen, kein Leerzustand. |
| Z-03 | Größerer Crawl öffnet nach Neustart nicht | unklar: Fehler bei N04 nicht nachgestellt (5 000-URL-Crawl öffnet); neu sind Fortschritt, Fehlermeldung mit „Technische Details“ und Logeintrag | Großen Crawl über „Zuletzt geöffnet“ öffnen → öffnet, oder es erscheint eine Meldung mit Grund. Nie „nichts passiert“. |
| Z-04 | „Zuletzt geöffnet“ nur UUID | behoben (N04 `d0ca97de`) | Einträge zeigen Domain/Start-URL und Datum. |
| Z-05 | MCP-Werkzeuge (wie A02-16) | behoben (N04 `d0ca97de`) | wie A02-16 |
| Z-06 | Speicherwarnung jede Sekunde | behoben (N04 `d0ca97de`, Messung unter macOS korrigiert) | Während des Crawls keine Dauerwarnung „Arbeitsspeicher knapp“; falls gedrosselt, steht „Gedrosselt“ sichtbar in der Statusleiste und verschwindet wieder. |
| Z-07 | Englische Reste („No Crawl“, „SERP Snippet“) | behoben (U33 `185fbec3`) | Ohne Crawl über der Tabelle deutscher Text; Detail-Tab heißt „SERP-Snippet“. |

## 2. Neu oder geändert seit A02

Seit dem A02-Build (`572e14db`) in `main` gemergt. Nur Änderungen, die man in der App sieht.

| Bereich (Paket) | Was ist neu | Prüfschritte |
|---|---|---|
| Navigation (U34, K2-77) | Gruppen „Übersicht / URLs / Inhalte ▾ / Technik ▾ / Links ▾ / Performance ▾ / Benutzerdefiniert ▾ / Konfiguration ▾“, Pillen-Tabs, Filter-Chips mit Zählern, „Exportieren ▾“, Umschalter Liste/Baum (Baum noch gesperrt) | (1) Jeden gewünschten Tab in höchstens zwei Klicks erreichen. (2) Nach Neustart sind Gruppe und Tab wie zuvor. |
| Kopfleiste, Statuskarte, Dashboard (U35) | Logo, URL-Feld mit Bereich, blauer Hauptknopf Starten/Pausieren/Fortsetzen, „Stoppen“, „Löschen“; Statuskarte mit Fortschritt, Rate, Lauf- und Restzeit; „Übersicht“ als Dashboard mit Statusverteilung und „Top-Probleme“ | (1) Statuskarte während des Crawls: Zahlen plausibel, nichts springt. (2) Dashboard → Klick auf ein Top-Problem → Tabelle mit passendem Filter. |
| Zeilenhöhe, Tab-Zeile, Kontraste (U35, U30, N12) | Einstellungen → Erscheinungsbild → „Zeilenhöhe in Tabellen“: Kompakt/Komfortabel; Tab-Beschriftungen ganzzahlig hoch; bessere Kontraste (Befunde „mittlere Priorität“ hell, Segment-Knöpfe dunkel) | (1) Kompakt ↔ Komfortabel umschalten → Tabelle, Detail und Seitenleiste ohne Überlappung, auch bei 1 024 × 640. (2) Hell und Dunkel: Seitenleiste „Befunde“ und „Segmente“ gut lesbar. |
| Sprache (U33, N18) | Deutsch vollständig inkl. Bulk-Export-/Berichtsnamen, Fehlermeldungen; Wechsel de/en ohne Neustart in allen offenen Fenstern | (1) Englisch ↔ Deutsch mit offenem Einstellungs- und Hauptfenster. (2) Menü Bulk-Export und Berichte auf Deutsch durchsehen; uneinheitliche Begriffe notieren (z. B. „Nahezu-“ vs. „Beinahe-Duplikate“). |
| Pause/Stoppen (N03, N18) | Zwischenzustände „Pausiere …“/„Stoppe …“, eigener Knopf „Stoppen“, Status „Gestoppt“ (nicht fortsetzbar), danach automatische Analyse | (1) Pause und Fortsetzen zügig hintereinander. (2) Zweiten Crawl stoppen → „Gestoppt“, „Fortsetzen“ nicht mehr angeboten. |
| Live-Ansicht während des Crawls (N03, N04) | Übersicht, Detail-Tabs, Filter und Tabellenwechsel liefern Werte schon während des Crawls | siehe A02-07, A02-09, A02-10, Z-02 |
| Zuletzt geöffnet, Öffnen (N04) | Einträge mit Domain und Datum; Öffnen mit Fortschritt; Fehler als Meldung mit „Technische Details“ | siehe A02-15, Z-03, Z-04 |
| Exporte (U34, U33, N11) | Mehrfachexport-Dialog (Ausgabe, Auswahl, lokale Dateien, Vorlagen); Exportnamen deutsch, Dateiinhalte sprachunabhängig; Sitemap-Export mit `lastmod` | (1) „Exportieren ▾ → Diese Ansicht exportieren…“ als XLSX → Datei öffnen, Spaltenköpfe und Zeilenzahl plausibel. (2) Mehrfachexport mit Zeitstempel-Ordner → Ordner und Dateien vorhanden. |
| Vergleichsansicht (N02b) | Vergleich zweier Crawls im gemeinsamen Tabellenraster: Spalten konfigurieren, Sortieren per Kopf, URL wählen → Linkvergleich, CSV-Export | (1) Knopf „Crawls vergleichen“ bei den Haupt-Tabs → zwei Crawls derselben Site → „Vergleichen“. (2) Spalte ausblenden/sortieren, URL wählen → „Vergleich der Links“ füllt sich. |
| Analyse-Tempo (N14, N16) | Links-Schritt der Crawl-Analyse und Nahezu-Duplikate deutlich schneller | (1) Crawl-Analyse auf dem Test-Crawl → endet zügig (Gefühl notieren, keine Stoppuhr nötig). (2) „Inhalte ▾ → Inhalt“, Filter „Beinahe-Duplikate“ → Treffer plausibel; Detail-Tab „Duplikate“ zeigt Partner. |
| Daten und Filter (N06–N11, Q02b) | Korrekturen an Spalten und Filtern, u. a. Filter „Größenattribute fehlen“ (Bilder) ohne JavaScript-Rendering, Lesbarkeitsspalten im Tab Inhalt, Pixelbreite der Titel, Soft-404-Seiten | (1) „Bilder“ → Filter „Größenattribute fehlen“ zählt Treffer. (2) Tab „Inhalt“ → Spalten „Lesbarkeit“ und „Flesch-Reading-Ease-Score“ gefüllt, Werte fachlich plausibel? |
| Tastatur (U30, U32, N12) | Tab-Leisten mit Pfeiltasten/Pos1/Ende, Alt+Umschalt+Pfeil verschiebt Tabs, Esc schließt Menüs, Tabelle ist ein einziger Tabstopp, sichtbarer Fokus; Hilfe → „Tastaturkürzel“ | (1) Nur mit Tastatur: Tab bis zur Tab-Leiste, Pfeile, Enter; weiter in die Tabelle, Pfeile, Kontextmenü mit Umschalt+F10 (auf dem Mac ggf. mit fn). (2) Hilfe → „Tastaturkürzel“ öffnen, zwei Kürzel ausprobieren. |
| In A02 nicht geprüft (R02, U17, schon im A02-Build) | JavaScript-Tab, Anmeldung per Formular, „Gerenderte Seite“, „Chrome-Konsolenprotokoll“ – brauchen einen Chromium-Pfad | Nur ansehen: Konfiguration → Authentifizierung; Detail-Tab „Gerenderte Seite“ ohne Rendering → verständlicher Hinweis statt leerer Fläche? |

Ohne sichtbare Änderung in der App: J01 (Entwicklerhandbuch), N13 und N17 (Build/CI), E07 (Engine bei sehr großen Crawls – nur
beobachten, ob das Tempo gleichmäßig bleibt). U31 (Leistungsmessung der Oberfläche) ist laut Git-Verlauf noch nicht in `main`.

## 3. Ablauf in 30 Minuten

**Testseite:** am besten wieder die eigene Website aus A02 und dem Zwischentest (vergleichbare Werte), auf **≤ 1 000 URLs**
begrenzt. Alternativ eine öffentliche Seite, die ausdrücklich zum Testen von Crawlern gedacht ist; dann Grenze 500 URLs.
**Tempo moderat halten:** Seit N03 beachtet OpenSpider `Crawl-delay` aus der robots.txt nicht mehr von selbst (K2-76). Deshalb vor
dem Start Konfiguration → Geschwindigkeit: „Max. Threads“ 3, „Max. URLs pro Sekunde“ 3 (fremde Seiten: 1–2, und unter
robots.txt „Crawl-delay aus der robots.txt beachten“ einschalten).

| Zeit | Schritt | Prüft |
|---|---|---|
| 0–3 min | **Vorbereitung.** DMG installieren, App starten. Einstellungen: Deutsch, Dunkel. Konfiguration: Geschwindigkeit wie oben; Grenzen → maximale Anzahl URLs 1 000; Spider: Voreinstellungen → Duplikate → „Nahezu-Duplikate suchen“ an. Leere Seitenleiste ansehen. Konfiguration → Segmente und Spider: Seitendarstellung kurz öffnen. | A02-19, A02-20, A02-21, Z-07 |
| 3–11 min | **Crawl 1 starten, Live-Ansicht.** Statuskarte und Statusleiste (Rate, Crawl-delay-Hinweis, Speicher). Seitenleiste „Übersicht“ 1 min beobachten. Zeile in „Intern“ wählen, Detail-Tabs „Eingehende Links“, „Ausgehende Links“, „Bilddetails“, „Ressourcen“ und zurück. Filter-Chip „HTML“. „Extern“ ↔ „Intern“. Pausieren, nach 10 s Fortsetzen. Dashboard „Übersicht“. | A02-07 … A02-12, Z-01, Z-02, Z-06; U35 |
| 11–17 min | **Oberfläche, Crawl läuft weiter.** Gruppen und Tabs durchgehen, Überlaufmenüs und Rechtsklick „Tabs konfigurieren…/zurücksetzen“ in allen drei Leisten. Fenster auf 1 024 × 640, dann Vollbild. Einstellungen offen lassen: Englisch ↔ Deutsch, Hell ↔ Dunkel, Kompakt ↔ Komfortabel. In Deutsch englische Reste notieren. | A02-01 … A02-06, A02-13; U30, U33, N12, N18 |
| 17–22 min | **Nach Crawl-Ende: Analyse und Daten.** Crawl-Analyse starten, Rückmeldung abwarten. „Inhalt“: Beinahe-Duplikate, Lesbarkeit; „Bilder“: Größenattribute fehlen; Detail-Tab „Duplikate“. **Exporte:** Ansicht als XLSX; Mehrfachexport mit zwei Positionen und Zeitstempel-Ordner; Menü Bulk-Export und Berichte auf Deutsch. | A02-04, A02-17, A02-18; N08, N14, N16, U33 |
| 22–25 min | **Crawl 2 und Vergleich.** Zweiten Crawl derselben Site mit Grenze 100 URLs starten (bestehende Ansicht ersetzen), nach ≈ 30 s „Stoppen“ → „Gestoppt“. „Crawls vergleichen“ → beide Crawls → „Vergleichen“: Spalten, Sortierung, URL wählen → Linkvergleich, CSV-Export der Ansicht. | A02-11 (Stoppen); N02b |
| 25–27 min | **Dialoge und Tastatur.** MCP → „Werkzeuge“. Nur Tastatur: Tab-Leiste (Pfeile, Enter, Esc im Menü), Tabelle (Pfeile, Umschalt+F10), Hilfe → „Tastaturkürzel“. | A02-16, Z-05; U30, U32 |
| 27–30 min | **Beenden und Neustart.** Crawl 1 aus „Zuletzt geöffnet“ öffnen (falls nicht offen), App-Menü → „OpenSpider beenden“. Neu starten: Gruppe/Tab wiederhergestellt? Datei → „Zuletzt geöffnet“: Domain und Datum, Crawl 1 öffnet. Optional (falls Zeit): Crawl pausiert lassen, ⌘Q → „Pausieren und beenden“, neu starten, öffnen, „Fortsetzen“. | A02-14, A02-15, Z-03, Z-04; U34 |

Wenn die Zeit knapp wird, zuerst den Schritt 17–22 kürzen (Daten und Filter sind durch den Paritätslauf Q02b belegt); die Schritte
3–11 und 27–30 betreffen die früheren Hoch-Befunde und haben Vorrang.

## 4. Befunde notieren

Format wie `beta/befunde.md`; der Orchestrator überträgt die Rückmeldung nach `beta-2/befunde.md`.

- **IDs** A03-01, A03-02, … in der Reihenfolge der Meldung. Besteht ein A02-/Z-Befund weiter, neue A03-ID mit Verweis
  („wie A02-09, besteht weiter“).
- **Schwere** (wie alpha/beta): **Blocker** = Hauptpfad unbenutzbar; **Hoch** = Kriterium oder Hauptfunktion scheitert;
  **Mittel** = Funktion verfälscht oder unzuverlässig, Umgehung möglich; **Niedrig** = Darstellung, Hinweis, Komfort. Wünsche
  ohne Fehler als „– (Wunsch)“.
- **Je Befund:** Ort (Bereich der Oberfläche), Reproduktion (Schritte, Fenstergröße, Sprache, Theme), Beobachtung, Erwartung,
  Screenshot (nur eigene App; Bilder mit Inhalten der eigenen Site bleiben lokal beim Orchestrator), Spec-ID falls bekannt.
- **Gegenprobe:** je A02-/Z-ID ein Wort genügt – „ok“, „besteht“ oder „nicht geprüft“.
- **Folgen** (K2-75): Blocker sowie Hoch, wenn ein Abnahmekriterium B1–B12 betroffen ist → N-Paket der Welle 4n vor Q04; alles
  Übrige → `docs/plan/M2-backlog.md` (M3) mit Verweis auf die A03-ID.
