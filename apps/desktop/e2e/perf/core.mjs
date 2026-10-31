import { spawnSync } from "node:child_process";
import { readdirSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
const deps = "target/release/deps";
const artifacts = (name) =>
  readdirSync(deps)
    .filter((n) => n.startsWith(`lib${name}-`) && n.endsWith(".rlib"))
    .map((n) => `${deps}/${n}`);
const binary = resolve(
  `target/ui-perf-core${process.platform === "win32" ? ".exe" : ""}`,
);
const native = readdirSync("target/release/build").flatMap((n) => [
  "-L",
  `native=target/release/build/${n}/out`,
]);
let built = false,
  last;
// Cargo feature unification can leave multiple metadata variants; rustc verifies which match os-analyze.
for (const model of artifacts("os_model"))
  for (const store of artifacts("os_store")) {
    if (built) break;
    const result = spawnSync(
      process.env.OS_PERF_RUSTC ?? "rustc",
      [
        "--edition=2021",
        "-O",
        "-Dwarnings",
        "apps/desktop/e2e/perf/core.rs",
        "-o",
        binary,
        "-L",
        `dependency=${deps}`,
        ...native,
        "--extern",
        `os_analyze=${artifacts("os_analyze").at(-1)}`,
        "--extern",
        `os_model=${model}`,
        "--extern",
        `os_store=${store}`,
      ],
      { encoding: "utf8" },
    );
    if (result.status === 0) built = true;
    else last = result.stderr;
  }
if (!built) throw new Error(last);
if (process.argv.includes("--compile-only")) process.exit(0);
mkdirSync("target/loadtest/raw", { recursive: true });
const result = spawnSync(
  binary,
  [
    resolve(process.env.OS_PERF_FIXTURE ?? "target/ui-perf.ospider"),
    process.env.OS_PERF_ROWS ?? "100000",
    new Date().toISOString(),
  ],
  { encoding: "utf8", timeout: 600000 },
);
if (result.status !== 0) {
  const failure = {
    scenario: "core",
    size: process.env.OS_PERF_ROWS === "5000000" ? "large" : "small",
    started_at: new Date().toISOString(),
    engine: "core",
    metrics: {},
    details: { error: result.stderr, errorStatus: result.status },
    passed: false,
  };
  writeFileSync(
    "target/loadtest/raw/core.json",
    JSON.stringify(failure, null, 2),
  );
  throw new Error(`Core runner failed: ${result.stderr}`);
}
writeFileSync("target/loadtest/raw/core.json", result.stdout);
console.log(result.stdout);
