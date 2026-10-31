#!/usr/bin/env bash
# spec: OUT-263
set -euo pipefail
artifacts=$1
chromium=$2
output=$3
mkdir -p "$output"
output=$(cd "$output" && pwd)
mount=$(mktemp -d)
install=$(mktemp -d /Applications/openspider-smoke.XXXXXX)
mounted=0
cleanup() {
  local status=$?
  trap - EXIT
  if [[ $mounted == 1 ]]; then hdiutil detach "$mount" || status=1; fi
  rm -rf "$install"
  if [[ -d "$mount" ]]; then rmdir "$mount" || status=1; fi
  exit "$status"
}
trap cleanup EXIT
shopt -s nullglob
dmgs=("$artifacts"/*.dmg)
[[ ${#dmgs[@]} == 1 ]]
hdiutil attach "${dmgs[0]}" -nobrowse -readonly -mountpoint "$mount"
mounted=1
apps=("$mount"/*.app)
[[ ${#apps[@]} == 1 ]]
ditto "${apps[0]}" "$install/$(basename "${apps[0]}")"
hdiutil detach "$mount"
mounted=0
python3 scripts/smoke/installed.py --install-dir "$install" --testsite target/debug/os-testsite \
  --chromium "$chromium" --output "$output/dmg"
