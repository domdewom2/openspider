# M2-U01 – Shell und Layout

Umgesetzt: **UI-001**, **UI-125 [P1]**, **UI-021 [P1] (Zustandsteil)**.
Clean-Room: ausschließlich Repo-Spezifikation und vorhandene Schnittstellen; kein Zugriff auf das Referenzprodukt.
Ausgangs-Worktree auf `m2/u01`: sauber, keine Änderungen eines früheren Laufs.

## Verhalten

- Dreiteiliges Fenster mit vorhandenen Header-/Tabellen-/Seitenleisten-/Status-Landmarks und zusätzlicher Region „Details“.
- Standardbreiten/-höhen nach UI-001, bewegliche Trenner, strikte Halbminimum-Schwelle, 6-px-Griffe, Wiederherstellen der letzten geöffneten Größe, Menüaktionen und unabhängiger Doppelklick-Reset. Pointer-Capture, Abbruch und Tastaturbedienung; die Tabelle behält mindestens 200 px Inhalts-Höhe zusätzlich zur Tab-Leiste.
- Anteile bleiben bei Resize erhalten, inklusive der Unterscheidung zwischen Einklappen und vollständigem Ausblenden. Erster Start unter 1280 px klappt die Seitenleiste ein. Die vorhandene native Mindestfenstergröße 1024×640 bleibt maßgeblich.
- Versionierter Zustand über `uistate.load/save`, Validierung/Migration mit zod, serieller Schreibpfad, 1-s-Debounce und explizites Flush. Fehler verhindern das Überschreiben eines ungelesenen Dokuments und sind wiederholbar. Atomare Dateien und `.bak` liefert die vorhandene Service-Schicht.
- Native Fenstergröße/-position werden wiederhergestellt und erfasst. Entfernte Monitore und unterschiedliche DPI-Faktoren werden berücksichtigt; minimierte/maximierte Größen ersetzen die normale Fenstergeometrie nicht.
- Globale Spalten-Rückfrage mit Abbrechen; sofortiges Leeren der Layouts aller Tabellen und Persistenz. Benannte Vorlagen und alle anderen Zustandsbereiche bleiben erhalten.
- Gemeinsamer Beenden-Dialog für Menüaktion und native Bestätigungsnachricht, Standardfokus „Pausieren und beenden“, explizites `app.confirmQuit({value:false/true})`, Fehler bleiben im Dialog bedienbar. Die laufenden Arbeiten und Pause/Checkpoint/Prozessende implementiert M2-K04.

## Zustandsvertrag für Folgepakete

`state/layout.ts` bietet `useLayout`, `createLayoutStore`, `persistLayout`, `load`, `setLayout`, `setSection`, `resetColumns`, `resetAll` und `flush`.
Vor Änderungen `loaded` abwarten. UI-Daten werden als gemeinsames Dokument mit `version: 2` gespeichert:

- `layout.sidebar/detail`: `{share, visible, collapsed}`; `share` bezieht sich auf gesamte Fensterbreite bzw. Inhaltshöhe zwischen Header und Status.
- `window`: physische Desktop-Position `x/y`, logische Inhaltsgröße `width/height`, `scaleFactor`.
- `tables[tableId]`: ausschließlich aktive Spaltenlayouts aller Top-, Detail- und Seitenleisten-Tabellen, einschließlich Sortierung/Anheftung. Ein fehlender Eintrag bedeutet Auslieferungsstandard. Folgepakete abonnieren diesen Slice für sofortiges Aktualisieren.
- `columnTemplates`: benannte Vorlagen, getrennt von `tables`. Weitere Slices (Tabs, Filter, Verlauf, Visualisierung) bleiben beim Speichern und Spalten-Reset erhalten. Die vorhandenen Erscheinungsbild-/Spracheinstellungen werden weiterhin von `settings.*` verwaltet.
- v1-Pixelmigration: `layout.sidebarWidth/detailHeight` plus `window.width/contentHeight`, optionale `<pane>Visible/Collapsed`; sonst validierte Standardwerte. Unbekannte Layoutfelder werden ignoriert, fremde Dokument-Slices erhalten.

`view.toggleDetail`, `view.toggleSidebar`, `view.resetColumns` sind gemeinsame Aktionen. Kontextmenüs und Einstellungsoberfläche können `view.resetColumns` aufrufen. Das Rendern der eigentlichen Tabellen/Details/Seitenleiste gehört U10/U11/U12; U01 stellt Layout und Zustandsvertrag.

## Fremdergänzungen (K2-61)

Ausschließlich additive Ergänzungen; keine bestehenden Signaturen, Testdateien oder Stub-Listen geändert. Keine neu erzeugten fremden Goldens/Fixtures. K2-60 wurde nicht benötigt.

`apps/desktop/src/App.tsx`: zusätzlicher Shell-Initialisierungsaufruf im bestehenden App-Gerüst:

```diff
+import { Shell } from "./features/shell/Shell";
 ...
       <div className="os-app">
+        <Shell />
```

`apps/desktop/src-tauri/capabilities/default.json`: zusätzliche Einträge für den nativen Geometrieadapter:

```diff
+    "core:window:allow-set-position",
+    "core:window:allow-set-size",
```

`crates/os-app/src/runtime/mod.rs`: zusätzlicher Ereignisaufruf nach einer bestätigungspflichtigen Entscheidung. Der vorhandene native Schließpfad lieferte zuvor nur `Confirm` an die Hülle; die Oberfläche hatte keinen Benachrichtigungspunkt. Die Entscheidung und der bestehende `Quit`-Pfad bleiben unverändert:

```diff
+        if let QuitDecision::Confirm { reasons } = &decision {
+            self.broadcast(UiEvent::Notice {
+                notice: Notice {
+                    level: crate::dto::NoticeLevel::Info,
+                    code: "app-quit-confirmation".to_string(),
+                    args: [("reasons".to_string(), serde_json::json!(reasons))].into(),
+                },
+            });
+        }
```

## Prüfungen und Grenzen

Prüfplattform: lokales macOS arm64, Node 22.20.0, Rust stable; Tests ausschließlich gegen lokale Fixtures/Loopback/Mocks. Neue Vitest-/Playwright-Tests sind mit `// spec:` markiert.

| Prüfung | Ergebnis |
|---|---|
| `npm ci` | grün, unveränderter Lockfile, keine neuen Abhängigkeiten |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` | grün, 1 min 31 s |
| `cargo test --workspace --exclude openspider-desktop --locked` | grün: 2.307 bestanden, 0 fehlgeschlagen, 40 bestehende Ignores; 258 Suiten, 9 min 46 s einschließlich Build |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | 162/162 grün, 20 Dateien, 9,65 s |
| `npm run -w apps/desktop check:static` | grün, einschließlich 28 statischer/Token-Tests und npm-Lizenzen |
| `OS_E2E_TRANSPORT=bridge npm run -w apps/desktop e2e -- e2e/shell --project=chromium --project=webkit --workers=2` | 18/18 grün, 22,4 s, keine Ignores |
| `OS_E2E_TRANSPORT=mock npm run -w apps/desktop e2e -- e2e/i18n e2e/settings --project=chromium --project=webkit --workers=2` | bestehende 34/34 grün, 30,5 s |
| `cargo deny check licenses bans sources` | grün |
| `python3 docs/plan/M2-check.py` | grün: 107 Pakete, 586 Spec-IDs |
| `scripts/check-neutral.sh` | grün; nur vorhandene verwaiste Allowlist-Ausnahmen gemeldet |

Zusätzlicher Gesamt-E2E-Lauf auf der gemeinsamen echten Bridge: zunächst 48/52 grün. Ein eigener Test übernahm den Zoom eines vorherigen Einstellungs-Tests; die Shell-Fixture setzt nun Zoom/Sprache und ihren Zustand ausdrücklich zurück, ihr abschließender 18er-Lauf ist grün. Fremde Sprach-/Einstellungs-Tests übernehmen ebenfalls die pro Worker persistierten Einstellungen. Diese Testdateien wurden nicht geändert. Gegenprobe: das ursprüngliche `App.tsx` von `ca88944` wurde nur im Speicher des Vite-Testservers eingesetzt, ohne Änderung einer Repo-Datei. Bereits damit scheitern die Systemsprachen- und System-Theme-Tests auf beiden Browsern (14/18 grün); die isolierten Mock-Läufe oben sind grün. Die gemeinsame Bridge-Fixture benötigt für einen vollständig grünen Gesamt-E2E-Lauf eine getrennte Sitzung je Test bzw. explizite Zustandsrücksetzung im zuständigen Paket.

Keine fachlichen Spec-Unklarheiten. Native WKWebView-/WebView2-/OS-DPI-Matrix und die echte Pause-/Checkpoint-Frist mit M2-K04 bleiben Integrationsabnahmen; Browser-DPR ist kein Nachweis realer Windows-Desktopskalierung. Kein neuer Zoom-Fallback: S-2 hat den nativen Zoompfad noch nicht abschließend bewertet.

Kein Push, Merge oder PR; daher keine neue CI-Lauf-ID. Keine neuen Abhängigkeiten.
