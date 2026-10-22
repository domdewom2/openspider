# M2-T02 – Implementierungsbericht

Umgesetzte Spec-IDs: **OUT-280, OUT-281, OUT-282**; mitgeprüft: **CRAWL-122, OUT-269**.

Kontenmetadaten werden atomar unter `settings/mail/` gespeichert; Kennwörter und OAuth-Geheimnisse ausschließlich über `SecretStoreMut` unter `mail/<id>`. Gmail und Microsoft 365 verwenden PKCE, Versand-Scopes, Refresh-Tokens und XOAUTH2. Die vier `mail.*`-Commands behalten ihre Verträge; `testAccount` liefert eine abbrechbare Operation und strukturierte SMTP-Fehlercodes.

UI- und CLI-Abschlüsse sowie geplante Läufe nutzen den gemeinsamen SMTP-Versand. Bei --no-store werden die Mail-Zahlen vor dem Löschen der temporären Crawl-Datei erhalten. OAuth-Code- und Token-Anfragen verwenden denselben injizierten Proxy-Schlüssel wie SMTP. UI-Mails warten auf die automatische Analyse; Öffnen einer Datei versendet nichts. Eine Aufgabe mit leerer Empfängerliste versendet nichts und bleibt gültig. Fehlermails haben `[FAILED]` im Betreff. SMTP unterstützt STARTTLS, implizites TLS und ausdrücklich bestätigtes Klartext-SMTP, HTTP-CONNECT/HTTPS-/SOCKS5-Proxys, Basic-/SOCKS-Authentifizierung und Ausnahmen. TLS prüft Plattform-Vertrauen plus die app-weiten Zusatz-Zertifikate.

HTML und Klartext enthalten Overview-Zahlen, Antwortcodes, Issues und optional die Vergleichstabelle. New/Missing und Change-Detection-Zahlen stammen aus den gemappten Paaren derselben Vergleichslogik. Prozentwerte bei Previous=0 bleiben leer, Nulländerungen folgen skip_empty, Veränderungen ab 10 URLs und 10 Prozent werden hervorgehoben.

## Prüfungen

- `cargo fmt --all --check`: grün, nach den letzten Änderungen erneut geprüft.
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings`: grün, nach den letzten Änderungen erneut geprüft.
- `cargo test --workspace --exclude openspider-desktop --locked`: grün; 3601 Tests bestanden, 63 bereits vorhandene Ignorierungen. Summe der von den Testläufen gemeldeten Laufzeiten: 866.90 s.
- Nach der abschließenden Pfadbereinigung und HTML-Geheimnis-Maskierung gezielt erneut geprüft: 12 Delivery-Tests + 4 Lifecycle-Tests + 1 SMTP-Proxy-Unit-Test = **17 eigene Mailtests grün**, zusätzlich 17 vorhandene os-app-Lib-Tests. Keine neuen Ignorierungen.
- `npm ci`: erfolgreich; `npm run -w apps/desktop typecheck`: erfolgreich; Frontend: 72 Dateien / 711 Tests erfolgreich (40,90 s); `npm run -w apps/desktop check:static`: 28 Tests grün (2,43 s).
- `cargo deny --offline check licenses bans sources`: erfolgreich. Keine neuen Abhängigkeiten.
- Eigene Mailtests ausschließlich mit Loopback-Mocks und MemorySecretStore; keine echten Konten, Provider oder Schlüsselbunde geöffnet.

Build-Umgebung wegen des lokalen Plattenlimits: `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_STRIP=debuginfo`, `CARGO_PROFILE_TEST_STRIP=debuginfo`, `CARGO_PROFILE_DEV_OPT_LEVEL=1`, `CARGO_PROFILE_TEST_OPT_LEVEL=1`, `CARGO_BUILD_JOBS=4`, `MACOSX_DEPLOYMENT_TARGET=13.0`, `OPENSPIDER_SECRET_STORE=memory`. Vorherige Vollbuild-Versuche mit mehr Debug-Symbolen wurden wegen des Plattenlimits verworfen; ein Zwischenlauf mit strip=symbols beeinflusste den Backtrace-Test und wurde durch strip=debuginfo ersetzt. Funktionssymbole bleiben erhalten, Produktcode und Backtrace-Test bleiben unverändert. Ausschließlich das eigene Cargo-Target wurde bereinigt.

## Abweichungen und offene Fragen

Keine fachlichen Abweichungen und keine offenen Rückfragen. Echte Provider und der OS-Schlüsselbund werden gemäß Auftrag nicht im Test angesprochen. Native Windows-Systemproxy-Auflösung ist lokal auf macOS nicht ausführbar. Keine CI-Lauf-ID, da gemäß Auftrag kein Push erfolgt. Kein Merge, kein PR.

Implementierungscommits: `60311980`, `891a9877`, `af8f16f2`, `6c62b508`.

Der Bericht liegt im eigenen Fixture-Namensraum, da T02 keine Berichtspfade unter docs/plan besitzt. Clean-Room eingehalten.

## Fremdergänzungen (K2-61)

Alle folgenden Änderungen sind additive Erweiterungspunkte; die bestehenden öffentlichen Signaturen bleiben erhalten. Bei AccountSettings wird die abgeleitete Default-Implementierung durch eine gleichwertige Implementierung für die bisherigen Felder ergänzt, damit ausschließlich das neue notify_on_failure-Feld den Default true erhält. Die beiden TypeScript-Dateien sind regenerierte Ausgaben ausschließlich eigener neuer Felder.

### `apps/desktop/src/ipc/__fixtures__/AppSettingsDto.json`

OUT-281: Roundtrip-Fixture ausschließlich um die drei eigenen optionalen Ausgabefelder (null) ergänzt; Testlogik unverändert.

````diff
diff --git a/apps/desktop/src/ipc/__fixtures__/AppSettingsDto.json b/apps/desktop/src/ipc/__fixtures__/AppSettingsDto.json
index 4d5da1f9..afa77b3d 100644
--- a/apps/desktop/src/ipc/__fixtures__/AppSettingsDto.json
+++ b/apps/desktop/src/ipc/__fixtures__/AppSettingsDto.json
@@ -41,6 +41,9 @@
   "accounts": {
     "mailAccounts": [],
     "notifyAccount": null,
+    "notifyTo": null,
+    "notifyOnSuccess": null,
+    "notifyOnFailure": null,
     "autoConnect": []
   },
   "mcp": {
````

### `crates/os-cli/src/exec.rs`

OUT-281/282: neues Outcome-Feld mit Default None und zusätzliche Zusammenfassung vor dem Löschen bei --no-store; bestehende Ergebnisfelder und Löschverhalten unverändert.

````diff
diff --git a/crates/os-cli/src/exec.rs b/crates/os-cli/src/exec.rs
index 4c0bc3f1..ac921eef 100644
--- a/crates/os-cli/src/exec.rs
+++ b/crates/os-cli/src/exec.rs
@@ -42,6 +42,8 @@ pub struct Outcome {
     pub crawl_id: Option<String>,
     /// Anzahl gecrawlter URLs.
     pub crawled: Option<u64>,
+    /// Optional completion data retained before --no-store discards the database (OUT-281/282).
+    pub mail_completion: Option<serde_json::Value>,
 }
 
 impl Outcome {
@@ -53,6 +55,7 @@ impl Outcome {
             crawl_path: None,
             crawl_id: None,
             crawled: None,
+            mail_completion: None,
         }
     }
 
@@ -155,6 +158,11 @@ async fn run(plan: &Plan, rep: &Arc<Reporter>, signals: &Signals) -> Result<Outc
         }
     }
 
+    let mail_completion = if plan.no_store {
+        crate::mail::capture_before_discard(&plan.paths.settings, &crawled.path)
+    } else {
+        None
+    };
     if plan.no_store {
         discard(&crawled.path, rep);
     }
@@ -166,7 +174,11 @@ async fn run(plan: &Plan, rep: &Arc<Reporter>, signals: &Signals) -> Result<Outc
         crawled.crawled, files, warnings
     );
     // Nach `--no-store` gibt es die Datei nicht mehr; ihre Kennung bleibt für die Historie.
-    let crawl_info = |outcome: Outcome| outcome.with_crawl(&crawled.path, Some(crawled.crawled));
+    let crawl_info = |outcome: Outcome| {
+        let mut outcome = outcome.with_crawl(&crawled.path, Some(crawled.crawled));
+        outcome.mail_completion = mail_completion.clone();
+        outcome
+    };
     if failures > 0 {
         return Ok(crawl_info(Outcome::new(
             Exit::ExportFailed,
````

### `crates/os-model/src/settings.rs`

OUT-281: neue globale Empfänger/Erfolg-/Fehler-Optionen mit Defaults; Default der bisherigen Felder bleibt gleich.

````diff
diff --git a/crates/os-model/src/settings.rs b/crates/os-model/src/settings.rs
index b93e6a67..ea35b0e6 100644
--- a/crates/os-model/src/settings.rs
+++ b/crates/os-model/src/settings.rs
@@ -220,17 +220,41 @@ pub struct NetworkSettings {
 }
 
 /// Verweise auf Konten; die Zugangsdaten liegen im Schlüsselbund.
-#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
 #[serde(default)]
 pub struct AccountSettings {
     /// Kennungen der E-Mail-Konten (OUT-280).
     pub mail_accounts: Vec<String>,
     /// Konto für Benachrichtigungen bei Crawl-Ende; nicht gesetzt = keine Mail.
     pub notify_account: Option<String>,
+    /// Global completion recipients (OUT-281); empty disables mail.
+    #[serde(default)]
+    pub notify_to: Vec<String>,
+    #[serde(default)]
+    pub notify_on_success: bool,
+    #[serde(default = "notify_failure_default")]
+    pub notify_on_failure: bool,
     /// API-Quellen, die beim Start automatisch verbunden werden (INTEG-011), als Quellen-Kennung (`ga4`, `gsc`, …).
     pub auto_connect: Vec<String>,
 }
 
+impl Default for AccountSettings {
+    fn default() -> Self {
+        Self {
+            mail_accounts: vec![],
+            notify_account: None,
+            auto_connect: vec![],
+            notify_to: vec![],
+            notify_on_success: false,
+            notify_on_failure: true,
+        }
+    }
+}
+
+fn notify_failure_default() -> bool {
+    true
+}
+
 /// MCP in der App.
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
 #[serde(default)]
````

### `crates/os-app/src/dto/app.rs`

OUT-281: optionale Spiegel der neuen Einstellungen; alte DTO-Eingaben bleiben gültig.

````diff
diff --git a/crates/os-app/src/dto/app.rs b/crates/os-app/src/dto/app.rs
index fdce25d5..d4d8ebfb 100644
--- a/crates/os-app/src/dto/app.rs
+++ b/crates/os-app/src/dto/app.rs
@@ -152,6 +152,12 @@ pub struct AccountSettingsDto {
     pub mail_accounts: Vec<String>,
     /// Konto für Benachrichtigungen.
     pub notify_account: Option<String>,
+    #[serde(default)]
+    pub notify_to: Option<Vec<String>>,
+    #[serde(default)]
+    pub notify_on_success: Option<bool>,
+    #[serde(default)]
+    pub notify_on_failure: Option<bool>,
     /// Beim Start automatisch zu verbindende API-Quellen.
     pub auto_connect: Vec<String>,
 }
````

### `crates/os-app/src/dto/mail.rs`

OUT-280: optionale Bestätigung für Klartext, OAuth-Art, Client-ID und Client-Secret; alle Felder mit Default.

````diff
diff --git a/crates/os-app/src/dto/mail.rs b/crates/os-app/src/dto/mail.rs
index abd5ae1a..bd047a66 100644
--- a/crates/os-app/src/dto/mail.rs
+++ b/crates/os-app/src/dto/mail.rs
@@ -58,6 +58,17 @@ pub struct MailAccountInput {
     pub from: String,
     /// Kennwort; `null` = unverändert lassen.
     pub password: Option<String>,
+    /// Explicit consent required for cleartext SMTP.
+    #[serde(default)]
+    pub allow_insecure: Option<bool>,
+    /// password (default), gmail or microsoft365.
+    #[serde(default)]
+    pub auth: Option<String>,
+    #[serde(default)]
+    pub client_id: Option<String>,
+    /// OAuth application secret; stored only in the secret store.
+    #[serde(default)]
+    pub client_secret: Option<String>,
 }
 
 /// Argument von `mail.saveAccount`.
````

### `apps/desktop/src/ipc/gen/AccountSettingsDto.ts`

OUT-281: ausschließlich Ausgabe der ergänzten DTO-Felder neu erzeugt.

````diff
diff --git a/apps/desktop/src/ipc/gen/AccountSettingsDto.ts b/apps/desktop/src/ipc/gen/AccountSettingsDto.ts
index 3597fdd7..0f4de320 100644
--- a/apps/desktop/src/ipc/gen/AccountSettingsDto.ts
+++ b/apps/desktop/src/ipc/gen/AccountSettingsDto.ts
@@ -11,7 +11,7 @@ mailAccounts: Array<string>,
 /**
  * Konto für Benachrichtigungen.
  */
-notifyAccount?: string | null, 
+notifyAccount?: string | null, notifyTo?: Array<string> | null, notifyOnSuccess?: boolean | null, notifyOnFailure?: boolean | null, 
 /**
  * Beim Start automatisch zu verbindende API-Quellen.
  */
````

### `apps/desktop/src/ipc/gen/MailAccountInput.ts`

OUT-280: ausschließlich Ausgabe der ergänzten DTO-Felder neu erzeugt.

````diff
diff --git a/apps/desktop/src/ipc/gen/MailAccountInput.ts b/apps/desktop/src/ipc/gen/MailAccountInput.ts
index 7275a8a1..4b18bf14 100644
--- a/apps/desktop/src/ipc/gen/MailAccountInput.ts
+++ b/apps/desktop/src/ipc/gen/MailAccountInput.ts
@@ -36,4 +36,16 @@ from: string,
 /**
  * Kennwort; `null` = unverändert lassen.
  */
-password?: string | null, };
+password?: string | null, 
+/**
+ * Explicit consent required for cleartext SMTP.
+ */
+allowInsecure?: boolean | null, 
+/**
+ * password (default), gmail or microsoft365.
+ */
+auth?: string | null, clientId?: string | null, 
+/**
+ * OAuth application secret; stored only in the secret store.
+ */
+clientSecret?: string | null, };
````

### `crates/os-app/src/runtime/services.rs`

OUT-281: zusätzlicher Startaufruf für den Mail-Lifecycle-Hörer.

````diff
diff --git a/crates/os-app/src/runtime/services.rs b/crates/os-app/src/runtime/services.rs
index 6f455609..f2491b35 100644
--- a/crates/os-app/src/runtime/services.rs
+++ b/crates/os-app/src/runtime/services.rs
@@ -12,6 +12,7 @@ pub fn start(app: &App) {
     crate::commands::api::on_startup(app);
     crate::commands::crawls::start_retention(app);
     crate::commands::tasks::on_startup(app);
+    crate::commands::mail::on_startup(app);
     app.crawl_events().subscribe(Box::new(|app, event| {
         if let CrawlEvent::ConfigChanged { key, old, new } = event {
             crate::commands::api::on_config_changed(app, key, old, new);
````

### `crates/os-app/src/session/analysis.rs`

OUT-281/282: zusätzlicher Abschluss-Hook nach veröffentlichter automatischer Analyse.

````diff
diff --git a/crates/os-app/src/session/analysis.rs b/crates/os-app/src/session/analysis.rs
index 7f8654ed..f2279305 100644
--- a/crates/os-app/src/session/analysis.rs
+++ b/crates/os-app/src/session/analysis.rs
@@ -141,6 +141,9 @@ async fn finish_operation(
         },
     };
     publish(&app, &s.key, final_status.clone());
+    if automatic {
+        crate::commands::mail::after_analysis(&app, &s.key);
+    }
     super::feed::invalidate(&app, &s.key);
     if cancel.is_cancelled() {
         return Err(CmdError::new(ErrorCode::Cancelled, "err-cancelled"));
````

### `crates/os-app/src/session/control.rs`

OUT-281: zusätzlicher Fehler-Hook für Engine-Fehler, die kein Completed-Ereignis erzeugen.

````diff
diff --git a/crates/os-app/src/session/control.rs b/crates/os-app/src/session/control.rs
index 210da4ed..39815318 100644
--- a/crates/os-app/src/session/control.rs
+++ b/crates/os-app/src/session/control.rs
@@ -407,6 +407,9 @@ pub async fn settle(app: &App, s: &SessionInfo, action: Option<bool>) -> Result<
                 .map_err(|e| crate::runtime::error::store_error(&e))
         })
         .await?;
+    if result.is_err() {
+        crate::commands::mail::on_failed(app, &s.key);
+    }
     let summary = result?;
     let event = match summary.state {
         CrawlState::Paused => CrawlEvent::Paused(s.key.clone()),
````

### `crates/os-cli/src/lib.rs`

OUT-281: zusätzliche globale Benachrichtigung nach normalem CLI-Crawl; Aufgaben haben ihren eigenen Abschluss-Hook.

````diff
diff --git a/crates/os-cli/src/lib.rs b/crates/os-cli/src/lib.rs
index ff9cbea7..96c40d8f 100644
--- a/crates/os-cli/src/lib.rs
+++ b/crates/os-cli/src/lib.rs
@@ -308,7 +308,13 @@ pub fn run(cli: Cli) -> i32 {
                 )
             }
         };
-        exec::execute(plan, Arc::clone(&rep), signals).await
+        let settings_dir = plan.paths.settings.clone();
+        let completion_mail = plan.resume || !matches!(&plan.source, plan::Source::Load { .. });
+        let outcome = exec::execute(plan, Arc::clone(&rep), signals).await;
+        if completion_mail {
+            mail::notify_global(&settings_dir, &outcome, &rep).await;
+        }
+        outcome
     });
     // Offene Hintergrundaufgaben (Signalwächter) nicht abwarten.
     runtime.shutdown_background();
@@ -353,3 +359,5 @@ mod tests {
         assert!(args.read_only);
     }
 }
+
+mod mail;
````

### `crates/os-cli/src/mail.rs`

OUT-281: neues Hilfsmodul für globale CLI-Benachrichtigungen; Pause/SIGINT versendet keine Abschlussmail.

````diff
diff --git a/crates/os-cli/src/mail.rs b/crates/os-cli/src/mail.rs
new file mode 100644
index 00000000..b54ca293
--- /dev/null
+++ b/crates/os-cli/src/mail.rs
@@ -0,0 +1,59 @@
+//! Global completion delivery for ordinary CLI crawls (OUT-281, additive K2-61).
+use crate::{exec::Outcome, reporter::Reporter};
+use std::path::Path;
+pub async fn notify_global(dir: &Path, outcome: &Outcome, rep: &Reporter) {
+    if outcome.exit == crate::exit::Exit::Interrupted {
+        return;
+    }
+    let Ok(settings) = os_model::settings::AppSettings::load(dir) else {
+        return;
+    };
+    let policy = os_sched::mail::Policy {
+        on_success: settings.accounts.notify_on_success,
+        on_failure: settings.accounts.notify_on_failure,
+        account: settings.accounts.notify_account,
+        to: settings.accounts.notify_to,
+    };
+    let failed = outcome.exit != crate::exit::Exit::Ok;
+    if !policy.enabled(failed) {
+        return;
+    }
+    let completion = outcome
+        .mail_completion
+        .clone()
+        .or_else(|| {
+            outcome
+                .crawl_path
+                .as_ref()
+                .and_then(|p| os_analyze::mail_completion(p, None, false, false).ok())
+        })
+        .and_then(|v| serde_json::from_value::<os_sched::mail::Completion>(v).ok());
+    let mut completion = completion.unwrap_or_default();
+    if failed {
+        completion.error = Some(outcome.summary.clone());
+    }
+    if let Err(e) = os_sched::mail::notify(
+        dir,
+        os_fetch::secret_store_from_env().as_ref(),
+        &settings.network,
+        &policy,
+        &completion,
+    )
+    .await
+    {
+        rep.warn(format!("Abschlussmail fehlgeschlagen: {e}"));
+    }
+}
+
+/// Preserve summary facts before a temporary crawl file is removed; disabled mail does no analysis work.
+pub fn capture_before_discard(dir: &Path, path: &Path) -> Option<serde_json::Value> {
+    let settings = os_model::settings::AppSettings::load(dir).ok()?;
+    let a = &settings.accounts;
+    if a.notify_account.is_none()
+        || a.notify_to.iter().all(|s| s.trim().is_empty())
+        || !(a.notify_on_success || a.notify_on_failure)
+    {
+        return None;
+    }
+    os_analyze::mail_completion(path, None, false, false).ok()
+}
````

### `crates/os-cli/src/run_task.rs`

OUT-281/282: zusätzlicher Abschlussaufruf und Hilfsfunktion mit RunCtx; neues opt-in am Hook-Trait (Default false), DefaultHooks aktiviert den eingebauten Versand. Bestehende benutzerdefinierte Hooks behalten ihren eigenen Versandpfad.

````diff
diff --git a/crates/os-cli/src/run_task.rs b/crates/os-cli/src/run_task.rs
index b3c7d227..38f3c330 100644
--- a/crates/os-cli/src/run_task.rs
+++ b/crates/os-cli/src/run_task.rs
@@ -48,6 +48,10 @@ pub const RUN_ID_ENV: &str = "OPENSPIDER_TASK_RUN_ID";
 /// ([`DefaultHooks`]) meldet jede angeforderte Erweiterung als Warnung (der Lauf endet dann mit
 /// „Completed with warnings“); Tests und spätere Pakete setzen eigene Umsetzungen ein.
 pub trait TaskHooks: Send + Sync {
+    /// Opt in to the built-in completion mail; custom hooks retain their own notification path.
+    fn builtin_completion_mail(&self) -> bool {
+        false
+    }
     /// API-Quellen der Aufgabe (OUT-164): Abruf für den fertigen Crawl; Rückgabe = Warnungen.
     fn fetch_apis(&self, task: &Task, _crawl: Option<&Path>) -> Vec<String> {
         task.apis
@@ -171,7 +175,11 @@ impl TaskHooks for SeriesHooks {
 #[derive(Debug, Default, Clone, Copy)]
 pub struct DefaultHooks;
 
-impl TaskHooks for DefaultHooks {}
+impl TaskHooks for DefaultHooks {
+    fn builtin_completion_mail(&self) -> bool {
+        true
+    }
+}
 
 /// Umgebung eines Laufs.
 pub struct RunCtx {
@@ -515,6 +523,11 @@ pub async fn run_in(
             exit.code(),
             record.warnings.len()
         );
+        if hooks.builtin_completion_mail() {
+            if let Err(e) = send_completion_mail(ctx, &task, &record) {
+                rep.warn(format!("Benachrichtigung nicht gesendet: {}", redact(&e)));
+            }
+        }
         if let Err(e) = hooks.notify(&task, &record) {
             rep.warn(format!("Benachrichtigung nicht gesendet: {}", redact(&e)));
         }
@@ -947,3 +960,71 @@ pub fn run_with(task_id: &str, args: &RunArgs, hooks: &dyn TaskHooks) -> i32 {
     );
     run.exit.code()
 }
+
+// Additive completion hook (OUT-281 / OUT-282, K2-61).
+fn send_completion_mail(ctx: &RunCtx, task: &Task, record: &RunRecord) -> Result<(), String> {
+    let dir = ctx.paths.settings.clone();
+    let crawls = ctx.paths.crawls.clone();
+    let task = task.clone();
+    let record = record.clone();
+    if task.notify.to.iter().all(|s| s.trim().is_empty())
+        || !(task.notify.on_success || task.notify.on_failure)
+    {
+        return Ok(());
+    }
+    let secrets = ctx.secrets.clone();
+    std::thread::spawn(move || {
+        let runtime = tokio::runtime::Builder::new_current_thread()
+            .enable_all()
+            .build()
+            .map_err(|_| "Mail-Laufzeit nicht verfügbar".to_string())?;
+        let current = record
+            .crawl_id
+            .as_ref()
+            .map(|id| crawls.join(format!("{id}.ospider")))
+            .filter(|p| p.exists());
+        let previous = if task.execution.auto_compare {
+            os_store::registry::list_crawls(&crawls)
+                .unwrap_or_default()
+                .into_iter()
+                .filter(|c| {
+                    Some(&c.path) != current.as_ref()
+                        && c.meta.project == task.project_name()
+                        && c.meta.state == os_model::CrawlState::Completed
+                        && c.meta.started_at.is_some_and(|t| {
+                            os_model::Timestamp::from_db(&record.started_at)
+                                .is_ok_and(|start| t < start)
+                        })
+                })
+                .max_by_key(|c| c.meta.started_at)
+                .map(|c| c.path)
+        } else {
+            None
+        };
+        let completion = match current {
+            Some(path) => serde_json::from_value(os_analyze::mail_completion(
+                &path,
+                previous.as_deref(),
+                task.execution.auto_compare,
+                task.export_options.as_ref().is_some_and(|o| o.skip_empty),
+            )?)
+            .map_err(|_| "Mail-Daten ungültig".to_string())?,
+            None => os_sched::mail::Completion::default(),
+        };
+        runtime
+            .block_on(os_sched::mail::notify_task(
+                &dir,
+                &crawls,
+                secrets
+                    .as_mut_store()
+                    .ok_or("Mail-Schlüsselbund ist schreibgeschützt")?,
+                &task,
+                &record,
+                completion,
+            ))
+            .map_err(|e| e.to_string())?;
+        Ok(())
+    })
+    .join()
+    .map_err(|_| "Mail-Versand abgebrochen".to_string())?
+}
````

### `crates/os-analyze/src/lib.rs`

OUT-282: neue Exportfunktion für einen lesenden Abschluss-Snapshot.

````diff
diff --git a/crates/os-analyze/src/lib.rs b/crates/os-analyze/src/lib.rs
index 34c70698..363e4d70 100644
--- a/crates/os-analyze/src/lib.rs
+++ b/crates/os-analyze/src/lib.rs
@@ -31,3 +31,14 @@ pub use defs::{
     Requirement, SearchOps, SortKey, TabDef,
 };
 pub use error::{AnalyzeError, NameError, ViewError};
+
+mod mail_completion;
+/// Completion email data using the same overview, issue and comparison sources as the UI.
+pub fn mail_completion(
+    path: &std::path::Path,
+    previous: Option<&std::path::Path>,
+    auto_compare: bool,
+    skip_empty: bool,
+) -> Result<serde_json::Value, String> {
+    mail_completion::snapshot(path, previous, auto_compare, skip_empty)
+}
````

### `crates/os-analyze/src/mail_completion.rs`

OUT-282: neues Snapshot-Modul, nutzt dieselben Overview-/Issue-/Vergleichsabfragen; Elementzählung anhand der gemappten Paare, einschließlich MinHash.

````diff
diff --git a/crates/os-analyze/src/mail_completion.rs b/crates/os-analyze/src/mail_completion.rs
new file mode 100644
index 00000000..d4b466e6
--- /dev/null
+++ b/crates/os-analyze/src/mail_completion.rs
@@ -0,0 +1,191 @@
+//! Read-only completion snapshot, sharing overview/issue/compare queries (OUT-282, K2-61).
+use os_store::{CrawlFile, OpenMode};
+use serde_json::{json, Value};
+use std::path::Path;
+
+pub fn snapshot(
+    path: &Path,
+    previous: Option<&Path>,
+    auto_compare: bool,
+    skip_empty: bool,
+) -> Result<Value, String> {
+    let file = CrawlFile::open(path, OpenMode::ReadOnly).map_err(|e| e.to_string())?;
+    let meta = file.meta().map_err(|e| e.to_string())?;
+    let cfg = file.config().map_err(|e| e.to_string())?;
+    let overview =
+        os_store::queries::app_support::overview_summary(file.conn()).map_err(|e| e.to_string())?;
+    let mut stmt=file.conn().prepare("SELECT COALESCE(CAST(status_code AS TEXT),'No Response'),count(*) FROM urls WHERE state='done' AND robots_blocked=0 GROUP BY status_code ORDER BY status_code").map_err(|e|e.to_string())?;
+    let codes = stmt
+        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, u64>(1)?)))
+        .map_err(|e| e.to_string())?
+        .collect::<Result<Vec<_>, _>>()
+        .map_err(|e| e.to_string())?;
+    let counts = crate::views::filter_counts(&file, &cfg).map_err(|e| e.to_string())?;
+    let issues=crate::issues::issues_from_counts(&counts).into_iter().map(|i|json!({"name":format!("{} / {}",i.tab,i.filter),"kind":i.issue_type.to_string(),"priority":i.priority.to_string(),"urls":i.count})).collect::<Vec<_>>();
+    let mut changes = None;
+    if auto_compare {
+        if let Some(previous) = previous {
+            let work = path.parent().unwrap_or(Path::new("."));
+            let cancel = os_model::CancelToken::new();
+            let current = crate::compare::CrawlSide::open(path, work).map_err(|e| e.to_string())?;
+            let previous =
+                crate::compare::CrawlSide::open(previous, work).map_err(|e| e.to_string())?;
+            let mut cmp =
+                crate::compare::Comparison::open(current, previous, &cfg.compare.mapping, &cancel)
+                    .map_err(|e| e.to_string())?;
+            let mut rows=cmp.overview(&cancel).map_err(|e|e.to_string())?.into_iter().filter_map(|f|Some(json!({"name":format!("{} / {}",f.tab,f.label),"previous":f.previous?,"current":f.current?}))).collect::<Vec<_>>();
+            let p = cmp.pairing();
+            rows.push(
+                json!({"name":"New","previous":0,"current":(&p.current-&p.paired_current).len()}),
+            );
+            rows.push(json!({"name":"Missing","previous":(&p.previous-&p.paired_previous).len(),"current":0}));
+            rows.extend(element_changes(&cmp, &cfg)?);
+            changes = Some(rows);
+        }
+    }
+    let start = meta.started_at.map(|t| t.to_db()).unwrap_or_default();
+    let end = meta.ended_at.map(|t| t.to_db()).unwrap_or_default();
+    let duration = match (meta.started_at, meta.ended_at) {
+        (Some(a), Some(b)) => (b.unix_millis() - a.unix_millis()).max(0),
+        _ => 0,
+    };
+    Ok(
+        json!({"task":null,"project":meta.project,"start_url":meta.start_url.unwrap_or_default(),"started":start,"ended":end,"duration_ms":duration,"overview":overview,"response_codes":codes,"issues":issues,"auto_compare":auto_compare,"changes":changes,"skip_empty":skip_empty,"error":if meta.state==os_model::CrawlState::Failed {Some("Crawl fehlgeschlagen")}else{None}}),
+    )
+}
+
+/// Counts use the mapped pairs from the manual comparison; each pair is read once per side.
+fn element_changes(
+    cmp: &crate::compare::Comparison,
+    cfg: &os_model::CrawlConfig,
+) -> Result<Vec<Value>, String> {
+    use os_model::ChangeElement;
+    use rusqlite::OptionalExtension;
+    fn query(cfg: &os_model::CrawlConfig) -> String {
+        let sd = &cfg.extraction.structured_data;
+        let types = if sd.json_ld || sd.microdata || sd.rdfa {
+            format!("COALESCE((SELECT group_concat(type_path,'|') FROM sd_items s WHERE s.url_id=u.id AND ((s.format='json-ld' AND {}) OR (s.format='microdata' AND {}) OR (s.format='rdfa' AND {}))),'')",sd.json_ld as u8,sd.microdata as u8,sd.rdfa as u8)
+        } else {
+            "NULL".into()
+        };
+        format!("SELECT json_array(u.indexability,u.indexability_status),u.title_1,u.meta_desc_1,u.h1_1,CAST(u.word_count AS TEXT),CAST(u.crawl_depth AS TEXT),CAST(c.inlinks AS TEXT),CAST(c.unique_inlinks AS TEXT),CAST(c.outlinks AS TEXT),CAST(c.unique_outlinks AS TEXT),CAST(c.external_outlinks AS TEXT),CAST(c.unique_external_outlinks AS TEXT),{types},(SELECT hex(sig) FROM minhash m WHERE m.url_id=u.id) FROM urls u LEFT JOIN url_counters c ON c.url_id=u.id WHERE u.id=?1 AND u.is_internal=1 AND u.content_class='HTML' AND u.status_code>0 AND u.state='done'")
+    }
+    let cq = query(&cmp.current().config);
+    let pq = query(&cmp.previous().config);
+    let mut current = cmp
+        .current()
+        .file
+        .conn()
+        .prepare(&cq)
+        .map_err(|e| e.to_string())?;
+    let mut previous = cmp
+        .previous()
+        .file
+        .conn()
+        .prepare(&pq)
+        .map_err(|e| e.to_string())?;
+    let mut counts = [0u64; 14];
+    for id in cmp.pairing().paired_current.iter() {
+        let pid = cmp
+            .partner(crate::compare::Side::Current, i64::from(id))
+            .ok_or("Vergleichspartner fehlt")?;
+        let read = |r: &rusqlite::Row<'_>| {
+            (0..14)
+                .map(|i| r.get::<_, Option<String>>(i))
+                .collect::<Result<Vec<_>, _>>()
+        };
+        let a = current
+            .query_row([id], read)
+            .optional()
+            .map_err(|e| e.to_string())?;
+        let b = previous
+            .query_row([pid], read)
+            .optional()
+            .map_err(|e| e.to_string())?;
+        let (Some(a), Some(b)) = (a, b) else {
+            continue;
+        };
+        for i in 0..14 {
+            let normalize = |value: &Option<String>| match i {
+                0 => {
+                    let v: Vec<Option<String>> =
+                        serde_json::from_str(value.as_deref().unwrap_or("[]")).unwrap_or_default();
+                    let state = v.first().and_then(|s| s.as_deref()).unwrap_or("");
+                    let reasons = v.get(1).and_then(|s| s.as_deref()).unwrap_or("");
+                    let mut reasons = reasons
+                        .split([';', ','])
+                        .map(str::trim)
+                        .filter(|s| !s.is_empty())
+                        .collect::<Vec<_>>();
+                    reasons.sort();
+                    reasons.dedup();
+                    Some(format!("{state}:{}", reasons.join(";")))
+                }
+                1..=3 => Some(
+                    value
+                        .as_deref()
+                        .unwrap_or("")
+                        .split_whitespace()
+                        .collect::<Vec<_>>()
+                        .join(" "),
+                ),
+                12 => value.as_ref().map(|v| {
+                    let mut types = v.split('|').filter(|s| !s.is_empty()).collect::<Vec<_>>();
+                    types.sort();
+                    types.dedup();
+                    types.join("|")
+                }),
+                _ => value.clone(),
+            };
+            let a = normalize(&a[i]);
+            let b = normalize(&b[i]);
+            if i == 13 {
+                if let (Some(a), Some(b)) = (a, b) {
+                    if (a.is_empty() || a.len() == 1024) && (b.is_empty() || b.len() == 1024) {
+                        if a.is_empty() || b.is_empty() {
+                            if a != b && cfg.compare.change_detection.content_threshold_pct < 100 {
+                                counts[i] += 1;
+                            }
+                            continue;
+                        }
+                        let same = a
+                            .as_bytes()
+                            .as_chunks::<8>()
+                            .0
+                            .iter()
+                            .zip(b.as_bytes().as_chunks::<8>().0.iter())
+                            .filter(|(x, y)| x == y)
+                            .count();
+                        let change = ((1.0 - same as f64 / 128.0) * 10000.0).round() / 100.0;
+                        if change > f64::from(cfg.compare.change_detection.content_threshold_pct) {
+                            counts[i] += 1;
+                        }
+                    }
+                }
+            } else if (i <= 3 || (a.is_some() && b.is_some())) && a != b {
+                counts[i] += 1;
+            }
+        }
+    }
+    let mut rows = vec![];
+    for element in &cfg.compare.change_detection.elements {
+        let (i, name) = match element {
+            ChangeElement::Indexability => (0, "Indexability"),
+            ChangeElement::PageTitles => (1, "Page Titles"),
+            ChangeElement::MetaDescription => (2, "Meta Description"),
+            ChangeElement::H1 => (3, "H1"),
+            ChangeElement::WordCount => (4, "Word Count"),
+            ChangeElement::CrawlDepth => (5, "Crawl Depth"),
+            ChangeElement::Inlinks => (6, "Inlinks"),
+            ChangeElement::UniqueInlinks => (7, "Unique Inlinks"),
+            ChangeElement::InternalOutlinks => (8, "Internal Outlinks"),
+            ChangeElement::UniqueInternalOutlinks => (9, "Unique Internal Outlinks"),
+            ChangeElement::ExternalOutlinks => (10, "External Outlinks"),
+            ChangeElement::UniqueExternalOutlinks => (11, "Unique External Outlinks"),
+            ChangeElement::StructuredDataUniqueTypes => (12, "Structured Data Unique Types"),
+            ChangeElement::Content => (13, "Content"),
+        };
+        rows.push(json!({"name":name,"previous":0,"current":counts[i]}));
+    }
+    Ok(rows)
+}
````

### `crates/os-fetch/src/net.rs`

OUT-280, CRAWL-122/OUT-269: neue TLS-Tunnel-, OAuth-Token- und Systemproxy-Helfer; bestehende Client-Funktionen unverändert.

````diff
diff --git a/crates/os-fetch/src/net.rs b/crates/os-fetch/src/net.rs
index 06d9bfff..be8c0b9d 100644
--- a/crates/os-fetch/src/net.rs
+++ b/crates/os-fetch/src/net.rs
@@ -77,3 +77,174 @@ pub async fn probe_proxy(settings: &NetworkSettings) -> Result<(), crate::FetchE
     .map_err(|_| crate::FetchError::Config("Proxy nicht erreichbar".into()))?;
     Ok(())
 }
+
+/// Verified TLS over an existing tunnel (SMTP and HTTPS proxies, OUT-280 / K2-61).
+/// Uses the same platform verifier and additional roots as other app connections.
+pub async fn tls_tunnel<S>(
+    mut stream: S,
+    host: &str,
+    settings: &NetworkSettings,
+) -> Result<tokio::io::DuplexStream, String>
+where
+    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin + 'static,
+{
+    use std::{
+        io::{Read, Write},
+        sync::Arc,
+    };
+    use tokio::io::{AsyncReadExt, AsyncWriteExt};
+    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
+    let verifier = rustls_platform_verifier::Verifier::new_with_extra_roots(
+        certificates::roots(settings)?,
+        provider.clone(),
+    )
+    .map_err(|_| "TLS-Vertrauensspeicher nicht verfügbar")?;
+    let config = rustls::ClientConfig::builder_with_provider(provider)
+        .with_safe_default_protocol_versions()
+        .map_err(|_| "TLS-Konfiguration ungültig")?
+        .dangerous()
+        .with_custom_certificate_verifier(Arc::new(verifier))
+        .with_no_client_auth();
+    let name = rustls::pki_types::ServerName::try_from(host.to_owned())
+        .map_err(|_| "TLS-Hostname ungültig")?;
+    let mut tls = rustls::ClientConnection::new(Arc::new(config), name)
+        .map_err(|_| "TLS-Konfiguration ungültig")?;
+    let (client, mut plain) = tokio::io::duplex(64 * 1024);
+    let (ready, wait) = tokio::sync::oneshot::channel();
+    tokio::spawn(async move {
+        let mut ready = Some(ready);
+        let result: Result<(), std::io::Error> = async {
+            let mut encrypted = [0u8; 16384];
+            let mut clear = [0u8; 16384];
+            loop {
+                while tls.wants_write() {
+                    let mut bytes = Vec::new();
+                    tls.write_tls(&mut bytes)?;
+                    stream.write_all(&bytes).await?;
+                    stream.flush().await?;
+                }
+                if !tls.is_handshaking() {
+                    if let Some(tx) = ready.take() {
+                        let _ = tx.send(());
+                    }
+                    loop {
+                        match tls.reader().read(&mut clear) {
+                            Ok(0) => return Ok(()),
+                            Ok(n) => plain.write_all(&clear[..n]).await?,
+                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
+                            Err(e) => return Err(e),
+                        }
+                    }
+                }
+                tokio::select! {
+                    _ = async { match ready.as_mut() { Some(tx) => tx.closed().await, None => std::future::pending::<()>().await } } => return Ok(()),
+                    n = stream.read(&mut encrypted) => {
+                        let n=n?; if n==0 { return Ok(()); }
+                        tls.read_tls(&mut &encrypted[..n])?;
+                        tls.process_new_packets().map_err(std::io::Error::other)?;
+                    }
+                    n = plain.read(&mut clear), if !tls.is_handshaking() => {
+                        let n=n?; if n==0 { tls.send_close_notify(); return Ok(()); }
+                        tls.writer().write_all(&clear[..n])?;
+                    }
+                }
+            }
+        }
+        .await;
+        let _ = result; // Never expose peer text or credentials in errors/logs.
+    });
+    wait.await
+        .map_err(|_| "TLS-Verbindung oder Zertifikatsprüfung fehlgeschlagen")?;
+    Ok(client)
+}
+
+/// OAuth client for mail, including the caller's injected proxy credential (OUT-280 / K2-33).
+pub fn mail_oauth_client(
+    settings: &NetworkSettings,
+    password: Option<&str>,
+) -> Result<reqwest::Client, String> {
+    client_builder_with_password(settings, password)
+        .redirect(reqwest::redirect::Policy::none())
+        .timeout(std::time::Duration::from_secs(30))
+        .build()
+        .map_err(|_| "OAuth-Client nicht verfügbar".into())
+}
+
+/// OAuth token request sharing app-wide proxy/roots; errors never contain the response body.
+pub async fn mail_token_response(
+    settings: &NetworkSettings,
+    password: Option<&str>,
+    endpoint: &str,
+    form: &[(&str, &str)],
+) -> Result<String, String> {
+    let client = mail_oauth_client(settings, password)?;
+    let body = url::form_urlencoded::Serializer::new(String::new())
+        .extend_pairs(form.iter().copied())
+        .finish();
+    let response = client
+        .post(endpoint)
+        .header(
+            reqwest::header::CONTENT_TYPE,
+            "application/x-www-form-urlencoded",
+        )
+        .body(body)
+        .send()
+        .await
+        .map_err(|_| "OAuth-Verbindung fehlgeschlagen")?;
+    if !response.status().is_success() {
+        return Err(format!(
+            "OAuth-Token abgelehnt (HTTP {})",
+            response.status().as_u16()
+        ));
+    }
+    response
+        .text()
+        .await
+        .map_err(|_| "OAuth-Antwort nicht lesbar".into())
+}
+
+/// Resolve a system proxy for SMTP. Manual settings never consult this resolver.
+pub fn mail_system_proxy(host: &str, _port: u16) -> Option<os_model::settings::ProxySettings> {
+    let bypass = std::env::var("no_proxy")
+        .or_else(|_| std::env::var("NO_PROXY"))
+        .unwrap_or_default()
+        .split(',')
+        .map(str::to_string)
+        .collect();
+    let value = [
+        "all_proxy",
+        "ALL_PROXY",
+        "https_proxy",
+        "HTTPS_PROXY",
+        "http_proxy",
+        "HTTP_PROXY",
+    ]
+    .iter()
+    .find_map(|key| std::env::var(key).ok());
+    let _ = host;
+    let value = value.or_else(mail_platform_proxy)?;
+    let url = url::Url::parse(&value).ok()?;
+    Some(os_model::settings::ProxySettings {
+        mode: ProxyMode::Manual,
+        kind: match url.scheme() {
+            "http" => ProxyKind::Http,
+            "https" => ProxyKind::Https,
+            "socks5" | "socks5h" => ProxyKind::Socks5,
+            _ => return None,
+        },
+        host: url.host_str()?.into(),
+        port: url.port_or_known_default().or(Some(1080))?,
+        username: (!url.username().is_empty()).then(|| url.username().into()),
+        bypass,
+    })
+}
+#[cfg(not(any(target_os = "macos", target_os = "windows")))]
+fn mail_platform_proxy() -> Option<String> {
+    None
+}
+#[cfg(any(target_os = "macos", target_os = "windows"))]
+fn mail_platform_proxy() -> Option<String> {
+    mail_proxy_platform::resolve()
+}
+#[cfg(any(target_os = "macos", target_os = "windows"))]
+mod mail_proxy_platform;
````

### `crates/os-fetch/src/net/mail_proxy_platform.rs`

CRAWL-122/OUT-269: neues Modul für native Systemproxy-Auflösung unter macOS/Windows (PAC bleibt gemäß Spec P2).

````diff
diff --git a/crates/os-fetch/src/net/mail_proxy_platform.rs b/crates/os-fetch/src/net/mail_proxy_platform.rs
new file mode 100644
index 00000000..3ede22b0
--- /dev/null
+++ b/crates/os-fetch/src/net/mail_proxy_platform.rs
@@ -0,0 +1,132 @@
+//! Native system proxy lookup for non-HTTP transports; PAC remains P2 (CRAWL-122).
+#[cfg(target_os = "macos")]
+pub fn resolve() -> Option<String> {
+    use std::{
+        ffi::{c_char, c_void, CString},
+        ptr,
+    };
+    type Ref = *const c_void;
+    #[link(name = "CFNetwork", kind = "framework")]
+    unsafe extern "C" {
+        fn CFNetworkCopySystemProxySettings() -> Ref;
+    }
+    #[link(name = "CoreFoundation", kind = "framework")]
+    unsafe extern "C" {
+        fn CFStringCreateWithCString(a: Ref, s: *const c_char, e: u32) -> Ref;
+        fn CFDictionaryGetValue(d: Ref, k: Ref) -> Ref;
+        fn CFNumberGetValue(n: Ref, t: i32, v: *mut i32) -> bool;
+        fn CFStringGetCString(s: Ref, b: *mut c_char, n: isize, e: u32) -> bool;
+        fn CFRelease(r: Ref);
+    }
+    // CFNetwork returns a retained dictionary; values are borrowed until CFRelease.
+    unsafe {
+        let dict = CFNetworkCopySystemProxySettings();
+        if dict.is_null() {
+            return None;
+        }
+        let get = |key: &str| {
+            let key = CString::new(key).ok()?;
+            let cf = CFStringCreateWithCString(ptr::null(), key.as_ptr(), 0x08000100);
+            if cf.is_null() {
+                return None;
+            }
+            let v = CFDictionaryGetValue(dict, cf);
+            CFRelease(cf);
+            (!v.is_null()).then_some(v)
+        };
+        let number = |key: &str| {
+            let value = get(key)?;
+            let mut out = 0;
+            CFNumberGetValue(value, 3, &mut out).then_some(out)
+        };
+        let string = |key: &str| {
+            let value = get(key)?;
+            let mut buffer = [0i8; 2048];
+            if !CFStringGetCString(
+                value,
+                buffer.as_mut_ptr(),
+                buffer.len() as isize,
+                0x08000100,
+            ) {
+                return None;
+            }
+            Some(
+                std::ffi::CStr::from_ptr(buffer.as_ptr())
+                    .to_string_lossy()
+                    .into_owned(),
+            )
+        };
+        let mut out = None;
+        for (prefix, scheme) in [("SOCKS", "socks5"), ("HTTPS", "http"), ("HTTP", "http")] {
+            if number(&format!("{prefix}Enable")) == Some(1) {
+                if let (Some(host), Some(port)) = (
+                    string(&format!("{prefix}Proxy")),
+                    number(&format!("{prefix}Port")),
+                ) {
+                    out = Some(format!("{scheme}://{host}:{port}"));
+                    break;
+                }
+            }
+        }
+        CFRelease(dict);
+        out
+    }
+}
+#[cfg(target_os = "windows")]
+pub fn resolve() -> Option<String> {
+    use std::ffi::c_void;
+    #[repr(C)]
+    struct Config {
+        detect: i32,
+        pac: *mut u16,
+        proxy: *mut u16,
+        bypass: *mut u16,
+    }
+    #[link(name = "winhttp")]
+    unsafe extern "system" {
+        fn WinHttpGetIEProxyConfigForCurrentUser(config: *mut Config) -> i32;
+    }
+    #[link(name = "kernel32")]
+    unsafe extern "system" {
+        fn GlobalFree(ptr: *mut c_void) -> *mut c_void;
+    }
+    unsafe {
+        let mut c = Config {
+            detect: 0,
+            pac: std::ptr::null_mut(),
+            proxy: std::ptr::null_mut(),
+            bypass: std::ptr::null_mut(),
+        };
+        if WinHttpGetIEProxyConfigForCurrentUser(&mut c) == 0 {
+            return None;
+        }
+        let out = if c.proxy.is_null() {
+            None
+        } else {
+            let mut len = 0;
+            while *c.proxy.add(len) != 0 {
+                len += 1;
+            }
+            let s = String::from_utf16_lossy(std::slice::from_raw_parts(c.proxy, len));
+            s.split(';').find_map(|p| {
+                let p = p.trim();
+                if let Some(v) = p.strip_prefix("socks=") {
+                    Some(format!("socks5://{v}"))
+                } else if let Some(v) = p.strip_prefix("https=").or_else(|| p.strip_prefix("http="))
+                {
+                    Some(format!("http://{v}"))
+                } else if !p.contains('=') {
+                    Some(format!("http://{p}"))
+                } else {
+                    None
+                }
+            })
+        };
+        for p in [c.pac, c.proxy, c.bypass] {
+            if !p.is_null() {
+                GlobalFree(p.cast());
+            }
+        }
+        out
+    }
+}
````

### `crates/os-fetch/src/secrets.rs`

OUT-280: neue optionale writable view mit Default None am bisherigen Lese-Trait; Memory-/Keyring-Implementierungen liefern die bestehende SecretStoreMut-Schnittstelle.

````diff
diff --git a/crates/os-fetch/src/secrets.rs b/crates/os-fetch/src/secrets.rs
index 9a6f7cd2..edae8fe3 100644
--- a/crates/os-fetch/src/secrets.rs
+++ b/crates/os-fetch/src/secrets.rs
@@ -11,6 +11,10 @@ use std::sync::{Arc, Mutex, OnceLock};
 
 /// Quelle für Geheimnisse, die nie in Konfigurationsdateien stehen (`secret_ref`).
 pub trait SecretStore: Send + Sync {
+    /// Optional writable view for services that receive the read-only contract (K2-61).
+    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
+        None
+    }
     /// Liefert das Geheimnis zur Kennung.
     fn get(&self, id: &str) -> Option<String>;
     fn try_get(&self, id: &str) -> Result<Option<String>, SecretError> {
@@ -49,6 +53,9 @@ impl MemorySecretStore {
 }
 
 impl SecretStore for MemorySecretStore {
+    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
+        Some(self)
+    }
     fn get(&self, id: &str) -> Option<String> {
         self.values
             .lock()
@@ -105,6 +112,9 @@ impl Default for KeyringSecretStore {
 }
 
 impl SecretStore for KeyringSecretStore {
+    fn as_mut_store(&self) -> Option<&dyn SecretStoreMut> {
+        Some(self)
+    }
     fn get(&self, id: &str) -> Option<String> {
         self.try_get(id).ok().flatten()
     }
````

### `crates/os-integrations/src/oauth.rs`

OUT-280: zusätzlicher PKCE-Konstruktor für Gmail-/Microsoft-SMTP-Scopes, nutzt bestehenden Callback/Tokenaustausch.

````diff
diff --git a/crates/os-integrations/src/oauth.rs b/crates/os-integrations/src/oauth.rs
index d30d8819..7ebf7dd6 100644
--- a/crates/os-integrations/src/oauth.rs
+++ b/crates/os-integrations/src/oauth.rs
@@ -359,3 +359,35 @@ impl Authorization {
         })
     }
 }
+
+/// SMTP OAuth with PKCE and the existing one-shot loopback callback (OUT-280, K2-61).
+impl Authorization {
+    pub async fn begin_mail(
+        client_id: &str,
+        microsoft: bool,
+        endpoints: &Endpoints,
+    ) -> Result<Self, ApiError> {
+        let mut auth = Self::begin_drive(client_id, endpoints).await?;
+        let pairs = auth
+            .url
+            .query_pairs()
+            .map(|(k, v)| {
+                (
+                    k.clone().into_owned(),
+                    if k == "scope" {
+                        if microsoft {
+                            "offline_access https://outlook.office.com/SMTP.Send".into()
+                        } else {
+                            "https://mail.google.com/".into()
+                        }
+                    } else {
+                        v.into_owned()
+                    },
+                )
+            })
+            .collect::<Vec<(String, String)>>();
+        auth.url.set_query(None);
+        auth.url.query_pairs_mut().extend_pairs(pairs);
+        Ok(auth)
+    }
+}
````

## Platzhalter-Ablösung (K2-71)

### `crates/os-sched/src/tasks/model.rs`

OUT-281: on_failure erhält bei fehlender Angabe und Default den Spec-Standard true (bisher false).

````diff
diff --git a/crates/os-sched/src/tasks/model.rs b/crates/os-sched/src/tasks/model.rs
index e2ee4e7a..6d767321 100644
--- a/crates/os-sched/src/tasks/model.rs
+++ b/crates/os-sched/src/tasks/model.rs
@@ -240,13 +240,13 @@ pub struct CustomSummary {
 }
 
 /// Benachrichtigung (OUT-281).
-#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
+#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
 pub struct Notify {
     /// Bei Erfolg senden.
     #[serde(default)]
     pub on_success: bool,
     /// Bei Fehler senden.
-    #[serde(default)]
+    #[serde(default = "crate::mail::default_on_failure")]
     pub on_failure: bool,
     /// Mail-Konto.
     #[serde(default, skip_serializing_if = "Option::is_none")]
@@ -256,6 +256,17 @@ pub struct Notify {
     pub to: Vec<String>,
 }
 
+impl Default for Notify {
+    fn default() -> Self {
+        Self {
+            on_success: false,
+            on_failure: true,
+            account: None,
+            to: vec![],
+        }
+    }
+}
+
 /// Eine geplante Aufgabe.
 #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
 pub struct Task {
````

### `crates/os-sched/src/tasks/validate.rs`

OUT-281: leere Empfängerlisten deaktivieren den Versand und erzeugen keinen Validierungsfehler; die Kontoprüfung gilt nur bei nichtleerer Empfängerliste.

````diff
diff --git a/crates/os-sched/src/tasks/validate.rs b/crates/os-sched/src/tasks/validate.rs
index 075342cb..61d69406 100644
--- a/crates/os-sched/src/tasks/validate.rs
+++ b/crates/os-sched/src/tasks/validate.rs
@@ -289,7 +289,9 @@ pub fn validate(task: &Task, env: &dyn Env) -> Vec<Issue> {
     }
 
     // Benachrichtigung
-    if task.notify.on_success || task.notify.on_failure {
+    if (task.notify.on_success || task.notify.on_failure)
+        && task.notify.to.iter().any(|a| !a.trim().is_empty())
+    {
         match task.notify.account.as_deref() {
             Some(id) if env.mail_account_exists(id) => {}
             other => out.push(
@@ -297,12 +299,6 @@ pub fn validate(task: &Task, env: &dyn Env) -> Vec<Issue> {
                     .with("account", other.unwrap_or("")),
             ),
         }
-        if task.notify.to.iter().all(|a| a.trim().is_empty()) {
-            out.push(Issue::error(
-                "notify.to",
-                "tasks-issue-mail-recipients-missing",
-            ));
-        }
     }
     out
 }
````

## Stub-Vertrag (K2-60)

`crates/os-app/tests/manifest_contract.rs`: nur M2-T02 aus der Liste erwarteter noch nicht implementierter Besitzer entfernt (OUT-280).

````diff
diff --git a/crates/os-app/tests/manifest_contract.rs b/crates/os-app/tests/manifest_contract.rs
index e5bedfb0..6259206f 100644
--- a/crates/os-app/tests/manifest_contract.rs
+++ b/crates/os-app/tests/manifest_contract.rs
@@ -376,7 +376,7 @@ async fn stubs_answer_unsupported_with_command_and_owner() {
         owners.insert(err.args["owner"].as_str().unwrap().to_string());
     }
     // Besitzer laut Modulkarte 3.6
-    for o in ["M2-K09", "M2-K17", "M2-D21", "M2-T02", "M2-U17"] {
+    for o in ["M2-K09", "M2-K17", "M2-D21", "M2-U17"] {
         assert!(owners.contains(o), "kein Stub mit Besitzer {o}");
     }
 }
````

## Erwartungsänderungen (K2-63)

`crates/os-app/tests/cmd_tasks_commands.rs` (OUT-281): Aus der erwarteten Fehlerliste werden ausschließlich `tasks-issue-mail-account-missing` und `tasks-issue-mail-recipients-missing` entfernt. Bei leerer Empfängerliste ist der Versand deaktiviert und die Aufgabe deshalb nicht ungültig. Die übrigen Erwartungen und die gesamte Testlogik bleiben unverändert.

````diff
diff --git a/crates/os-app/tests/cmd_tasks_commands.rs b/crates/os-app/tests/cmd_tasks_commands.rs
index 33980126..6c221f86 100644
--- a/crates/os-app/tests/cmd_tasks_commands.rs
+++ b/crates/os-app/tests/cmd_tasks_commands.rs
@@ -165,8 +165,6 @@ async fn invalid_tasks_are_reported_with_reasons_and_not_saved() {
         "tasks-issue-start-url-invalid",
         "tasks-issue-config-missing",
         "tasks-issue-api-credentials",
-        "tasks-issue-mail-account-missing",
-        "tasks-issue-mail-recipients-missing",
     ] {
         assert!(keys.contains(&expected), "{expected} in {keys:?}");
     }
````

Ausgangscommit für die obigen Diffs: `f137e7431e3d2b02acff52bf6c9e90bbb6a69872`.
