#!/bin/sh
# M1-P32: Messung M (nach jedem Schritt, Release) unter Messruhe: Store-Benchmark, perf-001 und perf-003 mit je 250 000
# Seiten. Jeder Block laeuft unter `lockf` (bench.lock), wartet hoechstens 10 min auf Load < 8 (wait_quiet.sh 8 10) und laeuft unter caffeinate
# (run_block.sh); Load und Zeiten stehen im Kopf der Logdateien.
#
# Aufruf:   scripts/bench/p32_measure.sh <label> [store|perf-001|perf-003 ...]   (ohne Angabe: alle drei Bloecke)
# Umgebung: OUT   Ausgabeordner (Standard docs/plan/M1-acceptance/loadtest/probe-p32/<label>)
#           WORK  Arbeitsordner fuer Crawl-Dateien AUSSERHALB des Repos (Standard ../p32-work), wird danach geleert
#           PAGES Seiten der Kurzproben (Standard 250000), STORE_PAGES Seiten des Store-Benchmarks (Standard 1000000)
#           EXTRA zusaetzliche Argumente fuer os-loadtest run (z. B. "--profile max-speed")
#           LOADTEST, STORE_BIN  fertig gebaute Programme (Pfad zu os-loadtest bzw. zur Testdatei bench_writer-<hash>), damit
#           ein spaeter geaenderter Quellstand die Ausgangsmessung nicht neu baut
# Voraussetzung: cargo build --release --locked -p os-loadtest und cargo test --release --locked -p os-store --test
#           bench_writer --no-run sind gebaut (die Befehle hier bauen bei Bedarf nach, ebenfalls unter der Sperre).
dir=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$dir/../.." && pwd)
label="${1:?Label fehlt}"; shift
out="${OUT:-$root/docs/plan/M1-acceptance/loadtest/probe-p32/$label}"
work="${WORK:-$root/../p32-work}"
pages="${PAGES:-250000}"
store_pages="${STORE_PAGES:-1000000}"
lock=/Users/dominikwojcik/Projects/openspider-wt/bench.lock
target="${CARGO_TARGET_DIR:-$root/target}"
loadtest="${LOADTEST:-$target/release/os-loadtest}"
store_bin="${STORE_BIN:-}"
mkdir -p "$out" "$work"
want="${*:-store perf-001 perf-003}"

block() { # <name> <befehl...>: nur der Messlauf selbst steht unter der Sperre (Builds und Tests nie)
  name="$1"; shift
  lockf -t 7200 "$lock" "$dir/run_block.sh" "$out/log-$name.txt" "$@"
}

for b in $want; do
  case "$b" in
    store)
      if [ -n "$store_bin" ]; then
        block store env OS_BENCH_PAGES="$store_pages" OS_BENCH_DIR="$work" OS_BENCH_OUT="$out/store.json" \
          "$store_bin" writer_1m_uniform_segments --ignored --nocapture --exact
      else
        block store env OS_BENCH_PAGES="$store_pages" OS_BENCH_DIR="$work" OS_BENCH_OUT="$out/store.json" \
          cargo test --release --locked -p os-store --test bench_writer writer_1m_uniform_segments -- --ignored --nocapture --exact
      fi
      ;;
    perf-001|perf-003)
      block "$b" sh -c "
        cd '$root'
        OUT_JSON='$out/$b-segments.json' '$dir/segments.sh' '$work' '$pages' 30 2 > '$out/$b-segments.txt' &
        seg=\$!
        '$loadtest' run --scenario $b --size large --pages $pages --work-dir '$work' --out '$out/$b.json' $EXTRA
        rc=\$?
        wait \$seg
        exit \$rc"
      ;;
    *) echo "unbekannter Block $b" >&2; exit 2 ;;
  esac
  rm -rf "$work"/os-loadtest-* 2>/dev/null
done
python3 "$dir/p32_summary.py" "$out"
