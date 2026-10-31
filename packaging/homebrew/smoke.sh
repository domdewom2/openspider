#!/usr/bin/env bash
set -euo pipefail
# Inputs are downloaded artifacts, never a public release or an external tap.
: "${PACKAGE_VERSION:?}" "${PREVIOUS_VERSION:?}" "${GITHUB_REPOSITORY:?}"
test "$PACKAGE_VERSION" != "$PREVIOUS_VERSION"
node --input-type=module -e '
const parse = v => { const m=/^(\d+)\.(\d+)\.(\d+)(?:-(beta|rc)\.(\d+))?$/.exec(v); if(!m)throw Error("Invalid version"); return [...m.slice(1,4).map(Number), m[4] === "beta" ? 0 : m[4] === "rc" ? 1 : 2, Number(m[5] || 0)]; };
const a=parse(process.env.PREVIOUS_VERSION), b=parse(process.env.PACKAGE_VERSION);
const i=a.findIndex((v,i)=>v!==b[i]); if(i<0 || a[i]>=b[i])throw Error("Previous build must be older");'
scratch=$(mktemp -d)
server=''
installed=false
label="de.openspider.task.p01-smoke"
domain="gui/$(id -u)"
plist="$HOME/Library/LaunchAgents/$label.plist"
sentinel="$HOME/Library/Application Support/OpenSpider/p01-retain-${GITHUB_RUN_ID:-local}.txt"
cleanup() {
  if $installed; then brew uninstall --cask p01/local/openspider || true; fi
  launchctl bootout "$domain/$label" 2>/dev/null || true
  rm -f "$plist"
  rm -f "$sentinel"
  if [[ -n "$server" ]]; then kill "$server" 2>/dev/null || true; fi
  brew untap p01/local 2>/dev/null || true
  rm -rf "$scratch"
}
trap cleanup EXIT
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_FROM_API=1
brew tap-new --no-git p01/local
tap=$(brew --repository p01/local)
mkdir -p "$tap/Casks"
start_server() {
  rm -f "$scratch/url"
  node packaging/homebrew/serve.mjs "$1" "$scratch/url" &
  server=$!
  for ((n=0; n<100; n++)); do [[ -s "$scratch/url" ]] && return; sleep 0.1; done
  return 1
}
render() {
  node packaging/homebrew/generate.mjs --directory "$1" --output "$scratch/$2" \
    --version "$3" --repository "$GITHUB_REPOSITORY" --mode homebrew --base-url "$(cat "$scratch/url")"
  ruby -c "$scratch/$2/openspider.rb"
  cp "$scratch/$2/openspider.rb" "$tap/Casks/openspider.rb"
}
start_server previous
render previous old "$PREVIOUS_VERSION"
brew install --cask p01/local/openspider
installed=true
mkdir -p "$(dirname "$sentinel")"
test ! -e "$sentinel"
printf 'retain\n' > "$sentinel"
cli="$(brew --prefix)/bin/openspider"
test -L "$cli"
"$cli" --version | tee "$scratch/old-version"
node -e 'if(!require("fs").readFileSync(process.argv[1],"utf8").trim().split(/\s+/).includes(process.argv[2]))process.exit(1)' "$scratch/old-version" "$PREVIOUS_VERSION"
mkdir -p "$(dirname "$plist")"
# Harmless task with the same stable CLI path as an actual registration.
cat > "$plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>$label</string>
<key>ProgramArguments</key><array><string>/Applications/OpenSpider.app/Contents/MacOS/openspider</string><string>--version</string></array>
<key>StartInterval</key><integer>86400</integer></dict></plist>
EOF
launchctl bootstrap "$domain" "$plist" || { domain="user/$(id -u)"; launchctl bootstrap "$domain" "$plist"; }
launchctl print "$domain/$label"
kill "$server"
wait "$server" || true
start_server incoming
render incoming new "$PACKAGE_VERSION"
# auto_updates true requires the explicit --greedy flag for an actual brew upgrade.
brew outdated --cask --greedy --json=v2 p01/local/openspider > "$scratch/outdated.json"
node -e 'const d=JSON.parse(require("fs").readFileSync(process.argv[1])); if(d.casks.length!==1)process.exit(1)' "$scratch/outdated.json"
brew upgrade --cask --greedy p01/local/openspider
"$cli" --version | tee "$scratch/new-version"
node -e 'if(!require("fs").readFileSync(process.argv[1],"utf8").trim().split(/\s+/).includes(process.argv[2]))process.exit(1)' "$scratch/new-version" "$PACKAGE_VERSION"
# OUT-166: losing the registration on upgrade must fail, never repair it in the test.
test -x /Applications/OpenSpider.app/Contents/MacOS/openspider
registration_lost=false
if [[ ! -e "$plist" ]] || ! launchctl print "$domain/$label"; then
  echo '::error::OUT-166: brew upgrade removed the scheduled registration (K3-09).'
  registration_lost=true
fi
brew uninstall --cask p01/local/openspider
installed=false
test ! -e "$cli"
test ! -L "$cli"
test ! -e /Applications/OpenSpider.app
test ! -e "$plist"
if launchctl print "$domain/$label" >/dev/null 2>&1; then exit 1; fi
test "$(cat "$sentinel")" = retain
echo 'ok: brew install, real upgrade, CLI link, uninstall and LaunchAgent removal' >> "$GITHUB_STEP_SUMMARY"
if $registration_lost; then
  echo 'OUT-166 FAIL: registration lost during upgrade; uninstall checks completed.' >> "$GITHUB_STEP_SUMMARY"
  exit 1
fi
