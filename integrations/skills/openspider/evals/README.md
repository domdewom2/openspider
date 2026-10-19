# Optional agent evaluation (AGENT-041)

`tasks.json` contains twelve tasks with fixture, prompt, relevant tools and observable
criteria. This is an optional behavioral evaluation, separate from the deterministic
format/install/protocol gates. Do not count a protocol smoke as agent success.

Build `openspider` and `os-testsite` from the repo root:

```sh
cargo build --locked -p os-cli -p os-testsite --bins
```

Use a temporary `OPENSPIDER_HOME` and a server started with:

```sh
target/debug/os-testsite tests/fixtures/out-basic-site 0
```

Use `target/debug/os-testsite.exe` on Windows. The server prints its ephemeral loopback
URL. Crawl that URL with `openspider --crawl <url> --headless`, then point the agent's
`openspider mcp` at the same temporary home. See `integrations/scripts/smoke.mjs` for
fully automated fixture startup and crawl setup. For redirect tasks, repeat with
`tests/fixtures/out-redirects`; its host-switch route needs `site.test`/`www.site.test`
resolution to the fixture server if evaluating that route. The tasks in this set use
relative-path redirect routes. For the comparison task, crawl the basic site twice
without restarting the fixture server (same origin), retaining both files.

The default server only reads saved crawls. To evaluate crawl-control behavior, an
operator may set `classes = ["read", "crawl"]` and `allow_private_targets = true` in
the **temporary** home's `mcp.toml`. Never change the user's production network policy
for an eval. No paid service, secret store or external target is required.

Run each prompt in a fresh agent session with the skill loaded. Supply the crawl ID,
fixture origin and, when relevant, snapshot order. Keep the transcript/tool calls and
grade every listed criterion. A pass requires all criteria; record failure or a skip
reason rather than inventing unavailable evidence. Results are an array, for example:

```json
[{"id":"basic-404","status":"pass","evidence":"artifact://run/transcripts/basic-404.txt; all three criteria verified"}]
```

This is a result-format example, not a measured result. From `integrations/`, run
`OPENSPIDER_EVAL_RESULTS=<results.json> npm run eval:report` (PowerShell:
`$env:OPENSPIDER_EVAL_RESULTS = '<results.json>'; npm run eval:report`). The report writes
`target/integrations/eval-report.json` and a CI summary if `GITHUB_STEP_SUMMARY` is set.
Without results, success rate is explicitly unmeasured and all tasks are `not_run`.
Evaluated success rate and completion rate are separate, so skipped tasks cannot hide
missing coverage. The workflow uploads the report; manual evaluation is not a gate.
