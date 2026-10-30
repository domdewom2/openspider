# Testing

The [repository rules](../../../CLAUDE.md) require a fixture-backed test for every implemented feature and a Spec-ID reference. Put the tag directly above the Rust test function or TypeScript `test`/`it`; [M2 testing rules](../../plan/M2.md), section 2, define `[P1]` and `[M2]` criterion tags and reasoned ignores. For example:

```rust
// spec: ONPAGE-012
#[test]
fn transferred_bytes_follow_the_spec() {
    // Assert explicit spec-derived values from a local fixture.
}
```

This is a tag-format illustration, not a runnable test. Follow existing tests such as [lifecycle determinism](../../../crates/os-engine/tests/lifecycle_determinism.rs). [Coverage implementation](../../../crates/os-parity/src/coverage.rs) and [M2 coverage CI](../../../.github/workflows/spec-coverage-m2.yml) connect tags to actual test results and package assignments. An ignored test is not passing evidence.

## Fixtures and expected results

[tests/fixtures](../../../tests/fixtures) contains local inputs; [os-testsite](../../../crates/os-testsite/src/lib.rs) serves scenarios, API mocks and synthetic sites. `scenario.toml` is the fixture manifest convention (M1 K-05); [M2 K2-24](../../plan/M2.md) gives package folder ownership precedence over older fixture names. Use loopback and mocks, not live services. Secrets in tests use `MemorySecretStore` / `OPENSPIDER_SECRET_STORE=memory`, per [M2 C13](../../plan/M2.md).

Derive expected values from the specification. [M1 section 2](../../plan/M1.md) distinguishes explicit spec assertions from broad regression goldens: regenerate broad outputs only after the spec tests pass, and explain golden changes. See [tests/golden](../../../tests/golden) and [fixture-server golden helpers](../../../crates/os-testsite/src/golden.rs).

## Stable time and synchronization

Inject `Clock` and `IdSource`, use `FixedClock`/`ManualClock`, seed randomness and use paused Tokio time for time/rate assertions; see [time abstractions](../../../crates/os-model/src/time.rs), [deterministic lifecycle tests](../../../crates/os-engine/tests/lifecycle_determinism.rs) and [M1 section 2, rule 8](../../plan/M1.md). Frontend tests use fake timers or Playwright's clock as required by [M2 rule 8](../../plan/M2.md).

For concurrent tests, synchronize on an observed state, channel, barrier or notification rather than a guessed sleep. Keep a timeout as a failure bound. The [engine testing helpers](../../../crates/os-engine/src/testing.rs) are the starting point for injected dependencies. Wall-clock performance measurements belong in dedicated benchmarks/load tests, not ordinary timing assertions; see [M1 rule 8](../../plan/M1.md).

## Frontend and end-to-end layers

| Layer | Sources and purpose |
|---|---|
| Unit/component tests | [desktop test script](../../../apps/desktop/package.json) runs Vitest; [static checks](../../../apps/desktop/scripts/check-static.mjs) add contract, source and token checks |
| E2E-A | [e2e/](../../../apps/desktop/e2e) with [Playwright config](../../../apps/desktop/playwright.config.ts): Vite UI plus IPC bridge or mock, Chromium and WebKit |
| E2E-B | [native harness README](../../../apps/desktop/e2e-b/README.md), [workflow](../../../.github/workflows/ui-e2e-b.yml): actual Tauri/WebView integration |
| Visual/performance acceptance | [M2 UI test strategy](../../plan/M2-ui-concept.md), section 7, and [plan](../../plan/M2.md): separate acceptance evidence; runner measurements can be indicative |

The main Playwright config selects engines through `OS_E2E_PROJECTS` (default Chromium and WebKit), uses `e2e/`, forbids focused tests in CI and retains failure traces. Performance projects depend on their corresponding main engine project. The [bridge support](../../../apps/desktop/e2e/support/bridge.ts) defines `OS_E2E_TRANSPORT=bridge|mock|auto`; [UI CI currently sets auto](../../../.github/workflows/ui-e2e.yml), so inspect transport evidence instead of assuming the Rust bridge ran.

The exact [M2 C4](../../plan/M2.md) E2E-A command is:

```sh
npm run -w apps/desktop e2e -- --project=chromium --project=webkit
```

Additional domain configs live below [e2e/](../../../apps/desktop/e2e), for example [export](../../../apps/desktop/e2e/export/playwright.config.ts) and [detail](../../../apps/desktop/e2e/detail/playwright.config.ts). Check their test selection before using one in place of the main suite.

## Load tests

[os-loadtest](../../../crates/os-loadtest/src/main.rs) provides `list` and `run`. The [CI workflow](../../../.github/workflows/ci.yml) builds both the runner and the child crawler example before the small suite:

```sh
cargo run --release --locked -p os-loadtest -- list
cargo build --release --locked -p os-loadtest
cargo build --release --locked -p os-engine --example crawl_child
cargo run --release --locked -p os-loadtest -- run --scenario all --size small --out target/loadtest/report.json
```

For a dedicated large run, the workflow documents:

```sh
cargo run --release -p os-loadtest -- run --scenario perf-001 --size large --work-dir <ordner>
```

Replace `<ordner>` with a measurement directory. Small scenarios use 10,000 URLs; large scenarios use one million by default. The [scenario definitions](../../../crates/os-loadtest/src/scenario.rs) and [runner](../../../crates/os-loadtest/src/runner.rs) cover throughput/RSS, backpressure and seeded kill/resume evidence. See [performance requirements](../../spec/07-performance.md) for acceptance limits. Schedule measurements on a quiet machine under [M2 C11](../../plan/M2.md), separately from builds and browser suites.
