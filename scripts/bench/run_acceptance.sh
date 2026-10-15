#!/bin/sh
# Aufrufhilfe fuer M1-N21 (Abnahmemessung A4): wartet auf Load < 2 (wait_quiet.sh), schreibt Load und Zeiten in die
# Logdatei, tastet den Load waehrend des Laufs jede Minute ab (<logdatei>.load) und fuehrt den Befehl unter
# caffeinate aus. Kein Produktcode.
# Aufruf: scripts/bench/run_acceptance.sh <logdatei> <befehl> [argumente...]
log="$1"; shift
dir=$(dirname "$0")
"$dir/wait_quiet.sh" 2 180 > "$log.wait" 2>&1 || { cat "$log.wait" >&2; exit 99; }
{ echo "Start $(date -u +%FT%TZ) ($(date +%H:%M) lokal)"; echo "wait_quiet: $(cat "$log.wait")"; uptime; echo "Befehl: $*"; } > "$log"
: > "$log.load"
( while :; do echo "$(date +%H:%M:%S) $(uptime | sed 's/.*load averages*: *//')" >> "$log.load"; sleep 60; done ) &
sampler=$!
caffeinate -dimsu "$@" >> "$log" 2>&1
rc=$?
kill "$sampler" 2>/dev/null
{ echo "Ende $(date -u +%FT%TZ) ($(date +%H:%M) lokal), Exit $rc"; uptime; } >> "$log"
exit $rc
