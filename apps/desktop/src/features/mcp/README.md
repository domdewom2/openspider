# M2-U25 – MCP-Oberfläche

Menü, Statusleiste und fünf Dialogreiter verwenden die typisierten IPC-Commands. Zugangsdaten werden nur nach ausdrücklichem Anzeigen ins DOM aufgenommen. Client-Vorschauen redigieren strukturierte JSON-/TOML-Werte vor dem Erzeugen des sichtbaren Diffs; unbekannte Syntax wird ausgeblendet. Kopieren verwendet den nativen Tauri-Clipboard-Schreibpfad.

Der vollständige [Paketbericht](../../../../../docs/plan/M2-reports/u25.md) enthält den Bericht je Reviewbefund, sämtliche Fremdergänzungen mit Diff, Prüfresultate und die verbindliche Zuordnung der offenen Integrationskriterien.

`mcp.test.tsx` und `e2e/mcp/dialog.spec.ts` prüfen den UI-Vertrag mit MockCore. Die vom Browsertest selbst geschriebenen Sicherungen und injizierten Aufrufe/Sitzungen sind ausschließlich Fixtures. Sie belegen keine Produktionsintegration.

`e2e/mcp/bridge.spec.ts` nutzt eine eigene Seite mit echtem Bridge-Transport und ohne Mock-Fallback. Der aktive Test prüft den aktuellen Servicezustand. Fünf vollständige Fälle gegen die noch fehlenden K17-Commands sind mit benannten C9-Zurückstellungen versehen: echte Sicherung, drei HTTP-Aufrufe gegen einen Crawl, Werkzeug-/Markdown-Gleichheit, Sitzungen einschließlich Ablauf sowie Auto-Start/Portkonflikt. Zuständigkeit und die notwendige Reihenfolge K17 vor Aktivierung sind im Paketbericht festgehalten.

Der macOS-Test `e2e/mcp/native.mjs` prüft den unveränderten Produktionsadapter und `AccessPanel` in einer echten WKWebView mit 1,5 s verzögerter Tokenantwort und tatsächlicher Systemzwischenablage. Nur die Tokenantwort ersetzt die noch fehlende K17-Implementierung.
