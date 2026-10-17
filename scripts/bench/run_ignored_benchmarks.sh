#!/bin/sh
# Aufrufhilfe fuer M1-N21 (B5/B6), M1-N28 nachgefuehrt: fuehrt jede Testdatei mit #[ignore = "Benchmark..."]-Tests
# einzeln in Release aus (vorher `cargo test --release --locked --no-run ...`, damit der Bau nicht in die Messung faellt),
# je Lauf scripts/bench/run_acceptance.sh (Load < 2, caffeinate, Lastproben). Kein Produktcode.
# Aufruf: scripts/bench/run_ignored_benchmarks.sh <ausgabeordner> [<arbeitsordner>]
# <arbeitsordner> (Standard: $TMPDIR) liegt ausserhalb des Repos; dort legen die N27-Benchmarks ihre Dateien an
# (OS_BENCH_DIR, OS_BENCH_MASTER) und werden danach geloescht.
# Listenzeile: <crate> <testdatei> [<namensfilter>|-] [VAR=wert ...]. Der Namensfilter waehlt einen Test der Datei
# (die Ausgabe heisst dann <crate>-<datei>-<filter>.txt), VAR=wert setzt Umgebungsvariablen fuer genau diesen Lauf.
out="$1"
work="${2:-${TMPDIR:-/tmp}}"
dir=$(dirname "$0")
rc=0
while read -r crate file filter envs; do
  name="$crate-$file"
  args=""
  if [ -n "$filter" ] && [ "$filter" != "-" ]; then name="$name-$filter"; args="$filter"; fi
  # shellcheck disable=SC2086
  "$dir/run_acceptance.sh" "$out/$name.txt" env OS_BENCH_DIR="$work" OS_BENCH_MASTER="$work/n28-links-master.ospider" $envs \
    cargo test --release --locked -p "$crate" --test "$file" -- --ignored --nocapture --test-threads=1 $args || rc=1
done <<'LIST'
os-engine lifecycle_pause
os-engine perf_store
os-engine lifecycle_list
os-store bench_writer
os-export pipeline_bench
os-export links_bench
os-export sitemap_gen
os-export xlsx_cells
os-fetch http_status
os-parse sitemap_gzip
os-parse pixel_width
os-testsite synthetic
os-analyze bench_duplicates - OS_BENCH_ROWS=1000000
os-store bench_links_build create_master OS_BENCH_PAGES=1000000
os-store bench_links_build build_variants OS_BENCH_VARIANTS=2:4096,4:4096
LIST
rm -f "$work/n28-links-master.ospider" "$work/n28-links-master.ospider.work"*
exit $rc
