# N22 core diagnostics (UI-141)

Run from the repository root after a locked release build of `os-analyze` and
`gen-ui-large`. No additional dependency or workspace manifest is used.

```sh
node crates/os-loadtest/diagnostics/n22-compile.mjs
# SQL plans and borrowed-cell extraction, no benchmark verdict:
target/n22-profile target/n22-5000000.ospider
```

The compiler can also link the **unchanged** U31 budget runner, preserving a
before binary before rebuilding the optimized release libraries:

```sh
node crates/os-loadtest/diagnostics/n22-compile.mjs \
  apps/desktop/e2e/perf/core.rs target/n22-core-before
# Repeat after rebuilding, with target/n22-core-after as output.
# Args: fixture, row count, ISO timestamp, generator seed.
target/n22-core-before target/n22-5000000.ospider 5000000 \
  "$(date -u +%FT%TZ)" 19
```

Wait for one-minute server load below 4 before budget runs. Use the required
`timeout 3600s nice -n 10` prefix for heavy builds/runs. Generate the fixture
with U31's `gen-ui-large` (5 million, seed 19; 1 million, seed 20). Both binaries
must receive the same fixture. These diagnostics neither change nor replace
U31's budgets, completion/error handling, warmup, row window or sampling logic.
The profile is a separate run; do not include its cache warming in budget times.
