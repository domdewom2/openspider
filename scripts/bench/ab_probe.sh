#!/bin/sh
# Aufrufhilfe fuer M1-N22 (PERF-002 Konstanz): A/B-Kurzprobe unter gleicher Last. Kein Produktcode.
#
# Fuehrt eine Folge von Laeufen `os-loadtest run --scenario perf-001 --size large --pages <n>` direkt hintereinander
# aus (z. B. A B B A), jeden mit einem eigenen Binaerordner und optionalen Umgebungsvariablen. Die ganze Folge laeuft
# unter EINEM `lockf` auf bench.lock (Messregel Welle 7c: `cargo build|clippy|test` laufen nie unter dem Lock) und
# unter `caffeinate -dimsu`. Vor jedem Lauf wartet `wait_quiet.sh 8 10` (Exit 1 nach 10 Minuten: der Lauf startet
# trotzdem, der Load steht im Log). Der Load wird jede halbe Minute abgetastet (<name>.load).
#
# Aufruf:
#   scripts/bench/ab_probe.sh <ausgabeordner> <arbeitsordner> <seiten> <lauf> [<lauf> ...]
# mit  <lauf> = <name>=<binaerordner>[,VAR=wert]...
#   <binaerordner> enthaelt release/os-loadtest und release/examples/crawl_child (so findet os-loadtest das Beispiel).
#   <arbeitsordner> liegt ausserhalb des Repos (der Lasttest legt dort die Crawl-Datei an und loescht sie am Ende).
# Beispiel (A, B, B, A bei 500 000 Seiten):
#   scripts/bench/ab_probe.sh docs/plan/M1-acceptance/loadtest/probe-n22/raw ../n22-work 500000 \
#     A1=../n22-base B1=../target-bin B2=../target-bin A2=../n22-base
# Ergebnis je Lauf: <ausgabeordner>/<nr>-<name>.json (Bericht), .log (Kopf, Konsole, Ende), .load (Lastproben).
# Weitere Umgebungsvariablen fuer alle Laeufe: LOCK (Standard: ../bench.lock neben dem Worktree), SCENARIO (perf-001),
# EXTRA_ARGS (zusaetzliche Argumente fuer `os-loadtest run`, z. B. "--kills 10").

if [ -z "$AB_PROBE_LOCKED" ]; then
  here=$(cd "$(dirname "$0")" && pwd)
  lock="${LOCK:-$(cd "$here/../../.." && pwd)/bench.lock}"
  AB_PROBE_LOCKED=1 exec lockf -t 7200 "$lock" caffeinate -dimsu "$here/ab_probe.sh" "$@"
fi

out="${1:?Ausgabeordner fehlt}"
work="${2:?Arbeitsordner fehlt}"
pages="${3:?Seitenzahl fehlt}"
shift 3
here=$(cd "$(dirname "$0")" && pwd)
scenario="${SCENARIO:-perf-001}"
mkdir -p "$out" "$work" || exit 2
n=0
rc_all=0
for spec in "$@"; do
  n=$((n + 1))
  name="${spec%%=*}"
  rest="${spec#*=}"
  bin="${rest%%,*}"
  envs=""
  [ "$rest" != "$bin" ] && envs="${rest#*,}"
  tag=$(printf '%02d-%s' "$n" "$name")
  log="$out/$tag.log"
  json="$out/$tag.json"
  if "$here/wait_quiet.sh" 8 10 > "$log.wait" 2>&1; then quiet="ruhig"; else quiet="Load blieb >= 8, Lauf startet trotzdem"; fi
  {
    echo "Lauf $tag: Start $(date -u +%FT%TZ) ($(date +%H:%M) lokal)"
    echo "wait_quiet: $quiet ($(tr '\n' ' ' < "$log.wait"))"
    echo "Load beim Start: $(uptime)"
    echo "Binaerordner: $bin; Umgebung: ${envs:-keine}"
  } > "$log"
  rm -f "$log.wait"
  : > "$out/$tag.load"
  ( while :; do echo "$(date +%H:%M:%S) $(uptime | sed 's/.*load averages*: *//')" >> "$out/$tag.load"; sleep 30; done ) &
  sampler=$!
  # shellcheck disable=SC2086
  env $(echo "$envs" | tr ',' ' ') "$bin/release/os-loadtest" run --scenario "$scenario" --size large --pages "$pages" \
    --work-dir "$work" --out "$json" $EXTRA_ARGS >> "$log" 2>&1
  rc=$?
  kill "$sampler" 2>/dev/null
  wait "$sampler" 2>/dev/null
  { echo "Ende $(date -u +%FT%TZ) ($(date +%H:%M) lokal), Exit $rc"; uptime; } >> "$log"
  [ "$rc" -ne 0 ] && rc_all=$rc
done
exit $rc_all
