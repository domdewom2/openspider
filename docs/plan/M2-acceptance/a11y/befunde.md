# Befunde M2-U30 · UI-126

Basis: Integrationsrunde 15, `27f92ff1`, Linux; Chromium und WebKit headless. Fremde Produktdateien und Manifeste wurden nicht geändert. Eigene Regressionstests bleiben bei einem offenen Befund rot; es gibt keine Regelabschaltung, keine erwarteten Fehler und keine Ignores zum Verdecken eines Befunds.

## U30-F01 · zusätzlicher Grid-Tabstopp in Chromium · offen, N-Paket

- Datei: `apps/desktop/src/grid/DataGrid.tsx`, Scrollcontainer `.os-grid-scroll` innerhalb von `.os-data-grid` (außerhalb `src/grid/a11y/**`).
- Element: scrollbares, mit `aria-hidden="true"` markiertes `div`, ohne expliziten `tabindex`.
- Nachweis: `apps/desktop/e2e/a11y/grid.spec.ts`, virtuelle Fixture mit 5 000 Zeilen. `aria-rowcount=5001`, letzte Zeile `aria-rowindex=5001` und aktive Zelle sind korrekt; ein Tab nach dem Grid erreicht in Chromium nicht unmittelbar „After grid“. WebKit besteht denselben Test.
- axe-Regel: kein gemeldeter critical/serious-axe-Verstoß; Tastaturbefund zu UI-126, Bezug `aria-hidden-focus` (fokussierbarer, für AT versteckter Container). Der Test prüft die tatsächliche Tabfolge unabhängig von axe.
- Vorschlag: im N-Paket den rein visuellen Scrollcontainer explizit aus dem Tab-Zyklus nehmen (`tabIndex={-1}`), Grid als einzigen Einstieg behalten. Native Scrollleisten/Scrollen und Tab/Shift+Tab in beiden Engines gegenprüfen. Keine globale CSS- oder Fokusumleitung als Umgehung.

## U30-F02 · Workspace-Clippy durch gelockte Desktop-Abhängigkeit blockiert · offen, N-/Integrationspaket

- Dateien: `Cargo.lock` (`tauri-plugin-deep-link 2.6.2`, `rust-ini 0.21.3`), Abhängigkeitscode `tauri-plugin-deep-link/src/lib.rs:539`.
- Element / axe-Regel: entfällt, Build-Befund.
- Nachweis: `cargo clippy --workspace --all-targets --locked -j 6 -- -D warnings` scheitert mit E0063: Initialisierung von `ini::ParseOption` ohne `enabled_indented_mutiline_value` und `enabled_preserve_key_leading_whitespace`.
- Kontrolllauf: derselbe Clippy mit `--exclude openspider-desktop` ist grün. Kein Rust-Code in U30 geändert; Tokio bleibt 1.53.3.
- Vorschlag: zuständiges Manifest-/Integrationspaket gleicht die beiden Versionen gezielt ab bzw. übernimmt einen geprüften Upstream-Fix mit Lizenzprüfung; anschließend vollständigen Workspace-Clippy wiederholen. U30 führt kein `cargo update` aus.

## Behobene Befunde im Besitz von U30

- `ui/primitives/ComboBox.tsx`: Filter-Popup meldete `aria-hidden-focus` (serious), weil die RAC-ComboBox Hintergrundbereiche für AT verbarg, deren Tabstopps jedoch aktiv blieben. Nichtmodale Eingabe mit `aria-activedescendant`, eigener Optionsnavigation und bestehendem RAC-Popover; Tab/Blur schließt ohne Fokusfalle. Drei Unit-Regressionen und Browserprüfungen für Filter, Auswahl, Escape und Fokusrückgabe.
- `ui/primitives/Button.tsx`, `contrast.css`: Primärbutton im dunklen Forced-Colors-Modus von Chromium meldete `color-contrast` (serious; etwa 1,2:1). Explizite Systemfarben `Canvas`/`CanvasText`, deaktiviert `GrayText`.
- `ui/primitives/{Tabs,TabBar}.tsx`, `contrast.css`: WebKits emulierte Kombination Highlight/HighlightText erreichte auf ausgewählten Tabs nur etwa 3,77:1 (`color-contrast`, serious). Systemtext auf Systemcanvas; Auswahl bleibt zusätzlich durch Schriftstärke und Rahmen erkennbar, Fokus durch eigenen Rahmen. Dasselbe gilt für fokussierte Menü-/Listeneinträge.

## U30-F03 · Issues: mittlere Priorität im hellen Theme · offen, N-Paket

- Dateien: `apps/desktop/src/features/sidebar/frame/sidebar.css:17`, `apps/desktop/src/features/sidebar/issues/Issues.tsx`.
- Elemente: `.os-sidebar-priority-medium`, Badge-Text und Prioritätsspalte in der Issue-Tabelle.
- axe-Regel: `color-contrast`, serious. In beiden Browsern gemeldet; Zahlenbeispiel Chromium: `#a86a00` auf `#ffffff` ergibt etwa 4,44:1 statt 4,5:1 bei 11/13 px.
- Nachweis: `views.spec.ts`, Scan `Crawl summary-issues-light`; kein Ausschluss des Panels.
- Vorschlag: zuständiges N-Paket verwendet eine kontraststärkere Textfarbe für mittlere Priorität (mindestens 4,5:1, mit Reserve); Badge und Priorität weiterhin als Text ausgeben. Keine globale Änderung des Status-Tokens aus U30.

## U30-F04 · Segment-Buttons im dunklen Theme · offen, N-Paket

- Dateien: `apps/desktop/src/features/sidebar/segments/Segments.tsx:51`, `apps/desktop/src/features/sidebar/frame/sidebar.css`.
- Elemente: „Configure segments“ und „Create segments“, Selektoren `.os-sidebar-actions > button` und `.os-sidebar-empty > button`.
- axe-Regel: `color-contrast`, serious. In beiden Browsern gemeldet; Zahlenbeispiel Chromium: geerbter Text `#e7e9ec` auf nativem Button-Hintergrund `#6b6b6b` ergibt etwa 4,38:1 bei 13 px.
- Nachweis: `views.spec.ts`, Scan `Crawl summary-segments-dark`.
- Vorschlag: Feature im N-Paket auf den gemeinsamen `Button` umstellen oder Vorder- und Hintergrund explizit über passende Theme-Tokens setzen; zusätzlich Forced Colors prüfen. Keine Feature-Selektoren als Umgehung in Primitive-CSS ergänzt.

Weitere behobene Primitive-Befunde: sichtbares Häkchen einer abgewählten Checkbox nach Forced-Colors-Ersetzung (Chromium), unzureichender Kontrast des Akzent-Badges im dunklen Forced-Colors-Modus, Systemkontur für Switches und Farbauswahl. Die bestehende Polish-Matrix deckte außerdem vertikale Schriftkappung auf; ganze 20-px-Zeilenboxen in TabBar beheben sie ohne Änderung der Tab-Höhe. Regressionen liegen in `contrast.spec.ts` und `keyboard.spec.ts` (UI-126) sowie in der unveränderten Polish-Matrix.

## Weitere axe-Hinweise unterhalb der Abnahmeschwelle

- `region` (moderate): `.os-shell-divider-sidebar` und `.os-shell-divider-detail`, Datei `features/shell/Divider.tsx`; benannte Separatoren liegen außerhalb eines Landmarks. Vorschlag: zuständiger Feature-Rahmen ordnet sie einer sinnvoll benannten Region zu.
- `landmark-one-main`, `page-has-heading-one`, `region` (moderate): Einstellungsfenster, Datei `features/settings/frame/SettingsFrame.tsx`; Hauptbereich/Seitentitel fehlen im Dokumentrahmen. Vorschlag: Feature-Rahmen um semantischen Hauptbereich und Fenstertitel als h1 ergänzen.
- `region` (moderate): temporäres, benanntes Filter-Listbox-Popup im eigenen ComboBox-Primitive liegt im Portal außerhalb der Haupt-Landmarks. Kein critical/serious-Befund und keine Fokusfalle; keine zusätzliche Landmark-Rolle nur zur Unterdrückung dieses Hinweises vergeben.
