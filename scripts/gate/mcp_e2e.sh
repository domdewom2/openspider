#!/bin/sh
# M1-N26, Gegenprobe Schritt 4: MCP-E2E ueber die Protokolltests von os-mcp (Binary ueber STDIO, SDK-Client):
# initialize, tools/list nur read, tools/call aller fuenf Werkzeuge, query_urls-Seitenvereinigung = Tab-Export ohne Dubletten.
# Aufruf: scripts/gate/mcp_e2e.sh <ausgabe-ordner>
set -u
out="${1:?Aufruf: mcp_e2e.sh <ausgabe-ordner>}"
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root" || exit 99
unset RUSTC_WRAPPER
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$out/target-gate}" CARGO_INCREMENTAL=0
mkdir -p "$out"
cargo test --locked -p os-mcp --test server_protocol --test server_process --test tools_read -- \
  standard_client_can_run_the_five_basic_requests rmcp_client_talks_to_the_binary_over_stdio \
  tools_list_without_switches_and_with_read_only_has_only_read_tools \
  internal_all_paged_with_limit_5_equals_the_tab_export_without_duplicates \
  paging_default_is_50_and_pages_union_to_the_full_list_without_duplicates \
  stdout_carries_only_json_rpc_and_logs_go_to_stderr > "$out/mcp-e2e.txt" 2>&1
rc=$?
grep -E '^test |^test result' "$out/mcp-e2e.txt"
ok=$(grep -c '^test .* ok$' "$out/mcp-e2e.txt")
echo "MCP-E2E: Exit $rc, $ok von 6 Tests ok"
[ "$rc" -eq 0 ] && [ "$ok" -eq 6 ]
