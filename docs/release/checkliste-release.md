# Release-Checkliste

Pro Release dokumentieren: Version/Commit/Tag, Kanal, Verantwortlicher, Datum, Actions-Lauf-ID/URL, SHA256SUMS, Signaturstatus, Ergebnisse und offenes Nutzer-Go. Ein grüner Lauf mit STUBs oder „unsigniert“ ist keine Veröffentlichungsfreigabe.

- [ ] Integrationsstand und CI geprüft; offene M2-Parität und L01-Lizenzen bewertet; keine Chromium-Bündelung.
- [ ] Version mit set-version vorbereitet, Cargo.lock nur lokale Paketversionen verändert, check-version grün, Changelog-Sektion vorhanden. `beta.0` nur technischer Trockenlauf.
- [ ] GitHub-Environment `release`, OIDC/Azure-Trust, Signierprofil und Secrets/Variablen nach H01 eingerichtet; keine Schlüsselwerte im Protokoll. Apple-Secrets für R01 auf Repository-Ebene verfügbar.
- [ ] `workflow_dispatch` mit passender `release_tag`, `dry_run=true` gestartet; Lauf-ID und Ergebnisse gesichert. Alle R01/R02/Linux-Jobs grün, keine fehlenden Artefakte verschwiegen.
- [ ] R03 installiert macOS arm64/x64 und Windows MSI/NSIS einschließlich Windows ARM-Emulation aus genau diesen Artefakten; Crawl/JS, Export, Sitemap und Aufgaben geprüft. STUB/TODO(R03) bis dahin offen.
- [ ] D01-Schnittstelle abgeglichen, Fixture-Archiv je Stand öffnen/migrieren/exportieren, Archivierung dieser Version und Herkunft prüfen. STUB/TODO(D01) bis dahin offen.
- [ ] CLI-Archive je Ziel extrahieren und starten, Linux headless/Golden prüfen; LICENSE/NOTICE enthalten. Kein Linux-Desktop oder arm64 zugesagt.
- [ ] DMGs: Signatur, Notarisierung, Ticket, beide Universal-Slices; Windows: Authenticode, Herausgeber, SmartScreen, MSI/NSIS still und WebView2. Unsigned-Builds ausdrücklich als solche dokumentieren.
- [ ] Vier Manifest-Zielschlüssel geprüft; jedes URL-Ziel entspricht Architektur/Installer und `.sig`. Updateprüfung mit öffentlichem Schlüssel, Manipulation/Downgrade/HTTP-Ablehnung und echter Update-Pfad durch Q02/A03 nachgewiesen. Beta liest latest-beta.json.
- [ ] Beide Manifeste vorhanden und plausibel. K3-04: Beta wird normaler Release, nicht Pre-Release; Stable-Clients lehnen Beta ab. `SHA256SUMS` über heruntergeladene Assets unabhängig prüfen.
- [ ] Q03: Office-Exporte, echter Schlüsselbund, RDP, Gatekeeper mit neuem Benutzer, Dateizuordnungen/Startmenü, geplante Aufgaben nach Update sowie Deinstallation geprüft.
- [ ] Nutzer-Go für Tag eingeholt. Erst dann Tag am geprüften Commit erzeugen/pushen. Workflow erzeugt ausschließlich Entwurf; alternativ bestehender Tag auf gleichem Commit und `dry_run=false`.
- [ ] Entwurf/Notes/Assets durch Nutzer geprüft; Warnungen, STUBs und offene M2-Befunde bewertet. Lauf-ID und Prüfsummen in Abnahmebericht übernehmen. Bestehende Releases nicht automatisch überschreiben.
- [ ] Separates ausdrückliches Nutzer-Go zur Veröffentlichung dokumentiert. M3-A01/M3-REL veröffentlicht manuell, Beta bis 1.0 als normalen Release und ausdrücklich als **Latest** markieren. R04 veröffentlicht nie.
- [ ] Nach Veröffentlichung `releases/latest/download/latest.json` und `latest-beta.json` sowie referenzierte URLs abrufen; Signatur/Update auf freigegebenen Testinstallationen prüfen. Entwurf-URLs funktionieren öffentlich erst nach Veröffentlichung.
- [ ] Website, Homebrew/winget und Einladungen/Ankündigungen nur mit eigenem Go nach den zuständigen Paketen. Rücknahme: weitere Verteilung stoppen und Nutzer/Orchestrator entscheiden lassen; veröffentlichte Assets nicht still überschreiben.
