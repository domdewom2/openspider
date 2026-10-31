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

To collect both sizes and variants on the shared server:

```sh
timeout 3600s nice -n 10 python3 crates/os-loadtest/diagnostics/n22-measure.py
```

This additive coordinator starts below load 2 and samples load throughout each
run. Every attempt is retained; only attempts whose peak one-minute load stayed
below 4 become the primary result. **Budget failures never trigger a retry or
change their verdict.** Linux also records GNU time's process peak RSS. `--rows`
and `--phase` can select a block; `--output` keeps later candidate runs separate.
The compiler uses the newest analyze artifact and checks compatible model/store
metadata, so a stale analyze variant cannot silently become the candidate.

For query plans without scanning any fixture rows, pass `--plans-only` to
`target/n22-profile`. Page-oracle plans and actual permutation/search statements
are labelled separately: the legacy numeric permutation already uses the
covering `urls_status` index, whereas the generic SQL page needs a temporary
sort. Extraction timings borrow cells and are lower bounds; they do not include
owned key allocation, collation key generation, sorting or result materialization.
