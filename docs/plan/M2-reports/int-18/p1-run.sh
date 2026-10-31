#!/usr/bin/env bash
set -euo pipefail
while [ "$(pgrep -fc '^([^ ]*/)?cargo test --workspace' || true)" -ge 2 ] || [ "$(df --output=avail -k / | tail -1)" -lt 26214400 ]; do
  date -u '+%FT%TZ Ressourcenwartezeit'
  sleep 60
done
date -u '+%FT%TZ Start Parität ohne feste Uhr'
df -h /
export OPENSPIDER_HOME="$PWD/target/int18-p1-home"
export OPENSPIDER_SECRET_STORE=memory
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
timeout 3600s nice -n 10 target/debug/os-parity run --all --openspider target/debug/openspider --out target/int18-p1-all
date -u '+%FT%TZ Ende Parität'
