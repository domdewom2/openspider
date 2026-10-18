# M2-U03 – Kopf-/Statusleiste und Meldungen

Stand: 18.10.2026, Branch `m2/u03`, lokale macOS-Ausführung. Clean-Room: ausschließlich Repository und eigene Fixtures; kein Referenzprodukt. Der Worktree war zu Beginn sauber. Keine neuen Abhängigkeiten, kein Push, Merge oder PR.

## Umgesetzte Spec-IDs

- **UI-003:** HTTP(S)-Validierung mit expliziter Host-Prüfung vor dem Browserparser, HTTPS-Ergänzung, Pfadkodierung, 20 eindeutige Vorschläge im versionierten UI-Zustand; Bereich, Modi Spider/Liste/APIs, TXT/CSV-Upload, manuelle/eingefügte URL-Listen und Sitemap-URLs (.xml/.xml.gz) über den vorhandenen Listenplaner; Start/Pause/Fortsetzen/Leeren, Enter und gemeinsame Aktionen; Rückfrage beim Ersetzen fertiger Ergebnisse; schreibgeschützte fremde Crawls. Vollständige Listen werden übergeben, unabhängig von der Vorschaugröße. Schaltflächenzustände kommen aus Ticks, nicht aus optimistischen Command-Antworten.
- **UI-013:** Kernwerte für Fortschritt, aktuelle und mittlere Rate, URLs, Warteschlange, Restzeit, Threads, RAM belegt/Grenze, Dateigröße, Zustände und Analysefortschritt. Sinkende Prozentwerte bleiben unverändert. Anzeige maximal viermal/s; Meldungen warten nicht darauf. MCP bleibt sichtbar und öffnet die registrierten MCP-Menüaktionen (bis zu deren Registrierung mit Einstellungszugang).
- **UI-019:** Serielle Start-/Kernfehlerdialoge mit Ursache, Status und getrennten technischen Details; DNS, Verbindung, TLS, robots.txt, Fremdhost-Redirect und Plattenplatz; Einstellungshinweis statt robots-Änderung; explizite HTTP-Übernahme ohne automatischen Neuversuch; Log öffnen, Diagnose kopieren; Abschlussbenachrichtigungen nur im Hintergrund bei aktivierter Einstellung. Native Benachrichtigungen beachten die OS-Berechtigung.
- **CRAWL-001 [M2]:** Autovervollständigung, keine neue Sitzung während laufendem Crawl, Wiederaufnahme und Ansicht leeren.
- **CRAWL-009 [M2]:** Schrägstrich-Hinweis nur für Unterordner ohne Dateiendung.
- **CRAWL-026 [M2]:** Moduswechsel mit Rückfrage; Konfiguration und gesicherte Datei bleiben erhalten.
- **CRAWL-112 [M2]:** Serverlast-Hinweis ab 20 konfigurierten oder aktiven Threads, auch bei Live-Änderungen.
- **CRAWL-117 [M2]:** Basic-Dialog mit Host/Realm, einmal je Host/Realm und Crawl, Benutzername/Passwort, zusätzliche HTTP-Bestätigung; verschlüsselte Profilablage über `auth.saveProfile`, nur Profilverweise in `config.set`, danach `crawl.resume`. Abbrechen setzt ohne gespeicherte Zugangsdaten fort. Kein Passwort im UI-Zustand, Verlauf oder in der Konfiguration.
- **CRAWL-130 [M2]:** UI-Coalescing, unveränderte Kernkennzahlen, Statistikdetails; alte Tick-Sequenzen werden verworfen.
- **CRAWL-097:** Zustand „Gedrosselt“ je Host, unabhängig vom Quittieren der Notice, Rücknahme bei Erholung/neuem Crawl/Leeren/Ende.
- **CRAWL-129:** Warnstufen ausschließlich vom Kern, farbige und textuelle RAM-Anzeige; Pausenbanner mit drei direkten Einstellungsverweisen. Keine eigenen Schwellwerte.

Fixture: `fixtures/fx-ui-basic.json` in diesem Feature, abgeleitet vom vorhandenen F05-Tickvertrag, ergänzt um eigene RAM-Messwerte. Jeder neue Funktionstest referenziert seine Spec-ID. Keine fremden Tests oder Goldens geändert.

## Meldungsvertrag und Integration

`Notice.code` ist im F05-Vertrag ein freier String. U03 verarbeitet diese Codes aus `Tick.notices` und direkten `UiEvent.notice` gleich:

| Code | Argumente / Wirkung |
| --- | --- |
| `start-dns`, `start-connection`, `start-tls`, `start-robots` | Fehlerdialog; optional `status`, `detail`, `startUrl`; bei HTTPS-Verbindungsfehler explizite HTTP-Übernahme |
| `start-redirect-host` | Fehlerdialog; optional `target` |
| `disk-low`, vorhandener Fixture-Alias `low-disk` | `path`, `freeBytes`; Dialog zur vom Kern ausgeführten Pause |
| `auth-required` | `host` (mit Schema bei HTTP), `realm`; Basic-Dialog |
| `crawl-throttled`, `crawl-throttle-cleared` | `host`; dauerhafter Status pro Host, keine Dialoge |
| `crawl-completed`, `analysis-completed` | `crawled`; Hintergrundbenachrichtigung mit Anzahl |
| sonstige Codes / Commandfehler | vorhandener Fluent-Text bzw. Fehlerklasse; technische Details separat |

K04, E02/E03, K07 und O01 liefern in dieser Ausgangsbasis noch Stubs. Die UI verwendet ausschließlich vorhandene Commands. Ob diese Kernpakete genau diese freien Notice-Codes liefern sowie Profiländerungen beim Fortsetzen wartender Basic-Anfragen anwenden, muss N01 beim Zusammenführen prüfen. Das Backend-Pausieren bei RAM/Plattenplatz, die wartenden Auth-Anfragen und die verschlüsselte Profilablage werden nicht durch Frontendzustand simuliert. Der vollständige Bridge-Lifecycle-Test hat dafür den erlaubten C9-Platzhalter `wartet auf M2-K04`; im Vertragstransport läuft derselbe Test ohne Ignore. E2E-B (echte OS-Benachrichtigung und Zwischenablage) bleibt Aufgabe der vorgesehenen Desktop-Smokes.

## Abweichungen und offene Fragen

1. **Spec-Widerspruch:** UI-003 fordert HTTPS und 20 Vorschläge, CRAWL-001 HTTP und 50. Die Kopfleiste folgt UI-003; keine stille HTTP-Umschaltung.
2. **Binäre Listenquellen:** `parseList(path)` liefert ausschließlich eine begrenzte Vorschau. Deshalb arbeitet U03 für Upload/Einfügen/manuell mit vollständigem TXT/CSV-Text und kürzt niemals auf `sample`. Binäre XLSX-Dateien benötigen ein Kern-Importergebnis mit vollständiger Liste und sind hier nicht als Uploadformat angeboten. Sitemap-URLs werden bereits von `os-engine::list_input::plan` als Sitemap-Quelle erkannt und über den vorhandenen `StartSource.List` weitergereicht. Kein direkter Netzwerk-/Dateisystemzugriff und keine erfundenen Commands im Frontend.
3. **Integration:** Freie Notice-Codes und Wiederaufnahme nach Basic-Dialog wie oben prüfen. Keine nicht-additive Fremdänderung vorgenommen.
4. **CI:** Keine CI-Lauf-ID, da gemäß Auftrag nicht gepusht wird. Die lokalen Prüfungen ersetzen keine native Windows-/macOS-Abnahme.

## Fremdergänzungen (K2-61)

Drei strikt additive Einhängepunkte, damit U03 im Hauptfenster und mit den bereits installierten nativen Plugins funktioniert. Keine bestehenden Signaturen, Handler oder Tests geändert; keine fremden Fixtures/Goldens neu erzeugt.

### `apps/desktop/src/App.tsx`

```diff
diff --git a/apps/desktop/src/App.tsx b/apps/desktop/src/App.tsx
index 9de7e22..ba94144 100644
--- a/apps/desktop/src/App.tsx
+++ b/apps/desktop/src/App.tsx
@@ -1,3 +1,6 @@
+import { Header } from "./features/header/Header";
+import { Statusbar } from "./features/statusbar/Statusbar";
+import { Notifications } from "./features/notifications/Notifications";
 import { useEffect, useState } from "react";
 import { RegionBoundary } from "./app/ErrorBoundary";
 import { Providers } from "./app/Providers";
@@ -33,6 +36,7 @@ export function App() {
       <div className="os-app">
         <header className="os-app-header" role="banner" aria-label={t("app-region-header")}>
           <h1 className="os-app-title">{t("app-title")}</h1>
+          <Header />
         </header>
         <nav className="os-app-tabs" aria-label={t("app-region-tabs")} />
         <main className="os-app-main" data-region="table" aria-label={t("app-region-table")}>
@@ -42,8 +46,10 @@ export function App() {
         </main>
         <aside className="os-app-sidebar" aria-label={t("app-region-sidebar")} />
         <footer className="os-app-status" role="contentinfo" aria-label={t("app-region-status")}>
+          <Statusbar />
           {version !== null && <span data-testid="version">{t("app-version", { version })}</span>}
         </footer>
+        <Notifications />
       </div>
     </Providers>
   );
```

### `apps/desktop/src/ipc/transport/tauri.ts`

```diff
diff --git a/apps/desktop/src/ipc/transport/tauri.ts b/apps/desktop/src/ipc/transport/tauri.ts
index 0ae9d68..a5eca51 100644
--- a/apps/desktop/src/ipc/transport/tauri.ts
+++ b/apps/desktop/src/ipc/transport/tauri.ts
@@ -40,3 +40,16 @@ export function createTauriTransport(): Transport {
     },
   };
 }
+
+/** Additive shell helper (M2-U03, K2-61): native notification plugin, no app command bypass. */
+export async function sendDesktopNotification(title: string, body: string): Promise<void> {
+  const granted = await tauriInvoke<boolean | null>("plugin:notification|is_permission_granted");
+  if (granted === false) return;
+  if (granted === null && await tauriInvoke<string>("plugin:notification|request_permission") !== "granted") return;
+  await tauriInvoke("plugin:notification|notify", { options: { title, body } });
+}
+
+/** Additive shell helper (M2-U03, K2-61): copy diagnostic text through the installed clipboard plugin. */
+export async function copyDesktopText(text: string): Promise<void> {
+  await tauriInvoke("plugin:clipboard-manager|write_text", { text });
+}
```

### `apps/desktop/src-tauri/capabilities/default.json`

```diff
diff --git a/apps/desktop/src-tauri/capabilities/default.json b/apps/desktop/src-tauri/capabilities/default.json
index 0991b9b..379d018 100644
--- a/apps/desktop/src-tauri/capabilities/default.json
+++ b/apps/desktop/src-tauri/capabilities/default.json
@@ -20,6 +20,7 @@
     "core:webview:allow-set-webview-zoom",
     "dialog:default",
     "notification:default",
+    "clipboard-manager:allow-write-text",
     "os:default",
     "os-app:default",
     "settings:default",
```

## Validierung

Lokale Plattform: macOS 26.3, arm64; Node 22.20.0. Tests ohne externe Webdienste und ohne OS-Schlüsselbund. Rust mit `OPENSPIDER_SECRET_STORE=memory`, deaktivierter inkrementeller Kompilierung und `line-tables-only`.

| Prüfung | Ergebnis |
| --- | --- |
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` | grün; 2 min 08 s |
| `cargo test --workspace --exclude openspider-desktop --locked` | grün; 2307 bestanden, 40 vorhandene Ignores; 258 Test-/Doc-Test-Suites; Kompilierung 4 min 18 s, Summe der Suitezeiten 315,26 s (ohne Cargo-/Link-/Doc-Test-Startzeiten) |
| `cargo deny check licenses bans sources` | grün |
| `npm ci` | grün; ca. 9 s, keine Manifest-/Lockfile-Änderung |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | grün; 169 Tests, davon 27 neue U03-Tests; letzter Lauf 5,07 s |
| `npm run -w apps/desktop check:static` | grün; Wrapper aktuell, npm-Lizenzen positiv, 28 statische/Token-Tests |
| `bash scripts/check-neutral.sh` | grün; nur vorhandene Warnungen über verwaiste Allowlist-Einträge |
| U03-E2E, Chromium + WebKit, Vertragstransport | 8 bestanden; 9,9 s |
| U03-E2E, Chromium + WebKit, echte Bridge | 6 bestanden, 2 C9-Platzhalter für K04; 7,4 s |
| Gesamte E2E-A-Suite, Chromium + WebKit, Bridge | 40 bestanden, 2 C9-Platzhalter; 28,2 s; mit CI-Retry-Konfiguration, kein Retry benötigt |
| `git diff --check` | grün |

Reproduzierbare Browseraufrufe (dynamischer, worktree-eigener Dev-Port; Artefakte im ignorierten `target/u03-e2e`):

```sh
cargo build -p os-app --features e2e-bridge --bin os-app-bridge --locked
OS_E2E_TRANSPORT=mock npm run -w apps/desktop e2e -- --config=e2e/header/playwright.config.ts --project=chromium --project=webkit
OS_E2E_TRANSPORT=bridge npm run -w apps/desktop e2e -- --config=e2e/header/playwright.config.ts --project=chromium --project=webkit
U03_E2E_ALL=1 OS_E2E_TRANSPORT=bridge npm run -w apps/desktop e2e -- --config=e2e/header/playwright.config.ts --project=chromium --project=webkit --retries=1
```

Beim zusätzlichen ersten Gesamtlauf scheiterte der vorhandene WebKit-Systemsprachentest (`e2e/i18n/language.spec.ts:45`, erwartet `de`, erhalten `en`); 39 andere Fälle bestanden, 2 waren zurückgestellt. Die Worker teilen ihr persistentes Bridge-Home zwischen Tests, sodass eine zuvor explizit gesetzte Sprache die Systemspracherwartung beeinflussen kann. Im vollständigen Folgelauf bestanden alle 40 aktiven Fälle ohne Retry. Keine fremde Testdatei geändert.

`npm ci` meldet Engine-Hinweise einzelner bestehender Abhängigkeiten, die mindestens Node 22.22.2 erwarten; die hier installierte Version 22.20.0 bestand alle genannten Frontendprüfungen. Keine CI-Lauf-ID und keine native Windows-/OS-Smoke-Abnahme erzeugt (Auftrag: kein Push).
