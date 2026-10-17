#!/usr/bin/env bash
# Überträgt den Schattenbetrieb E1 auf den VPS (M2-V02). Läuft lokal auf dem Mac des Orchestrators.
#
#   scripts/shadow/deploy.sh [--run-id <id>] [--host root@100.67.209.49] [--out <ordner>] [--dry-run]
#   scripts/shadow/deploy.sh --snapshot nachher.txt [--out <ordner>]   (nur die vier Listen vom VPS holen)
#
# Ablauf:
#   1. Artefakt openspider-linux-x86_64 des letzten grünen linux-core-Laufs auf main laden (gh run download)
#      und mit openspider.sha256 prüfen. Mit --run-id wird ein bestimmter Lauf verwendet (muss grün sein, auf main).
#   2. Vorher-Liste vom VPS holen (systemctl list-unit-files, list-timers, ss -ltn, df -h /).
#   3. Nur unter /opt/openspider-shadow ablegen: bin/ (openspider, run.sh, compare.sh, compare.py, uninstall.sh,
#      VERSION), etc/, systemd/. Systembenutzer openspider-shadow (nologin) anlegen, Units per
#      "systemctl link" einbinden (nur Symlinks), Timer bleibt aus (siehe Runbook). Kein offener Port, kein Caddy-, Docker-,
#      Firewall- oder Cron-Eintrag. Eine vorhandene etc/autonomo-source.toml bleibt erhalten.
#      Läuft openspider-shadow.service gerade (systemctl is-active), bricht deploy.sh vor jeder Änderung ab; die
#      Dateien in bin/ werden per Kopie neben das Ziel und mv atomar ersetzt.
#   4. Nachher-Liste holen und beide Listen unter --out ablegen. Den Timer aktiviert das Runbook erst nach dem
#      beobachteten Sofortlauf; danach eine weitere Liste mit --snapshot holen.
# Kein Auto-Update: eine neue Version entsteht nur durch einen erneuten Aufruf. Der Sofortlauf
# (systemctl start openspider-shadow.service) ist ein eigener Schritt, siehe docs/plan/shadow/E1-runbook.md.
set -euo pipefail

HOST="root@100.67.209.49"
RUN_ID=""
OUT=""
SNAPSHOT=""
DRY=0
REPO="${SHADOW_REPO:-domdewom2/openspider}"
while [ $# -gt 0 ]; do
  case "$1" in
    --run-id) RUN_ID="$2"; shift 2 ;;
    --host) HOST="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --dry-run) DRY=1; shift ;;
    --snapshot) SNAPSHOT="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "Unbekannte Option: $1" >&2; exit 2 ;;
  esac
done

HERE="$(cd "$(dirname "$0")" && pwd)"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
OUT="${OUT:-$PWD/shadow-deploy-$(date -u +%Y%m%dT%H%M%SZ)}"
mkdir -p "$OUT"
SSH=(ssh -o BatchMode=yes "$HOST")

snapshot() { # $1 = Dateiname unter $OUT
  "${SSH[@]}" 'echo "## systemctl list-unit-files"; systemctl list-unit-files --no-pager; echo; echo "## systemctl list-timers"; systemctl list-timers --all --no-pager; echo; echo "## ss -ltn"; ss -ltn; echo; echo "## df -h /"; df -h /' >"$OUT/$1"
}
if [ -n "$SNAPSHOT" ]; then snapshot "$SNAPSHOT"; echo "Liste: $OUT/$SNAPSHOT"; exit 0; fi

# 1. Artefakt
if [ -z "$RUN_ID" ]; then
  RUN_ID="$(gh run list -R "$REPO" --workflow linux-core.yml --branch main --status success --limit 1 --json databaseId --jq '.[0].databaseId')"
fi
[ -n "$RUN_ID" ] || { echo "Kein grüner linux-core-Lauf auf main gefunden." >&2; exit 1; }
gh run view -R "$REPO" "$RUN_ID" --json conclusion,headBranch,headSha,workflowName \
  --jq 'select(.conclusion == "success" and .headBranch == "main" and .workflowName == "linux-core")' | grep -q . \
  || { echo "Lauf $RUN_ID ist nicht grün, nicht auf main oder nicht linux-core." >&2; exit 1; }
SHA="$(gh run view -R "$REPO" "$RUN_ID" --json headSha --jq .headSha)"
gh run download -R "$REPO" "$RUN_ID" -n openspider-linux-x86_64 -D "$STAGE/artifact"
( cd "$STAGE/artifact" && shasum -a 256 -c openspider.sha256 ) || { echo "sha256-Prüfung fehlgeschlagen." >&2; exit 1; }
BIN_SHA="$(shasum -a 256 "$STAGE/artifact/openspider" | awk '{ print $1 }')"
echo "Artefakt: Lauf $RUN_ID, Commit $SHA, sha256 $BIN_SHA"

# Stage = künftiger Inhalt von /opt/openspider-shadow
mkdir -p "$STAGE/pkg/bin" "$STAGE/pkg/etc" "$STAGE/pkg/systemd"
install -m 0755 "$STAGE/artifact/openspider" "$STAGE/pkg/bin/openspider"
for f in run.sh compare.sh compare.py uninstall.sh; do install -m 0755 "$HERE/$f" "$STAGE/pkg/bin/$f"; done
for f in shadow.osconfig shadow.env rule-map.toml; do install -m 0644 "$HERE/$f" "$STAGE/pkg/etc/$f"; done
install -m 0640 "$HERE/autonomo-source.toml" "$STAGE/pkg/etc/autonomo-source.toml.dist"
install -m 0644 "$HERE/systemd/openspider-shadow.service" "$HERE/systemd/openspider-shadow.timer" "$STAGE/pkg/systemd/"
printf 'commit=%s\nrun_id=%s\nsha256=%s\ndeployed_at=%s\n' "$SHA" "$RUN_ID" "$BIN_SHA" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >"$STAGE/pkg/bin/VERSION"

if [ "$DRY" = 1 ]; then
  echo "Trockenlauf: Inhalt von /opt/openspider-shadow wäre:"; ( cd "$STAGE/pkg" && find . -type f | sort ); exit 0
fi

# Ein laufender Schattenlauf wird nicht überschrieben: Binärdatei und Skripte sind dann in Benutzung (ETXTBSY).
state="$("${SSH[@]}" 'systemctl is-active openspider-shadow.service 2>/dev/null || true' | head -n1)"
case "$state" in
  active|activating|deactivating|reloading)
    echo "Abbruch: openspider-shadow.service läuft ($state). Erst das Ende abwarten (journalctl -fu openspider-shadow.service) und deploy.sh danach erneut starten." >&2
    exit 1 ;;
esac

# 2. Vorher
snapshot vorher.txt
echo "Vorher-Liste: $OUT/vorher.txt"

# 3. Ablegen und einbinden
tar -C "$STAGE/pkg" -cf - . | "${SSH[@]}" 'set -euo pipefail
BASE=/opt/openspider-shadow
TMP="$(mktemp -d)"; trap "rm -rf $TMP" EXIT
tar --no-same-owner -C "$TMP" -xf -
id openspider-shadow >/dev/null 2>&1 || useradd --system --shell /usr/sbin/nologin --home-dir "$BASE" --no-create-home openspider-shadow
install -d -o root -g root -m 0755 "$BASE" "$BASE/bin" "$BASE/etc" "$BASE/systemd"
install -d -o openspider-shadow -g openspider-shadow -m 0750 "$BASE/var" "$BASE/var/crawls" "$BASE/var/exports" "$BASE/var/reports" "$BASE/var/home"
# Atomar ersetzen (Kopie neben das Ziel, dann mv): überschreibt nie eine Datei, die gerade ausgeführt wird.
for f in "$TMP"/bin/*; do n="$(basename "$f")"; cp -a "$f" "$BASE/bin/.$n.new"; mv -f "$BASE/bin/.$n.new" "$BASE/bin/$n"; done
cp -a "$TMP/systemd/." "$BASE/systemd/"
for f in shadow.osconfig shadow.env rule-map.toml; do cp -a "$TMP/etc/$f" "$BASE/etc/$f"; done
cp -a "$TMP/etc/autonomo-source.toml.dist" "$BASE/etc/autonomo-source.toml.dist"
[ -e "$BASE/etc/autonomo-source.toml" ] || cp -a "$TMP/etc/autonomo-source.toml.dist" "$BASE/etc/autonomo-source.toml"
chown -R root:root "$BASE/bin" "$BASE/etc" "$BASE/systemd"
chmod 0600 "$BASE/etc/autonomo-source.toml" "$BASE/etc/autonomo-source.toml.dist"
systemctl link "$BASE/systemd/openspider-shadow.service" "$BASE/systemd/openspider-shadow.timer"
systemctl daemon-reload
cat "$BASE/bin/VERSION"' 

# 4. Nachher
snapshot nachher.txt
echo "Nachher-Liste: $OUT/nachher.txt"
diff -u "$OUT/vorher.txt" "$OUT/nachher.txt" >"$OUT/unterschied.diff" || true
echo "Unterschied: $OUT/unterschied.diff (Erwartung: nur openspider-shadow.* und der Timer-Eintrag)"
