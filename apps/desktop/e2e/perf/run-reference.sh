#!/usr/bin/env bash
# Run from an active macOS GUI session. Default: complete reference-A dataset + 30 minute soak.
set -euo pipefail
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$script_dir/../../../.."
[[ "$(uname -s)" == Darwin ]] || { echo "Reference A requires macOS" >&2; exit 1; }
export OS_PERF_LOAD_LIMIT="${OS_PERF_LOAD_LIMIT:-2.5}"
export OS_PERF_ROWS=5000000
export OS_PERF_FIXTURE="${OS_PERF_FIXTURE:-$PWD/target/ui-perf-reference.ospider}"
export OS_PERF_SOAK_SECONDS=1800
export OS_PERF_SOAK_ENGINE=webkit OS_E2E_PROJECTS=webkit OS_PERF_REFERENCE=1 OPENSPIDER_SECRET_STORE=memory
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
if [[ "${1:-}" != --awake ]]; then exec caffeinate -dimsu "$script_dir/run-reference.sh" --awake; fi
quiet() {
  python3 - "$OS_PERF_LOAD_LIMIT" <<'PYQUIET'
import os,sys
sys.exit(0 if os.getloadavg()[0] < float(sys.argv[1]) else 1)
PYQUIET
}
python3 - "$OS_PERF_LOAD_LIMIT" <<'PYLIMIT'
import math,sys
limit=float(sys.argv[1])
assert math.isfinite(limit) and limit > 0, "OS_PERF_LOAD_LIMIT must be finite and positive"
PYLIMIT
while ! quiet; do echo "Waiting for load < $OS_PERF_LOAD_LIMIT"; sleep 15; done
mkdir -p target/loadtest
system_profiler SPHardwareDataType SPSoftwareDataType > target/loadtest/reference-hardware.txt
sw_vers >> target/loadtest/reference-hardware.txt
node --version >> target/loadtest/reference-hardware.txt
rustc --version >> target/loadtest/reference-hardware.txt
npm ci
cargo build --locked -j 6 -p os-testsite -p os-cli -p os-parity --bins
cargo build --release --locked -j 6 -p os-app --bin os-app-bridge --features e2e-bridge
cargo build --release --locked -j 6 -p os-loadtest --bin gen-ui-large
export OS_APP_BRIDGE_BIN="$PWD/target/release/os-app-bridge"
npm exec -w apps/desktop -- playwright install webkit
node apps/desktop/e2e/perf/run.mjs --prepare-only
node apps/desktop/e2e-b/perf/prepare.mjs
npm exec -w apps/desktop -- tauri build --no-bundle --features e2e-webdriver --config ../../target/ui-perf-native.conf.json
while ! quiet; do echo "Waiting after build for load < $OS_PERF_LOAD_LIMIT"; sleep 15; done
set +e
node apps/desktop/e2e/perf/run.mjs --measure-only
browser_status=$?
while ! quiet; do echo "Waiting before native measurement for load < $OS_PERF_LOAD_LIMIT"; sleep 15; done
node apps/desktop/e2e-b/perf/run.mjs target/release/openspider-desktop
native_status=$?
OS_PERF_REQUIRE_NATIVE=1 node apps/desktop/e2e/perf/report.mjs
report_status=$?
(( browser_status == 0 && native_status == 0 && report_status == 0 ))
