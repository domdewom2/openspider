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
mkdir -p target/n15/reference/tmp
export TMPDIR="$PWD/target/n15/reference/tmp"
{
  date -u
  git rev-parse HEAD
  sw_vers
  system_profiler SPHardwareDataType
  rustc -V
  df -h .
} > target/n15/reference/hardware.txt
cargo build --release -j 6 --locked -p os-loadtest
failures=0
for scenario in perf-001 perf-003; do
  while true; do
    uptime | tee -a target/n15/reference/load.log
    if python3 -c 'import os,sys; sys.exit(os.getloadavg()[0] >= 1.5)'; then break; fi
    sleep 15
  done
  if ! target/release/os-loadtest run --scenario "$scenario" --size large \
    --threads 8 --profile balanced --out "target/n15/reference/$scenario.json" \
    > "target/n15/reference/$scenario.log" 2>&1; then
    failures=$((failures + 1))
  fi
  cat "target/n15/reference/$scenario.log"
done
echo "$failures failed scenarios; reports: target/n15/reference/"
[[ $failures == 0 ]]
