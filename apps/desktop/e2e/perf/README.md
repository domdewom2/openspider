# UI performance (M2-U31)

Run from the repository root with Node 22, the locked npm installation and release
Rust tools. All traffic is loopback; the bridge uses a temporary home and the
in-memory secret store. Normal product sources/configuration remain unchanged.
The `.bench.ts` files are selected only by this directory's Playwright config,
so ordinary E2E discovery does not accidentally start a 30-minute soak.

```sh
npm ci
cargo build --locked -j 6 -p os-testsite -p os-cli -p os-parity --bins
cargo build --release --locked -j 6 -p os-app --bin os-app-bridge --features e2e-bridge
cargo build --release --locked -j 6 -p os-loadtest --bin gen-ui-large
npm exec -w apps/desktop -- playwright install chromium webkit
OS_APP_BRIDGE_BIN="$PWD/target/release/os-app-bridge" \
  OS_PERF_ROWS=100000 OS_PERF_SOAK_SECONDS=300 \
  node apps/desktop/e2e/perf/run.mjs
```

On the shared Linux server, use `timeout 3600s nice -n 10` for heavy commands;
run `~/ws-slot.sh` before workspace-wide test runs. Set `OPENSPIDER_CHROMIUM`
to the installed Chromium executable if necessary. Linux is an indication, not
a substitute for reference A or native WebView2 measurements.

The runner generates `gen-ui-large` with seed 19, checks the recorded row count,
builds Vite assets and runs the standalone Rust core benchmark before Playwright.
The standalone Rust file links against the locked release artifacts; no Cargo
manifest is introduced or changed. Rustc checks matching dependency metadata.
The second, one-million-row fixture uses seed 20 to avoid a duplicate crawl ID
when testing native import. Five-million-row runs also benchmark this fixture as
`core-1000000`, with the separate one-million-row budgets. Reusing a fixture requires its adjacent `.json`
generator report. Existing fixture files are never overwritten.

Outputs are `target/loadtest/ui-report.json` (PERF-009 suite/individual report
fields), `gen-ui-large.json`, raw per-scenario JSON, Playwright JSON and JUnit.
The report adds hardware/software provenance, exact budgets, raw frame intervals,
IPC samples and per-PID RSS. Nonfinite/missing values, test failures, absolute
budget failures and regressions produce a nonzero exit. A failed benchmark does
not prevent later scenarios from running. A slow core operation records its
error alongside completed measurements.

| Variable                 | Default                         | Meaning                                             |
| ------------------------ | ------------------------------- | --------------------------------------------------- |
| `OS_PERF_ROWS`           | `100000`                        | `100000`, `1000000` or `5000000`; row count checked |
| `OS_PERF_FIXTURE`        | `target/ui-perf.ospider`        | Generated fixture + adjacent metadata               |
| `OS_PERF_IMPORT_FIXTURE` | `target/ui-perf-import.ospider` | One-million-row import fixture                      |
| `OS_E2E_PROJECTS`        | `chromium,webkit`               | Engines, executed serially with one worker          |
| `OS_PERF_SOAK_SECONDS`   | `1800`                          | Short runs are explicitly marked `fullSoak: false`  |
| `OS_PERF_SOAK_ENGINE`    | `chromium`                      | One soak per suite; `none` disables browser soak    |
| `OS_APP_BRIDGE_BIN`      | existing E2E debug binary       | Use a release bridge for budgets                    |
| `OS_PERF_LOAD_LIMIT`     | `2.5`                           | Reference script: maximum one-minute load average   |

`--prepare-only` generates/builds without measuring; `--measure-only` reuses the
Vite build. Preparation clears the current raw results, so archive an earlier
run before starting a new one. Keep history under `target/loadtest/history/`.

## Reference A

From an active GUI session on the Mac mini M1 / 16 GB:

```sh
OS_PERF_LOAD_LIMIT=2.5 apps/desktop/e2e/perf/run-reference.sh
```

The script enforces five million rows and a full 1800-second soak, uses `caffeinate`, waits before measurements, records
`system_profiler`/OS/Node/Rust data, generates five million rows and runs a full
1800-second WebKit soak. It also builds/runs the native WKWebView entry for cold
start, opening, overview/issues and import. Memory uses the permitted macOS
E2E-A fallback. Verify the recorded hardware when accepting reference A.
Budgets remain strict on every platform. Archive `target/loadtest/` before cleanup.

## Nightly and history

`ui-nightly.yml` runs only on schedule or manual dispatch: macOS arm64 and
Windows x64, each with 100000 and 5000000 rows. The configurable nightly soak
default is 300 seconds (`UI_PERF_SOAK_SECONDS` repository variable or dispatch
input); dispatch `soak_seconds=1800` for full acceptance. Windows uses native
WebView2 RSS and a CDP trace; macOS uses the WebKit fallback for RSS.

Artifacts are retained for 90 days. The job downloads the previous seven
available reports for the same branch, matrix entry, OS and architecture.
Each metric is compared against the median of its last seven finite values;
latencies/bytes must be at most 110%, FPS must be at least median/1.1.
Absolute budgets always apply. No history is reported as `no-baseline`; seven
actual nightly runs are still required for acceptance. A missing native report
fails the final CI gate. Archived history is excluded from the next upload.

After integration/publishing by the orchestrator:

```sh
gh workflow run ui-nightly.yml --ref main -f soak_seconds=1800
```

## Scope and limits

Grid measurements use the production DataGrid/ViewDataSource with real
`crawl.open`, `view.open/update/rows` and the real Rust core. IPC builds an explicit 20-column view first, then measures 50
post-warmup wrapper round trips, 200 × 20 response shape and serialized payload.
The frame recorder retains rAF intervals and optional long-animation-frame data.
The 30-minute seeded soak performs vertical/horizontal jumps and tab changes,
records functional action failures, and samples attributable renderer RSS.
Backend RSS is deliberately excluded from the UI budget.

Visualization tests reuse U24's deterministic 10000-node graph/tree adapter;
the renderer, layout worker and SVG exporter are the production implementations.
The crawl-load scenario runs the actual application shell, 20 crawl threads and
a loopback 200000-page ring, verifies the observed rate is at least 200 URL/s,
then measures click-to-frame latency, FPS, event rate and idle renderer CPU.
Browser cold-context timing is explicitly distinguished from native process
cold start. Sidebar timings are core RPC latency, not a proof of every detailed
UI-145 rendering criterion. Hidden-window throttling, detail/source/diff edge
cases, native visualization RSS/release and operation cancellation are not new
reference measurements in U31; existing feature tests are unchanged.
