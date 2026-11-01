#!/bin/bash
set -u
cd ~/Projects/openspider-wt/m2-n21
export PATH=$HOME/.cargo/bin:$PATH CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only OPENSPIDER_SECRET_STORE=memory
export OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome
touch .claude-active
O=$HOME/n21-checks6; rm -rf $O; mkdir -p $O
r() { local n=$1; shift; local s=$(date +%s); timeout 5400s nice -n 10 "$@" > $O/$n.log 2>&1; echo "$n exit=$? secs=$(( $(date +%s)-s ))" >> $O/summary.txt; }
r fmt cargo fmt --all --check
r clippy cargo clippy --workspace --all-targets --locked -j 6 -- -D warnings
r crates cargo test --locked -j 6 -p os-app -p os-analyze -p os-store
~/ws-slot.sh
r workspace cargo test --workspace --locked -j 6
r typecheck npm run -w apps/desktop typecheck
r vitest npm run -w apps/desktop test
r bridge cargo build --locked -j 6 -p os-app --features e2e-bridge --bin os-app-bridge
cd apps/desktop
cat > n21-local.playwright.config.ts <<'CFG'
// Server-only wrapper: own Vite port so a foreign worktree dev server on 1420 is not reused.
import base from "./playwright.config";
const port = 1461;
export default { ...base, use: { ...base.use, baseURL: `http://localhost:${port}` }, webServer: { ...base.webServer, command: `npx vite --port ${port} --strictPort`, url: `http://localhost:${port}`, reuseExistingServer: false } };
CFG
r e2e-sidebar npx playwright test -c n21-local.playwright.config.ts e2e/sidebar e2e/sidebar-fach --project=chromium --project=webkit --workers=2
rm -f n21-local.playwright.config.ts
echo CHECKS_DONE >> $O/summary.txt
