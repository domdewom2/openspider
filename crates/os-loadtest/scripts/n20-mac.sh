#!/usr/bin/env bash
# PERF-003 macOS follow-up; also runnable on other Unix platforms. No sudo.
set -euo pipefail
if [[ $# != 1 ]]; then
  echo "Usage: $0 OUTPUT_DIRECTORY" >&2
  exit 2
fi
mkdir -p "$1"
output=$(cd "$1" && pwd)
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_INCREMENTAL=0 OPENSPIDER_SECRET_STORE=memory
export N20_MAX_LOAD=${N20_MAX_LOAD:-2.5}
python3 - "$N20_MAX_LOAD" <<'PY'
import math, sys
value = float(sys.argv[1])
if not math.isfinite(value) or value <= 0:
    sys.exit('N20_MAX_LOAD must be positive and finite')
PY
{
  date -u
  git rev-parse HEAD
  uname -a
  rustc -V
  echo "N20_MAX_LOAD=$N20_MAX_LOAD; threads=8; profile=balanced"
} > "$output/hardware.txt"
cargo build --release -p os-loadtest -j 8 --locked > "$output/build.log" 2>&1
failures=0
scratch=$(mktemp -d "$PWD/target/n20-mac.XXXXXXXX")
trap 'rm -rf "$scratch"' EXIT
run_scenario() {
  local scenario=$1 size=$2 label=$3
  while true; do
    date -u >> "$output/load.log"
    uptime >> "$output/load.log"
    if python3 -c 'import os,sys; sys.exit(os.getloadavg()[0] >= float(os.environ["N20_MAX_LOAD"]))'; then break; fi
    sleep 15
  done
  if ! target/release/os-loadtest run --scenario "$scenario" --size "$size" \
    --threads 8 --profile balanced --work-dir "$scratch" \
    --out "$output/$label.json" > "$output/$label.log" 2>&1; then
    failures=$((failures + 1))
  fi
  cat "$output/$label.log"
}
run_scenario perf-003 large perf-003-large-1
run_scenario perf-003 large perf-003-large-2
run_scenario perf-001 large perf-001-large-1
for run in 1 2 3; do
  run_scenario perf-001 small "perf-001-small-$run"
done
echo "$failures failed scenarios; reports: $output"
[[ $failures == 0 ]]
