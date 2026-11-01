#!/bin/bash
set -u
cd ~/Projects/openspider-wt/m2-n21
export PATH=$HOME/.cargo/bin:$PATH CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only OPENSPIDER_SECRET_STORE=memory
export OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome
O=$HOME/n21-claude-measure/checks; mkdir -p $O
r() { local n=$1; shift; local s=$(date +%s); timeout 3600s nice -n 10 "$@" > $O/$n.log 2>&1; echo "$n exit=$? secs=$(( $(date +%s)-s ))" >> $O/summary.txt; }
r fmt cargo fmt --all --check
r clippy cargo clippy --workspace --all-targets --locked -j 6 -- -D warnings
r crates cargo test --locked -j 6 -p os-app -p os-analyze -p os-store
~/ws-slot.sh
r workspace cargo test --workspace --locked -j 6
r deny cargo deny --locked check licenses
r neutral bash scripts/check-neutral.sh
r typecheck npm run -w apps/desktop typecheck
r desktop npm run -w apps/desktop test
r bins cargo build --locked -j 6 -p os-testsite -p os-cli -p os-parity --bins
r parity target/debug/os-parity run --all --openspider target/debug/openspider --out target/parity
echo CHECKS_DONE >> $O/summary.txt
