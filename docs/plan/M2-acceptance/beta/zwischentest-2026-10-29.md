# Zwischentest 29.10.2026 (Nutzer, macOS, Universal-DMG aus main 06c52c9d)

Durchgeführt von Dominik nach dem Prüfplan zur Zwischentest-DMG (Runde 14b: N02, N03, E07, U34). Testsite: echte Website, ca. 4.800 URLs, Crawl mit Pause/Fortsetzen, danach Beenden und Neustart. Format wie `befunde.md` (Schwere: Blocker/Hoch/Mittel/Niedrig).

## Bestätigt behoben
- Pause → Fortsetzen; Crawl läuft bis zum Ende durch (A02-08, A02-11).
- Sprache und Theme (inkl. eigene Farben, Zoom) wirken sofort (A02-13).
- „OpenSpider beenden“ wirkt (A02-14).
- Mehrfach-Export mit Auswahlbaum und Export (A02-17).
- Tab-Leisten einzeilig, gruppierte Navigation, ein „Exportieren“ (A02-01 … A02-05, Screenshot).

## Befunde

| ID | Titel | Schwere | Ort | Spec |
|---|---|---|---|---|
| Z-01 | Detail-Tabs während und kurz nach dem Crawl sehr langsam; lange nur „…“ bzw. Ladebalken (Duplikate); beim Zurückwechseln auf Eingehende/Ausgehende Links sind bereits geladene Werte wieder weg; nach einiger Zeit nach Crawl-Ende normal | Hoch | Detailfenster: Eingehende Links, Ausgehende Links, Bilddetails, Duplikate | UI-018, UI-030, UI-032, UI-033, UI-035, UI-145 |
| Z-02 | Filter „HTML“ während des laufenden Crawls zeigt den Leerzustand „Noch keine Daten – Crawl starten oder Crawl-Datei öffnen“, obwohl der Zähler bereits 1.884 HTML zeigt; Inhalt erst bei 100 % | Hoch | obere Tabelle, Filter-Chips | UI-018, UI-016 |
| Z-03 | Größerer Crawl (4.811 URLs, Datei 82 MB) lässt sich nach Neustart über „Zuletzt geöffnet“ nicht öffnen: nichts passiert, keine Fehlermeldung, kein Logeintrag; Übersicht bleibt „Einen Crawl öffnen …“. Kleiner Crawl (16 URLs) öffnet | Hoch | Datei → Zuletzt geöffnet | CRAWL-126, UI-124 |
| Z-04 | „Zuletzt geöffnet“ zeigt nur UUID-Dateinamen; Domain/Start-URL und Datum fehlen | Mittel | Datei → Zuletzt geöffnet | UI-124 |
| Z-05 | MCP → Werkzeuge: Spaltenköpfe brechen buchstabenweise um, Werkzeugliste leer (A02-16 in der echten App nicht behoben) | Mittel | MCP-Dialog → Werkzeuge | UI-129 |
| Z-06 | Speicherwächter meldet während des gesamten Crawls jede Sekunde „Wenig verfügbarer RAM – Crawl gedrosselt“ (16-GB-Mac; parallel lief ein Build). Zu prüfen: Messgröße `available_bytes` unter macOS (komprimierter/inaktiver Speicher), Schwelle 10 %, fehlende Anzeige der Drosselung in der Statusleiste | Mittel | Engine, Statusleiste | CRAWL-129 |
| Z-07 | Englische Resttexte: „No Crawl“ über der Tabelle, „SERP Snippet“ | Niedrig | Hauptfenster, Detail-Tabs | UI-121 (U33) |

Log: `~/Library/Logs/OpenSpider/openspider.log` (Crawl 01a18894…, 17:53–17:56). Z-07 erledigt U33.
