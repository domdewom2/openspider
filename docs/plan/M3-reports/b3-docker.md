# Bericht B3-DOCKER: Docker-Image für den Serverbetrieb

Branch `m3/docker` (Basis origin/main `f90bb133`), Worktree `c-docker`, Server `m3-docker`. Grundlage:
`docs/plan/beta3.md` §4b, Spec `docs/spec/06-output-ops.md` (OUT-273, OUT-207, OUT-180 ff., Abschnitt 13), AGENT-001/003.
Nichts gepusht, kein Workflow gestartet, kein Image gebaut oder veröffentlicht, kein Website-Deploy. Docker stand weder
lokal noch auf dem Server zur Verfügung: Bauen und Smoke-Test laufen erstmals im neuen Workflow nach dem Merge.

## Ergebnis in Kürze

| Teil | Datei | Stand |
|---|---|---|
| Dockerfile | `packaging/docker/Dockerfile` | vier Stufen, Basis `debian:trixie-20261027-slim@sha256:f6b08b09…`, hadolint 2.15.1 ohne Befund |
| Chromium-Starter | `packaging/docker/openspider-chromium` | shellcheck 0.11.0 ohne Befund |
| Build-Kontext | `packaging/docker/.dockerignore` | nur der Starter |
| Smoke-Test | `packaging/docker/smoke.mjs` | 13 Prüfungen, nur Node-Stdlib, lokal auf Linux ausführbar |
| OCI-/SBOM-Prüfung | `packaging/docker/oci.mjs` | SBOM/Provenance, Gleichheit mit dem geprüften Image |
| Vertragstest | `packaging/docker/docker.test.mjs` | 16/16 grün (Server), je Test `// spec: OUT-274` |
| Workflow | `.github/workflows/docker.yml` | actionlint 1.7.12 (mit shellcheck) ohne Befund |
| Spec | `docs/spec/06-output-ops.md` | neu **OUT-274** (P1) in Abschnitt 13 |
| Doku | Handbuch de/en, Download-Seite de/en, `README.md`, `docs/release/docker.md` | `check-site` und `check-neutral` grün |

Neue Spec-ID: **OUT-274** „Docker-Image für den Serverbetrieb (x86_64)“, nächste freie Nummer im Abschnitt 13
(OUT-260 … OUT-273 belegt, Abschnitt 14 beginnt mit OUT-280).

## Aufbau des Images

Stufen: `fetch` (Debian + curl) → `archive` (Download) → `verify` (Prüfsumme, Inhalt, Version) → `runtime`.

- **Basis:** `debian:trixie-20261027-slim`, per Digest gepinnt (`ARG DEBIAN_IMAGE`). Der Linux-Release-Build läuft auf
  `ubuntu-22.04` (`release.yml`, Job `linux-core`) mit glibc 2.35; trixie hat glibc 2.41, also abwärtskompatibel.
  Gegen bookworm habe ich mich entschieden: bookworm ist seit Juni 2026 oldstable/LTS, Chromium-Sicherheitsupdates
  gibt es verlässlich nur in stable (trixie-security).
- **Archiv:** `openspider_<v>_linux-x86_64.tar.gz` per HTTPS von GitHub Releases (`--proto '=https' --proto-redir '=https'`,
  `OPENSPIDER_ARCHIVE_URL` überschreibt die Adresse) oder lokal über `--build-context archive=<ordner>` (BuildKit ersetzt
  damit die Stufe `archive`; so nutzt der Workflow die Artefakte eines Release-Laufs). Build-Argumente
  `OPENSPIDER_VERSION`, `OPENSPIDER_SHA256` (Pflicht, kein Standardwert), `OPENSPIDER_REVISION`.
- **Prüfungen in `verify`, in dieser Reihenfolge:** Format der Prüfsumme → `sha256sum --check --strict` (vor jedem
  Lesen des Archivs) → Archivinhalt genau `LICENSE NOTICE THIRD_PARTY_NOTICES.txt openspider` → Entpacken ohne
  Eigentümer/Rechte des Archivs → `openspider --version` muss `openspider <version> (…, linux-x86_64)` sein.
- **Laufzeit-Pakete** (`--no-install-recommends`, bewusst ohne Versions-Pin, damit ein Neubau Sicherheitsupdates
  bekommt): `ca-certificates` (rustls-platform-verifier liest die System-CAs), `chromium`, `fonts-liberation`,
  `fonts-noto-color-emoji`, `tini`. Dazu ihre Abhängigkeiten aus Debian (u. a. GTK/GLib-, NSS-, Mesa-, Font-Bibliotheken).
- **Benutzer:** `openspider`, **UID 10001, GID 10001**, Shell `/usr/sbin/nologin`, `USER 10001:10001`. Kein Schritt nach
  `USER` läuft als root.
- **Ordner/Volumes:** `WORKDIR /data` (Standard-Ausgabeordner der CLI), `/config` = `OPENSPIDER_HOME` als `VOLUME`
  (Crawl-Verwaltung, Einstellungen, Logs; muss für `--compare-with-previous` erhalten bleiben). Beide gehören 10001.
- **Umgebung:** `OPENSPIDER_HOME=/config`, `OPENSPIDER_CHROMIUM=/usr/local/bin/openspider-chromium`,
  `OPENSPIDER_SECRET_STORE=memory`, `OPENSPIDER_DISABLE_UPDATE_CHECK=1` (Updates kommen als neues Image).
- **Einstiegspunkt:** `["/usr/bin/tini", "--", "/usr/local/bin/openspider"]`, `CMD ["--help"]`. tini als PID 1 reicht
  SIGINT/SIGTERM weiter (OUT-206) und räumt verwaiste Chromium-Prozesse ab; stdin/stdout bleiben unberührt (MCP).
- **Labels (OCI):** `title`, `description` („… No web UI …“), `source` = GitHub-Repo, `url`, `documentation`
  (`https://openspider.de/en/manual/#docker`), `licenses=Apache-2.0`, `version`, `revision`, `base.name`.
- **Lizenzhinweise:** `/usr/share/doc/openspider/{LICENSE,NOTICE,THIRD_PARTY_NOTICES.txt}` aus dem Archiv; die
  `copyright`-Dateien der Debian-Pakete bleiben in Debian-slim erhalten (`/usr/share/doc/<paket>/copyright`).
- **Geheimnisse:** keine im Image (Vertragstest prüft `ENV`/`ARG`/`LABEL`). Secret-Store unter Linux headless:
  `secret_store_from_env()` in `os-fetch` nimmt mit `OPENSPIDER_SECRET_STORE=memory` einen prozessweiten
  Speicher-Store ohne D-Bus/Schlüsselbund; ohne die Variable würde `keyring` den Secret Service über D-Bus suchen, den
  es im Container nicht gibt. Schlüssel kommen zur Laufzeit über `OPENSPIDER_PSI_KEY`, `OPENSPIDER_AHREFS_KEY`,
  `OPENSPIDER_MAJESTIC_KEY`, `OPENSPIDER_MOZ_KEY` (OUT-201) bzw. `OPENSPIDER_AUTH_PASSPHRASE` (`--auth-config`),
  per `-e NAME` oder `--env-file`. Fehlendes Geheimnis → klarer Fehler (Smoke-Prüfung wie `linux_server.rs`).

### Chromium: Debian statt Chrome for Testing

| Kriterium | Debian `chromium` (gewählt) | Chrome for Testing aus `resources/chromium/versions.json` |
|---|---|---|
| Lizenz/Weiterverteilung im Image | BSD-3-Clause plus Open-Source-Drittlizenzen, frei weiterverteilbar, `copyright` im Image | Google-Build unter Googles Nutzungsbedingungen; Weiterverteilung in einem öffentlichen Image unklar |
| Verfügbarkeit für Linux | Paket für amd64 in trixie | `versions.json` hat **keinen** `linux64`-Eintrag (Test `bundled_manifest_is_official_and_complete` verlangt das sogar); Hash müsste neu ermittelt und außerhalb der Datei gepflegt werden |
| Sicherheitsupdates | trixie-security, jeder Neubau holt sie | nur durch manuelles Anheben der festen Version |
| Größe | Chromium + Abhängigkeiten ca. 350–450 MB unkomprimiert | ähnlich (Archiv ~170 MB, entpackt ~400 MB) plus dieselben Systembibliotheken |
| Reproduzierbarkeit | Version folgt Debian (nicht bitgleich wiederholbar); der Smoke-Test protokolliert die Version | bitgenau über SHA-256 |
| Übereinstimmung mit der App | App prüft CfT 156.0.8078.80; Debian trixie: stable 150.0.7871.181, in Vorbereitung 156.0.8078.73/.79 (ftp-master madison, 05.11.) | gleiche Version wie die App |

Ausschlaggebend waren Lizenz und Sicherheitsupdates für einen Browser, der fremde Seiten rendert. Unter Linux wurde
ohnehin nie gegen CfT geprüft (`linux-core` nutzt Runner-Chrome bzw. Playwright-Chromium).

**Sandbox:** Dockers Standard-Seccomp-Profil verhindert die Chromium-Sandbox (keine Benutzer-Namensräume). `os-render`
kennt `LaunchOpts.no_sandbox`, aber kein Konfigurationsschlüssel setzt es. Deshalb startet der Starter
`openspider-chromium` Chromium mit `--no-sandbox` (Grenze ist der Container), außer bei `OPENSPIDER_CHROMIUM_SANDBOX=1`;
außerdem `--disable-dev-shm-usage` (64-MB-`/dev/shm`) und `HOME=/tmp`, wenn `HOME` nicht beschreibbar ist
(`docker run --user`). Siehe Spec-Fragen.

### Erwartete Größe (Schätzung)

Debian trixie-slim ~80 MB, Chromium samt Bibliotheken ~350–450 MB, Schriften ~15 MB, `openspider` ~70–90 MB (Archiv
29,9 MB bei beta.2): **etwa 550–650 MB unkomprimiert, etwa 200–250 MB komprimiert** in der Registry. Die tatsächliche
Größe meldet die letzte Smoke-Prüfung („Image-Größe“). CJK-Schriften (`fonts-noto-cjk`, ~90 MB) sind bewusst nicht
enthalten.

### Quellcode-Variante

Nicht umgesetzt, nur Option: Ein Multi-Stage-Build mit `rust:<version>-trixie` und `cargo build --release -p os-cli
--locked` wäre technisch einfach, aber (1) es fehlte `THIRD_PARTY_NOTICES.txt` (entsteht im Release über `licenses.yml`
mit cargo-about und SHA-gepinnten Texten), (2) `rust-toolchain.toml` nennt nur `stable`, das Image müsste eine Version
pinnen und `RUSTUP_TOOLCHAIN` setzen, (3) ohne Docker hier nicht prüfbar. Wer aus Quellen bauen will, packt das Archiv
wie `release.yml` (`openspider`, `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.txt`) und übergibt es mit
`--build-context archive=<ordner>`; das Dockerfile bleibt dasselbe.

## Workflow `.github/workflows/docker.yml`

| Job | Wann | Inhalt |
|---|---|---|
| `contract` | immer (bei `workflow_run` nur für erfolgreiche Tag-Läufe) | `node --test packaging/docker/*.test.mjs` |
| `image` | `workflow_dispatch`; `workflow_run` nach erfolgreichem „Release orchestration“ eines Tags `v*` | Quelle bestimmen → `os-testsite` bauen → Image laden (`openspider:smoke`) → `smoke.mjs` → gleicher Build als OCI-Archiv mit `sbom: true`, `provenance: mode=max` → `oci.mjs` → Artefakte |
| `publish` | nur `workflow_dispatch` mit `publish=true`, `source=release`, Ref `main`, `identical == 'true'` | Environment `ghcr`, prüft Pflichtprüfer, `skopeo copy --all --preserve-digests` des geprüften OCI-Archivs |

Auslöser: push auf `main`/PR für `packaging/docker/**`, den Workflow selbst, `release.yml` und Spec 06 (nur Vertragstest). Kein `push: tags`,
kein `release:`-Trigger. Rechte: oben `contents: read`; `image` zusätzlich `actions: read` (Artefakte eines anderen
Laufs, Commit des Laufs); nur `publish` hat `packages: write`. Actions SHA-gepinnt: checkout/setup-node/
download-/upload-artifact wie in `release.yml`, `docker/setup-buildx-action@f87e5991… # v4.4.1` (16.09.2026),
`docker/build-push-action@c3c9e263… # v7.4.0` (15.09.2026), SHAs per `gh api …/git/matching-refs/tags` geprüft.
`docker/login-action` wird nicht gebraucht (skopeo meldet sich selbst an).

Quellen des Archivs: `source=run` lädt das Artefakt `release-assets` des Laufs (enthält Linux-Archiv und
`SHA256SUMS`), nimmt Version aus dem Dateinamen und Prüfsumme aus `SHA256SUMS`, übergibt das Archiv als Build-Kontext;
`source=release` prüft, dass `v<version>` veröffentlicht (kein Entwurf) ist, lädt nur `SHA256SUMS` und lässt das
Dockerfile das Archiv selbst herunterladen (derselbe Weg wie beim Nutzer). Revision: `head_sha` des Laufs bzw. Commit
des Tags (`gh api repos/…/commits/v<version>`; die Tags sind annotiert).

Artefakte: `openspider-docker-report` (30 Tage: `smoke.md`, `smoke.json`, `sbom.spdx.json`, `provenance.json`,
`oci.md`), `openspider-docker-oci` (7 Tage: OCI-Archiv, das `publish` pusht).

### Anleitung für den Orchestrator nach dem Merge

1. **Automatisch mit beta.3:** Der Tag-Push `v1.0.0-beta.3` startet „Release orchestration“; nach dessen Erfolg startet
   „Docker image“ per `workflow_run` (nur wenn `docker.yml` dann schon auf `main` liegt) und baut aus genau diesen
   Artefakten. Kein Push.
2. **Vorab mit einem Trockenlauf** (empfohlen vor Mittwoch): Release-Trockenlauf wie gewohnt; dann
   `gh workflow run docker.yml --ref main -f source=run -f run_id=<ID des Release-Laufs>`.
   Hinweis: Gegen **beta.2** (`-f source=release -f version=1.0.0-beta.2`) baut das Image, aber die Prüfung „Vergleich
   … Crawl Changes“ schlägt erwartungsgemäß fehl, weil der Report „Crawl Changes“ (B3-DIFF) erst nach beta.2 kam.
3. **Nach Veröffentlichung von beta.3**, ohne Push: `gh workflow run docker.yml --ref main -f source=release -f version=1.0.0-beta.3`.

Der Smoke-Test muss in der Job-Zusammenfassung 13 × „ok“ zeigen:

| Prüfung | Erwartung |
|---|---|
| Metadaten | `User 10001:10001`, Labels (Apache-2.0, Version, Quelle, Titel, Beschreibung), Entrypoint/Cmd, `/data`, Volume `/config`, Umgebung |
| Nicht root | `id` → `uid=10001(openspider) gid=10001(openspider)` |
| `--version` | `openspider 1.0.0-beta.3 (<hash>, linux-x86_64)` |
| Standard-CMD | Hilfe mit `--crawl`, `--export-tabs`, `--output-folder` |
| Chromium | `Chromium 1xx…` (Version steht im Detail) |
| Lizenzhinweise | `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.txt`, `copyright` von tini, ca-certificates, chromium |
| Crawl mit CSV-Export | `out-basic-site` als Benutzer 10001: `internal_all.csv` mit so vielen Zeilen wie das Golden, `crawl_overview.csv`, `crawl.ospider` |
| Vergleich | zwei Projektcrawls `out-b3-diff` v1 → v2 mit `--user`, gemountetem `/config`, `--compare-with-previous`: erster Report nur Kopfzeile, zweiter **gleich** `tests/golden/out-b3-diff/crawl_changes.csv` |
| JS-Rendering | `crawl-m2-render`: Titel der Startseite `Rendered`, `/js-only` gefunden |
| Fehlendes Geheimnis | Exit ≠ 0, stderr „nicht verfügbar“ + Kennung, nach wenigen Sekunden |
| MCP über STDIO | `initialize` → `tools/list`: `serverInfo.name = openspider`, `list_crawls` ja, `start_crawl` nein (nur lesend); `tools/call list_crawls` sieht das Projekt aus `/config`; stdout nur JSON-RPC; Exit 0 nach stdin-Ende |
| MCP `--allow read,crawl` | `start_crawl` in `tools/list` |
| Image-Größe | Zahl in MB (Information) |

Danach „OCI-Export und Attestierungen“: SBOM mit `chromium`, `tini`, `ca-certificates`, Provenance (SLSA) und
„Gleich dem geprüften Image: ja“. Steht dort „nein“, ist die Veröffentlichung gesperrt (Warnung, kein Fehler).

## Freigabeweg ghcr.io

1. Nutzer legt einmalig das Environment **`ghcr`** mit **Required reviewers** an. Der Job prüft das per API und bricht
   ohne Pflichtprüfer ab (fail closed), auch wenn GitHub das Environment beim ersten Lauf ohne Regeln anlegt.
2. Release ist veröffentlicht (bisheriges Go).
3. `gh workflow run docker.yml --ref main -f source=release -f version=1.0.0-beta.3 -f publish=true`.
4. Smoke grün und OCI gleich → `publish` wartet auf die Freigabe des Nutzers im Environment.
5. Push von `ghcr.io/wojcikdominik/openspider:1.0.0-beta.3` und `:beta` (stabile Versionen `:latest`); eine
   vorhandene Version wird nie überschrieben. Digest steht in der Zusammenfassung.
6. Einmalig: Paket in den GitHub-Paketeinstellungen auf öffentlich stellen und prüfen, dass es mit dem Repository
   verknüpft ist (Label `org.opencontainers.image.source`).

Die Website-Abschnitte schreiben „wird mit dem Release unter ghcr.io … veröffentlicht“ und nennen den Selbstbau als
Weg ohne Registry; sie sollten erst deployt werden, wenn der Push freigegeben ist oder unmittelbar bevorsteht.

## Spec-Änderung

`docs/spec/06-output-ops.md`: neuer Eintrag **OUT-274** nach OUT-273 (Beschreibung, Eingaben, Ausgabe, acht
Akzeptanzkriterien: nicht root/UID 10001, Prüfsumme vor dem Entpacken samt Inhalt/Version, gepinnte Basis ohne
`latest`, kein Geheimnis, Smoke-Test-Umfang, SBOM/Provenance am geprüften Image, kein Push ohne Freigabe, Lizenzhinweise),
Inhaltsübersicht „OUT-260 – OUT-274 … Docker-Image“ und ein Nachtrag in Anhang B (fehlender Sandbox-Schlüssel).
`docs/spec/PARITY.md` nicht ergänzt (OUT-273 fehlt dort ebenfalls; ID ohne Paritätsbezug).

## Geprüft (Server `m3-docker`)

- `node --test packaging/docker/*.test.mjs`: 16/16.
- hadolint 2.15.1: ohne Befund (bewusst ignoriert mit Begründung: DL3008 Versions-Pin, DL3064 für den Namen
  `OPENSPIDER_SECRET_STORE`). Befund am Werkzeug: hadolint setzt die Shell nach `ARG` auf sh zurück; `SHELL` steht
  deshalb nach den `ARG`s jeder Stufe.
- shellcheck 0.11.0 (`openspider-chromium`), actionlint 1.7.12 mit shellcheck (`docker.yml`): ohne Befund.
- `node website/check-site.mjs`: OK (20 Seiten); `scripts/check-neutral.sh`: keine Fundstellen.
- `node --test scripts/release/*.test.mjs scripts/release/licenses/generate.test.mjs tests/release/orchestration.test.mjs`:
  35/35 (mit `zip` aus `~/b3rel-tools/root/usr/bin`; ohne es scheitert nur der bekannte Zip-Test).
- `cargo fmt --all --check`: grün. Kein Rust-Code geändert, daher kein Workspace-Test und kein `target/`.
- Werkzeuge: hadolint und shellcheck als statische Binaries nach `~/b3rel-tools/hadolint` bzw.
  `~/b3rel-tools/shellcheck` geladen (GitHub-Releases, SHA-256 gegen die Release-Angabe geprüft); nur Prüfwerkzeuge,
  nicht im Repo.

Nicht prüfbar ohne Docker (Annahmen, die der erste Workflow-Lauf bestätigt): Ersetzen der Stufe `archive` per
`--build-context`, Paketnamen in trixie (`fonts-liberation`, `fonts-noto-color-emoji`, `tini`, `chromium`), Pfade
`/usr/bin/chromium` und `/usr/bin/tini`, Paketnamen im BuildKit-SBOM, `skopeo` auf `ubuntu-latest`, gleiche Schicht-IDs
von geladenem Image und OCI-Export.

## Fremde Dateien (K2-92)

| Datei | Eigentümer/Bereich | Änderung |
|---|---|---|
| `docs/spec/06-output-ops.md` | Spec (Planer) | OUT-274 neu, Inhaltsübersicht Abschnitt 13, Nachtrag Anhang B |
| `README.md` | Projekt | Absatz „Docker (self-hosted server)“ im Abschnitt Installation |
| `website/site/handbuch/index.html` | Website (W01/W02) | Abschnitt `#docker`, Inhaltsnavigation, Meta-Beschreibung um „Docker“ |
| `website/site/en/manual/index.html` | Website | dasselbe auf Englisch |
| `website/site/download/index.html` | Website | kurzer Abschnitt `#docker` mit Link ins Handbuch |
| `website/site/en/download/index.html` | Website | dasselbe auf Englisch |

Eigene neue Dateien: `packaging/docker/*`, `.github/workflows/docker.yml`, `docs/release/docker.md`, dieser Bericht.
Nicht angefasst: `release.yml`, `release-scripts.yml` (der Vertragstest läuft im eigenen Workflow), `PARITY.md`.

## Im Image enthaltene Pakete und Lizenzen

Direkt installiert: `chromium` (BSD-3-Clause, Drittkomponenten laut `copyright`), `tini` (MIT), `ca-certificates`
(MPL-2.0 für die Zertifikatsdaten, GPL-2+ für die Hilfsskripte), `fonts-liberation` (SIL OFL 1.1),
`fonts-noto-color-emoji` (SIL OFL 1.1). Aus der Basis und als Abhängigkeiten kommen übliche Debian-Systempakete, auch
unter GPL/LGPL (bash, coreutils, glibc, GLib/GTK u. a.); das sind Systempakete im Sinne des Auftrags, kein Teil des
OpenSpider-Abhängigkeitsbaums. Die vollständige Liste mit Versionen liefert das SBOM-Artefakt (`sbom.spdx.json`).
OpenSpider selbst: Apache-2.0 mit `THIRD_PARTY_NOTICES.txt` aus dem Release.

## Spec-Fragen und offene Punkte

1. **Chromium-Sandbox:** `os-render` hat `LaunchOpts.no_sandbox` („Explizite Freigabe für Container ohne
   Chromium-Sandbox“), aber keine Spec und keinen Konfigurationsschlüssel. Das Image löst es über den Starter. Soll es
   einen Schlüssel geben (z. B. `rendering.no_sandbox`) oder bleibt es Sache der Paketierung?
2. **`--install-browser` unter Linux:** OUT-207 verlangt, dass die CLI bei fehlendem Chromium diesen Befehl nennt;
   unter Linux gibt es aber keinen Download (`linux64` fehlt in `versions.json`). Im Image unerheblich
   (`OPENSPIDER_CHROMIUM` ist gesetzt), für die Linux-CLI ohne Docker eine Lücke.
3. **Geheimnisse aus Dateien:** Docker-/Compose-Secrets liegen als Dateien unter `/run/secrets`; die CLI liest nur
   Umgebungsvariablen. Vorschlag: `OPENSPIDER_<NAME>_FILE` (OUT-201/OUT-187). Bis dahin `-e`/`--env-file`.
4. **Sicherheits-Neubauten:** Eine veröffentlichte Version wird nie überschrieben; neue Chromium-Updates kommen also
   nur mit dem nächsten Release ins Image (bei wöchentlichen Betas vertretbar). Für 1.0 entscheiden: Neubau-Tags
   (`1.0.0-r2`) oder bewusstes Überschreiben der Kanal-Tags.
5. **Gemischte Benutzer auf einem `/config`:** Wer abwechselnd als 10001 und mit `--user` startet, bekommt Ordner mit
   verschiedenen Eigentümern. Doku empfiehlt eine Variante; ein Startskript, das Rechte korrigiert, würde root brauchen.
6. **arm64-Image** gibt es nicht (Linux-CLI nur x86_64, OUT-264). Docker Desktop auf Apple Silicon emuliert.
7. **Absturzberichte im Container:** `OPENSPIDER_DISABLE_CRASH_REPORTS` habe ich nicht gesetzt (keine Spec-Aussage
   zum Containerbetrieb); bitte entscheiden.
8. **Kopplung an `release.yml`:** `workflow_run` hängt am Namen „Release orchestration“ und am Artefakt
   `release-assets`. Der Vertragstest liest beides aus `release.yml` (ohne die Datei zu ändern), und `docker.yml` läuft
   bei Änderungen an `release.yml`; eine Umbenennung fällt also im PR auf statt still den Docker-Lauf abzuschalten.
