import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
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
const label = process.env.OS_PERF_CORE_LABEL ?? "core";
if (!/^core(?:-\d+)?$/.test(label))
  throw new Error("Invalid core benchmark label");
const fixture = resolve(
  process.env.OS_PERF_FIXTURE ?? "target/ui-perf.ospider",
);
const seed = JSON.parse(readFileSync(`${fixture}.json`)).seed;
if (!Number.isSafeInteger(seed)) throw new Error("Invalid fixture seed");
const output = `target/loadtest/raw/${label}.json`;
const result = spawnSync(
  binary,
  [
    fixture,
    process.env.OS_PERF_ROWS ?? "100000",
    new Date().toISOString(),
    String(seed),
  ],
  { encoding: "utf8", timeout: 600000 },
);
if (result.status !== 0) {
  const failure = {
    scenario: label,
    size: process.env.OS_PERF_ROWS === "5000000" ? "large" : "small",
    started_at: new Date().toISOString(),
    engine: "core",
    metrics: {},
    details: { error: result.stderr, errorStatus: result.status },
    passed: false,
  };
  writeFileSync(output, JSON.stringify(failure, null, 2));
  throw new Error(`Core runner failed: ${result.stderr}`);
}
const report = JSON.parse(result.stdout);
report.scenario = label;
if (process.env.OS_PERF_CORE_SUITE_SIZE)
  report.size = process.env.OS_PERF_CORE_SUITE_SIZE;
writeFileSync(output, JSON.stringify(report, null, 2));
console.log(JSON.stringify(report));
