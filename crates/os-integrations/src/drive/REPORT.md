# M2-X03 – Implementierung und Integrationsbefund

Historischer Erstbericht. Der aktuelle Stand der Fable-Fix-Runde steht im
[kanonischen Paketbericht](../../../../docs/plan/M2-reports/x03.md).

Stand: 2026-10-19, macOS arm64, Branch `m2/x03`. Beim Start war der Worktree sauber.
Clean-Room: ausschließlich Projekt-Spec und offizielle Google-Protokolldokumentation; kein Referenzprodukt.

## Ergebnis und Spec-Abdeckung

- **OUT-005 (Bibliothek):** OAuth mit PKCE S256, einmaligem Loopback-Callback und ausschließlich `drive.file`; Speicherung über `Credentials`/`SecretStoreMut`. Systembrowser-Einstieg `drive::connect_in_browser`, injizierbarer Browser für lokale Tests, Refresh-Funktion. App-weite Proxy-/Truststore-Einstellungen über I01s HTTP-Client. RAW-Zellen, Standardlimit 10 Mio. Zellen, konfigurierbares `Destination.cell_limit`, Kürzungswarnung mit abgeschnittener Zeilenzahl und CSV/XLSX-Hinweis. Blöcke bis 10 000 Zeilen; fünf Wiederholungen bei Timeout/429/allen 5xx nach 1/2/4/8/16 Sekunden. Namensauflösung unter `OpenSpider/<Projekt>/<Crawl>`, Wiederverwendung der Datei-ID beim Ersatz; gelöschte oder in den Papierkorb verschobene Dateien werden neu aufgelöst. Ein oder mehrere Blätter, feste Schreibbereiche gegen doppelte Zeilen nach einem Timeout. Dateianlage wird nach einem unklaren Timeout vor einem neuen POST erneut aufgelöst; Struktur-Batches prüfen nach einer bereits ausgeführten Anlage/Löschung die Nachbedingungen. Lokale Exporte können unabhängig weiterlaufen.
- **OUT-183 [M2] (Bibliotheksteil):** validierte Google-Sheets-URLs; Zellen aller Blätter in Blatt-/Zeilen-/Zellreihenfolge. Duplikate bleiben für die Uploadpositionen erhalten. Integrationstest: 10 Positionen → 8 URLs im vorhandenen Engine-Listenplaner, 2 Duplikate.
- **OUT-210:** Zeitreihen nach `OpenSpider/<Projekt>/<Task>_custom_summary_report`, Blatt `data`, `Date` und `Start URL` vorn, numerische Werte, explizites Datumsformat `yyyy-mm-dd hh:mm:ss`. Drei Läufe, Aufgabenumbennung und lokale Zeitverschiebung sind getestet. `export.customSummaryMetrics` liefert den vorhandenen dynamischen Metrikkatalog; `export.customSummary` misst über den Lese-Pool und liefert eine abbrechbare Operation mit Sheets-Link als `FileResult.path`. App-Aufrufe werden serialisiert; auch nur für die Sitzung gespeicherte Konten werden verwendet.
- **OUT-212:** bestehende Spalten bleiben an ihrer Position, neue Metriken kommen rechts hinzu, entfernte Metriken erhalten leere Zellen; Zuordnung über kanonische englische Kopfzeilen. Hinweise aus dem bestehenden Spaltenabgleich werden geloggt und als App-Notices weitergereicht.

Die Bibliothek und die beiden Summary-Commands sind implementiert. **Das Paket ist wegen des folgenden fremden Anschlusses noch nicht vollständig abnahmefähig.**

## Blocker: Drive-Konto über die App verbinden

Der I01-Rahmen in `crates/os-app/src/commands/api.rs` führt nur sechs Datenquellen in `KINDS` auf. `api.list` liefert Drive daher nicht. `connect_now` behandelt nur GA4/GSC als OAuth und erwartet für Drive einen API-Key; `client_id` kennt Drive ebenfalls nicht. Der generische Prüfabruf in `crates/os-integrations/src/http.rs` hat für Drive noch `NotImplemented`. `DriveConfig` besitzt noch keine OAuth-Client-ID.

Die neue Bibliotheksfunktion kann Drive-Konten bereits verbinden und über den gemeinsamen SecretStore bereitstellen; die Summary-Commands funktionieren mit diesen Konten. Der bestehende App-Command `api.connect({kind:"drive"})` erreicht diesen Fluss aber noch nicht. Die Fachtests prüfen echte Library-OAuth-Kommunikation mit dem OAuth-Mock und verwenden für Summary die gemeinsame Sitzungskontosicht.

Für den vollständigen Anschluss sind mindestens folgende Änderungen am fremden Verhalten nötig (hier **nicht vorgenommen**, gemäß Besitzregel/Abbruchbedingung des Auftrags):

```diff
--- crates/os-app/src/commands/api.rs
+++ crates/os-app/src/commands/api.rs
-const KINDS: [ApiKind; 6] = [
+const KINDS: [ApiKind; 7] = [
     /* bestehende sechs Datenquellen */
+    ApiKind::Drive,
 ];
-        } else if matches!(args.kind, ApiKind::Ga4 | ApiKind::Gsc) {
+        } else if matches!(args.kind, ApiKind::Ga4 | ApiKind::Gsc | ApiKind::Drive) {
             /* Drive über Authorization::begin_drive; andere Quellen unverändert */
```

Zusätzlich braucht der Integrator die passende Drive-Verzweigung für OAuth-Anzeige, Client-ID, Refresh, Konto-Prüfung sowie Disconnect/Forget. Das ist keine bloße neue Funktion oder zusätzliche Init-Registrierung: mehrere vorhandene Command-Verzweigungen ändern ihr Verhalten. K2-61 gestattet das ausdrücklich nicht. Ein neues `DriveConfig.oauth_client_id: Option<String>` mit Default wäre additiv, löst diese Verzweigungen allein aber nicht.

**Offene Freigabe/Frage:** Der Integrator muss diesen eng begrenzten Fremdanschluss übernehmen oder die Änderung des vorhandenen Drive-Verhaltens ausdrücklich erlauben. Bis dahin wird OUT-005 nicht als vollständig end-to-end erfüllt gemeldet. Kein neuer frei erfundener Command als Umgehung des verbindlichen Manifests.

## Nutzung durch Folgepakete

`drive::connect_in_browser` beziehungsweise `drive::connect` stellt das Konto bereit. Die Kontosicht muss dieselbe `Credentials`-Instanz wie die App verwenden (`commands::api::export_credentials`). `DriveClient::new` nimmt einen `ApiCtx`, Konto-ID und injizierten Zeitpunkt. `DriveClient::list_items` liefert Eingaben für den Engine-Listenplaner.

Für reguläre Exporte kann der vorhandene synchrone Tabellenpfad in `target_gsheet::StagingWriter` schreiben. `staged_table` liefert einen dateibasierten Zeileniterator; `target_gsheet::export` lädt einen oder mehrere solche Tabellen unter Beachtung des gemeinsamen Zellenbudgets hoch. Blattnamen verwenden die bestehende gültige/mehrdeutigkeitsfreie Benennung der XLSX-Bibliothek. Einfache Iteratoren lassen sich auch direkt als `Table` übergeben.

`CustomSummaryArgs.target` wird als Aufgaben-/Berichtsname interpretiert; die Dateinamensendung `_custom_summary_report` ergänzt die Bibliothek. Die Aufgabe-/CLI-/Dialoganbindung bleibt bei T01/L01/L02/U27 gemäß Paketplan. P2-Segmente und Looker-Vorlagen sind nicht Teil dieses Pakets. Die Datumskonvertierung verwendet denselben initialisierten lokalen Offset wie bestehende Exporte.

## Prüfungen

Abschließender Stand, alle folgenden Befehle mit Exit 0:

- `cargo fmt --all --check`
- `cargo clippy --workspace --exclude openspider-desktop --all-targets --locked -- -D warnings` (20,91 s)
- `cargo test --workspace --exclude openspider-desktop --locked`: **3 041 bestanden, 0 fehlgeschlagen, 89 bestehende Ignores**, einschließlich Doc-Tests; 1 717,70 s (28 min 37,70 s). Nach der letzten Transportkorrektur vollständig neu gelaufen.
- 13 neue X03-Fachtests ohne Ignore: 6 Drive/OAuth, 5 Sheets-Ziel/Listenplaner, 2 App-Summary; alle auch im vollständigen Workspace-Lauf grün.
- `cargo deny check licenses bans sources advisories`
- `bash scripts/check-neutral.sh` (bestehende Hinweise auf vier verwaiste Allowlist-Einträge, keine Fundstellen)
- `python3 -I docs/plan/M2-check.py`: OK.
- `git diff --check`

Die X03-Tests verwenden nur Loopback-Mocks und MemorySecretStore; kein externer API-Aufruf und kein echter Schlüsselbund. Fixture: bestehendes `tests/fixtures/out-m2-export/crawl.sql` und deterministisch vorbelegte Drive-/Sheets-Zustände.

## Fremdergänzungen (K2-61)

Alle folgenden Änderungen sind strikt additiv. Bestehende Signaturen bleiben gleich. Es wurden keine Versionen oder neuen externen Bibliotheken aufgenommen; `os-integrations` und `os-fetch` sind vorhandene Workspace-Abhängigkeiten.

### `crates/os-integrations/src/oauth.rs`

```diff
diff --git a/crates/os-integrations/src/oauth.rs b/crates/os-integrations/src/oauth.rs
index 790323d..d30d881 100644
--- a/crates/os-integrations/src/oauth.rs
+++ b/crates/os-integrations/src/oauth.rs
@@ -322,0 +323,39 @@ fn encode_form(form: &[(&str, &str)]) -> String {
+
+// Additive constructor for the drive.file-only exporter (M2-X03, K2-61).
+impl Authorization {
+    pub async fn begin_drive(client_id: &str, endpoints: &Endpoints) -> Result<Self, ApiError> {
+        if client_id.trim().is_empty() {
+            return Err(ApiError::Unauthorized(
+                "Eigene Google-Client-ID erforderlich".into(),
+            ));
+        }
+        let listener = TcpListener::bind(("127.0.0.1", 0))
+            .await
+            .map_err(|_| transport())?;
+        let redirect = format!(
+            "http://127.0.0.1:{}/callback",
+            listener.local_addr().map_err(|_| transport())?.port()
+        );
+        let state = CsrfToken::new_random();
+        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
+        let mut url = Url::parse(&endpoints.authorize).map_err(|_| transport())?;
+        url.query_pairs_mut().extend_pairs([
+            ("response_type", "code"),
+            ("client_id", client_id),
+            ("redirect_uri", redirect.as_str()),
+            ("scope", crate::drive::SCOPE),
+            ("state", state.secret()),
+            ("code_challenge", challenge.as_str()),
+            ("code_challenge_method", "S256"),
+            ("access_type", "offline"),
+            ("prompt", "consent"),
+        ]);
+        Ok(Self {
+            url,
+            redirect,
+            listener,
+            state,
+            verifier,
+        })
+    }
+}
```

### `crates/os-app/src/commands/api.rs`

```diff
diff --git a/crates/os-app/src/commands/api.rs b/crates/os-app/src/commands/api.rs
index ae500fe..1413413 100644
--- a/crates/os-app/src/commands/api.rs
+++ b/crates/os-app/src/commands/api.rs
@@ -1588,0 +1589,5 @@ pub fn register(r: &mut Registry) {
+
+// Additive export access to the same persisted/session-only accounts (M2-X03, K2-61).
+pub fn export_credentials(app: &App) -> Arc<Credentials> {
+    app.ext::<ApiState>().credentials(app).clone()
+}
```

### `crates/os-export/Cargo.toml`

```diff
diff --git a/crates/os-export/Cargo.toml b/crates/os-export/Cargo.toml
index d4b5695..dc7198c 100644
--- a/crates/os-export/Cargo.toml
+++ b/crates/os-export/Cargo.toml
@@ -14,0 +15 @@ toml.workspace = true
+os-integrations.workspace = true
@@ -28,0 +30 @@ url.workspace = true
+os-fetch.workspace = true
```

### `Cargo.lock`

```diff
diff --git a/Cargo.lock b/Cargo.lock
index fb574bc..84fe975 100644
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -5112,0 +5113,2 @@ dependencies = [
+ "os-fetch",
+ "os-integrations",
```

## Stub-Ablösung (K2-60)

In `crates/os-app/tests/manifest_contract.rs` wurde ausschließlich das X03-Tupel mit `export.customSummaryMetrics` und `export.customSummary` aus dem Unsupported-Vertrag entfernt; rustfmt zieht das verbleibende O01-Tupel zusammen. Eigene Integrationstests prüfen stattdessen das echte Verhalten. Keine Änderung fremder Erwartungen nach K2-63, keine fremden Goldens/Fixtures geändert.

## Protokollquellen

- [Sheets values.update: RAW-Option und drive.file](https://developers.google.com/workspace/sheets/api/reference/rest/v4/spreadsheets.values/update)
- [Drive-Dateianlage: native Google-Tabellen unterstützen keine vorab vergebenen IDs](https://developers.google.com/workspace/drive/api/guides/create-file#generate-ids-to-use-with-your-files)

## Abgabe

Implementierungscommits:

- `58ac2ff` – `integrations: implement Drive OAuth and resilient Sheets transport`
- `8e7b848` – `export: add streaming Sheets target and stable crawl time series`
- `dd74dbe` – `app: implement custom summary metrics and Sheets operations`

Der abschließende Bericht steht in einem separaten Dokumentationscommit. Kein Push, Merge oder PR. Frontend unverändert; npm-Prüfungen sind für diesen Auftrag deshalb nicht erforderlich. CI-Lauf-ID entfällt, weil ausdrücklich kein Push beauftragt ist. Alle Commits tragen den vereinbarten Co-Author-Trailer.
