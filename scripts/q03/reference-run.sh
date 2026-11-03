#!/usr/bin/env bash
# Stage 2 only: run on the 16-GB Apple Silicon reference Mac.
set -euo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
jobs=$(sysctl -n hw.ncpu 2>/dev/null || echo 8)
export CARGO_BUILD_JOBS=$jobs
export OPENSPIDER_SECRET_STORE=memory
if [[ $(uname -s) != Darwin ]]; then
  echo 'Stage 2 requires the reference Mac; this script must not run on the server.' >&2
  exit 2
fi
if [[ ${Q03_CAFFEINATED:-0} != 1 ]]; then
  exec caffeinate -dims env Q03_CAFFEINATED=1 bash "$PWD/scripts/q03/reference-run.sh" "$@"
fi
mkdir -p target/loadtest/tmp
export TMPDIR="$PWD/target/loadtest/tmp"
cargo build --release --locked -j "$jobs" -p os-loadtest -p os-cli -p os-testsite
cargo build --release --locked -j "$jobs" -p os-engine --example crawl_child
cargo test --release --locked -j "$jobs" -p os-analyze --test near_dup_scale --test link_score_benchmarks --test link_score_m2 --test near_dup_d02 \
  -p os-export --test pipeline_perf -p os-render --test render_chromium \
  -p os-engine --test render_integration --no-run
# Each block checks the 1-minute load immediately before starting. No cgroups on macOS.
# Gate wie bei den N15F/N20-Referenzläufen (Last < 4); überschreibbar mit Q03_MAX_LOAD.
threshold=${Q03_MAX_LOAD:-4}
quiet() {
  while ! python3 -c 'import os,sys; sys.exit(0 if os.getloadavg()[0] < float(sys.argv[1]) else 1)' "$threshold"; do
    echo "Waiting for load < $threshold (now $(python3 -c 'import os; print(round(os.getloadavg()[0], 2))')) ..."
    sleep 15
  done
}
run() {
  quiet
  python3 scripts/q03/measure.py "$@" || failures=$((failures+1))
}
failures=0
{
  date -u
  git rev-parse HEAD
  sw_vers
  system_profiler SPHardwareDataType
  df -h .
  rustc -V
} > target/loadtest/reference-hardware.txt
for profile in balanced min-memory max-speed; do
  run "perf001-$profile" -- target/release/os-loadtest run --scenario perf-001 --size large \
    --profile "$profile" --out "target/loadtest/perf001-$profile-report.json"
done
export OS_LOADTEST_MEMORY_LOG="$PWD/target/loadtest/perf003-memory.jsonl"
run perf003 -- target/release/os-loadtest run --scenario perf-003 --size large --out target/loadtest/perf003-report.json
unset OS_LOADTEST_MEMORY_LOG
run perf005 -- target/release/os-loadtest run --scenario perf-005 --size large --out target/loadtest/perf005-report.json
# Non-ignored tests deliberately run without --ignored; otherwise these checks execute zero tests.
run perf006-recycle -- python3 scripts/q03/test-bin.py os-render/render_chromium requires_chromium_recycles_and_recovers_after_hanging_js --nocapture --test-threads=1
run perf006-tabs -- python3 scripts/q03/test-bin.py os-render/render_chromium requires_chromium_limits_tabs_and_recovers_from_cancellation --nocapture --test-threads=1
run perf006-engine -- python3 scripts/q03/test-bin.py render_integration --nocapture --test-threads=1
run perf006-real-engine -- python3 scripts/q03/test-bin.py os-engine/render_chromium headless_engine_reuses_response_and_stores_browser_facts --nocapture --test-threads=1
run perf007-cancel-links -- python3 scripts/q03/test-bin.py link_score_m2 cancellation_during_iterations_keeps_the_previous_complete_result --nocapture --test-threads=1
run perf007-cancel-near -- python3 scripts/q03/test-bin.py near_dup_d02 analysis_is_deterministic_excludes_exact_pairs_and_can_cancel --nocapture --test-threads=1
run perf007-near -- python3 scripts/q03/test-bin.py near_dup_scale --ignored --nocapture --test-threads=1
run perf007-links -- python3 scripts/q03/test-bin.py link_score_benchmarks --ignored --nocapture --test-threads=1
run perf008 -- python3 scripts/q03/test-bin.py pipeline_perf --ignored --nocapture --test-threads=1
quiet
python3 scripts/q03/cli-run.py || failures=$((failures+1))
python3 scripts/q03/summarize.py
printf '%s\n' "$failures failed blocks; see target/loadtest/*.json and *.log"
[[ $failures == 0 ]]
