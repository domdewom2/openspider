#!/bin/sh
# Aufrufhilfe fuer M1-N21 (B5/B6): fuehrt jede Testdatei mit #[ignore = "Benchmark..."]-Tests einzeln in Release aus
# (vorher `cargo test --release --locked --no-run ...`, damit der Bau nicht in die Messung faellt), je Lauf
# scripts/bench/run_acceptance.sh (Load < 2, caffeinate, Lastproben). Kein Produktcode.
# Aufruf: scripts/bench/run_ignored_benchmarks.sh <ausgabeordner>
out="$1"
dir=$(dirname "$0")
rc=0
while read -r crate file; do
  "$dir/run_acceptance.sh" "$out/$crate-$file.txt" cargo test --release --locked -p "$crate" --test "$file" -- --ignored --nocapture --test-threads=1 || rc=1
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
LIST
exit $rc
