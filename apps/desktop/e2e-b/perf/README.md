# Native UI performance

This entry uses the existing optional `e2e-webdriver` feature. It requires an
active macOS/Windows desktop and changes no Tauri product files. From repository
root, after browser preparation (including the import fixture):

```sh
node apps/desktop/e2e-b/perf/prepare.mjs
npm exec -w apps/desktop -- tauri build --no-bundle --features e2e-webdriver --config ../../target/ui-perf-native.conf.json
OS_PERF_ROWS=5000000 node apps/desktop/e2e-b/perf/run.mjs target/release/openspider-desktop
node apps/desktop/e2e/perf/report.mjs
```

On Windows append `.exe` to the binary name. Default fixture paths match
`e2e/perf/run.mjs`. Override `OS_PERF_FIXTURE` when using another path.
The release process uses a temporary home and no OS keychain. WebDriver and CDP
ports are allocated on loopback. Startup includes process launch, native window,
frontend boot and the first successful readiness observation (50 ms polling);
this is a conservative upper bound. Opening waits for real grid addresses.

Windows additionally records renderer RSS over a configurable seeded soak and
writes `target/loadtest/webview2-trace.json`. macOS memory uses Playwright WebKit
as permitted in M2-ui-concept 7.4. Failed native readiness/IPC/RSS attribution is
reported as failure, never as zero memory or a successful native result.
