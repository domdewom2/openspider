#!/bin/sh
# Aufrufhilfe fuer M1-N14: wartet, bis der 1-Minuten-Load-Average unter der Grenze liegt (Standard 2),
# und gibt Load und freien Platz aus. Kein Produktcode.
# Aufruf: scripts/bench/wait_quiet.sh [grenze] [max-minuten]
limit="${1:-2}"
max="${2:-30}"
i=0
while :; do
  load=$(uptime | sed 's/.*load averages*: *//' | awk '{gsub(",", ".", $1); print $1}')
  if awk -v l="$load" -v m="$limit" 'BEGIN{exit !(l < m)}'; then
    echo "load1=$load (< $limit), frei: $(df -g / | awk 'NR==2{print $4}') GB"
    exit 0
  fi
  i=$((i + 1))
  [ "$i" -gt "$((max * 2))" ] && { echo "load1=$load blieb >= $limit" >&2; exit 1; }
  sleep 30
done
