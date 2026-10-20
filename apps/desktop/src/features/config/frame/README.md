# Konfigurationsdialog – Rahmen (M2-U13)

Modaler In-App-Dialog „Crawl-Konfiguration“ (M2-ui-concept 1.5 / W3). Der Rahmen hält den Entwurf, prüft über den Kern
(`config.validate`), schreibt bei „OK“ mit `config.set` und verwirft bei „Abbrechen“. Die Seiten liegen in
`features/config/pages/<id>/index.tsx`.

## Einbindung

- `ConfigHost` (einmal im Hauptfenster, `App.tsx`): Aktionen `config.spider` (Mod+K), `config.profiles`, `config.diff`
  (Mod+Umschalt+C), Menübeitrag „Konfiguration“, Dialog.
- `openConfig({ anchor?, diff? })` öffnet den Dialog; `anchor` ist ein TOML-Pfad (`rendering.mode`, `extraction.store_html`,
  `speed.threads`) oder ein Seitenpräfix (`rendering`). Das Ziel-Steuerelement erhält den Fokus (UI-016).
  Bestehende Aufrufer von `openSettingsWindow(<Konfigurationsanker>)` (Benachrichtigungen, Seitenleiste) wechseln beim
  Integrationspaket (N01) auf `openConfig`.

## Seitenvertrag (für M2-U13B und weitere Pakete)

Ein Modul `pages/<id>/index.tsx` exportiert `page` oder `pages` (`ConfigPage`, siehe `types.ts`). Der Rahmen findet es per
`import.meta.glob`, ohne dass sich der Rahmen ändert. Die Kennungen der erwarteten Seiten stehen in `KNOWN_PAGES`
(`custom-search`, `custom-extraction`, `custom-js`, `link-positions`, `api`, `segments`, `analysis`, `change-detection`);
bis das Modul existiert, zeigt der Rahmen einen Platzhalter. Die Seitentitel `config-page-<id>` und die Gruppen
`config-group-<gruppe>` sind im Rahmen bereits übersetzt – nicht erneut definieren (Fluent-Schlüssel sind global eindeutig).

- `definePage({ id, group?, order, fields, anchors?, Before?, After? })` erzeugt Überschrift, Feldliste und Hooks für eigene
  Bereiche. Felder: `FieldDef { path, kind?, hintKey?, sectionKey?, unitKey?, nullLabelKey?, hidden? }`.
- Beschriftung eines Feldes: `config-f-<pfad>` (Punkte und Unterstriche werden zu `-`), Aufzählungswerte
  `config-e-<pfad>-<wert>`. Typ, Grenzen und Aufzählungen liest der Rahmen aus `config.schema`; Fehler je Pfad kommen aus
  `config.validate` und erscheinen am Feld (`FieldFrame`).
- Eigene Steuerelemente lesen mit `useConfigValue(path)` den Wert, die Sperre (`lock`) und die Meldungen des Kerns und
  schreiben mit `set`. Pfade, die die Seite selbst zeichnet, stehen mit `hidden: true` in `fields` (Suche, Sprungziel).
- Seitenmodule importieren aus den Einzeldateien des Rahmens (`../../frame/fields`, `…/context`, `…/definePage`), nie aus
  `frame/index.ts` (Importzyklus über das Register).

## Sperren (CRAWL-126, F-13/F-14)

`lockReason(path, runMode)` in `model.ts`: fremd gestartet → alles gesperrt; `mode`, `rendering.mode`, `scope.all_subdomains`,
`scope.subfolder_only`, `scope.crawl_outside_start_folder` → „neuer Crawl nötig“ (laufend und pausiert); während „Läuft“
bleiben `PRESET_KEYS`, `duplicates.*`, `links.*`, `segments.*`, `analysis.*`, `speed.threads`, `issues.ignore_non_indexable`
editierbar, der Rest ist „Crawl pausieren, um zu ändern“. Die Liste ist ein Abbild von `live_field`/`check_state` in
`crates/os-app/src/commands/config.rs`; ändert sich dort etwas, ist `model.ts` nachzuführen.

## Testkern

`testing.ts` liefert einen Mock der `config.*`-/`profile.*`-Commands, dessen Werkseinstellungen aus
`docs/spec/schemas/osconfig.schema.json` stammen (vitest und `e2e/config/harness.tsx`). Das Verhalten gegen den echten Kern prüft
`e2e/config/bridge.spec.ts`.
