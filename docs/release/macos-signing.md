# macOS: Signieren und Notarisieren (M2-Z01)

Der Workflow `.github/workflows/release-macos.yml` baut die Desktop-App, signiert sie mit einer **Developer ID Application**, notarisiert sie bei Apple, heftet das Ticket und liefert ein signiertes, notarisiertes **DMG als Actions-Artefakt** (14 Tage). Er veröffentlicht nichts: kein GitHub Release, kein Upload außerhalb von GitHub Actions.

## Voraussetzungen (einmalig, beim Apple-Developer-Konto)

1. **Apple Developer Program** (Mitgliedschaft aktiv). Die **Team-ID** steht unter developer.apple.com → Account → Membership details.
2. **Zertifikat „Developer ID Application"** erzeugen (nicht „Apple Development" oder „Mac App Distribution"):
   - Xcode → Settings → Accounts → Team wählen → Manage Certificates → `+` → *Developer ID Application*, oder
   - developer.apple.com → Certificates → `+` → *Developer ID Application* (CSR aus Schlüsselbundverwaltung → Zertifikatsassistent → Zertifikat von einer Zertifizierungsstelle anfordern).
3. **Als .p12 exportieren:** Schlüsselbundverwaltung → Anmelde → Meine Zertifikate → das Zertifikat „Developer ID Application: Name (TEAMID)" aufklappen, Zertifikat samt privatem Schlüssel markieren → Rechtsklick → *Exportieren* → Format .p12, ein Exportpasswort vergeben (das wird `APPLE_CERTIFICATE_PASSWORD`).
4. **Base64 erzeugen** (eine Zeile, in die Zwischenablage): `base64 -i zertifikat.p12 | pbcopy`. Die .p12-Datei danach sicher löschen oder in den Passwortmanager legen.
5. **App-spezifisches Passwort** für die Notarisierung: account.apple.com → Anmeldung und Sicherheit → App-spezifische Passwörter → neues erzeugen (Name z. B. „OpenSpider notarytool"). Das normale Apple-ID-Passwort funktioniert nicht.
6. **Name der Signaturidentität** auslesen: `security find-identity -v -p codesigning` → Eintrag in der Form `Developer ID Application: Vorname Nachname (ABCDE12345)`.

## Secrets und Variablen im GitHub-Repo

Repo → Settings → Secrets and variables → Actions.

| Name | Art | Inhalt |
|---|---|---|
| `APPLE_CERTIFICATE` | Secret | .p12 als Base64 (Schritt 4) |
| `APPLE_CERTIFICATE_PASSWORD` | Secret | Exportpasswort der .p12 (Schritt 3) |
| `APPLE_ID` | Secret | Apple-ID (E-Mail) des Developer-Kontos |
| `APPLE_PASSWORD` | Secret | App-spezifisches Passwort (Schritt 5) |
| `KEYCHAIN_PASSWORD` | Secret | frei gewähltes, zufälliges Passwort für den temporären Schlüsselbund im Runner |
| `APPLE_SIGNING_IDENTITY` | Variable | vollständiger Name der Identität (Schritt 6) |
| `APPLE_TEAM_ID` | Variable | Team-ID, 10 Zeichen |

Per CLI, ohne dass ein Wert in der Shell-History oder im Log landet:

```sh
base64 -i zertifikat.p12 | gh secret set APPLE_CERTIFICATE
gh secret set APPLE_CERTIFICATE_PASSWORD     # fragt den Wert verdeckt ab
gh secret set APPLE_ID
gh secret set APPLE_PASSWORD
openssl rand -base64 24 | gh secret set KEYCHAIN_PASSWORD
gh variable set APPLE_SIGNING_IDENTITY --body 'Developer ID Application: Vorname Nachname (ABCDE12345)'
gh variable set APPLE_TEAM_ID --body 'ABCDE12345'
```

Fehlt eines dieser Elemente, endet der Lauf nach dem ersten Schritt mit grünem Status, einer Warnung (`::warning`) und einer Zusammenfassung, welche Namen fehlen. Es wird dann nichts gebaut.

## Auslösen

- Manuell: Actions → *Release macOS* → *Run workflow*, Eingabe `arch` = `universal` (Standard, ein Binary für Apple Silicon und Intel), `aarch64` oder `x86_64`. Per CLI: `gh workflow run release-macos.yml -f arch=aarch64`.
- Automatisch bei jedem Tag `v*` (immer `universal`). Auch dann entsteht nur das Artefakt, kein Release.
- Laufzeit: etwa 20 bis 40 Minuten, davon meist 2 bis 15 Minuten Wartezeit auf Apples Notarisierungsdienst.

Das Artefakt `openspider-macos-<arch>` enthält das DMG und die Prüfprotokolle `verify-app.log` und `verify-dmg.log`.

## Ablauf im Workflow

1. Prüfung, ob alle Secrets/Variablen gesetzt sind (sonst Meldung und Ende mit Erfolg).
2. Checkout, Rust (Ziele `aarch64-apple-darwin` und `x86_64-apple-darwin`), Node 22, `npm ci`.
3. Temporärer Schlüsselbund (`KEYCHAIN_PASSWORD`), Import der .p12, Partition-Liste für `codesign`, Prüfung, dass `APPLE_SIGNING_IDENTITY` darin vorkommt.
4. `npm run -w apps/desktop tauri -- build --target <ziel> --bundles app,dmg`: Tauri signiert mit Hardened Runtime und `entitlements.plist`, notarisiert die App (`notarytool`, wartet), heftet das Ticket und baut danach das DMG.
5. Das DMG wird zusätzlich signiert, notarisiert und geheftet (Tauri notarisiert nur die App darin; ein DMG ohne eigenes Ticket würde `stapler validate` nicht bestehen).
6. `scripts/release/verify-macos.sh` prüft App und DMG; die Ausgabe steht im Job-Log und in der Job-Zusammenfassung.
7. Upload des Artefakts; danach (`always()`) wird der Schlüsselbund gelöscht.

Keine Werte werden ausgegeben: Secrets sind in GitHub maskiert, die Schritte schreiben nur Namen fehlender Einträge ins Log. `APPLE_CERTIFICATE` wird bewusst nicht an Tauri übergeben (sonst importierte Tauri das Zertifikat ein zweites Mal in einen eigenen Schlüsselbund); der Workflow hat es bereits importiert.

## Prüfen

Lokal, nach dem Download und Entpacken des Artefakts (der Download setzt das Quarantäne-Attribut, genau wie bei einem späteren Nutzer):

```sh
scripts/release/verify-macos.sh ~/Downloads/OpenSpider_0.1.0_universal.dmg
scripts/release/verify-macos.sh /Applications/OpenSpider.app
```

Geprüft wird:

- `codesign --verify --deep --strict --verbose=2` (App) bzw. `--strict` (DMG)
- Hardened Runtime aktiv (Flag `runtime` in `codesign -dvv`)
- `spctl -a -t exec -vv` für die App bzw. `spctl -a -t open --context context:primary-signature -vv` für das DMG; erwartet wird `accepted` mit `source=Notarized Developer ID`
- `xcrun stapler validate`
- beim DMG zusätzlich: Einhängen (schreibgeschützt) und dieselbe App-Prüfung für die enthaltene App

Das Skript läuft alle Prüfungen durch, gibt ein Protokoll aus und endet mit Exitcode 0 nur, wenn alles besteht (2 bei falscher Bedienung). Für einen lokalen Build ohne Notarisierung: `--no-notarization` (Gatekeeper-Quelle und Heftung werden dann nur gemeldet).

Von Hand, falls nötig:

```sh
codesign -dvv --entitlements - /Applications/OpenSpider.app
xcrun notarytool history --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID"   # fragt das Passwort ab
xcrun notarytool log <submission-id> --apple-id … --team-id …                 # Begründung bei „Invalid"
```

## Entitlements

`apps/desktop/src-tauri/entitlements.plist` ist bewusst leer (nur ein leeres `<dict/>`); sie wird in `tauri.conf.json` unter `bundle.macOS.entitlements` eingebunden, zusammen mit `hardenedRuntime: true` und `minimumSystemVersion: "13.0"` (Mindestplattform laut `docs/ARCHITECTURE.md`).

Begründung, warum keine Ausnahmen nötig sind:

- **JIT und ausführbarer Speicher** (`com.apple.security.cs.allow-jit`, `allow-unsigned-executable-memory`): Das JavaScript der Oberfläche läuft in WKWebView, im von Apple signierten WebContent-Prozess, nicht im Prozess der App. Die App selbst enthält keinen JS- oder Wasm-Interpreter.
- **Netzwerk** (`com.apple.security.network.client`): betrifft nur die App Sandbox. OpenSpider ist nicht sandboxed; ausgehende Verbindungen sind erlaubt.
- **Bibliotheksvalidierung / DYLD-Variablen:** Es werden keine fremd signierten Bibliotheken geladen. Chromium für das JS-Rendering (`os-render`) ist ein externer Kindprozess der Nutzerinstallation, für den die Hardened Runtime der App nicht gilt.
- **Schlüsselbund** (`keyring`): Zugriff auf den Anmelde-Schlüsselbund braucht keine Entitlements. `keychain-access-groups` (Data-Protection-Keychain) bräuchte ein Provisioning Profile und wird nicht verwendet.

Kommt später eine Funktion dazu, die eine Ausnahme braucht (z. B. ein eingebetteter Interpreter mit JIT), wird sie einzeln in die Plist eingetragen und hier begründet. Stürzt die signierte App beim Start ab, zeigt `log stream --predicate 'process == "OpenSpider"'` die verletzte Regel.

## Fehlersuche

| Meldung | Ursache |
|---|---|
| Lauf endet nach Schritt 1 mit Warnung | ein Secret oder eine Variable fehlt (Namen stehen in der Warnung) |
| „Die Identität aus APPLE_SIGNING_IDENTITY steht nicht im importierten Zertifikat" | falscher Zertifikatstyp (nicht *Developer ID Application*), Variable weicht im Namen ab, oder die .p12 enthält den privaten Schlüssel nicht |
| `security import` scheitert | falsches `APPLE_CERTIFICATE_PASSWORD` oder Base64 mit Zeilenumbruch-/Kopierfehlern |
| `notarytool`: `Invalid credentials` / 401 | `APPLE_PASSWORD` ist nicht das App-spezifische Passwort, oder `APPLE_ID`/`APPLE_TEAM_ID` passen nicht zusammen |
| Notarisierung „Invalid" | `notarytool log` zeigt die Dateien (meist unsignierte Binärdatei oder fehlender Zeitstempel/Hardened Runtime); der Workflow gibt das Log aus |
| `verify-macos.sh`: „Quelle ist nicht Notarized Developer ID" | Tauri hat nicht notarisiert (Variablen am Build-Schritt prüfen) oder das Ticket fehlt |
