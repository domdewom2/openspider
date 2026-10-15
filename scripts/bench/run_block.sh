#!/bin/sh
# Aufrufhilfe fuer M1-N14: wartet auf Load < 2, protokolliert Load/Zeit vor dem Lauf und fuehrt den Befehl
# unter caffeinate aus. Kein Produktcode.
# Aufruf: scripts/bench/run_block.sh <logdatei> <befehl> [argumente...]
log="$1"; shift
dir=$(dirname "$0")
# Welle 7 ist Entwicklung (Orchestrator-Entscheidung): hoechstens 10 min auf Load < 8 warten, danach messen;
# der Load steht im Kopf der Logdatei. Absolute Abnahmewerte misst Welle 7b.
"$dir/wait_quiet.sh" 8 10 >/dev/null 2>&1
{ echo "Start $(date -u +%FT%TZ) ($(date +%H:%M) lokal)"; uptime; } > "$log"
caffeinate -dimsu "$@" >> "$log" 2>&1
rc=$?
{ echo "Ende $(date -u +%FT%TZ) ($(date +%H:%M) lokal), Exit $rc"; uptime; } >> "$log"
exit $rc
