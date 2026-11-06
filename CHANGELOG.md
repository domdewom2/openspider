# Changelog

## [Unreleased]

- Frühe Beta für SEO-Agenturen in Vorbereitung. Plattform-Smoke, Archiv-Kompatibilität und noch offene M2-Parität bleiben vor der Freigabe zu prüfen.

## [1.0.0-beta.3] - 2026-11-06

Faster crawls, Chromium on demand, one-click agent connections and a report for checking deployments.

- Faster large crawls: crawl data is written in bigger batches, about 30 % less time for 100,000 and 1 million URLs in our benchmarks. The link score analysis now runs in memory (1 million URLs: 113 s → 23 s).
- JavaScript rendering no longer needs an installed Chrome: OpenSpider downloads a tested Chromium version on request and can remove it again.
- Connect Claude Desktop, Claude Code, Codex or Cursor to the MCP server in one step. Agents can only read by default; starting crawls needs the "Agent may start crawls" option. Existing configuration files keep their format, a backup is created, and read-only files are never changed.
- New report "Crawl Changes" compares a crawl with the previous crawl of the same project: removed pages, new redirects, changed canonicals and indexability. Useful after a deployment; the sub-filters can be exported.
- Crash reports are stored locally and only sent if you agree.
- Visualisations: 3D crawl and directory graphs, word cloud, and a chart for the selected entry of the overview.
- New "About OpenSpider" dialog with developer, website and source code links.
- Docker: a Dockerfile for running the CLI, exports and the MCP server on Linux servers (build the image yourself, see the user guide).
- Fixes: a stopped crawl stays "Stopped" after a restart; the status card shows "Finished (limit reached)" like the status bar; the findings sidebar follows the UI language; repairing a crawl file no longer counts write errors as lost rows; more reliable "Check for updates"; updated spreadsheet import library (security advisory RUSTSEC-2026-0436).

### Deutsch

- Große Crawls rund 30 % schneller, Link-Score-Analyse bei 1 Mio. URLs von 113 auf 23 s.
- Chromium für JavaScript-Rendering per Klick, kein installiertes Chrome mehr nötig.
- Claude Desktop, Claude Code, Codex oder Cursor mit einem Klick verbinden; Agenten dürfen standardmäßig nur lesen.
- Neuer Bericht „Änderungen seit dem letzten Crawl“ zum Prüfen von Deployments, dazu Absturzberichte nur mit Zustimmung.

## [1.0.0-beta.2] - 2026-11-03

Wartungsversion ohne neue Funktionen: Sie prüft die automatische Aktualisierung von beta.1 auf eine neuere Version.

- App-Funktionen unverändert gegenüber beta.1.
- Bekannt: Der Chrome-Download auf Klick für JavaScript-Rendering folgt mit beta.3 (statt wie angekündigt mit beta.2).

## [1.0.0-beta.1] - 2026-11-03

Erste offene Beta. Rückmeldungen bitte über Hilfe → „Feedback geben…“ oder https://openspider.de/beta/.

- Desktop-App für macOS (Apple Silicon und Intel, signiert und notarisiert) und Windows x64 (signiert über Azure Artifact Signing).
- Automatische Update-Prüfung (abschaltbar) mit Download im Hintergrund; Installation sofort oder beim Beenden, nie während eines laufenden Crawls.
- Live-Crawl flüssiger: gemeinsame, gedrosselte Zählung für Übersicht, Probleme und Filter; Übersicht zeichnet nur geänderte Zeilen neu.
- „Top-Probleme“ und „Befunde“ laden nach Crawl-Ende schnell; Pause, Fortsetzen und Beenden halten die Crawl-Datei sauber.
- Alte Crawls und Einstellungen werden beim Öffnen übernommen.
- Bekannt: JavaScript-Rendering nutzt in dieser Beta ein installiertes Chrome; der Download auf Klick folgt mit beta.2.

## [1.0.0-beta.0] - 2026-10-30

- Technischer Trockenlauf der Release-Orchestrierung: Desktop-Installer, CLI-Archive, Update-Manifeste und SHA-256-Prüfsummen.
- Dieser Stand dient der Prüfung des Release-Prozesses. Kein freigegebener Produktionsstand.
