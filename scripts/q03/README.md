# Q03 measurement blocks

Prerequisites: Rust toolchain from the repository, Python 3, `zstd`, local Chromium
(`OPENSPIDER_CHROMIUM` when discovery is insufficient), enough free disk for the
5-million native crawl and temporary analysis files. No external website is crawled.

Build before measuring:

```sh
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release --locked -j 12 -p os-loadtest -p os-cli -p os-testsite
cargo build --release --locked -j 12 -p os-engine --example crawl_child
cargo test --release --locked -j 12 -p os-analyze --test near_dup_scale --test link_score_benchmarks --test link_score_m2 --test near_dup_d02 \
  -p os-export --test pipeline_perf -p os-render --test render_chromium \
  -p os-engine --test render_integration --no-run
```

Linux stage 1 (one suite at a time):

```sh
export XDG_RUNTIME_DIR=/run/user/$(id -u)
systemd-run --user --scope --quiet -p MemoryMax=16G -p MemorySwapMax=0 \
  bash scripts/q03/server-run.sh
python3 scripts/q03/summarize.py
```

The server script verifies both limits and places temporary fixtures on the repository disk under `target/loadtest/tmp` (never an inherited RAM-backed `/tmp`). `Q03_FROM=perf006-recycle` resumes at that named block after archiving previous failures; the default runs everything. Set `OPENSPIDER_CHROMIUM` to override its
local default. Failures do not prevent subsequent blocks; the final exit status
indicates failed blocks. Raw output and process metrics go to `target/loadtest`.
`test-bin.py` selects the most recently built matching release integration test, rejects ambiguous package names and zero matching tests,
so build the documented test targets immediately before the suite.

Stage 2, **only on the 16-GB reference Mac**:

```sh
bash scripts/q03/reference-run.sh
```

It builds first, uses `caffeinate -dims`, waits for 1-minute load below 1.5 before
each block, runs without cgroups, also compares the three storage profiles, and
writes `target/loadtest/report.json`. App frame/interaction measurements still
need the desktop app; the headless report explicitly leaves these unmeasured.

`cli-run.py` starts an ephemeral local `os-testsite`, isolates `OPENSPIDER_HOME`,
and runs `openspider --headless --crawl … --progress json` without
configuration overrides (five threads). It records 100k progress windows, all
progress samples including crawl-file/WAL bytes, RSS, errors and gaps >30 s.
The first `analysis` phase supplies the final crawl count because the CLI emits
no final `crawl` progress event. Crawl duration ends at the first analysis phase; the report separately records the last increasing fetch counter, observed final report gap and post-crawl work. The final report gap includes any commit/index work and progress-reader delay; it is not an isolated SQL commit timer. The native registry crawl file is used directly, avoiding an additional save-copy. The final file is compressed with zstd levels 3
and 19, twelve threads each. Each compressed copy is deleted immediately; the
temporary crawl/home directory is deleted when the block exits.
`Q03_PAGES=1000` is a setup smoke test, **not** a full-run result; it uses the same
output name and must be archived separately before a real run.

`OS_LOADTEST_MEMORY_LOG=/absolute/path.jsonl` enables opt-in child profiling every
100k URLs and at completion: cumulative engine structure peaks, current GNU
allocator accounting, and Linux process status. RSS is reported in bytes.
Repeated runs append and include PID and completion markers; use a fresh path
for independent runs. Counter byte estimates are not full heap attribution.
