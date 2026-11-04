# Beta 3 – Arbeitsliste

Stand 04.11.2026. Sammelt alles, was nach beta.1/beta.2 für die nächste Beta beschlossen oder gefunden wurde. Reihenfolge innerhalb eines Abschnitts = Priorität. Pakete bekommen beim Start eine ID und einen Bericht unter `docs/plan/M3-reports/`.

## 1 MCP mit einem Klick verbinden (Nutzerentscheidung 04.11.2026)

Anlass: Ein Nutzer bat Codex, über OpenSpider einen Crawl zu starten. Codex kannte den Server nicht, und selbst nach dem Eintragen wäre nur Lesen freigegeben gewesen. Heute braucht die Einrichtung fünf Schritte in zwei Reitern (MCP → „Mit … verbinden…“ → Vorschau → Bestätigen → Schreiben, dann Reiter „Zugang“ → „Crawlen“).

- **Ein Dialog je Programm:** „Mit Codex verbinden“ (ebenso Claude Code, Claude Desktop) zeigt einen Haken „Agent darf Crawls starten“ (Klasse `crawl`, standardmäßig an in diesem Dialog, Lesen immer an) und einen Button „Verbinden“. Sicherung, Eintrag und Freigabe passieren in einem Schritt; die Vorschau bleibt aufklappbar. Abschluss mit klarer Anweisung: „Starte in <Programm> eine neue Sitzung.“ Bestehende Einträge werden erkannt („Bereits verbunden – aktualisieren?“).
- **Nur anhängen statt neu schreiben:** `os_mcp::client_config::merged` parst die Client-Datei und schreibt sie komplett neu (Codex-TOML: 130 von 176 Zeilen geändert, Reihenfolge und Format anders, Inhalt laut Prüfung vom 04.11. identisch). Künftig formatbewahrend ergänzen (z. B. `toml_edit`, JSON mit erhaltener Reihenfolge), sodass die Vorschau nur die neuen Zeilen zeigt.
- **Vorschau ohne Geheimnisse:** Die Vorschau zeigt die komplette Konfiguration fremder Programme. Werte, die nach Schlüssel/Token/Passwort aussehen (`env`-Einträge, `headers`, `*_KEY`, `*_TOKEN` …), maskieren; nur der eigene Eintrag erscheint vollständig.
- **Cursor:** Ein-Klick-Installation über Cursors Installationslink (App und Website).
- **Claude Desktop:** Installationspaket (Erweiterung) prüfen und, falls tragfähig, anbieten.
- **Website:** Abschnitt „MCP in 2 Minuten“ für Claude Code, Codex und Cursor (DE/EN), verlinkt von Start- und Funktionsseite.
- **Hilfetext korrigieren:** `mcp-classes-help` verspricht Kostenschätzung und Bestätigung für kostenpflichtige Aufrufe; das kommt erst mit AGENT-030 (M4). Bis dahin nicht zusagen.
- **Handbuch:** Abschnitt „MCP einrichten“ auf den neuen Ablauf umstellen, Hinweis auf die Freigabe `crawl`.
- Spec: AGENT-003 (Freigaben) und das zuständige UI-Kapitel um den Verbindungsdialog ergänzen, bevor gebaut wird.

## 2 Leistung

- **Rückschritt beim Crawl:** Messung 04.11.2026 mit dem veröffentlichten beta.2-CLI (Mac mini M2, lokale Testseite, Standardwerte, inkl. Speichern und Export): 100.000 URLs 48 s / 0,85 GB, 1 Mio. URLs 12:51 min / ~1,05 GB. Frühere Werte (Vor-Alpha, 18./19.10.): 27 s / 0,76 GB bzw. ~4,5 min / < 0,5 GB. Diagnose (Methode vs. echter Rückschritt, Bisect) läuft auf dem Build-Server; danach Fix-Paket.
- Nach dem Fix 1-Mio.-Messung wiederholen; README-Abschnitt „Performance“ nachziehen.
- Aussage „5 Mio. URLs bleiben in der App flüssig“ (Werte aus Entwicklungsständen im Oktober) mit beta.2/beta.3 nachmessen.

## 3 Funde aus Beta-Tests (UI)

- Befunde-Seitenleiste: Namen auf Englisch in der deutschen Oberfläche.
- „Kein Crawl geöffnet.“ ohne Innenabstand.
- Start-URL leer, nachdem ein Crawl wieder geöffnet wurde.
- Fenstertitel der Visualisierung in der falschen Sprache.
- Update-Dialog: Buttons „Beim Beenden installieren“ / „Jetzt installieren und neu starten“ brechen zweizeilig um.
- „Hilfe → Nach Updates suchen“ öffnete in einer seit über 17 Stunden laufenden Instanz keinen Dialog; nach Neustart sofort. Ursache klären.
- Funde aus dem Windows-Test des Nutzers (stehen noch aus).
- V02 (Graphen/Wortwolke): offene Low-Punkte; V01 (Übersicht nach Teilmengen): Backlog aus dem Bericht.

## 4 Funktionen

- Chrome-Download auf Klick für JavaScript-Rendering (in beta.1/beta.2 als „folgt mit einer der nächsten Betas“ angekündigt).

## 5 Release-Ablauf

- Entwurfshinweise („unsigniert“, „Installations-Smoke“, „TODO M2-Parität“) nur in die Zusammenfassung des Actions-Laufs, nicht in den Release-Text (Fable-Empfehlung zu m3/rel-assets).
- Feedback-Link im Download-Kasten: bei stabilen Releases auf GitHub Discussions statt `/beta/`.
- Beta-Seiten der Website (`/beta/`, `/en/beta/`) direkt auf die versionslosen Downloads verlinken.
- Release-Notes stehen vor dem Bauen fest: Die Update-Notes in `latest.json` entstehen beim Bauen; eine spätere Änderung am Release-Text erreicht den Update-Dialog nicht.
- `scripts/release/licenses/generate.mjs`: Wiederholung bzw. Zwischenspeicher für nachgeladene Lizenztexte (beta.2-Windows-Job brach an einem Netz-Timeout ab).
- CHANGELOG-Einträge von `rel/beta1` und `rel/beta2` nach main; `compat_1.0.0-beta.1` nach `tests/fixtures/migrations/releases/` (K3-11).

## 6 Tests und CI

- Pause-vs.-Abschluss-Wettlauf in Fixtures (header.spec:69 WebKit, n04-live:74, `beta_resume.rs` a_new_paused_crawl… auf Windows): Testserver hält Antworten bis zur Pause zurück. Dazu Spec-Frage CRAWL-001: Was gilt, wenn Pause und Crawl-Ende zusammenfallen?
- macOS-Zeitgrenzen in `render_chromium.rs` (Zeile ~213 pixel.png, ~635 late_paint 1500 ms vs. 1533 ms).
- Pfadfilter: Commits, die nur `website/`, `README.md` oder `docs/` ändern, sollen laufende main-CI nicht abbrechen bzw. keine volle CI auslösen.
- Weitere bekannte Flakes: Intel, SMTP, `robots_step_m2`.
