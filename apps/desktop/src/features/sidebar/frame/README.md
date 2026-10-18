# Seitenleiste (M2-U12)

`SidebarMount` hängt den Rahmen in das vorhandene Seitenleisten-Landmark und den
Analysefortschritt in die Statusleiste ein. `App.tsx` enthält dafür lediglich einen
zusätzlichen Import und Aufruf (K2-61). Der Layout-Besitzer kann später
`SidebarFrame` und `AnalysisProgress` unmittelbar komponieren. Sichtbarkeit und
Geometrie des gesamten Landmarks bleiben beim Shell-/Layout-Paket.

Weitere Seitenleisten-Pakete exportieren aus `features/sidebar/<bereich>/index.tsx`
einen `panel: SidebarPanel` mit `id`, `titleKey`, `Component` und optional
`embeddingsOnly`. Die Fluent-Ressourcen liegen beim jeweiligen Feature.
`SidebarFrame.embeddingsEnabled` schaltet Embedding-Panels frei. Die nicht
implementierten Fachpanels zeigen einen Hinweis und rufen keine Stubs auf.

## Navigation und Bereich

`useSidebarNavigation` ist der Übergabepunkt an die obere Tabelle:

- `scope` enthält den aktuellen Segmentfilter und die Vergleichsklasse.
  Der Tabellen-/Segment-Besitzer ruft `setScope(scope)` auf, wenn sich diese ändern.
- `target.spec` ist eine vollständige Tab-`ViewSpec`, `scrollToTop` fordert die erste
  Zeile an, `focus` den Tabellenfokus (Doppelklick). `revision` ändert sich auch bei
  erneuter Auswahl desselben Filters. Der Tabellen-Besitzer abonniert diesen Store.
- Zusätzlich wird `os:table-navigate` mit demselben Ziel auf `document` gemeldet.
  Scroll und Fokus am Tabellen-Landmark funktionieren bereits mit dem Gerüst.
- Segmentbalken setzen denselben `scope`. Weitere Fachpanels verwenden den Store;
  API-Panels rufen ihre bereichsunabhängigen Commands ohne `scope` auf.

Overview/Issues verwenden die vom Kern gelieferten Anzahlen und Prozente. Nur die
Summary-Prozente werden entsprechend UI-061 auf Encountered bezogen. Die Filter-
Zustände einschließlich `needsRecrawl` und `incomplete` bleiben sichtbar.
`analysisStep`/`stale` werden vom Kern bestimmt; Segmentwechsel starten keine Analyse.

Alle Abfrageschlüssel enthalten Crawl, Bereich und die passenden Tick-Versionen;
inaktive Versionsstände werden sofort aus dem Query-Cache entfernt. Erklärungen
werden als Text aus `sidebar.issueExplain` gerendert und bei Sprach-/Issue-Version-
Wechsel erneut geladen. Es gibt keine duplizierten Issue-Texte in Fluent.

Der Rahmen speichert ausschließlich `state.sidebar` über `uistate` und erhält die
anderen Namensräume. Kopieren liefert die Tabellenzeilen des aktiven Panels. Export läuft
über `export.issues` bzw. `export.sidebar`, einschließlich der Kern-Erklärungen.
Der bestehende `ExportPanelArgs`-Vertrag hat keine Such-/Bereichsargumente; der
Export verwendet damit den vom Kern definierten Panelumfang.

Die Browser-Vertragstests verwenden `fx-ui-basic.json` und die vorhandene
WebSocket-Testbridge. Die echten `sidebar.*`-/`analysis.*`-Commands sind auf der
Integrationsbasis noch K06-/K04-Stubs; eine fachliche Kernintegration ist daher
nach deren Merge zu prüfen. U10 ist für die Übernahme des Navigationsziels in die
sichtbare Tabelle zuständig.
