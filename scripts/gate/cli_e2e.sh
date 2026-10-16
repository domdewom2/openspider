#!/bin/sh
# M1-N26, Gegenprobe Schritt 3: CLI-E2E mit dem Zielbild-Befehl aus docs/plan/M1.md 1.2 an der Fixture-Site out-basic-site.
# Erwartet: Exit 0, .ospider-Datei vorhanden, jeder Export mit derselben Zeilenzahl wie das Golden unter tests/golden/out-basic-site.
# Abweichung vom Wortlaut in M1.md 1.2: http statt https und --resolve '*.test=...' wie in den CLI-Integrationstests (die Test-CA des
# Testservers steht nicht im Systemspeicher; mit https crawlt der Lauf 0 Seiten und wuerde mit leeren Exporten truegerisch gruen).
# Aufruf: scripts/gate/cli_e2e.sh <ausgabe-ordner>
set -u
out="${1:?Aufruf: cli_e2e.sh <ausgabe-ordner>}"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 99
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$out/target-gate}" CARGO_INCREMENTAL=0
mkdir -p "$out/cli-e2e"; rm -rf "$out/cli-e2e/out" "$out/cli-e2e/home"; mkdir -p "$out/cli-e2e/out" "$out/cli-e2e/home"
cargo build --release --locked -p os-cli -p os-testsite > "$out/cli-e2e/build.txt" 2>&1 || { tail -n 20 "$out/cli-e2e/build.txt"; exit 97; }
bin="$CARGO_TARGET_DIR/release"
"$bin/os-testsite" out-basic-site > "$out/cli-e2e/site.txt" 2>&1 &
site=$!
trap 'kill $site 2>/dev/null' EXIT
i=0; port=""
while [ -z "$port" ] && [ "$i" -lt 50 ]; do
  port=$(sed -n 's/.*127\.0\.0\.1:\([0-9][0-9]*\).*/\1/p;s/.*localhost:\([0-9][0-9]*\).*/\1/p' "$out/cli-e2e/site.txt" | head -1)
  [ -z "$port" ] && { sleep 0.2; i=$((i + 1)); }
done
[ -n "$port" ] || { echo "Testsite meldet keinen Port:"; cat "$out/cli-e2e/site.txt"; exit 96; }
OPENSPIDER_HOME="$out/cli-e2e/home" "$bin/openspider" --crawl http://site.test/ --resolve "*.test=127.0.0.1:$port" \
  --export-tabs "Internal:All,Response Codes:Client Error (4xx)" --bulk-export "Links:All Inlinks" \
  --save-report "Crawl Overview,Redirects:Redirect Chains" --save-crawl --output-folder "$out/cli-e2e/out" \
  > "$out/cli-e2e/run.txt" 2>&1
rc=$?
echo "openspider: Exit $rc"
missing=0
for f in internal_all.csv response_codes_client_error_4xx.csv links_all_inlinks.csv crawl_overview.csv redirect_chains.csv; do
  got=$(wc -l < "$out/cli-e2e/out/$f" 2>/dev/null | tr -d ' ')
  want=$(wc -l < "tests/golden/out-basic-site/$f" | tr -d ' ')
  if [ -f "$out/cli-e2e/out/$f" ] && [ "${got:-x}" = "$want" ]; then echo "vorhanden: $f ($got Zeilen = Golden)"; else echo "ABWEICHUNG: $f (${got:-fehlt} Zeilen, Golden $want)"; missing=1; fi
done
ls "$out/cli-e2e/out"/*.ospider > /dev/null 2>&1 && echo "vorhanden: .ospider" || { echo "FEHLT: .ospider"; missing=1; }
[ "$rc" -eq 0 ] && [ "$missing" -eq 0 ]
