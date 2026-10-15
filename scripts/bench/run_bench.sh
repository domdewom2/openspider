#!/bin/sh
# Aufrufhilfe fuer M1-N14: fuehrt einen #[ignore = "Benchmark ..."]-Test (alle ignorierten Tests einer Testdatei)
# einzeln in Release aus; wartet vorher auf Load < 2 und schreibt Load, Zeiten und Rohausgabe in <logdatei>.
# Aufruf: scripts/bench/run_bench.sh <logdatei> <crate> <testdatei-ohne-.rs>
log="$1"; crate="$2"; file="$3"
dir=$(dirname "$0")
"$dir/wait_quiet.sh" 2 120 || exit 1
t0=$(date +%s)
{ echo "Start $(date -u +%FT%TZ) ($(date +%H:%M) lokal)"; uptime; echo "cargo test --release --locked -p $crate --test $file -- --ignored --nocapture --test-threads=1"; } > "$log"
caffeinate -dimsu cargo test --release --locked -p "$crate" --test "$file" -- --ignored --nocapture --test-threads=1 >> "$log" 2>&1
rc=$?
{ echo "Ende $(date -u +%FT%TZ) ($(date +%H:%M) lokal), Exit $rc, Laufzeit $(( $(date +%s) - t0 )) s (inkl. Warten auf Load)"; uptime; } >> "$log"
exit $rc
