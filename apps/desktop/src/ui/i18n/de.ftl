## Gemeinsame UI-Texte: Dialoge, Fehler, Leerzustände, Farben, Kürzel, Menütitel

ui-dialog-close = Schließen
ui-dialog-cancel = Abbrechen
ui-dialog-ok = OK
ui-dialog-apply = Übernehmen
ui-dialog-reset = Zurücksetzen
ui-dismiss = Ausblenden
ui-clear-search = Suche leeren
ui-region-reload = Bereich neu laden
ui-region-failed = Dieser Bereich konnte nicht angezeigt werden.
ui-search-placeholder = Suchen
ui-toasts = Benachrichtigungen

## Fehler (Fehlerklassen des Kerns, messageKey von CmdError)
err-cancelled = Der Vorgang wurde abgebrochen.
err-stale = Die Daten sind veraltet.
err-internal = Es ist ein unerwarteter Fehler aufgetreten.
err-unsupported = Diese Funktion ist noch nicht verfügbar.
err-bridge-closed = Die Verbindung zum Kern wurde geschlossen.
err-code-invalidinput = Die Eingabe ist ungültig.
err-code-notfound = Der Eintrag wurde nicht gefunden.
err-code-stale = Die Daten sind veraltet.
err-code-cancelled = Der Vorgang wurde abgebrochen.
err-code-busy = Der Kern ist beschäftigt. Bitte erneut versuchen.
err-code-timeout = Die Zeit für den Vorgang ist abgelaufen.
err-code-io = Ein Dateivorgang ist fehlgeschlagen.
err-code-permission = Zugriff verweigert.
err-code-unsupported = Diese Funktion ist noch nicht verfügbar.
err-code-nocrawl = Es ist kein Crawl geöffnet.
err-code-crawlrunning = Es läuft ein Crawl.
err-code-store = Die Crawl-Datei konnte nicht gelesen werden.
err-code-engine = Die Crawl-Engine hat einen Fehler gemeldet.
err-code-internal = Es ist ein unerwarteter Fehler aufgetreten.

## Leer- und Voraussetzungszustände (UI-016)
empty-no-selection = Wähle in der oberen Tabelle eine Zeile aus, um ihre Details zu sehen.
empty-mode-unsupported = Diese Ansicht ist im aktuellen Modus nicht verfügbar.
empty-needs-js-rendering = Nur im JavaScript-Rendering-Modus verfügbar. Stelle den Rendering-Modus in den Einstellungen um.
empty-feature-disabled = { $arg ->
    [cookies] Cookies werden nicht erfasst. Schalte die Cookie-Erfassung in den Einstellungen ein.
    [http_headers] HTTP-Header werden nicht gespeichert. Schalte das Speichern der Header in den Einstellungen ein.
    [accessibility] Die Barrierefreiheitsprüfung ist ausgeschaltet. Schalte sie in den Einstellungen ein.
    [spelling] Die Rechtschreib- und Grammatikprüfung ist ausgeschaltet. Schalte sie in den Einstellungen ein.
    [near_duplicates] Die Erkennung ähnlicher Inhalte ist ausgeschaltet. Schalte sie in den Einstellungen ein.
    [structured_data] Die Extraktion strukturierter Daten ist ausgeschaltet. Schalte sie in den Einstellungen ein.
   *[other] Die Funktion „{ $arg }“ ist ausgeschaltet. Schalte sie in den Einstellungen ein.
}
empty-html-not-stored = Der HTML-Quelltext wird nicht gespeichert. Aktiviere „HTML speichern“ in den Einstellungen.
empty-rendered-html-not-stored = Das gerenderte HTML wird nicht gespeichert. Aktiviere „Gerendertes HTML speichern“ in den Einstellungen.
empty-api-not-connected = { $arg ->
    [gsc] Die Search Console ist nicht verbunden. Verbinde sie, um diese Daten zu sehen.
    [ga4] Google Analytics 4 ist nicht verbunden. Verbinde es, um diese Daten zu sehen.
    [psi] PageSpeed Insights ist nicht verbunden. Verbinde es, um diese Daten zu sehen.
   *[other] Die Datenquelle „{ $arg }“ ist nicht verbunden. Verbinde sie, um diese Daten zu sehen.
}
empty-needs-crawl-analysis = Der Teilschritt „{ $arg }“ der Crawl-Analyse ist noch nicht gelaufen oder veraltet. Starte die Crawl-Analyse.
empty-no-data-yet = Noch keine Daten. Starte einen Crawl oder öffne eine Crawl-Datei.
empty-no-match = Keine Zeile passt zum aktuellen Filter.
empty-not-applicable = Für diesen URL-Typ gibt es keine Daten dieser Art.
empty-action-open-setting = Einstellung öffnen
empty-action-start-analysis = Crawl-Analyse starten
empty-action-clear-search = Suche aufheben
empty-action-connect-api = Verbinden

## Farben
color-green = Grün
color-blue = Blau
color-teal = Petrol
color-violet = Violett
color-pink = Rosa
color-red = Rot
color-orange = Orange
color-olive = Oliv
color-custom = Eigene Farbe
color-invalid = Gib die Farbe als #rrggbb ein.
color-low-contrast = Der Kontrast von { $ratio }:1 zum Hintergrund liegt unter 3:1. Markierungen erhalten zusätzlich einen Rahmen.

## Menütitel (UI-002)
menu-app = OpenSpider
menu-file = Datei
menu-edit = Bearbeiten
menu-config = Konfiguration
menu-mode = Modus
menu-bulk-export = Bulk-Export
menu-reports = Berichte
menu-sitemaps = Sitemaps
menu-visualisations = Visualisierungen
menu-crawl-analysis = Crawl-Analyse
menu-view = Ansicht
menu-mcp = MCP
menu-window = Fenster
menu-help = Hilfe

## Tastaturkürzel (UI-113), Beschriftungen in Menüs und im Hilfe-Dialog
shortcut-crawl-start = Crawl starten oder fortsetzen
shortcut-crawl-pause = Crawl pausieren
shortcut-crawl-new = Neuer Crawl
shortcut-crawl-open = Crawl öffnen
shortcut-crawl-save-as = Crawl speichern unter…
shortcut-app-settings = Einstellungen
shortcut-config-spider = Spider-Konfiguration
shortcut-config-diff = Konfigurations-Diff
shortcut-search-focus = Suchfeld fokussieren
shortcut-search-clear = Suche aufheben
shortcut-table-copy = Kopieren
shortcut-table-select-all = Alles markieren
shortcut-export-view = Aktuelle Ansicht exportieren
shortcut-toptab-next = Nächster Top-Tab
shortcut-toptab-prev = Vorheriger Top-Tab
shortcut-toptab-goto-1 = Top-Tab 1
shortcut-toptab-goto-2 = Top-Tab 2
shortcut-toptab-goto-3 = Top-Tab 3
shortcut-toptab-goto-4 = Top-Tab 4
shortcut-toptab-goto-5 = Top-Tab 5
shortcut-toptab-goto-6 = Top-Tab 6
shortcut-toptab-goto-7 = Top-Tab 7
shortcut-toptab-goto-8 = Top-Tab 8
shortcut-toptab-goto-9 = Top-Tab 9
shortcut-filter-open = Filter-Dropdown öffnen
shortcut-focus-next = Fokus wechseln: Tabelle, Details, Seitenleiste
shortcut-focus-prev = Fokus rückwärts wechseln
shortcut-view-toggle-detail = Detailfenster ein- oder ausblenden
shortcut-view-toggle-sidebar = Seitenleiste ein- oder ausblenden
shortcut-nav-back = Zurück
shortcut-nav-forward = Vor
shortcut-browser-open = Im Browser öffnen
shortcut-crawl-recrawl = Erneut crawlen
shortcut-table-remove = Entfernen
shortcut-analysis-start = Crawl-Analyse starten
shortcut-source-next-diff = Nächster Unterschied
shortcut-source-prev-diff = Vorheriger Unterschied
shortcut-zoom-in = Vergrößern
shortcut-zoom-out = Verkleinern
shortcut-zoom-reset = Zoom auf 100 % zurücksetzen
shortcut-context-menu = Kontextmenü
