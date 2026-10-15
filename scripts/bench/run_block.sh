#!/bin/sh
# Aufrufhilfe fuer M1-N14: wartet auf Load < 2, protokolliert Load/Zeit vor dem Lauf und fuehrt den Befehl
# unter caffeinate aus. Kein Produktcode.
# Aufruf: scripts/bench/run_block.sh <logdatei> <befehl> [argumente...]
log="$1"; shift
dir=$(dirname "$0")
"$dir/wait_quiet.sh" 2 120 || exit 75   # 75 = Maschine blieb unruhig (Aufrufer darf wiederholen)
{ echo "Start $(date -u +%FT%TZ) ($(date +%H:%M) lokal)"; uptime; } > "$log"
caffeinate -dimsu "$@" >> "$log" 2>&1
rc=$?
{ echo "Ende $(date -u +%FT%TZ) ($(date +%H:%M) lokal), Exit $rc"; uptime; } >> "$log"
exit $rc
