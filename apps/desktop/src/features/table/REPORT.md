# M2-U10 – Paketbericht

Umgesetzt auf `m2/u10`, macOS 26.3 arm64. Ausgangs-Worktree sauber; Clean-Room ausschließlich anhand der Projekt-Spec. Kein Push, Merge oder PR; deshalb keine CI-Lauf-ID.

## Umgesetzte Spec-IDs

- **UI-004**, einschließlich [P1]: Tabs ausschließlich aus `catalog.tabs`; Überlauf, Drag & Drop, Checkbox-Sichtbarkeit, Auf/Ab, Reset im Dropdown und Ansicht-Menü. Reihenfolge/Sichtbarkeit in `uistate`; Filter und Sortierung je Tab während der Sitzung. URL-Auswahl wird beim Wechsel lokalisiert und angesprungen.
- **UI-006**: Katalogfilter mit Zählern, Voraussetzungen, Analyse-Kennzeichen, Issue-Typ und Priorität. Native Tastaturbedienung; `useTabs.select(tab, filter)` als gemeinsamer Navigationspunkt für Overview/Issues.
- **UI-008**, einschließlich [P1]: dynamische Spalten aus dem Katalog, sofortiges Ein-/Ausblenden, festes Address, Spaltenbewegung über DataGridProps sowie Konfigurator, gespeicherte Breiten/Reihenfolge und Wiederherstellung ausgeblendeter Positionen.
- **UI-010**: aufsteigend → absteigend → ohne Sortierung; Shift-Sekundärsortierung, Pfeile/Rangnummern. Der vollständige Sortierauftrag geht an den Kern; keine Sortierung einer lokalen Datenseite.
- **UI-014**: sprachgerechte Zähler nach Filter/Suche, All-Grundmenge, Suchhinweis und Auswahlzahl.
- **UI-018**: versionierte Tick-Anbindung, Nachladen über ViewDataSource, Fokus-/Scrollanker und unsortierter Tail-Modus. Verspätete View-/Zählerantworten werden verworfen, alte Views geschlossen. Die zentrale Ereignisdrosselung bleibt beim bestehenden Kern.
- **UI-110 – P0 gemäß Pakettitel**: Kontextmenü per Zeiger/Tastatur, URL-basierte Mehrfachauswahl, Zeilen/URLs/Zelle/Spaltenwerte kopieren, Standardbrowser (nach Mehrfach-Rückfrage, höchstens zehn URLs), Recrawl/Entfernen und Auswahl-Exporte für Inlinks/Outlinks/Images/Resources/Crawl Path.
- **UI-114**: bestätigtes Entfernen mit Anzahl, persistenter Opt-out nur für das Kontextmenü und Rücksetzen in Einstellungen; Tastatur fragt immer. Recrawl/Entfernen nutzen Operationen mit Fortschritt und Abbruch; Analyse-Veraltet-Hinweis und Aktualisierung über Ticks.
- **UI-090 – Tabellennavigation**: `focusUrl` über EventBus; Crawl-ID-Prüfung, Internal/External, Aufheben einschränkender Filter/Suche, URL-Auswahl und Scrollen. Listener werden abgemeldet.
- **OUT-009 – Spaltenkonfigurator**: `ui.columns` wird über `config.get/set` aktualisiert; andere Konfigurationsfelder bleiben erhalten. Export erhält sichtbare Spalten in sichtbarer Reihenfolge; „alle Spalten“ ist explizit.

`features/table/selection.ts` veröffentlicht URL-Auswahl/View-ID/Version für anschließende Detail- und Sidebar-Pakete. Ein einfacher Exportdialog steht im Paket bereit; die umfassenden Exportdialoge bleiben M2-U16. Erweiterte Suche/benannte Spaltenvorlagen bleiben M2-U20. Zusätzliche P1-Kontextaktionen sind nicht Teil des im Pakettitel festgelegten P0-Menüs.

## Prüfungen

- `cargo fmt --all --check`: grün.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün.
- `cargo test --workspace --exclude openspider-desktop --locked`: **2307 bestanden**, bestehende Ignores unverändert. Build 2m59s, Summe der Suite-Laufzeiten ca. 358s.
- Zusätzlich `cargo clippy -p openspider-desktop --all-targets --locked -- -D warnings` und `cargo test -p openspider-desktop --locked`: grün, **16 bestanden**; zusätzlicher Test für erlaubte Browser-URLs und die Zehnergrenze.
- `npm ci`: grün, keine Vulnerabilities; bestehende Engine-Warnung: Node 22.20.0 liegt unter der von jsdom genannten 22.22.2. Keine Manifest-/Lockdatei geändert.
- `npm run -w apps/desktop typecheck`: grün.
- `npm run -w apps/desktop test`: **164 bestanden** in 18 Dateien (6,53s im letzten Lauf).
- `npm run -w apps/desktop build`: grün.
- `npm run -w apps/desktop check:static`: grün (28 Prüfungen), einschließlich i18n und npm-Lizenzen.
- `npm run -w apps/desktop e2e -- --config e2e/table/playwright.config.ts --project=chromium --project=webkit`: **10 bestanden**, **2 C9-Platzhalter**, 10,9s. Echte Grid-Gesten, Tastatur-Kontextmenü, Mehrfachsortierung, Filter/Suche, Neustart-Persistenz und Operationsabbruch.
- `cargo deny check licenses bans sources`, `scripts/check-neutral.sh`, `docs/plan/M2-check.py`, `git diff --check`: grün. Bestehende Hinweise auf verwaiste Neutralitäts-Ausnahmen bleiben unverändert.

## Abweichungen und offene Integration

- Gemäß C9 enthält `e2e/table/bridge.spec.ts` einen vollständigen Test gegen die echte Bridge (pro Browser ein Platzhalter), markiert `wartet auf M2-K01/M2-K04/M2-K06`. Aktuell liegen dort fremde F05-Stubs. Die UI-Vertragstests laufen ohne Ignore. Echte Crawl-/Recrawl-/Entfernen-Daten und die zentrale 60s-Drosselungsmessung müssen nach deren Integration geprüft werden.
- Die deterministische `fx-ui-basic`-Fixture liegt innerhalb von `features/table/fixtures`, damit Fixtures und Test-Einstieg im eigenen Besitz bleiben. Sie wird vom Produktions-Einstieg nicht importiert. Der Paket-Playwright-Server verwendet einen ephemeren Loopback-Port und übernimmt keinen fremden Vite-Server am geteilten Standardport.
- Windows-Native-Ausführung und echte WKWebView-/WebView2-Smokes sind lokal nicht ausgeführt; keine CI-ID, da Push ausdrücklich ausgeschlossen ist. Der macOS-Desktop kompiliert, Clippy und die plattformlokalen Tests sind grün. Der Browser wurde im Test nicht tatsächlich geöffnet.
- Keine fachlichen Spec-Unklarheiten. Die genannten Integrationsprüfungen bleiben beim Integrator/N01; U10 ersetzt keine fremde Fachlogik und entfernt keine fremden Stub-Einträge.

## Fremdergänzungen (K2-61)

Alle Ergänzungen lassen bestehende Signaturen und das Standardverhalten ohne neue Registrierung/optionale Felder bestehen. Keine bestehenden fremden Tests, Goldens, Manifeste oder Stub-Listen wurden verändert.

| Datei | Ergänzung und Grund |
|---|---|
| `apps/desktop/src/App.tsx` | Optionale Main-Region-Registrierung mit leerem Default; bestehender Leerzustand bleibt ohne Registrierung erhalten. |
| `apps/desktop/src/main.tsx` | Zusätzlicher Init-Aufruf registriert U10-Tabs/Tabelle. |
| `apps/desktop/src/grid/DataGrid.tsx` | Zwei optionale DataGridProps-Felder: Sortiergeste mit Shift und Kopfzeilentext; bisheriger Sortierpfad bleibt Default. |
| `apps/desktop/src/features/settings/sections/general/index.tsx` | Zusätzliches Einstellungsfeld für die Bestätigung beim Entfernen (Default an); persistiert separat im UI-Zustand. |
| `apps/desktop/src-tauri/src/desktop.rs` | Neue native Plattformfunktion `browser_open`, HTTP(S)-Prüfung/Limit und native Launch-Services-/ShellExecuteW-Funktionen ohne Shell-Aufruf; neue eigenständige Testfunktion. |
| `apps/desktop/src-tauri/src/lib.rs` | Zusätzlicher Invoke-Handler für diese reine Betriebssystemaktion; Frontend-Aufruf durch den bestehenden IPC-Client. |

Vollständiger Diff dieser Ergänzungen gegenüber dem Paketstart:

### `apps/desktop/src/App.tsx`

```diff
diff --git a/apps/desktop/src/App.tsx b/apps/desktop/src/App.tsx
index 9de7e22..dc0e5d6 100644
--- a/apps/desktop/src/App.tsx
+++ b/apps/desktop/src/App.tsx
@@ -1,15 +1,23 @@
-import { useEffect, useState } from "react";
+import { useEffect, useState, type ComponentType } from "react";
 import { RegionBoundary } from "./app/ErrorBoundary";
 import { Providers } from "./app/Providers";
 import { useT } from "./i18n";
 import { app as appApi } from "./ipc/app";
 import { EmptyState } from "./ui/primitives/EmptyState";

+/** Optional main-window regions; absent registrations retain the scaffold defaults. */
+const mainRegions: { tabs?: ComponentType; table?: ComponentType } = {};
+export function registerMainRegions(regions: Partial<typeof mainRegions>): void {
+  Object.assign(mainRegions, regions);
+}
+
 /**
  * Hauptfenster (Gerüst, M2-F04): Landmarks nach M2-ui-concept 1.2 mit leeren Regionen. Die Fachpakete (U01 Layout,
  * U03 Kopf-/Statusleiste, U10 Tabelle, U11 Details, U12 Seitenleiste) füllen sie.
  */
 export function App() {
+  const TabsRegion = mainRegions.tabs;
+  const TableRegion = mainRegions.table;
   const t = useT();
   const [version, setVersion] = useState<string | null>(null);

@@ -34,10 +42,10 @@ export function App() {
         <header className="os-app-header" role="banner" aria-label={t("app-region-header")}>
           <h1 className="os-app-title">{t("app-title")}</h1>
         </header>
-        <nav className="os-app-tabs" aria-label={t("app-region-tabs")} />
+        <nav className="os-app-tabs" aria-label={t("app-region-tabs")}>{TabsRegion && <TabsRegion />}</nav>
         <main className="os-app-main" data-region="table" aria-label={t("app-region-table")}>
           <RegionBoundary name="table">
-            <EmptyState state={{ code: "NO_DATA_YET" }} />
+            {TableRegion ? <TableRegion /> : <EmptyState state={{ code: "NO_DATA_YET" }} />}
           </RegionBoundary>
         </main>
         <aside className="os-app-sidebar" aria-label={t("app-region-sidebar")} />
```

### `apps/desktop/src/main.tsx`

```diff
diff --git a/apps/desktop/src/main.tsx b/apps/desktop/src/main.tsx
index 20af82f..739341e 100644
--- a/apps/desktop/src/main.tsx
+++ b/apps/desktop/src/main.tsx
@@ -4,6 +4,9 @@ import { App } from "./App";
 import { bootWindow } from "./app/runtime";
 import "./ui/ui.css";
 import "./app/app.css";
+import { registerMainRegions } from "./App";
+import { TopTabs } from "./features/toptabs/TopTabs";
+import { UpperTable } from "./features/table/UpperTable";

 const params = new URLSearchParams(location.search);
 const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
@@ -12,6 +15,7 @@ if (import.meta.env.DEV && params.has("harness")) {
   // Harness für visuelle Tests: nur im Dev-Build, deterministische Daten über den Mock-Transport.
   void import("./harness/entry").then((m) => m.mountHarness(root));
 } else {
+  registerMainRegions({ tabs: TopTabs, table: UpperTable });
   const runtime = bootWindow({ kind: "main" });
   void runtime.ready.then(() => {
     root.render(
```

### `apps/desktop/src/grid/DataGrid.tsx`

```diff
diff --git a/apps/desktop/src/grid/DataGrid.tsx b/apps/desktop/src/grid/DataGrid.tsx
index 6c4205a..c5ace8d 100644
--- a/apps/desktop/src/grid/DataGrid.tsx
+++ b/apps/desktop/src/grid/DataGrid.tsx
@@ -13,6 +13,9 @@ import "./grid.css";
 export interface ColumnState { key: string; width: number; visible: boolean; pinned: boolean; sort?: "asc" | "desc" }
 export interface GridTheme { tokens: TokenMap; fontFamily: string; fontSize: number; zebra: boolean }
 export interface DataGridProps {
+  /** Optional upper-table sorting gesture and title decoration (M2-U10). */
+  onSortGesture?: (column: string, extend: boolean) => void;
+  headerTitle?: (column: string) => string;
   source: ViewDataSource;
   columns: ColumnState[];
   selection: SelectionModel;
@@ -53,7 +56,7 @@ export function DataGrid(props: DataGridProps) {
   const visible = useMemo(() => props.columns.filter(c => c.visible || c.key === "Row").sort((a, b) =>
     a.key === "Row" ? -1 : b.key === "Row" ? 1 : Number(b.pinned) - Number(a.pinned)), [props.columns]);
   const metadata = useMemo(() => new Map(source.columns.map((c, i) => [c.key, { column: c, index: i }])), [source]);
-  const columns = useMemo(() => visible.map(c => ({ id: c.key, title: c.key, width: c.width, hasMenu: true })), [visible]);
+  const columns = useMemo(() => visible.map(c => ({ id: c.key, title: props.headerTitle?.(c.key) ?? c.key, width: c.width, hasMenu: true })), [visible, props.headerTitle]);
   const freeze = visible.findIndex(c => !c.pinned && c.key !== "Row");
   const tokens = theme.tokens;
   const canvasTheme: Partial<Theme> = { bgCell: tokens["bg.surface"], bgCellMedium: tokens["bg.zebra"], bgHeader: tokens["bg.app"],
@@ -150,7 +153,10 @@ export function DataGrid(props: DataGridProps) {
         next.splice(target, 0, column);
         props.onColumnsChange(next);
       }}
-      onHeaderClicked={(col, event) => { event.preventDefault(); const c = visible[col]; if (metadata.get(c.key)?.column.sortable) props.onSortChange([{ column: c.key, dir: c.sort === "asc" ? "desc" : "asc" }]); }}
+      onHeaderClicked={(col, event) => { event.preventDefault(); const c = visible[col]; if (metadata.get(c.key)?.column.sortable) {
+        if (props.onSortGesture) { props.onSortGesture(c.key, event.shiftKey); return; }
+        props.onSortChange([{ column: c.key, dir: c.sort === "asc" ? "desc" : "asc" }]);
+      } }}
       onCellContextMenu={([col, row], event) => { event.preventDefault(); props.onContextMenu({ column: visible[col].key, row, urlId: source.rowKey(row), x: event.bounds.x, y: event.bounds.y }); }}
       onHeaderContextMenu={(col, event) => { event.preventDefault(); props.onContextMenu({ column: visible[col].key, x: event.bounds.x, y: event.bounds.y }); }}
       onHeaderMenuClick={(col, bounds) => props.onContextMenu({ column: visible[col].key, x: bounds.x, y: bounds.y + bounds.height })}
```

### `apps/desktop/src/features/settings/sections/general/index.tsx`

```diff
diff --git a/apps/desktop/src/features/settings/sections/general/index.tsx b/apps/desktop/src/features/settings/sections/general/index.tsx
index 22ee265..575c12a 100644
--- a/apps/desktop/src/features/settings/sections/general/index.tsx
+++ b/apps/desktop/src/features/settings/sections/general/index.tsx
@@ -1,3 +1,4 @@
+import { RemovalConfirmationSettings } from "../../../table/RemovalConfirmationSettings";
 import { useMemo } from "react";
 import { i18n, useT } from "../../../../i18n";
 import { usePrefs } from "../../../../state/prefs";
@@ -39,6 +40,7 @@ function General() {
       <Switch isSelected={notifications} anchor="general.notifications" onChange={(v) => void patch({ general: { notifications: v } })}>
         {t("settings-general-notifications")}
       </Switch>
+      <RemovalConfirmationSettings />
     </div>
   );
 }
```

### `apps/desktop/src-tauri/src/desktop.rs`

```diff
diff --git a/apps/desktop/src-tauri/src/desktop.rs b/apps/desktop/src-tauri/src/desktop.rs
index 374a249..2514fa1 100644
--- a/apps/desktop/src-tauri/src/desktop.rs
+++ b/apps/desktop/src-tauri/src/desktop.rs
@@ -236,3 +236,144 @@ mod linux_tests {
         assert_eq!(reveal(&[]).unwrap_err().code, ErrorCode::Unsupported);
     }
 }
+
+/// Native platform action for the upper table; no crawl data crosses this boundary.
+#[tauri::command]
+pub fn browser_open(urls: Vec<String>) -> Result<(), CmdError> {
+    let urls = browser_urls(&urls)?;
+    for url in urls {
+        launch_browser(url.as_str())?;
+    }
+    Ok(())
+}
+
+fn browser_urls(urls: &[String]) -> Result<Vec<tauri::Url>, CmdError> {
+    if urls.is_empty() || urls.len() > 10 {
+        return Err(CmdError::new(ErrorCode::InvalidInput, "err-invalid-input"));
+    }
+    urls.iter()
+        .map(|text| {
+            let url = tauri::Url::parse(text).map_err(failure)?;
+            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
+                return Err(CmdError::new(ErrorCode::InvalidInput, "err-invalid-input"));
+            }
+            Ok(url)
+        })
+        .collect()
+}
+
+#[cfg(target_os = "macos")]
+fn launch_browser(url: &str) -> Result<(), CmdError> {
+    use std::ffi::{c_char, c_void, CString};
+    type Obj = *mut c_void;
+    #[link(name = "objc")]
+    unsafe extern "C" {
+        fn objc_getClass(name: *const c_char) -> Obj;
+        fn sel_registerName(name: *const c_char) -> Obj;
+        fn objc_msgSend();
+    }
+    #[link(name = "AppKit", kind = "framework")]
+    unsafe extern "C" {}
+    let text = CString::new(url).map_err(failure)?;
+    // SAFETY: all selectors match their Objective-C signatures; the pool owns the temporary
+    // NSString and NSURL. NSWorkspace opens the validated HTTP(S) URL through Launch Services.
+    let opened = unsafe {
+        let send0: unsafe extern "C" fn(Obj, Obj) -> Obj =
+            std::mem::transmute(objc_msgSend as *const ());
+        let send1: unsafe extern "C" fn(Obj, Obj, Obj) -> Obj =
+            std::mem::transmute(objc_msgSend as *const ());
+        let string: unsafe extern "C" fn(Obj, Obj, *const c_char) -> Obj =
+            std::mem::transmute(objc_msgSend as *const ());
+        let open: unsafe extern "C" fn(Obj, Obj, Obj) -> bool =
+            std::mem::transmute(objc_msgSend as *const ());
+        let pool = send0(
+            objc_getClass(c"NSAutoreleasePool".as_ptr()),
+            sel_registerName(c"new".as_ptr()),
+        );
+        let value = string(
+            objc_getClass(c"NSString".as_ptr()),
+            sel_registerName(c"stringWithUTF8String:".as_ptr()),
+            text.as_ptr(),
+        );
+        let ns_url = send1(
+            objc_getClass(c"NSURL".as_ptr()),
+            sel_registerName(c"URLWithString:".as_ptr()),
+            value,
+        );
+        let workspace = send0(
+            objc_getClass(c"NSWorkspace".as_ptr()),
+            sel_registerName(c"sharedWorkspace".as_ptr()),
+        );
+        let opened =
+            !ns_url.is_null() && open(workspace, sel_registerName(c"openURL:".as_ptr()), ns_url);
+        send0(pool, sel_registerName(c"drain".as_ptr()));
+        opened
+    };
+    if opened {
+        Ok(())
+    } else {
+        Err(failure("Browser could not open URL"))
+    }
+}
+
+#[cfg(target_os = "windows")]
+fn launch_browser(url: &str) -> Result<(), CmdError> {
+    use std::{ffi::c_void, ptr};
+    #[link(name = "shell32")]
+    unsafe extern "system" {
+        fn ShellExecuteW(
+            window: *mut c_void,
+            operation: *const u16,
+            file: *const u16,
+            parameters: *const u16,
+            directory: *const u16,
+            show: i32,
+        ) -> *mut c_void;
+    }
+    let operation: Vec<u16> = "open\0".encode_utf16().collect();
+    let value: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
+    // SAFETY: strings are NUL-terminated and live for the duration of the native call.
+    let result = unsafe {
+        ShellExecuteW(
+            ptr::null_mut(),
+            operation.as_ptr(),
+            value.as_ptr(),
+            ptr::null(),
+            ptr::null(),
+            1,
+        )
+    } as isize;
+    if result > 32 {
+        Ok(())
+    } else {
+        Err(failure(format!("ShellExecuteW: {result}")))
+    }
+}
+
+#[cfg(not(any(target_os = "macos", target_os = "windows")))]
+fn launch_browser(_url: &str) -> Result<(), CmdError> {
+    Err(CmdError::new(
+        ErrorCode::Unsupported,
+        "err-desktop-platform",
+    ))
+}
+
+#[cfg(test)]
+mod browser_tests {
+    use super::*;
+
+    // spec: UI-110
+    #[test]
+    fn browser_accepts_only_bounded_web_urls() {
+        let fixture = vec!["https://fixture.test/a?b=c#d".to_string()];
+        assert_eq!(browser_urls(&fixture).unwrap()[0].as_str(), fixture[0]);
+        for invalid in [
+            vec![],
+            vec![fixture[0].clone(); 11],
+            vec!["file:///tmp/x".into()],
+            vec!["javascript:alert(1)".into()],
+        ] {
+            assert!(browser_urls(&invalid).is_err());
+        }
+    }
+}
```

### `apps/desktop/src-tauri/src/lib.rs`

```diff
diff --git a/apps/desktop/src-tauri/src/lib.rs b/apps/desktop/src-tauri/src/lib.rs
index eedd831..3b05753 100644
--- a/apps/desktop/src-tauri/src/lib.rs
+++ b/apps/desktop/src-tauri/src/lib.rs
@@ -66,6 +66,7 @@ pub fn run() {
     }

     builder = builder
+        .invoke_handler(tauri::generate_handler![desktop::browser_open])
         .plugin(tauri_plugin_dialog::init())
         .plugin(tauri_plugin_clipboard_manager::init())
         .plugin(tauri_plugin_notification::init())
```
