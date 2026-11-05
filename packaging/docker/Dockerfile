# OpenSpider im Container (OUT-274): CLI, Exporte, geplante Läufe per cron auf dem Host und MCP über STDIO.
# Keine Weboberfläche. Die Desktop-App bleibt die Oberfläche; sie öffnet die Crawl-Dateien (*.ospider) aus dem Container.
#
# Build-Kontext ist dieser Ordner (laut .dockerignore nur der Chromium-Starter):
#
#   docker build -t openspider:1.0.0-beta.3 \
#     --build-arg OPENSPIDER_VERSION=1.0.0-beta.3 \
#     --build-arg OPENSPIDER_SHA256=<SHA-256 von openspider_1.0.0-beta.3_linux-x86_64.tar.gz aus SHA256SUMS> \
#     packaging/docker
#
# Das Linux-Release-Archiv kommt von GitHub Releases (OPENSPIDER_ARCHIVE_URL überschreibt die Adresse, nur HTTPS)
# oder aus einem lokalen Ordner, der die Datei openspider_<version>_linux-x86_64.tar.gz enthält:
#   --build-context archive=<ordner>
# In beiden Fällen wird die SHA-256-Prüfsumme vor dem Entpacken geprüft. Eine Prüfsumme hat keinen Standardwert.

# Debian 13 (trixie, glibc 2.41): neuer als die glibc 2.35 des Release-Builds (ubuntu-22.04, release.yml), Chromium mit
# Sicherheitsupdates aus trixie-security. Tag und Digest gepinnt; Aktualisierung siehe docs/release/docker.md.
ARG DEBIAN_IMAGE=debian:trixie-20261027-slim@sha256:f6b08b09a05725f9431502d3b5714f5c4576e82c1a84fa1debe0c24a7e0c3ec4

# --- Werkzeuge zum Laden und Prüfen des Archivs (nicht im fertigen Image) ------------------------------------------
FROM ${DEBIAN_IMAGE} AS fetch
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
# Paketversionen bewusst nicht gepinnt: die Pakete kommen zum Bauzeitpunkt aus trixie/trixie-security.
# hadolint ignore=DL3008
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

# --- Release-Archiv laden (wird durch --build-context archive=<ordner> ersetzt) -------------------------------------
FROM fetch AS archive
ARG OPENSPIDER_VERSION
ARG OPENSPIDER_ARCHIVE_URL=""
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
RUN [[ "${OPENSPIDER_VERSION:-}" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-(beta|rc)\.[0-9]+)?$ ]] \
      || { echo "OPENSPIDER_VERSION fehlt oder ist ungültig: '${OPENSPIDER_VERSION:-}'" >&2; exit 1; }; \
    name="openspider_${OPENSPIDER_VERSION}_linux-x86_64.tar.gz"; \
    url="${OPENSPIDER_ARCHIVE_URL:-https://github.com/wojcikdominik/openspider/releases/download/v${OPENSPIDER_VERSION}/${name}}"; \
    curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --tlsv1.2 \
      --retry 3 --output "/${name}" "${url}"

# --- Prüfsumme vor dem Entpacken, dann Inhalt und Version prüfen ---------------------------------------------------
FROM fetch AS verify
ARG OPENSPIDER_VERSION
ARG OPENSPIDER_SHA256
# SHELL nach den ARGs: hadolint 2.15 setzt die Shell bei ARG sonst auf sh zurück (Stufen erben SHELL von fetch).
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]
COPY --from=archive /openspider_${OPENSPIDER_VERSION}_linux-x86_64.tar.gz /tmp/openspider.tar.gz
RUN [[ "${OPENSPIDER_SHA256:-}" =~ ^[0-9a-f]{64}$ ]] \
      || { echo "OPENSPIDER_SHA256 fehlt oder ist kein SHA-256 (64 Hexzeichen, klein)" >&2; exit 1; }; \
    echo "${OPENSPIDER_SHA256}  /tmp/openspider.tar.gz" | sha256sum --check --strict -; \
    entries="$(tar -tzf /tmp/openspider.tar.gz | LC_ALL=C sort | tr '\n' ' ')"; \
    [[ "${entries}" == "LICENSE NOTICE THIRD_PARTY_NOTICES.txt openspider " ]] \
      || { echo "Unerwarteter Archivinhalt: ${entries}" >&2; exit 1; }; \
    mkdir /out; \
    tar -xzf /tmp/openspider.tar.gz -C /out --no-same-owner --no-same-permissions; \
    chmod 0755 /out/openspider; \
    chmod 0644 /out/LICENSE /out/NOTICE /out/THIRD_PARTY_NOTICES.txt; \
    version="$(/out/openspider --version)"; \
    [[ "${version}" == "openspider ${OPENSPIDER_VERSION} ("*", linux-x86_64)" ]] \
      || { echo "Archiv passt nicht zur Version ${OPENSPIDER_VERSION}: ${version}" >&2; exit 1; }

# --- Laufzeit-Image ----------------------------------------------------------------------------------------------
FROM ${DEBIAN_IMAGE} AS runtime
ARG DEBIAN_IMAGE
ARG OPENSPIDER_VERSION
ARG OPENSPIDER_REVISION=""
SHELL ["/bin/bash", "-euo", "pipefail", "-c"]

# Chromium aus Debian (BSD-Lizenz, frei weiterverteilbar, Sicherheitsupdates über trixie-security) für JS-Rendering,
# Schriften für Layout und Screenshots, CA-Zertifikate für HTTPS, tini als PID 1 (Signale, Zombie-Prozesse von Chromium).
# Paketversionen bewusst nicht gepinnt (Sicherheitsupdates beim Neubau); die Lizenzdateien der Pakete bleiben unter
# /usr/share/doc/<paket>/copyright erhalten (Debian-slim behält sie).
# hadolint ignore=DL3008
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      chromium \
      fonts-liberation \
      fonts-noto-color-emoji \
      tini \
 && rm -rf /var/lib/apt/lists/* \
 && groupadd --system --gid 10001 openspider \
 && useradd --system --uid 10001 --gid 10001 --home-dir /home/openspider --create-home \
      --shell /usr/sbin/nologin openspider \
 && install -d -o 10001 -g 10001 -m 0755 /data /config

COPY --from=verify /out/openspider /usr/local/bin/openspider
COPY --from=verify /out/LICENSE /out/NOTICE /out/THIRD_PARTY_NOTICES.txt /usr/share/doc/openspider/
COPY --chmod=0755 openspider-chromium /usr/local/bin/openspider-chromium

LABEL org.opencontainers.image.title="OpenSpider" \
      org.opencontainers.image.description="OpenSpider SEO crawler for servers: CLI, exports, scheduled runs and MCP over STDIO. No web UI; open the crawl files in the desktop app." \
      org.opencontainers.image.source="https://github.com/wojcikdominik/openspider" \
      org.opencontainers.image.url="https://openspider.de/" \
      org.opencontainers.image.documentation="https://openspider.de/en/manual/#docker" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.version="${OPENSPIDER_VERSION}" \
      org.opencontainers.image.revision="${OPENSPIDER_REVISION}" \
      org.opencontainers.image.base.name="${DEBIAN_IMAGE}"

# Keine Geheimnisse im Image: Schlüssel und Passphrasen nur zur Laufzeit über Umgebungsvariablen (OPENSPIDER_PSI_KEY,
# OPENSPIDER_AUTH_PASSPHRASE …). Ohne D-Bus und Schlüsselbund hält OPENSPIDER_SECRET_STORE=memory Geheimnisse nur im
# laufenden Prozess. Updates kommen über ein neues Image, nicht aus dem Container. DL3064 meldet den Namen
# OPENSPIDER_SECRET_STORE; der Wert wählt nur die Ablage und ist kein Geheimnis.
# hadolint ignore=DL3064
ENV OPENSPIDER_HOME=/config \
    OPENSPIDER_CHROMIUM=/usr/local/bin/openspider-chromium \
    OPENSPIDER_SECRET_STORE=memory \
    OPENSPIDER_DISABLE_UPDATE_CHECK=1

WORKDIR /data
VOLUME ["/config"]
USER 10001:10001
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/openspider"]
CMD ["--help"]
