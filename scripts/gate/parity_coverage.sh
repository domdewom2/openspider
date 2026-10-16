#!/bin/sh
# M1-N26, Gegenprobe Schritt 2: os-parity run --all (Exit 0) und os-parity coverage --strict (0 open) aus der Testausgabe von Schritt 1.
# Aufruf: scripts/gate/parity_coverage.sh <ausgabe-ordner>   (enthaelt cargo-test.txt aus workspace_tests.sh)
set -u
out="${1:?Aufruf: parity_coverage.sh <ausgabe-ordner>}"
[ -f "$out/cargo-test.txt" ] || { echo "cargo-test.txt fehlt in $out (zuerst workspace_tests.sh)" >&2; exit 98; }
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 99
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$out/target-gate}" CARGO_INCREMENTAL=0
cargo build --release --locked -p os-cli -p os-parity > "$out/parity-build.txt" 2>&1 || { tail -n 20 "$out/parity-build.txt"; exit 97; }
bin="$CARGO_TARGET_DIR/release"
"$bin/os-parity" run --all --openspider "$bin/openspider" --out "$out/parity" > "$out/parity-run.txt" 2>&1
run_rc=$?
echo "os-parity run --all: Exit $run_rc"
tail -n 15 "$out/parity-run.txt"
# Status in eine Kopie schreiben, nicht in docs/plan/M1-status.json (das uebernimmt M1-N26 aus dem CI-Artefakt).
"$bin/os-parity" coverage --packages docs/plan/M1-packages.json --test-output "$out/cargo-test.txt" \
  --parity-report "$out/parity/report.json" --evidence tests/parity/evidence.toml \
  --write "$out/M1-status.local.json" --strict > "$out/coverage.txt" 2>&1
cov_rc=$?
echo "os-parity coverage --strict: Exit $cov_rc"
tail -n 15 "$out/coverage.txt"
[ "$run_rc" -eq 0 ] && [ "$cov_rc" -eq 0 ]
