#!/bin/sh
# M1-N26, Gegenprobe Schritt 1: cargo test --workspace --exclude openspider-desktop --locked in frischem Target ohne RUSTC_WRAPPER.
# Aufruf: scripts/gate/workspace_tests.sh <ausgabe-ordner>   (Ordner ausserhalb des Repos)
set -u
out="${1:?Aufruf: workspace_tests.sh <ausgabe-ordner>}"
mkdir -p "$out"
target="$out/target-gate"
rm -rf "$target"
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="$target" CARGO_INCREMENTAL=0
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 99
{ echo "Start $(date -u +%FT%TZ) Commit $(git rev-parse --short HEAD)"; uptime; } > "$out/cargo-test.start"
cargo test --workspace --exclude openspider-desktop --no-fail-fast --locked > "$out/cargo-test.txt" 2>&1
rc=$?
{ echo "Ende $(date -u +%FT%TZ), Exit $rc"; } >> "$out/cargo-test.start"
tail -n 5 "$out/cargo-test.txt"
grep -E '^test result:' "$out/cargo-test.txt" | awk '{p+=$4; f+=$6; i+=$8} END {print "Summe: " p " passed, " f " failed, " i " ignored"}'
exit $rc
