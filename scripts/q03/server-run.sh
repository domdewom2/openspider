#!/usr/bin/env bash
# Invoke this whole script inside the mandatory 16-GB systemd scope.
set -uo pipefail
cd "$(dirname "$0")/../.."
export PATH="$HOME/.cargo/bin:$PATH"
export OPENSPIDER_SECRET_STORE=memory
export CARGO_BUILD_JOBS=12
mkdir -p target/loadtest/tmp
export TMPDIR="$PWD/target/loadtest/tmp"
# Refuse accidental unconstrained measurement on Linux/cgroup v2.
python3 - <<'PY'
from pathlib import Path
path = Path('/sys/fs/cgroup') / Path('/proc/self/cgroup').read_text().strip().split('::')[1].lstrip('/')
assert (path/'memory.max').read_text().strip() == str(16*1024**3), 'MemoryMax=16G required'
assert (path/'memory.swap.max').read_text().strip() == '0', 'MemorySwapMax=0 required'
PY
[[ $? == 0 ]] || exit 2
failures=0
start_at=${Q03_FROM:-perf001}
case "$start_at" in
  perf001|perf003|perf005|perf006-recycle|perf006-tabs|perf006-engine|perf007-cancel-links|perf007-cancel-near|perf007-near|perf007-links|perf008|cli-5m) ;;
  *) echo "Unknown Q03_FROM=$start_at" >&2; exit 2 ;;
esac
started=false
should_run() {
  [[ $started == true ]] && return 0
  [[ $1 == "$start_at" ]] || return 1
  started=true
}
run() {
  should_run "$1" || return 0
  python3 scripts/q03/measure.py "$@" || failures=$((failures+1))
}
run perf001 -- target/release/os-loadtest run --scenario perf-001 --size large --out target/loadtest/perf001-report.json
export OS_LOADTEST_MEMORY_LOG="$PWD/target/loadtest/perf003-memory.jsonl"
run perf003 -- target/release/os-loadtest run --scenario perf-003 --size large --out target/loadtest/perf003-report.json
unset OS_LOADTEST_MEMORY_LOG
run perf005 -- target/release/os-loadtest run --scenario perf-005 --size large --out target/loadtest/perf005-report.json
export OPENSPIDER_CHROMIUM="${OPENSPIDER_CHROMIUM:-$HOME/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome}"
run perf006-recycle -- python3 scripts/q03/test-bin.py os-render/render_chromium requires_chromium_recycles_and_recovers_after_hanging_js --nocapture --test-threads=1
run perf006-tabs -- python3 scripts/q03/test-bin.py os-render/render_chromium requires_chromium_limits_tabs_and_recovers_from_cancellation --nocapture --test-threads=1
run perf006-engine -- python3 scripts/q03/test-bin.py render_integration --nocapture --test-threads=1
run perf007-cancel-links -- python3 scripts/q03/test-bin.py link_score_m2 cancellation_during_iterations_keeps_the_previous_complete_result --nocapture --test-threads=1
run perf007-cancel-near -- python3 scripts/q03/test-bin.py near_dup_d02 analysis_is_deterministic_excludes_exact_pairs_and_can_cancel --nocapture --test-threads=1
run perf007-near -- python3 scripts/q03/test-bin.py near_dup_scale --ignored --nocapture --test-threads=1
run perf007-links -- python3 scripts/q03/test-bin.py link_score_benchmarks --ignored --nocapture --test-threads=1
run perf008 -- python3 scripts/q03/test-bin.py pipeline_perf --ignored --nocapture --test-threads=1
if should_run cli-5m; then
  python3 scripts/q03/cli-run.py || failures=$((failures+1))
fi
python3 scripts/q03/summarize.py
printf '%s\n' "$failures failed blocks; see target/loadtest/*.json and *.log"
[[ $failures == 0 ]]
