# M2-U31 Linux evidence

`linux/ui-report.json` is the final PERF-009 aggregate, `passed: false`.
It combines measured blocks, not an atomic suite run. `linux/raw/` retains
per-scenario samples, frames, payloads, renderer RSS and incomplete/error states.
Hardware and tool/browser versions are in the aggregate `environment`.

- `playwright-named.json`: final 65 named-column suite (11 passed, 18 failed, 1 skipped), with a short 120-second functional soak.
- `playwright-range.json`: final whole-crawl IPC samples, both engines passed.
- `playwright.json`: latest WebKit RSS/load after WPE process attribution fix; both budget failures.
- `full-soak.json`: exactly one completed 1800-second measurement phase, original default-empty projection; peak RSS and actions failed. Its duration includes neither preparation nor warmup. No second full soak was completed.
- `attempt-1/`: interrupted soak (system-wide ps output ENOBUFS); progress retained separately.
- `attempt-2/`: startup failure before warmup (worker preview port), not a soak measurement.
- `projection-diagnostic/`, `initial-suite-ui-report.json`: earlier projections and intermediate reruns, superseded by final named columns / explicit IPC20 view.
- `attribution-diagnostic/`: former WebKit renderer attribution failure, superseded by the latest measured WebKit values.
- `logs/*.gz`: gzip-compressed execution and validation logs.

Historical raw start times were aligned with the matching Playwright
TestResult.startTime. The original timestamp and source report remain in
`details`; no measured metric was changed. The reused generator has elapsed
and fixture metadata but no standalone wall-clock start timestamp.
The five-million suite uses seed 19; its supplemental one-million direct core
series uses the native-import seed-20 fixture. No database/binary is committed.

Reference A, Windows CI, native timing and seven actual nightly history runs
remain open. Commands, limitations, unchanged budgets and the foreign-owner
`err-stale` finding are documented in `../../M2-reports/u31.md`.
