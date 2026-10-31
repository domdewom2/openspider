// Execute from repository root. Build tools and npm ci before invoking.
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  existsSync,
  rmSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { resolve } from "node:path";
const root = process.cwd();
if (
  !["100000", "1000000", "5000000"].includes(
    process.env.OS_PERF_ROWS ?? "100000",
  )
)
  throw new Error("OS_PERF_ROWS must be 100000, 1000000 or 5000000");
const seconds = Number(process.env.OS_PERF_SOAK_SECONDS ?? 1800);
if (!Number.isInteger(seconds) || seconds < 1)
  throw new Error("OS_PERF_SOAK_SECONDS must be a positive integer");
const run = (cmd, args, cwd = root, env = {}) => {
  const p = spawnSync(cmd, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: "inherit",
    shell: process.platform === "win32" && cmd === "npm",
  });
  if (p.status !== 0)
    throw new Error(`${cmd} exited ${p.status}: ${p.error ?? ""}`);
};
for (const file of [
  "ui-report.json",
  "playwright.json",
  "playwright-suite.json",
  "playwright-rerun.json",
  "playwright-soak.json",
])
  rmSync(resolve("target/loadtest", file), { force: true });
mkdirSync("target/loadtest/raw", { recursive: true });
rmSync("target/loadtest/raw", { recursive: true, force: true });
mkdirSync("target/loadtest/raw", { recursive: true });
const rows = process.env.OS_PERF_ROWS ?? "100000";
const fixture = resolve(
  process.env.OS_PERF_FIXTURE ?? "target/ui-perf.ospider",
);
const metadata = `${fixture}.json`;
if (!existsSync(fixture)) {
  const p = spawnSync(
    resolve(
      `target/release/gen-ui-large${process.platform === "win32" ? ".exe" : ""}`,
    ),
    ["--rows", rows, "--seed", "19", "--output", fixture],
    { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  if (p.status !== 0) throw new Error(`Generator exited ${p.status}`);
  writeFileSync(metadata, p.stdout);
}
const generator = JSON.parse(readFileSync(metadata));
writeFileSync("target/loadtest/gen-ui-large.json", JSON.stringify(generator));
if (generator.seed !== 19) throw new Error("Fixture seed must be 19");
if (generator.rows !== Number(rows))
  throw new Error("Fixture row count does not match OS_PERF_ROWS");
const importFixture = resolve(
  process.env.OS_PERF_IMPORT_FIXTURE ?? "target/ui-perf-import.ospider",
);
if (!existsSync(importFixture)) {
  const p = spawnSync(
    resolve(
      `target/release/gen-ui-large${process.platform === "win32" ? ".exe" : ""}`,
    ),
    ["--rows", "1000000", "--seed", "20", "--output", importFixture],
    { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] },
  );
  if (p.status !== 0)
    throw new Error(`Import fixture generator exited ${p.status}`);
  writeFileSync(`${importFixture}.json`, p.stdout);
}
const importReport = JSON.parse(readFileSync(`${importFixture}.json`));
if (importReport.rows !== 1000000)
  throw new Error("Import fixture must have one million rows");
writeFileSync(
  "target/loadtest/gen-ui-import.json",
  JSON.stringify(importReport),
);
if (!process.argv.includes("--measure-only"))
  run("npm", [
    "exec",
    "-w",
    "apps/desktop",
    "--",
    "vite",
    "build",
    "--config",
    "e2e/perf/vite.config.ts",
  ]);
if (process.argv.includes("--prepare-only")) process.exit(0);
let failed = false;
try {
  run(process.execPath, ["apps/desktop/e2e/perf/core.mjs"], root, {
    OS_PERF_FIXTURE: fixture,
  });
} catch {
  failed = true;
}
try {
  run(
    "npm",
    [
      "exec",
      "-w",
      "apps/desktop",
      "--",
      "playwright",
      "test",
      "--config",
      "e2e/perf/playwright.config.ts",
    ],
    root,
    { OS_PERF_FIXTURE: fixture },
  );
} catch {
  failed = true;
}
// Report even when budgets failed. Playwright errors remain failures independently of metrics.
try {
  run(process.execPath, ["apps/desktop/e2e/perf/report.mjs"], root, {
    OS_PERF_REQUIRE_NATIVE: "0",
  });
} catch {
  failed = true;
}
if (failed) process.exitCode = 1;
