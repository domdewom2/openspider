# M2-U25 – MCP-Oberfläche

Menü, Statusleiste und fünf Dialogreiter verwenden die typisierten IPC-Commands. Ein passiver `Unsupported`-Status bleibt beim App-Start ohne Fehlerbanner; beim Öffnen des Dialogs oder nach einer Nutzeraktion wird der Fehler angezeigt. Zugangsdaten werden nur nach ausdrücklichem Anzeigen ins DOM aufgenommen. Programme werden seit B3-MCP (AGENT-043, Nutzerentscheidung 04.11.2026) mit einem Klick verbunden: je Programm ein Dialog „Mit … verbinden“ (`ConnectPanel`), Haken „Agent darf Crawls starten“ (Klasse `crawl`), Sicherung, Eintrag und Freigabe in einem Schritt. Der Kern (`os-mcp::client_config`) schreibt formatbewahrend und liefert nur maskierte, geänderte Zeilen; die Oberfläche erhält nie die fremde Datei und redigiert zusätzlich mit `redactText`. Kopieren verwendet den nativen Tauri-Clipboard-Schreibpfad.

Der vollständige [Paketbericht](../../../../../docs/plan/M2-reports/u25.md) enthält den Bericht je Reviewbefund, sämtliche Fremdergänzungen mit eingebettetem Diff, Prüfresultate und die verbindliche Zuordnung der offenen Integrationskriterien. Die separate Patch-Datei im Feature-Verzeichnis ist entfernt.

`mcp.test.tsx` und `e2e/mcp/dialog.spec.ts` prüfen den UI-Vertrag mit MockCore. Die vom Browsertest selbst geschriebenen Sicherungen und injizierten Aufrufe/Sitzungen sind ausschließlich Fixtures. Sie belegen keine Produktionsintegration.

`e2e/mcp/bridge.spec.ts` nutzt eine eigene Seite mit echtem Bridge-Transport und ohne Mock-Fallback. Der aktive Test prüft den aktuellen Servicezustand. Fünf vollständige Fälle gegen die noch fehlenden K17-Commands sind mit benannten C9-Zurückstellungen versehen: echte Sicherung, drei HTTP-Aufrufe gegen einen Crawl, Werkzeug-/Markdown-Gleichheit, Sitzungen einschließlich Ablauf sowie Auto-Start/Portkonflikt. Zuständigkeit und die notwendige Reihenfolge K17 vor Aktivierung sind im Paketbericht festgehalten.

Der macOS-Test `e2e/mcp/native.mjs` prüft den unveränderten Produktionsadapter und `AccessPanel` in einer echten WKWebView mit 1,5 s verzögerter Tokenantwort und tatsächlicher Systemzwischenablage. Nur die Tokenantwort ersetzt die noch fehlende K17-Implementierung.
