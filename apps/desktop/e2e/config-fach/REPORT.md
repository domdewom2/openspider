# M2-U13B – Konfigurationsdialog II

Stand: 21.10.2026, Branch `m2/u13b`, macOS arm64. Der Worktree war beim Start sauber.
Clean-Room: Projektcode, Plan und `docs/spec`; kein Zugriff auf das Referenzprodukt.

## Umsetzung und Spec-IDs

`spec_ids` ist laut Paketmanifest leer. Alle 14 `extra_spec_ids` werden durch eigene, mit `// spec:` markierte Tests geprüft:

| Spec-ID | Dialogverhalten |
|---|---|
| INTEG-020 | Suchfilter als Entwurf mit Name, Bedingung, Text/Regex, mehrzeiligem Ausdruck, Suchbereich und Text-Case-Schalter; maximal 100, Reihenfolge, Entfernen, Profil-/Konfigurationspersistenz. |
| INTEG-026 | XPath-Suchbereich mit bedingtem XPath-Eingabefeld. |
| INTEG-028 | Unveränderte Weitergabe des gemeinsamen Regex-Dialekts an den Kern, Erklärung der Inline-Flags und konfigurierbares Zeitbudget. Keine Regex-Auswertung im Browser. |
| INTEG-032 | Eigener modaler Bulk-Dialog; gemeinsame Optionen, einspaltiges CSV/TXT einschließlich quoted multiline cells; leere/doppelte Zeilen und Überhang gezählt, Limit 100. |
| INTEG-040 | Extraktoren mit automatischem Namen, Modus, Ausdruck, Ausgabe, Reihenfolge und Limit 100. |
| INTEG-043 | CSS mit optionalem Attribut; bei gesetztem Attribut ist die ignorierte Ausgabeauswahl deaktiviert. |
| INTEG-048 | Funktionswert nur für XPath; Hinweis auf den String-Wert des ersten Knotens. Moduswechsel bereinigt unzulässige Optionen. |
| INTEG-051 | Kernvalidierung bei Eingabe, Prüfstatus/Häkchen/Kreuz, Fehlermeldung mit Position/Tooltip, `aria-invalid`; OK gesperrt bei ausstehender Prüfung oder Fehlern, auch nach Seitenwechsel. XPath 3.1 gegen echten Kern geprüft. |
| INTEG-060 | Snippets mit Namen, Extraktion/Aktion, MIME-Liste, Timeout, aufklappbarem mehrzeiligem Code-Editor und Reihenfolge; Limit 100, Warnbanner ohne JS-Rendering. |
| INTEG-080, INTEG-081, INTEG-082 | Geordnete Positionsregeln mit Teilstrings; Content bleibt als unveränderbare Auffangregel zuletzt. Neue Regeln vor Content, Verschieben/Entfernen und Standardliste wiederherstellen. |
| CRAWL-134 | Segmentliste mit Farbpalette/Farbwahl, allen Konfigurationsoperatoren, UND-/ODER-Gruppen, Verweisen nur auf obere Segmente, Fehlermeldungen aus dem Kern. JSON-Austausch kompatibel zum vorhandenen Segmentdialog und zu nativen Profildefinitionen. Vorschau über abbrechbare Kernoperationen; alte Ergebnisse werden bei Regel-/Prioritätswechsel sofort verborgen. Ausstehende Analyse-/API-Spalten anhand von `catalog.columns`. |
| TECH-300 | 14 Elemente in Spec-Reihenfolge; Standard 1–13 aktiv, Content aus; Schwelle aus dem Schema. Content nur bei positivem Kernnachweis gespeicherter Inhalte in beiden Vergleichscrawls wählbar. |

Zusätzlich: API-Zugänge als Einstieg in den vorhandenen Integrationsdialog mit Quellenstatus; der Konfigurationsentwurf bleibt beim Öffnen erhalten. Die Analyseseite zeigt alle Schalter aus dem Schema, Bilder nur mit JS-Rendering (eigene Prüfungen zu INTEG-001 und CRAWL-141).
Alle acht Seiten werden vom bestehenden `import.meta.glob`-Register gefunden. Übersetzungen Deutsch/Englisch sind eigene Formulierungen. Speichern, Abbrechen, Profilwechsel, Feldsperren und Laufzeit-Neuberechnung verwenden die vorhandenen Rahmen-/Kernpfade.

## Tests

Lokale Konfigurationsfixture: `src/features/config/pages/custom-search/fixtures/fach.json`. Die kanonische Liste mit genau 14 Kennungen und Beschriftungen in Spec-Reihenfolge steht unabhängig von der Implementierung in `src/features/config/pages/change-detection/fixtures/elements.json`.
Eigene Vitest-Suites: `custom-search/fach.test.tsx` (22 Fälle) und `segments/preview.test.tsx` (2 Fälle).
E2E: `e2e/config-fach/fach.spec.ts` (4 Fälle × Chromium/WebKit): echte Bridge, echte Syntaxprüfung, Persistenz, deutsche belegte Fachseiten bei 1024 × 640, axe ohne schwere/kritische Befunde und drei lokale Crawls zur Content-Voraussetzung. Nur Loopback-HTTP.

| Prüfung | Ergebnis |
|---|---|
| `npm ci` | grün |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | 74 Dateien, 724 Tests grün |
| `npm run -w apps/desktop check:static` | grün |
| `cargo build --locked -p os-app --features e2e-bridge --bin os-app-bridge` | grün |
| `npm run -w apps/desktop e2e -- e2e/config-fach --workers=2` | 8/8 grün |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets -- -D warnings` | grün |
| `OPENSPIDER_SECRET_STORE=memory cargo test --workspace --exclude openspider-desktop` | grün: 3555 bestanden, 0 Fehler, 67 vorhandene Ignores, einschließlich Doc-Tests; Exit 0 |

## Fremdergänzungen (K2-61)

Ausschließlich additive Erweiterungen. Bestehende Signaturen bleiben kompatibel und bestehende Seiten ohne `saveGuard` behalten ihr Verhalten. Keine neuen Abhängigkeiten oder Crates.

- `frame/types.ts`, `frame/definePage.tsx`, `frame/registry.ts`: optionaler Seitenguard (`undefined` = keine zusätzliche Sperre), Durchreichen und Auswertung über alle geladenen Seiten.
- `frame/draft.ts`: optionaler zuletzt erfolgreich geprüfter Entwurfsstand mit Default `undefined`. Die Fachseiten unterscheiden damit noch ausstehende von abgeschlossenen Prüfungen; die vorhandene Validierung wird mitbenutzt.
- `frame/ConfigDialog.tsx`: zusätzlicher Aufruf des optionalen Guards an der vorhandenen OK-Schaltfläche. Ohne Guard bleibt deren bisherige Sperrbedingung erhalten.
- `crates/os-app/src/commands/compare.rs`: neue lesende Funktion und zusätzlicher Registeraufruf `compare.contentAvailable`, Argument `void`, Rückgabe `JsonValue` (Boolean), `longRunning = false`. Ohne Vergleich `false`, sonst vorhandenes `Comparison::content_comparable` in `with_active`/`spawn_blocking`. Kein Aufruf von `compare.open`/`swap` zur Statusabfrage, keine Mutation bestehender Sitzungen.
- `apps/desktop/src/ipc/commands.json`: genau ein neuer Command-Eintrag (additive Ausgabe des Registers; Reihenfolge gemäß Register).
- `apps/desktop/src/ipc/compare.ts`: nur diesen Wrapper aus dem Generator erzeugt; zusätzlicher Import und zusätzliche Funktion. Keine anderen generierten Dateien beschrieben.

Vollständige Diffs aller Fremdergänzungen:

### `apps/desktop/src/features/config/frame/types.ts`

```diff
diff --git a/apps/desktop/src/features/config/frame/types.ts b/apps/desktop/src/features/config/frame/types.ts
index bb149481..12574186 100644
--- a/apps/desktop/src/features/config/frame/types.ts
+++ b/apps/desktop/src/features/config/frame/types.ts
@@ -1,3 +1,4 @@
+import type { ConfigDraft } from "./draft";
 import type { FC } from "react";

 export type FieldKind = "auto" | "bool" | "int" | "number" | "enum" | "radio" | "text" | "lines" | "storeCrawl";
@@ -28,6 +29,8 @@ export interface FieldDef {

 /** Eine Seite der Seitennavigation. Seitenmodule liegen unter `features/config/pages/<id>/index.tsx` und exportieren `page` oder `pages`. */
 export interface ConfigPage {
+  /** Optional save constraint supplied by a specialist page. */
+  saveGuard?: (draft: ConfigDraft) => boolean;
   id: string;
   /** Gruppe in der Navigation (`spider`, `custom`); ohne Gruppe steht die Seite auf der obersten Ebene. */
   group?: string;
```

### `apps/desktop/src/features/config/frame/definePage.tsx`

```diff
diff --git a/apps/desktop/src/features/config/frame/definePage.tsx b/apps/desktop/src/features/config/frame/definePage.tsx
index 907146bc..0ce62b7a 100644
--- a/apps/desktop/src/features/config/frame/definePage.tsx
+++ b/apps/desktop/src/features/config/frame/definePage.tsx
@@ -1,3 +1,4 @@
+import type { ConfigDraft } from "./draft";
 // Hilfsfunktion für Seitenmodule: Überschrift, optionaler Kopf- und Fußbereich und die Feldliste.
 import type { ComponentType } from "react";
 import { useT } from "../../../i18n";
@@ -5,6 +6,7 @@ import { FieldList, PageHeading } from "./fields";
 import { pageTitleKey, type ConfigPage, type FieldDef } from "./types";

 export function definePage(opts: {
+  saveGuard?: (draft: ConfigDraft) => boolean;
   id: string;
   group?: string;
   order: number;
@@ -30,6 +32,7 @@ export function definePage(opts: {
     );
   }
   return {
+    saveGuard: opts.saveGuard,
     id: opts.id,
     group: opts.group,
     order: opts.order,
```

### `apps/desktop/src/features/config/frame/registry.ts`

```diff
diff --git a/apps/desktop/src/features/config/frame/registry.ts b/apps/desktop/src/features/config/frame/registry.ts
index b8fb2e6c..8e06471c 100644
--- a/apps/desktop/src/features/config/frame/registry.ts
+++ b/apps/desktop/src/features/config/frame/registry.ts
@@ -1,3 +1,4 @@
+import type { ConfigDraft } from "./draft";
 import { GROUP_ORDER, KNOWN_PAGES, pageTitleKey, type ConfigPage } from "./types";

 type PageModule = { page?: ConfigPage; pages?: ConfigPage[] };
@@ -43,3 +44,8 @@ export function navigation(pages: readonly ResolvedPage[]): NavGroup[] {
   }
   return out.sort((a, b) => (GROUP_ORDER[a.group ?? ""] ?? a.pages[0]?.order ?? 0) - (GROUP_ORDER[b.group ?? ""] ?? b.pages[0]?.order ?? 0));
 }
+
+/** Optional constraints of loaded pages; absent guards do not affect the dialog. */
+export function pageSaveBlocked(pages: readonly ResolvedPage[], draft: ConfigDraft): boolean {
+  return pages.some(page => page.saveGuard?.(draft) === true);
+}
```

### `apps/desktop/src/features/config/frame/draft.ts`

```diff
diff --git a/apps/desktop/src/features/config/frame/draft.ts b/apps/desktop/src/features/config/frame/draft.ts
index 201ff22d..3bb9f524 100644
--- a/apps/desktop/src/features/config/frame/draft.ts
+++ b/apps/desktop/src/features/config/frame/draft.ts
@@ -14,6 +14,8 @@ import { asSchema, fieldMeta, type ConfigSchema, type FieldMeta } from "./schema
 const VALIDATE_DELAY_MS = 200;

 export interface ConfigDraft {
+  /** Snapshot checked by config.validate; undefined while the first check is pending. */
+  validatedConfig?: ConfigJson;
   status: "loading" | "ready" | "error";
   loadError: CmdError | null;
   /** Gespeicherter Stand (zuletzt gelesen oder geschrieben). */
@@ -53,6 +55,7 @@ export function useConfigDraft(active: boolean): ConfigDraft {
   const [cfg, setCfg] = useState<ConfigJson>({});
   const [schema, setSchema] = useState<ConfigSchema | null>(null);
   const [defaultRows, setDefaultRows] = useState<DiffRow[]>([]);
+  const [validatedConfig, setValidatedConfig] = useState<ConfigJson>();
   const [issues, setIssues] = useState<ValidationIssue[]>([]);
   const [saveError, setSaveError] = useState<CmdError | null>(null);
   const [deferred, setDeferred] = useState<string[]>([]);
@@ -71,6 +74,7 @@ export function useConfigDraft(active: boolean): ConfigDraft {
     const mine = ++seq.current;
     try {
       const found = await configApi.validate(dtoOf(base, config));
+      if (mine === seq.current) setValidatedConfig(config);
       if (mine === seq.current) setIssues(found);
     } catch (e) {
       if (mine === seq.current) setIssues([{ path: "", messageKey: toCmdError(e).messageKey, args: {}, warning: false, line: null, column: null }]);
@@ -177,6 +181,7 @@ export function useConfigDraft(active: boolean): ConfigDraft {
   }, [issues, saveError]);

   return {
+    validatedConfig,
     status,
     loadError,
     saved,
```

### `apps/desktop/src/features/config/frame/ConfigDialog.tsx`

```diff
diff --git a/apps/desktop/src/features/config/frame/ConfigDialog.tsx b/apps/desktop/src/features/config/frame/ConfigDialog.tsx
index 0ba38211..135e4fa5 100644
--- a/apps/desktop/src/features/config/frame/ConfigDialog.tsx
+++ b/apps/desktop/src/features/config/frame/ConfigDialog.tsx
@@ -1,3 +1,4 @@
+import { pageSaveBlocked } from "./registry";
 // Rahmen des Konfigurationsdialogs (M2-U13, M2-ui-concept 1.5 / W3): Seitennavigation mit Suche, Profilwahl, Hinweise zum
 // Crawl-Zustand (CRAWL-126, F-13/F-14), Abweichungen und die Fußleiste. Die Seiten kommen aus `features/config/pages/*`.
 import "./config.css";
@@ -252,6 +253,7 @@ export function ConfigDialog() {
             onOk={() => void onOk()}
             onCancel={closeConfig}
             okDisabled={draft.status !== "ready" || draft.saving}
+            {...(pageSaveBlocked(pages, draft) ? { okDisabled: true } : {})}
             extra={
               <Button variant="secondary" onPress={() => setShowDiff((v) => !v)} isDisabled={draft.status !== "ready"} data-testid="config-deviations">
                 {t("config-deviations", { count: deviationCount })}
```

### `crates/os-app/src/commands/compare.rs`

```diff
diff --git a/crates/os-app/src/commands/compare.rs b/crates/os-app/src/commands/compare.rs
index c5f4d147..64dabab5 100644
--- a/crates/os-app/src/commands/compare.rs
+++ b/crates/os-app/src/commands/compare.rs
@@ -415,8 +415,38 @@ fn close<'a>(app: &'a App, ctx: CallCtx, _: serde_json::Value) -> BoxFuture<'a,
     })
 }

+/// Read-only capability for TECH-300: selection requires stored content on both sides.
+fn content_available<'a>(
+    app: &'a App,
+    ctx: CallCtx,
+    _: serde_json::Value,
+) -> BoxFuture<'a, CmdResult> {
+    Box::pin(async move {
+        if active_version(app, &ctx.window).is_none() {
+            return to_json(&false);
+        }
+        let available = with_active(app, &ctx.window, |active| {
+            active
+                .comparison
+                .content_comparable()
+                .map_err(compare_error)
+        })
+        .await?;
+        to_json(&available)
+    })
+}
+
 /// Trägt die Commands dieser Domäne in die Registry ein (Namen und Typen unverändert laut Katalog).
 pub fn register(r: &mut Registry) {
+    r.register(
+        CommandMeta {
+            name: "compare.contentAvailable",
+            args: "void",
+            ret: "JsonValue",
+            long_running: false,
+        },
+        content_available,
+    );
     for (name, args, ret, handler) in [
         (
             "compare.open",
```

### `apps/desktop/src/ipc/commands.json`

```diff
diff --git a/apps/desktop/src/ipc/commands.json b/apps/desktop/src/ipc/commands.json
index cb3e0fc0..ab37c043 100644
--- a/apps/desktop/src/ipc/commands.json
+++ b/apps/desktop/src/ipc/commands.json
@@ -731,6 +731,12 @@
     "ret": "FileResult",
     "longRunning": true
   },
+  {
+    "name": "compare.contentAvailable",
+    "args": "void",
+    "ret": "JsonValue",
+    "longRunning": false
+  },
   {
     "name": "compare.open",
     "args": "CompareOpenArgs",
```

### `apps/desktop/src/ipc/compare.ts`

```diff
diff --git a/apps/desktop/src/ipc/compare.ts b/apps/desktop/src/ipc/compare.ts
index c9d59c33..83031de9 100644
--- a/apps/desktop/src/ipc/compare.ts
+++ b/apps/desktop/src/ipc/compare.ts
@@ -1,11 +1,14 @@
 // Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
 import type { CompareOpenArgs } from "./gen/CompareOpenArgs";
 import type { CompareSession } from "./gen/CompareSession";
+import type { JsonValue } from "./gen/serde_json/JsonValue";
 import type { RuleTrace } from "./gen/RuleTrace";
 import type { TestMappingArgs } from "./gen/TestMappingArgs";
 import { client } from "./client";

 export const compare = {
+  /** `compare.contentAvailable` */
+  contentAvailable: (signal?: AbortSignal) => client.invoke<JsonValue>("compare.contentAvailable", undefined, signal),
   /** `compare.open` */
   open: (args: CompareOpenArgs, signal?: AbortSignal) => client.invoke<CompareSession>("compare.open", args, signal),
   /** `compare.swap` */
```

## Erwartungsanpassungen (K2-63)

- `apps/desktop/src/features/config/frame/dialog.test.tsx`: CRAWL-134 liefert die Segmentseite. Erwartung `data-pending="true"` wird zu „kein data-pending“; statt des Platzhalters wird die Schaltfläche „Add segment“ erwartet. Navigation und Testablauf unverändert.
- `crates/os-app/tests/manifest_contract.rs`: TECH-300 benötigt den lesenden Capability-Command; genau dessen Name zur erwarteten Command-Menge ergänzt. Testlogik unverändert.

```diff
diff --git a/apps/desktop/src/features/config/frame/dialog.test.tsx b/apps/desktop/src/features/config/frame/dialog.test.tsx
index 6f80b6e7..f07d6005 100644
--- a/apps/desktop/src/features/config/frame/dialog.test.tsx
+++ b/apps/desktop/src/features/config/frame/dialog.test.tsx
@@ -57,9 +57,9 @@ describe("Rahmen: Navigation, Suche, Entwurf", () => {
     for (const name of ["Crawl", "Extraction", "Limits", "Rendering", "Advanced", "Presets", "robots.txt", "URL rewriting", "Include / Exclude", "Speed", "User-Agent", "HTTP headers", "Authentication", "Profiles"]) {
       expect(within(nav).getByRole("button", { name }), name).toBeInTheDocument();
     }
-    expect(within(nav).getByRole("button", { name: "Segments" })).toHaveAttribute("data-pending", "true");
+    expect(within(nav).getByRole("button", { name: "Segments" })).not.toHaveAttribute("data-pending");
     fireEvent.click(within(nav).getByRole("button", { name: "Segments" }));
-    expect(await within(dialog).findByTestId("config-placeholder")).toBeInTheDocument();
+    expect(await within(dialog).findByRole("button", { name: "Add segment" })).toBeInTheDocument();
   });

   // spec: OUT-220
```

```diff
diff --git a/crates/os-app/tests/manifest_contract.rs b/crates/os-app/tests/manifest_contract.rs
index e5bedfb0..025e02e6 100644
--- a/crates/os-app/tests/manifest_contract.rs
+++ b/crates/os-app/tests/manifest_contract.rs
@@ -111,6 +111,7 @@ const CATALOG_4_4: &[&str] = &[
     "compare.open",
     "compare.swap",
     "compare.testMapping",
+    "compare.contentAvailable", // TECH-300: read-only prerequisite for Content selection
     "compare.close",
     "viz.open",
     "viz.graph",
```

## Abweichungen, Abhängigkeiten und offene Fragen

Keine funktionale Abweichung innerhalb des Dialogpakets. K2-60 (Stub-Ablösung) wird nicht benötigt.

Bereits vorhandene Kernabhängigkeit: `crates/os-app/src/session/control.rs::config` liest vor dem ersten Crawl ohne explizites Profil die Werk-/Defaultprofilkonfiguration statt des fensterlokalen `config.set`-Entwurfs. Das entspricht dem bereits geplanten Befund K2-69/A01-04 (Besitzer M2-N01b). Die zusätzlichen echten Crawl-Fixtures dieses Pakets starten deshalb mit einem explizit gespeicherten Profil. Dieser fremde Lebenszykluspfad wurde nicht verändert; Dialogspeicherung und Profile sind vollständig implementiert und geprüft. Keine neue ungeklärte Schnittstellenfrage.

Der Bericht liegt bewusst unter `apps/desktop/e2e/config-fach/` und damit innerhalb von `owns`. Keine Änderungen an Plan-/Spec-Dateien, kein Push, Merge oder PR.

## Commits

- `29f7c288` – optionaler Validierungs-Hook im Rahmen (K2-61).
- `5aa0de5e` – lesende Content-Capability und Vertrag (K2-61/K2-63).
- `2f64226d` – acht Fachseiten, Übersetzungen und Tests.
- `d23c25dd` – unabhängige Spec-Fixture für genau 14 Elemente in kanonischer Reihenfolge.
- Dieser Bericht folgt in einem eigenen `docs:`-Commit; dessen Kennung steht im Abschlussbericht der Unterhaltung.

Alle Commits tragen die geforderte Co-Authored-By-Zeile.
