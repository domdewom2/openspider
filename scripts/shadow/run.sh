#!/usr/bin/env bash
# Schattencrawls E1 (M2-V02): je Site ein OpenSpider-Crawl, nacheinander, mit CSV-Exporten.
# Aufruf: ExecStart von openspider-shadow.service (Benutzer openspider-shadow); auf dem VPS unter bin/.
# Schreibt nur unter $BASE/var. Ablauf je Site:
#   var/exports/<site>/<stamp>/   CSV-Exporte, run.json (Messwerte), crawl.ospider wird nach var/crawls verschoben
#   var/crawls/<site>/<stamp>.ospider und <stamp>.progress.log (JSON-Fortschrittszeilen)
#   var/latest-run.json           Stempel und Sites des jüngsten Laufs (für compare.sh)
# Exit-Code: 75/78 bei fehlendem Platz bzw. fehlender Binärdatei, sonst 0 (Crawl-Fehler stehen in run.json).
# Umgebung (etc/shadow.env): SHADOW_SITES, SHADOW_KEEP, SHADOW_MIN_FREE_GB.
set -euo pipefail

BASE="${SHADOW_BASE:-$(cd "$(dirname "$0")/.." && pwd)}"
SITES="${SHADOW_SITES:-caloone.de rotweine.org}"
KEEP="${SHADOW_KEEP:-4}"
MIN_FREE_GB="${SHADOW_MIN_FREE_GB:-8}"
URL_TEMPLATE="${SHADOW_START_URL_TEMPLATE:-https://%s/}"   # nur für lokale Tests anders
FREE_PATH="${SHADOW_FREE_PATH:-/}"
SAMPLE_S="${SHADOW_SAMPLE_S:-5}"                              # Abstand der RSS-Messung in Sekunden
BIN="$BASE/bin/openspider"
export OPENSPIDER_HOME="${OPENSPIDER_HOME:-$BASE/var/home}"
export OPENSPIDER_SECRET_STORE="${OPENSPIDER_SECRET_STORE:-memory}"

log() { printf '%s run.sh: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >&2; }

# Platzprüfung: Abbruch mit Meldung, bevor irgendetwas geschrieben wird.
free_gb="$(df -Pk "$FREE_PATH" | awk 'NR==2 { print int($4 / 1048576) }')"
if [ "${free_gb:-0}" -lt "$MIN_FREE_GB" ]; then
  log "Abbruch: auf $FREE_PATH sind nur ${free_gb:-0} GB frei, verlangt sind mindestens $MIN_FREE_GB GB"
  exit 75
fi
[ -x "$BIN" ] || { log "Abbruch: $BIN fehlt oder ist nicht ausführbar"; exit 78; }

mkdir -p "$OPENSPIDER_HOME" "$BASE/var/crawls" "$BASE/var/exports" "$BASE/var/reports"
EXPORTS="$(python3 "$BASE/bin/compare.py" exports --rule-map "$BASE/etc/rule-map.toml")"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
done_sites=()

# Behält die jüngsten $KEEP Einträge (nach Name = Zeitstempel) unter $1; $2 = Muster der Einträge.
prune() {
  local dir="$1" pattern="$2" old
  { ls -1 "$dir" 2>/dev/null || true; } | { grep -E "$pattern" || true; } | sort \
    | awk -v keep="$KEEP" '{ a[NR] = $0 } END { for (i = 1; i <= NR - keep; i++) print a[i] }' \
    | while read -r old; do rm -rf -- "${dir:?}/$old"; done
}

crawl_site() {
  local site="$1" url out crawl_dir progress started t0 pid rss peak rc finished
  # shellcheck disable=SC2059
  url="$(printf "$URL_TEMPLATE" "$site")"
  out="$BASE/var/exports/$site/$STAMP"
  crawl_dir="$BASE/var/crawls/$site"
  progress="$crawl_dir/$STAMP.progress.log"
  mkdir -p "$out" "$crawl_dir"
  started="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  t0="$(date +%s)"
  log "Start $site ($url) nach $out"

  # --no-store: keine zweite Kopie in OPENSPIDER_HOME; --save-crawl legt crawl.ospider im Ausgabeordner ab.
  "$BIN" --crawl "$url" --config "$BASE/etc/shadow.osconfig" --headless --json-progress \
    --project-name shadow --task-name "$site-$STAMP" \
    --output-folder "$out" --overwrite --save-crawl --no-store \
    --export-format csv --export-tabs "$EXPORTS" 2>"$progress" &
  pid=$!

  # Spitzen-RSS: VmHWM des Prozesses alle 5 s lesen (höchster beobachteter Wert).
  peak=0
  while kill -0 "$pid" 2>/dev/null; do
    rss="$(awk '/^VmHWM:/ { print $2 }' "/proc/$pid/status" 2>/dev/null || true)"
    if [ -n "${rss:-}" ] && [ "$rss" -gt "$peak" ]; then peak="$rss"; fi
    sleep "$SAMPLE_S"
  done
  rc=0
  wait "$pid" || rc=$?
  finished="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

  if [ -f "$out/crawl.ospider" ]; then mv -f "$out/crawl.ospider" "$crawl_dir/$STAMP.ospider"; fi
  printf '{"site": "%s", "stamp": "%s", "start_url": "%s", "started_at": "%s", "finished_at": "%s", "duration_s": %d, "exit_code": %d, "peak_rss_kb": %d}\n' \
    "$site" "$STAMP" "$url" "$started" "$finished" "$(( $(date +%s) - t0 ))" "$rc" "$peak" >"$out/run.json"
  log "Ende $site: Exit-Code $rc, Spitzen-RSS ${peak} kB"

  prune "$BASE/var/exports/$site" '^[0-9]{8}T[0-9]{6}Z$'
  prune "$crawl_dir" '^[0-9]{8}T[0-9]{6}Z\.ospider$'
  prune "$crawl_dir" '^[0-9]{8}T[0-9]{6}Z\.progress\.log$'
  return "$rc"
}

for site in $SITES; do
  # Ein fehlgeschlagener Crawl hält die nächste Site nicht auf; der Exit-Code steht in run.json und
  # wird von compare.sh (ExecStartPost) ausgewertet, das den Dienst dann als fehlgeschlagen meldet.
  crawl_site "$site" || log "Hinweis: Crawl von $site endete mit Fehler (siehe run.json)"
  done_sites+=("$site")
done

printf '{"stamp": "%s", "sites": [%s]}\n' "$STAMP" "$(printf '"%s", ' "${done_sites[@]}" | sed 's/, $//')" >"$BASE/var/latest-run.json"
exit 0
