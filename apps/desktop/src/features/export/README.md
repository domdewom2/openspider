# M2-U16 – Exportdialoge

Die Oberfläche nutzt die bestehenden IPC-Verträge für Ansichten, Details,
Seitenleisten, Bulk-Exporte, Berichte, Sitemaps und Mehrfachexport. CSV/XLSX sind
verfügbar; XLS und Google Sheets werden vom aktuellen Formatvertrag nicht angeboten.
Namen einschließlich Crawl-Startdatum und Segmentsuffix liefert ausschließlich
`export.suggestName`. Der Client lädt keine Exportzeilen in den Arbeitsspeicher.

## Einbindung für die Regionsbesitzer

`ExportHost` ist einmal im Hauptfenster gemountet. `ExportViewButton` liegt über
der Haupttabelle. U10 veröffentlicht den jeweils aktiven Stand mit
`registerExportContext("view", { kind: "view", tab, filter, viewId, version, columns })`.
Der Rückgabewert meldet genau diesen Stand wieder ab. Bei einem Versions- oder
Spaltenwechsel wird der neue Stand veröffentlicht. Mod+E verwendet denselben Stand.

U11/U12 können `ExportButton` direkt in die jeweilige tabellarische Detailansicht
bzw. Seitenleiste setzen und ihren Kontext über `registerExportContext` für
`export.detail`/`export.sidebar` bereitstellen. Die Detailauswahl ist das bestehende
`Selection`-DTO einschließlich `all`/`except`; ein fehlender Datenbestand wird über
`disabledReason` erklärt. `openExport(request)` kopiert den Stand beim Öffnen,
sodass spätere UI-Änderungen einen geöffneten Export nicht verändern.

Die Paketbasis enthält noch keine U10/U11/U12-Regionsoberflächen. Daher ist der
Haupttabellenknopf ohne veröffentlichten Kontext deaktiviert. Diese öffentlichen
Anschlüsse sind gegen Fixtures getestet; es werden keine Regionszustände erfunden.

## Spec-Abdeckung

- OUT-006: Optionen, native Speichern-/Ordnerdialoge, persistente Format-/Ordnerwahl,
  Betriebssystem-Überschreibdialog für einzelne Dateien, Fortschritt/Abbruch,
  Fehler und „Ordner öffnen“ nach bestätigtem Erfolg.
- UI-015: Schaltfläche über der Tabelle, wiederverwendbarer Detailknopf, Mod+E.
- OUT-020 [M2]: unveränderte View-Version, sichtbare Spaltenreihenfolge/alle Spalten,
  Dateinamensvorschlag des Kerns, Upload-Reihenfolge.
- OUT-021 [M2]/[P1]: alle ausgewählten URLs, Detail-Spec samt Filter/Sortierung,
  deaktivierte Exporte mit Erklärung.
- OUT-007: Konsolidierung nur bei XLSX, Auswahlreihenfolge unverändert.
- OUT-075: durchsuchbarer Auswahlbaum, Presets laden/speichern/löschen,
  unbekannte/nicht verfügbare Einträge mit Warnung, leere Exporte überspringen.
- OUT-124: Priority 0–1 in Schritten von 0,1, editierbare Tiefenwerte,
  Listenmodus-Hinweis.
- OUT-128: hreflang standardmäßig aus, Filter auf gültige Ziele an,
  ohne `spider.hreflang.store` deaktiviert.

## Fremdergänzungen (K2-61)

Der vollständige, unveränderte Git-Diff aller drei Ergänzungen liegt in
[`K2-61.patch`](K2-61.patch).

Keine F05-Stubs wurden ersetzt; K2-60 wird nicht verwendet. Keine fremden Tests,
Goldens oder Fixtures wurden geändert. Folgende fehlende Einhängepunkte wurden
strikt additiv ergänzt:

1. `apps/desktop/src/App.tsx` – Import und zwei zusätzliche Komponentenaufrufe:

   ```diff
   +import { ExportHost, ExportViewButton } from "./features/export";
        <Providers>
   +      <ExportHost />
          ...
          <main ...>
   +        <ExportViewButton />
   ```

2. `apps/desktop/src-tauri/src/desktop.rs` – neue Funktionen und eigenes Testmodul: `open_export_folder(State<App>, String)` delegiert nach Prüfung
   eines existierenden absoluten Pfads an `App::desktop_actions().reveal_paths`;
   `export_output_path` führt die Prüfung im Hintergrund aus. Zusätzlich ein neuer
   eigener OUT-006-Test mit der Paketfixture. Bestehende Funktionen unverändert.

   ```diff
   +#[tauri::command]
   +pub async fn open_export_folder(app: tauri::State<'_, App>, path: String) -> Result<(), CmdError>
   +fn export_output_path(path: &str) -> Result<PathBuf, CmdError>
   +#[cfg(test)] mod export_tests { /* OUT-006: Fixturepfad, relativ, fehlend */ }
   ```

3. `apps/desktop/src-tauri/src/lib.rs` – ausschließlich zusätzlicher Eintrag in
   der vorhandenen Command-Registrierung:

   ```diff
   -tauri::generate_handler![app_version]
   +tauri::generate_handler![app_version, desktop::open_export_folder]
   ```

Der Zusatzcommand liegt in der nativen Hülle, weil der F05-Vertrag keinen
Exportpfad an die vorhandene Desktop-Aktion weiterleitet. Der gemeinsame
os-app-Commandkatalog bleibt unverändert. Der Frontend-Aufruf läuft über den
vorhandenen IPC-Client; es gibt keinen zweiten Tauri-Transport und keinen Shellaufruf.

## Prüfungen und Einschränkungen

Befehle und abschließende Ergebnisse stehen im Abgabebericht. Der isolierte
Browserlauf lautet:

```sh
npm run -w apps/desktop e2e -- --config e2e/export/playwright.config.ts --project=chromium --project=webkit
```

Der eigene Testserver nutzt einen dynamischen Loopback-Port. Die aktiven
Browsertests verwenden Exportverträge mit deterministischen Antworten und einem
vorgegebenen Zielpfad. `bridge.spec.ts` enthält den vollständigen Test gegen den
realen Kern und ist nach C9 mit `wartet auf M2-K04/M2-K06` markiert; N01 entfernt
nur diese Markierung, sobald die bisher noch vorhandenen F05-Stubs ersetzt sind.
Native OS-Fenster sind durch Browsertests nicht nachgewiesen.

Das aktuelle Multi/Bulk/Report-DTO akzeptiert ein Zielverzeichnis und feste
Backend-Dateinamen. Per Exportauftrag abweichende Überschreib-/Zeitstempel- oder
Drive-Einstellungen werden nicht angeboten; dafür fehlen Felder im Vertrag.
Sitemapoptionen werden über die bestehende Crawl-Konfiguration gespeichert,
bevor `export.sitemap` startet. Protokollaufteilung und XML-Ausgabe gehören X02.

CI-Lauf-ID: keiner; gemäß Auftrag kein Push, Merge oder PR. Plattform: macOS arm64.
Offene fachliche Fragen: keine. Integrationsnachweis: C9-Test nach K04/K06 ausführen;
Regionsbesitzer verbinden ihre Kontexte über die oben dokumentierte API.

## Validierungsprotokoll (18.10.2026)

| Prüfung | Ergebnis / Laufzeit |
| --- | --- |
| `npm ci` | grün, 209 installierte Pakete, 0 Audit-Funde; 2 s |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | 162 Tests grün, davon 20 neue Exporttests; 14,82 s |
| `npm run -w apps/desktop check:static` | grün; 28 statische/Token-Tests, 263 Pakete mit erlaubter Lizenz; 8,51 s für die Tests |
| Export-Playwright, Chromium + WebKit | 14 grün, 2 C9-Markierungen; 21,1 s |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` | grün; 3 min 21 s |
| `cargo clippy -p openspider-desktop --all-targets --locked -- -D warnings` | grün; 6 min 2 s |
| `cargo test -p openspider-desktop --lib --locked` | 2 Tests grün einschließlich OUT-006-Pfadfixture; Build 2 min 52 s |
| `cargo deny check licenses bans sources` | grün |
| `bash scripts/check-neutral.sh` | keine Fundstellen; vier bestehende Warnungen über verwaiste Ausnahmen in der CI-Allowlist |

`cargo test --workspace --exclude openspider-desktop --locked`: grün; 2307 Tests bestanden, 40 vorhandene Ignores, keine Fehler. Summe der gemeldeten Laufzeiten der Testbinaries/Doctests: 347.73 s (ohne Build).
