# Beta-Abnahme M2-β (M2-A02): Durchklickplan

Dauer 30–45 min. Build siehe `build.md`. Jede Auffälligkeit bitte kurz notieren (wo, was, was erwartet, Fenstergröße, Sprache,
ggf. Screenshot) – ich übertrage sie nach `befunde.md` (A02-nn). Keine Fixes in dieser Runde.

**Ausgangslage:** Sprache Deutsch, Theme dunkel, Fenster normal (≈ 1 500 × 1 000). Als Site eignet sich jede eigene kleine
Website (≤ 500 Seiten); alternativ `os-testsite ui/fx-ui-basic` aus dem Repo.

## 1. Crawl (5 min)
1. Site-URL eingeben, Crawl starten; Statusleiste (Fortschritt, URLs/s, Rest) beobachten.
2. Pausieren, fortsetzen, am Ende stoppen bzw. auslaufen lassen; Status „Abgeschlossen“ stimmt mit dem Rest überein?

## 2. Haupttabelle und Tab-Leisten (10 min)
3. Haupt-Tab-Leiste durchgehen (Internal, External, Response Codes, Page Titles, Meta Description, … bis zum Ende): Beschriftungen
   vollständig lesbar? Überlauf (»-Knopf/Menü) erreichbar? Gegenprobe Nutzerpunkte 27.10. Nr. 1 (zweizeilige Labels abgeschnitten).
4. „Tabs konfigurieren“ und „Tabs zurücksetzen“ für die Haupt-Tabs: Reihenfolge ändern, Tab ausblenden, zurücksetzen.
5. Filter (Dropdown „Alle/…“), Suche, Sortierung per Spaltenkopf, Spalten ein-/ausblenden und verschieben, Auswahl mehrerer Zeilen.
6. Sprachmischung notieren (Nutzerpunkt 4: „No Crawl“, „All (–)“, englische Tab-Namen neben deutschen).

## 3. Detail- und Seitenleisten-Tabs (7 min)
7. Zeile auswählen → unteres Detailfenster: alle Detail-Tabs (URL Details, Inlinks, Outlinks, Image Details, SERP Snippet,
   Gerenderte Seite, HTTP Headers, Strukturierte Daten, Chrome Console, Lighthouse Details …) öffnen; Beschriftungen lesbar?
   (Nutzerpunkt 2: „Chrome Console“ oben abgeschnitten.) „Detail-Tabs konfigurieren“/„Tabs zurücksetzen“ probieren.
8. Rechte Seitenleiste: Overview, Issues, Site Structure, Response Times, API-Tabs; Überlauf der Seitenleisten-Tabs
   (Nutzerpunkt 3: „Ant…“ abgeschnitten, „Href“ vor dem »-Knopf gekappt); Klick auf eine Overview-/Issues-Zeile filtert die Tabelle?
9. Bedienelemente vergleichen (Nutzerpunkt 6: „Tabs anpassen“ als Aufklapptext vs. Knöpfe unten) – Eindruck notieren.

## 4. Export (4 min)
10. „Exportieren“ in der Werkzeugleiste: Ansicht als CSV und XLSX in einen Ordner; Datei öffnen, Spalten und Zeilenzahl plausibel?
11. Bulk-Export (Menü/Dialog): zwei Berichte wählen, exportieren.
12. Nutzerpunkt 5 prüfen: leere linke Spalte mit zweitem „Exportieren“-Knopf – noch vorhanden?

## 5. Dialoge und Verwaltung (8 min)
13. Konfigurationsdialog (Spider, Limits, Rendering, User-Agent …): Wert ändern, speichern, Dialog erneut öffnen → Wert erhalten?
14. Crawl-Verwaltung: Crawl speichern/benennen, Liste der Crawls, zweiten kurzen Crawl derselben Site starten.
15. Vergleichsmodus / Change Detection: beide Crawls vergleichen; Vergleichstabelle und Link-Vergleich sichten
    (Hinweis: diese Ansicht wird in M2-N02b noch auf das gemeinsame Tabellen-Raster umgestellt – Darstellung ist vorläufig).
16. Visualisierung (Crawl-Baum/Verzeichnisbaum): öffnen, Zoom, Knoten anklicken.
17. MCP-Dialog öffnen: Anzeige des Status, Start/Stopp des Servers (nur ansehen, kein externer Client nötig).

## 6. Einstellungen und Fenster (5 min)
18. Sprache auf Englisch, zurück auf Deutsch; Theme hell/dunkel umschalten – Kontraste, Reste der anderen Sprache?
19. Fenster auf 1 024 × 640 verkleinern: Tab-Leisten, Detailfenster, Seitenleiste bedienbar? Dann Vollbild.
20. App beenden und neu starten: letzter Crawl/Einstellungen wiederhergestellt?

## Nutzerpunkte vom 27.10. (werden A02-01 … A02-06, falls noch sichtbar)
1. Haupt-Tab-Leiste: zweizeilige Beschriftungen unten abgeschnitten.
2. Detail-Tab-Leiste: „Chrome Console“ oben abgeschnitten, weitere zweizeilige Labels.
3. Seitenleisten-Tabs ohne Überlauf-Steuerung („Ant…“), „Href“ vor »-Knopf gekappt.
4. Sprachmischung bei Deutsch.
5. Leere linke Spalte mit zweitem „Exportieren“-Knopf.
6. Uneinheitliche Bedienelemente für die Tab-Konfiguration.
