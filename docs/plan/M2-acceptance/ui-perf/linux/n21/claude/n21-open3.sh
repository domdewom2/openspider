#!/bin/bash
set -u
cd ~/Projects/openspider-wt/m2-n21
touch .claude-active
export OPENSPIDER_SECRET_STORE=memory OPENSPIDER_CHROMIUM=/home/build/.cache/ms-playwright/chromium-1248/chrome-linux64/chrome
export OS_APP_BRIDGE_BIN=$PWD/target/release/os-app-bridge OS_PERF_ROWS=5000000 OS_PERF_FIXTURE=$PWD/target/ui-perf.ospider
OUT=$HOME/n21-claude-measure/open6; mkdir -p $OUT
for i in 1 2 3; do
  for t in $(seq 1 120); do l=$(cut -d' ' -f1 /proc/loadavg); awk "BEGIN{exit !($l<3)}" && break; sleep 30; done
  python3 -c "import sqlite3;c=sqlite3.connect('$OS_PERF_FIXTURE');c.execute('delete from view_cache');c.commit()"
  echo "run $i load $(cut -d' ' -f1-3 /proc/loadavg)" >> $OUT/load.txt
  rm -rf target/loadtest/raw; mkdir -p target/loadtest/raw
  (cd apps/desktop && OS_E2E_PROJECTS=chromium timeout 900s nice -n 10 npx playwright test --config e2e/perf/playwright.config.ts --workers=1 --grep "cold browser context" > $OUT/run$i.log 2>&1)
  cp target/loadtest/raw/chromium-open.json $OUT/chromium-open-$i.json
done
echo OPEN_DONE >> $OUT/load.txt
