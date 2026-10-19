# M2-U14 – Paketbericht

## Umsetzung

- **UI-122**: registrierte Abschnitte Speicher, Crawls, Exporte, Netzwerk, Konten und MCP; disk-first ohne Moduswahl; drei Leistungsprofile.
- **UI-123**: aktueller Verbrauch von Engine und Renderer, sekündliche Aktualisierung, RAM-Eingabe mit einer Nachkommastelle und Hardwaregrenze `[1 GB, physischer RAM − 2 GB]`. Ungültige Entwürfe werden nicht gespeichert; RAM-Änderungen benötigen keinen Neustart.
- **CRAWL-124 [M2]**: nativer Ordnerdialog und bearbeitbarer Pfad; Risikoabfrage für Netzwerk-/Sync-/FAT-Standorte vor dem Speichern; Umzug nur ausdrücklich, mit Fortschritt, Abbruch, Fehleranzeige und sequenziellen OpHandle-Batches von höchstens 1.000 IDs. Laufende Crawls werden ausgelassen.
- **OUT-267**: wirksame Cache- und Renderergrenzen aus dem Kern, validierbare Überschreibungen; Standardwerte kommen aus vorhandener Hardware-/Ressourcenlogik.
- **OUT-229**: alle P1-Aufbewahrungsintervalle einschließlich benutzerdefinierter Tage; die Tageszahl wird vor dem Aktivieren der benutzerdefinierten Aufbewahrung bestätigt; ausdrücklicher Hinweis auf nicht mehr verfügbare Vergleiche vor Aktivierung. Löschschutz und Löschdurchführung bleiben beim vorhandenen Kern.
- **OUT-222**: ausschließlich App-Einstellungen, Exportordner und Standardformat, Anzeige von Neustartanforderungen, Fehler mit Wiederherstellung des gespeicherten Wertes. Kontenübersicht, Benachrichtigungskonto, automatische API-Verbindung und Trennen; MCP-Verweis auf das Hauptmenü.
- **CRAWL-122 / OUT-269**: System-/Direkt-/manueller Proxy, HTTP/HTTPS/SOCKS5, Host/Port, Benutzername, Ausnahmen; Kennwort ausschließlich über den Schlüsselbund-Command, anschließend aus dem Formular entfernt. App-weiter Geltungsbereich und Wirksamkeit ohne Neustart erklärt.
- **OUT-270**: OS-Truststore als Standard, Hinzufügen/Entfernen zusätzlicher CA-Dateien, Zertifikatsketten ermitteln, Aussteller anzeigen und erst nach ausdrücklicher Auswahl importieren. Ungültige Zertifikate ändern keine Vertrauenseinstellung.
- Alle neuen UI-Texte sind eigene Formulierungen, vollständig in de/en. Fixtures liegen innerhalb der eigenen Abschnittsordner; die öffentliche Test-CA enthält keinen privaten Schlüssel.

## Prüfungen

Ausführung lokal auf macOS arm64, ohne Zugriff auf das Referenzprodukt. Ausgangsworktree sauber; vorhandene Paketänderungen mussten nicht übernommen werden. Kein Push, Merge oder PR; deshalb keine CI-Lauf-ID.

| Prüfung | Ergebnis |
|---|---|
| `cargo fmt --all --check` | grün |
| `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` | grün, 14,66 s |
| `cargo test --workspace --exclude openspider-desktop --locked` | grün (Exit 0); 3030 bestanden, 89 vorhandene Ignores; summierte Test-Binary-Zeit 17.5 min, zusätzlich 48,53 s Build |
| `npm ci` | grün |
| `npm run -w apps/desktop typecheck` | grün |
| `npm run -w apps/desktop test` | 22 Dateien, 205 Tests grün, 8,02 s |
| Eigene Funktionstests | 14 Frontend-Tests und 2 Rust-Tests, jeweils mit Spec-Tags; keine eigenen Ignores |
| Eigene Playwright-Suite, Chromium + WebKit | 14 Tests grün, 11,4 s; vier echte Bridge-Fälle einschließlich Objekt-IPC für Zertifikate sowie de/1024×640 und axe-Prüfung aller sechs Abschnitte |
| `npm run -w apps/desktop check:static` | IPC-Generator, vollständige de/en-Kataloge, Quelltextregeln, Tokens und npm-Lizenzen grün |
| `cargo deny check licenses bans sources` | grün |
| `python3 docs/plan/M2-check.py` | grün: 109 Pakete, 590 Spec-IDs |

E2E-Reproduktion aus der Repository-Wurzel:

```sh
cargo build -p os-app --features e2e-bridge --bin os-app-bridge --locked
npm run -w apps/desktop e2e -- --config e2e/settings-sections/playwright.config.ts --project chromium --project webkit
```

Für Rust liefen die Prüfungen mit `CARGO_INCREMENTAL=0`, Debug-Informationen `line-tables-only` und beim Testlauf `OPENSPIDER_SECRET_STORE=memory`. Der Frontend-Test, der alle sechs Panels besucht, hat ein Funktions-Testbudget von 15 s für geteilte CI-Rechner; die Browserprüfungen bleiben unverändert.

### Umgebungsbedingter Wiederholungslauf

Der erste Workspace-Testlauf stoppte bei zwei CLI-Fortsetzungstests: Nur noch 0,4–0,5 GB freier Plattenplatz lösten die reguläre automatische Crawl-Pause aus (Exit 3). Aus den ausführbaren Dateien im eigenen `CARGO_TARGET_DIR` wurden Symbol-/Debug-Daten entfernt; dadurch wurden rund 9,65 GiB freigegeben. Der isolierte Wiederholungslauf `cargo test -p os-cli --test cli_load --locked` bestand anschließend alle 9 Tests. Der vollständige Workspace-Lauf wurde mit dem finalen Code erneut ausgeführt und erfolgreich abgeschlossen; das Ergebnis steht in der Prüftabelle.

## Fremdergänzungen (K2-61)

Die vorhandenen Verträge liefern weder physische RAM-Daten für die Eingabeprüfung noch eine Möglichkeit, ausdrücklich akzeptierte PEM-Ketten abzulegen. Es werden ausschließlich neue Funktionen und Registereinträge ergänzt. Vorhandene Command-Signaturen und ihr Verhalten bleiben erhalten.

| Datei | Additiver Inhalt |
|---|---|
| `crates/os-app/src/commands/settings.rs` | `settings.resources` (lesender Hardware-/Verbrauchssnapshot über `spawn_blocking`) und `settings.importCertificates` (Objekt-Payload für Tauri; begrenzte, validierte PEM-Ablage mit injizierter Kennungsquelle), beide registriert; eigene getaggte Funktionstests. |
| `apps/desktop/src/ipc/commands.json` | Zwei zusätzliche Einträge aus dem Register; dies ist zugleich das aktualisierte Manifest-Golden. |
| `apps/desktop/src/ipc/settings.ts` | Generierte Wrapper und zusätzlicher JsonValue-Typimport. |

Vollständiger Diff dieser Ergänzungen:

### `crates/os-app/src/commands/settings.rs`

```diff
diff --git a/crates/os-app/src/commands/settings.rs b/crates/os-app/src/commands/settings.rs
index 9c1ee85..b1d011f 100644
--- a/crates/os-app/src/commands/settings.rs
+++ b/crates/os-app/src/commands/settings.rs
@@ -29,8 +29,98 @@ fn set<'a>(app: &'a App, _ctx: CallCtx, args: serde_json::Value) -> BoxFuture<'a
     })
 }

+/// Read-only hardware snapshot for the settings UI (UI-123, OUT-267).
+fn resources<'a>(app: &'a App, _: CallCtx, _: serde_json::Value) -> BoxFuture<'a, CmdResult> {
+    let storage = app.settings_snapshot().storage;
+    Box::pin(async move {
+        tokio::task::spawn_blocking(move || {
+            let sample = os_engine::memory::sample();
+            let cores = std::thread::available_parallelism().map_or(1, |n| n.get() as u32);
+            let (ram, cache, renderers) =
+                os_engine::memory::limits(&storage, sample.physical_bytes, cores);
+            Ok(serde_json::json!({
+                "physicalBytes": sample.physical_bytes,
+                "usedBytes": sample.engine_bytes.saturating_add(sample.renderer_bytes),
+                "ramLimitBytes": ram,
+                "cacheLimitMb": cache >> 20,
+                "renderProcessLimit": renderers,
+            }))
+        })
+        .await
+        .map_err(|e| {
+            CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
+        })?
+    })
+}
+
+/// Persists explicitly accepted PEM certificates, after validating the entire chain.
+fn import_certificates<'a>(
+    app: &'a App,
+    _: CallCtx,
+    args: serde_json::Value,
+) -> BoxFuture<'a, CmdResult> {
+    let directory = app.config().home.join("certificates");
+    let id = app.config().ids.new_crawl_id(app.config().clock.now());
+    Box::pin(async move {
+        let certificates: Vec<String> =
+            parse_args(args.get("certificates").cloned().unwrap_or(args))?;
+        tokio::task::spawn_blocking(move || {
+            if certificates.is_empty()
+                || certificates.len() > 16
+                || certificates.iter().any(|c| c.len() > 1024 * 1024)
+            {
+                return Err(CmdError::invalid_input(
+                    "err-invalid-certificate",
+                    "network.trustedCertificates",
+                ));
+            }
+            let io = |e: std::io::Error| {
+                CmdError::new(ErrorCode::Io, "err-io").with_detail(e.to_string())
+            };
+            std::fs::create_dir_all(&directory).map_err(io)?;
+            let path = directory.join(format!("{id}.pem"));
+            std::fs::write(&path, certificates.join("\n")).map_err(io)?;
+            let network = os_model::settings::NetworkSettings {
+                trusted_certificates: vec![path.to_string_lossy().into_owned()],
+                ..Default::default()
+            };
+            if let Err(detail) = os_fetch::net::certificates::roots(&network) {
+                let _ = std::fs::remove_file(&path);
+                return Err(CmdError::invalid_input(
+                    "err-invalid-certificate",
+                    "network.trustedCertificates",
+                )
+                .with_detail(detail));
+            }
+            to_json(&network.trusted_certificates)
+        })
+        .await
+        .map_err(|e| {
+            CmdError::new(ErrorCode::Internal, "err-internal").with_detail(e.to_string())
+        })?
+    })
+}
+
 /// Trägt die Commands dieser Domäne in die Registry ein.
 pub fn register(r: &mut Registry) {
+    r.register(
+        CommandMeta {
+            name: "settings.resources",
+            args: "void",
+            ret: "JsonValue",
+            long_running: false,
+        },
+        resources,
+    );
+    r.register(
+        CommandMeta {
+            name: "settings.importCertificates",
+            args: "JsonValue",
+            ret: "string[]",
+            long_running: false,
+        },
+        import_certificates,
+    );
     r.register(
         CommandMeta {
             name: "settings.get",
@@ -50,3 +140,78 @@ pub fn register(r: &mut Registry) {
         set,
     );
 }
+
+#[cfg(test)]
+mod u14_tests {
+    use super::*;
+    use os_fetch::secrets::MemorySecretStore;
+    use std::sync::Arc;
+
+    fn app(dir: &std::path::Path) -> App {
+        let mut config = crate::AppConfig::new(dir);
+        config.secrets = Arc::new(MemorySecretStore::new());
+        App::new(config).unwrap()
+    }
+
+    // spec: UI-123
+    #[tokio::test]
+    async fn settings_resources_reports_live_limits_without_changing_settings() {
+        let dir = tempfile::tempdir().unwrap();
+        let app = app(dir.path());
+        let before = app.settings_snapshot();
+        let result = app
+            .dispatch(
+                CallCtx::for_window("settings"),
+                "settings.resources",
+                serde_json::Value::Null,
+            )
+            .await
+            .unwrap();
+        assert!(result["physicalBytes"].as_u64().unwrap() > 0);
+        assert!(result["ramLimitBytes"].as_u64().unwrap() >= 1 << 30);
+        assert!(result["cacheLimitMb"].as_u64().unwrap() >= 512);
+        assert!(result["renderProcessLimit"].as_u64().unwrap() >= 1);
+        assert_eq!(before, app.settings_snapshot());
+    }
+
+    // spec: OUT-270
+    #[tokio::test]
+    async fn explicitly_imported_fixture_ca_is_valid_and_invalid_input_leaves_no_file() {
+        let dir = tempfile::tempdir().unwrap();
+        let app = app(dir.path());
+        let pem = include_str!(
+            "../../../../apps/desktop/src/features/settings/sections/network/fixtures/ca.pem.txt"
+        );
+        let paths = app
+            .dispatch(
+                CallCtx::for_window("settings"),
+                "settings.importCertificates",
+                serde_json::json!({ "certificates": [pem] }),
+            )
+            .await
+            .unwrap();
+        let path = paths[0].as_str().unwrap();
+        assert_eq!(std::fs::read_to_string(path).unwrap(), pem);
+        // Discovering/importing a chain never silently changes trust settings.
+        assert!(app
+            .settings_snapshot()
+            .network
+            .trusted_certificates
+            .is_empty());
+        let error = app
+            .dispatch(
+                CallCtx::for_window("settings"),
+                "settings.importCertificates",
+                serde_json::json!({ "certificates": ["invalid"] }),
+            )
+            .await
+            .unwrap_err();
+        assert_eq!(error.code, ErrorCode::InvalidInput);
+        assert_eq!(
+            std::fs::read_dir(dir.path().join("certificates"))
+                .unwrap()
+                .count(),
+            1
+        );
+    }
+}
```

### `apps/desktop/src/ipc/commands.json`

```diff
diff --git a/apps/desktop/src/ipc/commands.json b/apps/desktop/src/ipc/commands.json
index 8d88835..9a887ce 100644
--- a/apps/desktop/src/ipc/commands.json
+++ b/apps/desktop/src/ipc/commands.json
@@ -17,6 +17,18 @@
     "ret": "void",
     "longRunning": false
   },
+  {
+    "name": "settings.resources",
+    "args": "void",
+    "ret": "JsonValue",
+    "longRunning": false
+  },
+  {
+    "name": "settings.importCertificates",
+    "args": "JsonValue",
+    "ret": "string[]",
+    "longRunning": false
+  },
   {
     "name": "settings.get",
     "args": "void",
```

### `apps/desktop/src/ipc/settings.ts`

```diff
diff --git a/apps/desktop/src/ipc/settings.ts b/apps/desktop/src/ipc/settings.ts
index 512297c..eaecc41 100644
--- a/apps/desktop/src/ipc/settings.ts
+++ b/apps/desktop/src/ipc/settings.ts
@@ -2,11 +2,16 @@
 import type { AppSettingsDto } from "./gen/AppSettingsDto";
 import type { CertificateChainDto } from "./gen/CertificateChainDto";
 import type { CertificateProbeArgs } from "./gen/CertificateProbeArgs";
+import type { JsonValue } from "./gen/serde_json/JsonValue";
 import type { ProxyPasswordArgs } from "./gen/ProxyPasswordArgs";
 import type { SettingsPatch } from "./gen/SettingsPatch";
 import { client } from "./client";

 export const settings = {
+  /** `settings.resources` */
+  resources: (signal?: AbortSignal) => client.invoke<JsonValue>("settings.resources", undefined, signal),
+  /** `settings.importCertificates` */
+  importCertificates: (args: JsonValue, signal?: AbortSignal) => client.invoke<string[]>("settings.importCertificates", args, signal),
   /** `settings.get` */
   get: (signal?: AbortSignal) => client.invoke<AppSettingsDto>("settings.get", undefined, signal),
   /** `settings.set` */
```

## Fremde Erwartungen (K2-63)

- `crates/os-app/tests/manifest_contract.rs`: Erwartungsmenge um die zwei notwendigen Commands erweitert (**UI-123 / OUT-267**, **OUT-270**). Keine Testlogik geändert.
- `apps/desktop/src/features/settings/frame/settings.test.tsx`: Erwartung der verfügbaren Abschnitte um die sechs gelieferten Module erweitert; für Speicher wird die echte Abschnittsmarkierung statt des Platzhalters erwartet (**UI-122**, **OUT-222**). Keine Testlogik geändert.

```diff
diff --git a/crates/os-app/tests/manifest_contract.rs b/crates/os-app/tests/manifest_contract.rs
index 0d31c97..cd389ee 100644
--- a/crates/os-app/tests/manifest_contract.rs
+++ b/crates/os-app/tests/manifest_contract.rs
@@ -20,6 +20,8 @@ const CATALOG_4_4: &[&str] = &[
     "events.subscribe",
     "events.setVisibility",
     "events.setInterval",
+    "settings.resources", // UI-123 / OUT-267: hardware limits in settings
+    "settings.importCertificates", // OUT-270: explicitly accepted probe certificates
     "settings.get",
     "settings.set",
     "uistate.load",
```

```diff
diff --git a/apps/desktop/src/features/settings/frame/settings.test.tsx b/apps/desktop/src/features/settings/frame/settings.test.tsx
index 9409f17..82f6082 100644
--- a/apps/desktop/src/features/settings/frame/settings.test.tsx
+++ b/apps/desktop/src/features/settings/frame/settings.test.tsx
@@ -36,7 +36,7 @@ describe("Einstellungsfenster: Rahmen", () => {
   it("kennt alle Abschnitte aus 1.5; Allgemein und Oberfläche sind fertig, die übrigen Platzhalter", () => {
     const sections = resolveSections();
     expect(sections.map((s) => s.id)).toEqual(KNOWN_SECTIONS.map((k) => k.id));
-    expect(sections.filter((s) => s.Component).map((s) => s.id)).toEqual(["general", "appearance"]);
+    expect(sections.filter((s) => s.Component).map((s) => s.id)).toEqual(["general", "appearance", "storage", "crawls", "exports", "network", "accounts", "mcp"]);
     expect(sectionOfAnchor("appearance.zoom")).toBe("appearance");
   });

@@ -45,7 +45,7 @@ describe("Einstellungsfenster: Rahmen", () => {
     const nav = screen.getByRole("tablist", { name: "Settings sections" });
     expect(within(nav).getAllByRole("tab")).toHaveLength(KNOWN_SECTIONS.length);
     fireEvent.click(within(nav).getByRole("tab", { name: "Storage" }));
-    expect(document.querySelector('[data-settings-placeholder="storage"]')).not.toBeNull();
+    expect(document.querySelector('[data-settings-section="storage"]')).not.toBeNull();
   });

   // spec: UI-016
```

## Abweichungen und offene Fragen

Keine fachlichen Abweichungen und keine offenen Fragen im Paketumfang. P2-Aufbewahrung nach Anzahl bleibt außerhalb dieses Pakets. Großcrawl-Messungen und die Ausführung der vorhandenen Lösch-/Netzwerk-/Ressourcenschutzlogik gehören den jeweiligen Kernpaketen; U14 prüft ihre Einstellungsoberfläche und Verträge. Native Systemdialoge werden im Frontend-Vertrag geprüft; WKWebView/WebView2 und die CI-Matrix wurden nicht gesondert gestartet.

K2-60 wurde nicht benötigt; keine fremden F05-Stubs ersetzt. Keine Fremdergänzung außerhalb der oben dokumentierten Dateien.

## Lokale Commits

- `f0756b4` – Ressourcen- und Zertifikats-Erweiterungspunkte.
- `dc062cc` – sechs Einstellungsabschnitte und Übersetzungen.
- `1140991` – Objekt-Argumente für den nativen Zertifikatsimport.
- `a60b6c5` – benutzerdefinierte Aufbewahrungsdauer vor Aktivierung bestätigen.
- `1d7049a` – Fixture-, Browser- und IPC-Tests.
- Der abschließende `docs:`-Commit enthält diesen Bericht; seine Kennung steht in der Abschlussnachricht.
