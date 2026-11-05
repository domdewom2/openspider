# Docker-Image (OUT-274, B3-DOCKER)

Stand: 05.11.2026. Das Image bündelt die Linux-CLI aus dem Release-Archiv mit Chromium für den Serverbetrieb:
CLI, Exporte, geplante Läufe (cron auf dem Host) und MCP über STDIO. **Keine Weboberfläche.** Veröffentlichung nach
`ghcr.io/wojcikdominik/openspider` nur nach ausdrücklichem Go des Nutzers.

| Datei | Zweck |
|---|---|
| `packaging/docker/Dockerfile` | Stufen `fetch` → `archive` (Download) → `verify` (SHA-256, Inhalt, Version) → `runtime` |
| `packaging/docker/openspider-chromium` | Chromium-Starter (`OPENSPIDER_CHROMIUM`): `--no-sandbox` außer mit `OPENSPIDER_CHROMIUM_SANDBOX=1`, `--disable-dev-shm-usage` |
| `packaging/docker/.dockerignore` | Build-Kontext `packaging/docker` enthält nur den Starter |
| `packaging/docker/smoke.mjs` | Smoke-Test gegen `os-testsite` (auch lokal auf Linux) |
| `packaging/docker/oci.mjs` | SBOM/Provenance aus dem OCI-Export prüfen, Gleichheit mit dem geprüften Image |
| `packaging/docker/docker.test.mjs` | Vertragstest (`node --test packaging/docker/*.test.mjs`) |
| `.github/workflows/docker.yml` | Vertragstest, Bauen, Smoke, Attestierungen, Veröffentlichung hinter Freigabe |

## Aufbau

- Basis `debian:trixie-<datum>-slim@sha256:…` (Tag und Digest gepinnt). glibc 2.41 ≥ 2.35 des Release-Builds
  (`release.yml`, Job `linux-core` auf `ubuntu-22.04`).
- Archiv `openspider_<version>_linux-x86_64.tar.gz`: per HTTPS von GitHub Releases oder aus einem lokalen Ordner
  (`--build-context archive=<ordner>`). `OPENSPIDER_SHA256` ist Pflicht und wird vor jedem Lesen des Archivs geprüft;
  danach müssen genau `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.txt`, `openspider` enthalten sein und
  `openspider --version` muss `openspider <version> (…, linux-x86_64)` melden.
- Laufzeit: Pakete `ca-certificates chromium fonts-liberation fonts-noto-color-emoji tini` (ohne Empfehlungen, nicht
  versionsgepinnt, damit ein Neubau Sicherheitsupdates bekommt), Benutzer `openspider` UID/GID 10001, `/data`
  (Arbeits- und Ausgabeordner), `/config` = `OPENSPIDER_HOME` (Volume), `OPENSPIDER_SECRET_STORE=memory`,
  `OPENSPIDER_DISABLE_UPDATE_CHECK=1`, Einstiegspunkt `tini -- openspider`, Standardbefehl `--help`.
- Lizenzen: `/usr/share/doc/openspider/{LICENSE,NOTICE,THIRD_PARTY_NOTICES.txt}` aus dem Archiv,
  `/usr/share/doc/<paket>/copyright` der Debian-Pakete.

## Lokal bauen

```sh
docker build -t openspider:1.0.0-beta.3 \
  --build-arg OPENSPIDER_VERSION=1.0.0-beta.3 \
  --build-arg OPENSPIDER_SHA256=<Zeile aus SHA256SUMS> \
  packaging/docker
# Smoke (Linux, Docker mit Host-Netz):
cargo build --locked -p os-testsite
node packaging/docker/smoke.mjs --image openspider:1.0.0-beta.3 --version 1.0.0-beta.3 --testsite target/debug/os-testsite
```

## Workflow `docker.yml`

| Auslöser | Ablauf |
|---|---|
| Push auf `main`/PR mit Änderungen unter `packaging/docker/` oder am Workflow | nur Vertragstest |
| `workflow_run` nach erfolgreichem Lauf von „Release orchestration“ für einen Tag (`v*`) | Image aus dem Artefakt `release-assets` dieses Laufs, Smoke, SBOM/Provenance, OCI-Archiv als Artefakt; **kein Push** |
| `workflow_dispatch`, `source=run`, `run_id=<Lauf>` | wie oben für einen beliebigen Release-Lauf (auch Trockenlauf) |
| `workflow_dispatch`, `source=release`, `version=<x.y.z-beta.n>` | Archiv per Download aus dem veröffentlichten Release, Prüfsumme aus dessen `SHA256SUMS` |
| zusätzlich `publish=true` (nur `source=release`, nur auf `main`) | nach bestandenem Smoke und gleichem OCI-Export: Job `publish` im Environment `ghcr` |

Der Workflow baut mit Dockerfile, Smoke-Test und Fixtures aus dem **Tag-Commit** des Release-Laufs
(`workflow_run.head_sha`; bei `workflow_dispatch` aus dem gewählten Ref). Ein Tag von einem Branch ohne
`packaging/docker` scheitert deshalb im Vertragstest bzw. beim Bauen; Release-Branches müssen von einem `main` mit
`packaging/docker` abzweigen. `source=run` prüft vor dem Download den Lauf über die API: Workflow „Release
orchestration“, dieses Repository, Ergebnis `success`, Auslöser `push` oder `workflow_dispatch`. Die Jobs `image` und
`publish` laufen fest auf `ubuntu-24.04`.

Job `image`: Quelle bestimmen → `os-testsite` bauen → Image laden (`openspider:smoke`) → `smoke.mjs` → derselbe Build als
OCI-Archiv mit `sbom: true`, `provenance: mode=max` → `oci.mjs` (SBOM nennt `chromium`, `tini`, `ca-certificates`;
Schichten und Konfiguration gleich dem geprüften Image) → Artefakte `openspider-docker-report` (Smoke-Tabelle,
`sbom.spdx.json`, `provenance.json`) und `openspider-docker-oci` (7 Tage).

SBOM und Provenance sind **unsignierte** BuildKit-Attestierungen (in-toto, am Image-Index angehängt). Eine Signatur
(z. B. Sigstore/cosign oder GitHub-Attestierungen) ist eine Entscheidung für 1.0.

## Freigabeweg ghcr.io

1. Einmalig: Environment `ghcr` anlegen mit **Required reviewers** (Nutzer). Ohne Pflichtprüfer bricht der Job `publish`
   selbst ab (fail closed), auch wenn GitHub das Environment beim ersten Lauf automatisch anlegt.
2. Release veröffentlichen (Go des Nutzers wie bisher).
3. `workflow_dispatch` auf `main`: `source=release`, `version=<Version>`, `publish=true`.
4. Nach grünem Smoke wartet `publish` auf die Freigabe im Environment. Danach pusht `skopeo` genau das geprüfte
   OCI-Archiv als `:<version>` und `:beta` (stabile Versionen: `:latest`). Eine vorhandene Version wird nie überschrieben.
5. Einmalig nach dem ersten Push: Paket `openspider` in den GitHub-Paketeinstellungen auf **öffentlich** stellen und
   mit dem Repository verknüpfen (Label `org.opencontainers.image.source` verknüpft es normalerweise selbst).

Website, Handbuch und README nennen ghcr.io nur als vorbereitet und zeigen alle Beispiele mit dem lokalen Tag
`openspider:<version>` aus dem Selbstbau. Nach dem ersten freigegebenen Push können sie auf
`ghcr.io/wojcikdominik/openspider:<version>` umgestellt werden (eigener Doku-Commit).

## Basis-Image aktualisieren

Neuen Digest für `debian:trixie-<datum>-slim` holen (Docker Hub, Tag-Liste) und `DEBIAN_IMAGE` im Dockerfile ersetzen;
der Vertragstest verlangt Tag und Digest. Chromium und die übrigen Pakete werden bei jedem Bau aktuell aus trixie und
trixie-security installiert; für Sicherheitsupdates genügt ein Neubau (neuer Workflow-Lauf, neue Patch-Version).
