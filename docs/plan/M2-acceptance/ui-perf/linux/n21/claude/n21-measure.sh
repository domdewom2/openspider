#!/bin/bash
# N21 measurement: U31 harness, unchanged assertions; cold fixture first.
set -u
cd ~/Projects/openspider-wt/m2-n21
export PATH=$HOME/.cargo/bin:$PATH OPENSPIDER_SECRET_STORE=memory
export OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome
export OS_APP_BRIDGE_BIN=$PWD/target/release/os-app-bridge OS_PERF_ROWS=5000000 OS_PERF_FIXTURE=$PWD/target/ui-perf.ospider
OUT=${OUT:-target/n21-claude}; mkdir -p $OUT
waitload() { for i in $(seq 1 120); do l=$(cut -d' ' -f1 /proc/loadavg); awk "BEGIN{exit !($l<4)}" && { echo "load $l" >> $OUT/load.txt; return; }; sleep 30; done; echo "load-timeout $(cat /proc/loadavg)" >> $OUT/load.txt; }
cache() { python3 -c "import sqlite3;c=sqlite3.connect('$OS_PERF_FIXTURE');print(c.execute('select key,built_seq,length(data) from view_cache order by key').fetchall())"; }
clear() { python3 -c "import sqlite3;c=sqlite3.connect('$OS_PERF_FIXTURE');c.execute('delete from view_cache');c.commit()"; }
pw() { (cd apps/desktop && timeout 3600s nice -n 10 npx playwright test --config e2e/perf/playwright.config.ts --workers=1 "$@"); }
mkdir -p $OUT; clear; echo "A start $(date -u +%FT%TZ) cache=$(cache)" >> $OUT/phases.txt
waitload; rm -rf target/loadtest/raw; mkdir -p target/loadtest/raw
pw --grep "cold browser context|tab filter numeric|ten seconds vertical|sidebar" > $OUT/A.log 2>&1
cp -r target/loadtest/raw $OUT/A-raw; cp target/loadtest/playwright.json $OUT/A-playwright.json 2>/dev/null
echo "A end $(date -u +%FT%TZ) cache=$(cache)" >> $OUT/phases.txt
# Core benchmark (U31 kernel runner, unchanged) after the browser blocks.
waitload; node apps/desktop/e2e/perf/core.mjs > $OUT/core.log 2>&1; cp target/loadtest/raw/core.json $OUT/core.json 2>/dev/null
echo "C end $(date -u +%FT%TZ)" >> $OUT/phases.txt
echo MEASURE_DONE
