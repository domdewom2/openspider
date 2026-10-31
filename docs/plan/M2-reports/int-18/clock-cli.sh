#!/usr/bin/env bash
# C8: ausschließlich die Paritäts-CLI auf den Exporttag der Fixture setzen.
# Echte monotone Uhr für Tokio/Timeouts erhalten; keine Stat-Zeiten verändern.
# Vorbereitung siehe int-18.md, keine Systeminstallation nötig.
set -euo pipefail
INT18_ROOT=$(git rev-parse --show-toplevel)
INT18_CLOCK_LIB=${INT18_FAKETIME_LIB:-$INT18_ROOT/target/int18-clock/extracted/usr/lib/x86_64-linux-gnu/faketime/libfaketimeMT.so.1}
if [ ! -f "$INT18_CLOCK_LIB" ]; then
  echo 'libfaketime fehlt; lokale Vorbereitung aus int-18.md ausführen.' >&2
  exit 2
fi
export LD_PRELOAD="${LD_PRELOAD:+$LD_PRELOAD:}$INT18_CLOCK_LIB"
export FAKETIME='@2026-10-30 12:00:00'
export FAKETIME_DONT_FAKE_MONOTONIC=1 NO_FAKE_STAT=1 TZ=UTC
exec "$INT18_ROOT/target/debug/openspider" "$@"
