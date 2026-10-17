#!/usr/bin/env bash
# Vergleich nach dem Crawl (M2-V02): ExecStartPost=+ von openspider-shadow.service.
# Zweistufig, damit erhöhte Rechte nur dort gelten, wo der Lesezugang sie braucht:
#   1. (root) compare.py fetch-autonomo liest die Autonomo-Befunde nur lesend (GET bzw. SELECT) und legt sie
#      als var/reports/<datum>/autonomo-<site>.json ab (Eigentümer openspider-shadow, Modus 0640).
#   2. (openspider-shadow, ohne Zusatzrechte) compare.py compare schreibt <site>.json und <site>.md.
# Autonomo wird nie verändert; der Token wird nicht ausgegeben. Exit-Code ungleich 0, wenn ein Crawl oder
# ein Vergleich fehlgeschlagen ist.
set -euo pipefail

BASE="${SHADOW_BASE:-$(cd "$(dirname "$0")/.." && pwd)}"
SHADOW_USER="${SHADOW_USER:-openspider-shadow}"
SOURCE="${SHADOW_AUTONOMO_SOURCE:-api}"
PY="$BASE/bin/compare.py"

log() { printf '%s compare.sh: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }
json_get() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$1" "$2"; }

STAMP="$(json_get "$BASE/var/latest-run.json" stamp)"
SITES="$(python3 -c 'import json,sys; print(" ".join(json.load(open(sys.argv[1]))["sites"]))' "$BASE/var/latest-run.json")"
REP="$BASE/var/reports/$(date -u +%Y-%m-%d)"
mkdir -p "$REP"
if [ "$(id -u)" = 0 ]; then chown "$SHADOW_USER": "$REP"; fi

as_shadow() {
  if [ "$(id -u)" = 0 ]; then
    setpriv --reuid="$SHADOW_USER" --regid="$SHADOW_USER" --clear-groups "$@"
  else
    "$@"
  fi
}

fail=0
for site in $SITES; do
  dir="$BASE/var/exports/$site/$STAMP"
  meta="$dir/run.json"
  if [ ! -f "$meta" ]; then log "$site: run.json fehlt"; fail=1; continue; fi
  if [ "$(json_get "$meta" exit_code)" != 0 ]; then
    log "$site: der Crawl endete mit Exit-Code $(json_get "$meta" exit_code), kein Vergleich"
    fail=1
    continue
  fi
  finished="$(json_get "$meta" finished_at)"
  auto="$REP/autonomo-$site.json"
  extra=()
  # SHADOW_AUTONOMO_SOURCE=file: bereits normalisierte Datei (SHADOW_AUTONOMO_FILE) statt Lesezugang, z. B. zum Wiederholen
  if [ "$SOURCE" = file ]; then extra=(--file "${SHADOW_AUTONOMO_FILE:?SHADOW_AUTONOMO_FILE fehlt}"); fi
  if ! ( umask 027; python3 "$PY" fetch-autonomo --site "$site" --near "$finished" --source "$SOURCE" \
           --config "$BASE/etc/autonomo-source.toml" ${extra[@]+"${extra[@]}"} --out "$auto" ); then
    log "$site: Autonomo-Befunde nicht lesbar"
    fail=1
    continue
  fi
  if [ "$(id -u)" = 0 ]; then chown "$SHADOW_USER": "$auto"; fi
  if ! as_shadow python3 "$PY" compare --site "$site" --exports "$dir" --autonomo "$auto" \
         --rule-map "$BASE/etc/rule-map.toml" --run-meta "$meta" --osconfig "$BASE/etc/shadow.osconfig" \
         --progress "$BASE/var/crawls/$site/$STAMP.progress.log" --out "$REP"; then
    log "$site: Vergleich fehlgeschlagen"
    fail=1
  fi
done
exit "$fail"
