#!/usr/bin/env bash
# PERF-001/003 only; run on the 16-GB Apple Silicon reference Mac.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../../.."
if [[ $(uname -s) != Darwin ]]; then
  echo 'Run this script on the reference Mac.' >&2
  exit 2
fi
if [[ ${N15_CAFFEINATED:-0} != 1 ]]; then
  exec caffeinate -dims env N15_CAFFEINATED=1 bash "$PWD/crates/os-loadtest/scripts/n15-reference.sh"
fi
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_INCREMENTAL=0
export OPENSPIDER_SECRET_STORE=memory
# Default reference gate; measured Mac mini M1 (16 GB): N15_MAX_LOAD=2.5.
export N15_MAX_LOAD=${N15_MAX_LOAD:-1.5}
N15_SMALL_RUNS=${N15_SMALL_RUNS:-5}
python3 - "$N15_MAX_LOAD" "$N15_SMALL_RUNS" <<'PYVALIDATE'
import math, sys
load, runs = float(sys.argv[1]), int(sys.argv[2])
if not math.isfinite(load) or load <= 0 or runs < 5:
    sys.exit("N15_MAX_LOAD must be positive; N15_SMALL_RUNS must be >= 5")
PYVALIDATE
mkdir -p target/n15/reference/tmp
export TMPDIR="$PWD/target/n15/reference/tmp"
{
  date -u
  git rev-parse HEAD
  sw_vers
  system_profiler SPHardwareDataType
  rustc -V
  df -h .
  echo "Load gate: $N15_MAX_LOAD; PERF-001 small repetitions: $N15_SMALL_RUNS"
  python3 - <<'PYENV'
import os
keys = sorted(k for k in os.environ if k.startswith(("MALLOC_", "MIMALLOC_", "OS_STORE_"))
              or k in ("GLIBC_TUNABLES", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES"))
print("Allocator/store environment:", {k: os.environ[k] for k in keys})
PYENV
} > target/n15/reference/hardware.txt
cargo build --release -j 6 --locked -p os-loadtest
failures=0
run_scenario() {
  local scenario=$1 size=$2 label=$3
  while true; do
    date -u >> target/n15/reference/load.log
    uptime | tee -a target/n15/reference/load.log
    if python3 -c 'import os,sys; sys.exit(os.getloadavg()[0] >= float(os.environ["N15_MAX_LOAD"]))'; then break; fi
    sleep 15
  done
  if ! target/release/os-loadtest run --scenario "$scenario" --size "$size" \
    --threads 8 --profile balanced --out "target/n15/reference/$label.json" \
    > "target/n15/reference/$label.log" 2>&1; then
    failures=$((failures + 1))
  fi
  cat "target/n15/reference/$label.log"
}
for ((run = 1; run <= N15_SMALL_RUNS; run++)); do
  run_scenario perf-001 small "perf-001-small-$run"
done
for scenario in perf-001 perf-003; do
  run_scenario "$scenario" large "$scenario"
done
echo "$failures failed scenarios; reports: target/n15/reference/"
[[ $failures == 0 ]]
