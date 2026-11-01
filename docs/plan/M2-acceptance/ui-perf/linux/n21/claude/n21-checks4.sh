#!/bin/bash
set -u
cd ~/Projects/openspider-wt/m2-n21
export PATH=$HOME/.cargo/bin:$PATH CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only OPENSPIDER_SECRET_STORE=memory
export OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome
touch .claude-active
O=$HOME/n21-checks4; rm -rf $O; mkdir -p $O
r() { local n=$1; shift; local s=$(date +%s); timeout 5400s nice -n 10 "$@" > $O/$n.log 2>&1; echo "$n exit=$? secs=$(( $(date +%s)-s ))" >> $O/summary.txt; }
r fmt cargo fmt --all --check
r clippy cargo clippy --workspace --all-targets --locked -j 6 -- -D warnings
~/ws-slot.sh
r workspace cargo test --workspace --locked -j 6
r deny cargo deny --locked check licenses
r neutral bash scripts/check-neutral.sh
r typecheck npm run -w apps/desktop typecheck
r desktop npm run -w apps/desktop test
r bins cargo build --locked -j 6 -p os-testsite -p os-cli -p os-parity --bins
r bridge cargo build --locked -j 6 -p os-app --features e2e-bridge --bin os-app-bridge
r parity target/debug/os-parity run --all --openspider target/debug/openspider --out target/parity
cd apps/desktop
r e2e-beta-a11y npx playwright test -c n21-local.playwright.config.ts e2e/beta e2e/a11y e2e/sidebar --project=chromium --project=webkit --workers=2

r visual-after npx playwright test -c e2e/visual/playwright.config.ts --workers=2
echo CHECKS_DONE >> $O/summary.txt
