#!/usr/bin/env bash
# M3-R01: static bundle checks; --plist-only also runs against fixtures on Linux.
set -euo pipefail
trap 'echo "Bundle-Prüfung fehlgeschlagen (Zeile $LINENO)." >&2' ERR
mode="${1:-}"
if [[ "$mode" == --plist-only || "$mode" == --smoke ]]; then shift; fi
[[ $# -eq 1 ]] || { echo "Usage: $0 [--plist-only|--smoke] OpenSpider.app" >&2; exit 2; }
app="${1%/}"
python3 - "$app" <<'PY'
import pathlib, plistlib, sys
app = pathlib.Path(sys.argv[1])
with (app / 'Contents/Info.plist').open('rb') as f:
    info = plistlib.load(f)
assert info['CFBundleIdentifier'] == 'org.openspider.desktop', 'Bundle-ID'
assert info['LSMinimumSystemVersion'] == '13.0', 'Minimum macOS version'
assert (app / 'Contents/MacOS' / info['CFBundleExecutable']).is_file(), 'App executable missing'
assert (app / 'Contents/MacOS/openspider').is_file(), 'CLI missing'
# // spec: OUT-272 [M3]
types = info.get('CFBundleDocumentTypes', [])
for ext in ('ospider', 'osconfig', 'osauth', 'ospreset', 'ostask'):
    matches = [t for t in types if ext in t.get('CFBundleTypeExtensions', [])]
    assert len(matches) == 1, f'File association missing/duplicate: {ext}'
    assert matches[0].get('CFBundleTypeRole') == 'Editor', f'File role: {ext}'
icon = info.get('CFBundleIconFile', '')
assert icon and (app / 'Contents/Resources' / icon).is_file(), 'App icon missing'
print('ok: OUT-272 [M3] file associations and app icon; OUT-260 minimum OS')
PY
[[ "$mode" == --plist-only ]] && exit 0
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS required' >&2; exit 2; }
count=0
while IFS= read -r -d '' binary; do
  # Inspect file contents rather than executable bits or suffixes (includes dylibs/helpers).
  if ! file -b "$binary" | grep -q 'Mach-O'; then continue; fi
  count=$((count + 1))
  codesign --verify --strict --all-architectures "$binary"
  archs="$(lipo -archs "$binary")"
  case "${MACOS_EXPECTED_ARCH:-}" in
    universal) [[ " $(echo "$archs" | tr ' ' '\n' | sort | tr '\n' ' ')" == ' arm64 x86_64 ' ]] ;;
    aarch64) [[ "$archs" == arm64 ]] ;;
    x86_64) [[ "$archs" == x86_64 ]] ;;
  esac
  # Every slice must carry runtime, a secure timestamp and no entitlement exceptions.
  for slice in $archs; do
    info="$(codesign -dvv --arch "$slice" "$binary" 2>&1)"
    grep -Eq 'flags=0x[0-9a-f]+\([^)]*runtime' <<< "$info"
    grep -Eq '^Timestamp=.+' <<< "$info"
    ! grep -q 'Signature=adhoc' <<< "$info"
    entitlements="$(mktemp)"
    codesign -d --arch "$slice" --entitlements :- "$binary" > "$entitlements" 2>/dev/null
    python3 - "$entitlements" <<'PY'
import pathlib, plistlib, sys
raw = pathlib.Path(sys.argv[1]).read_bytes()
assert not raw or plistlib.loads(raw) == {}, 'Unexpected entitlements'
PY
    rm -f "$entitlements"
  done
  echo "ok: OUT-265 signed runtime/timestamp/minimal entitlements: $binary ($archs)"
done < <(find "$app" -type f -print0)
[[ $count -ge 2 ]] || { echo 'App and CLI Mach-O files required' >&2; exit 1; }
codesign --verify --deep --strict --all-architectures "$app"
if [[ "$mode" == --smoke ]]; then
  # Intel has no proc_translated key. Any other sysctl failure must not pass silently.
  translated="$(sysctl -in sysctl.proc_translated 2>/dev/null || true)"
  [[ "$translated" == 0 || ( -z "$translated" && "$(uname -m)" == x86_64 ) ]]
  echo "ok: OUT-260 sysctl.proc_translated=0 (Intel: key absent = native)"
  "$app/Contents/MacOS/openspider" --version
  python3 - "$app" <<'PY'
import pathlib, plistlib, subprocess, sys, time
app = pathlib.Path(sys.argv[1]).resolve()
with (app / 'Contents/Info.plist').open('rb') as f:
    executable = plistlib.load(f)['CFBundleExecutable']
process = subprocess.Popen([str(app / 'Contents/MacOS' / executable)])
try:
    time.sleep(10)
    assert process.poll() is None, 'App exited during native launch smoke'
    print('ok: OUT-260/OUT-261 native app launch')
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
PY
fi
