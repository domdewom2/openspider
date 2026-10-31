import { readdirSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import os from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
export const median = (values) => {
  const s = [...values].sort((a, b) => a - b);
  return s.length % 2
    ? s[s.length >> 1]
    : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};
export function regression(value, previous, direction = "max") {
  if (!previous.length) return { status: "no-baseline", samples: 0 };
  const baseline = median(previous.slice(-7));
  return {
    baseline,
    samples: Math.min(previous.length, 7),
    limit:
      direction === "max"
        ? baseline + Math.abs(baseline) * 0.1
        : baseline / 1.1,
    status: (
      direction === "max"
        ? value <= baseline + Math.abs(baseline) * 0.1
        : value >= baseline / 1.1
    )
      ? "ok"
      : "regression",
  };
}
export function report(directory = "target/loadtest") {
  const raw = resolve(directory, "raw");
  const reports = readdirSync(raw)
    .filter((n) => n.endsWith(".json"))
    .sort()
    .map((n) => JSON.parse(readFileSync(resolve(raw, n))));
  if (!reports.length) throw new Error("No measurements");
  for (const r of reports)
    for (const metric of Object.values(r.metrics))
      if (metric && typeof metric === "object" && "budget" in metric) {
        metric.direction ??= "max";
        metric.passed =
          metric.completed !== false &&
          Number.isFinite(metric.value) &&
          (metric.direction === "min"
            ? metric.value >= metric.budget
            : metric.value <= metric.budget);
        if (!metric.passed) r.passed = false;
      }
  const generator = JSON.parse(
    readFileSync(resolve(directory, "gen-ui-large.json")),
  );
  reports.push({
    scenario: "generator",
    engine: "core",
    size: reports[0].size,
    started_at: reports[0].started_at,
    metrics: {
      elapsedMs: {
        value: generator.elapsed_ms,
        budget: 600000,
        direction: "max",
        passed: generator.elapsed_ms <= 600000,
      },
    },
    details: generator,
    passed: generator.elapsed_ms <= 600000,
  });
  for (const required of [
    "core",
    ...(generator.rows === 5000000 ? ["core-1000000"] : []),
  ]) {
    if (!reports.some((r) => r.scenario === required))
      reports.push({
        scenario: `${required}-missing`,
        engine: "core",
        size: reports[0].size,
        started_at: reports[0].started_at,
        metrics: {},
        passed: false,
      });
  }
  if (
    process.env.OS_PERF_REQUIRE_NATIVE === "1" &&
    !reports.some((r) => r.scenario === "native")
  ) {
    reports.push({
      scenario: "native-missing",
      engine: "native",
      size: reports[0].size,
      started_at: reports[0].started_at,
      metrics: {},
      passed: false,
    });
  }
  const resultFiles = readdirSync(directory).filter((name) =>
    /^playwright(?:-.+)?\.json$/.test(name),
  );
  if (!resultFiles.length) throw new Error("Playwright result report missing");
  for (const file of resultFiles) {
    const playwright = JSON.parse(readFileSync(resolve(directory, file)));
    if (playwright.stats.unexpected || playwright.stats.interrupted)
      reports.push({
        scenario: `playwright-failures-${file}`,
        engine: "runner",
        size: reports[0].size,
        started_at: reports[0].started_at,
        metrics: {},
        details: playwright.stats,
        passed: false,
      });
  }
  const historyDir = resolve(directory, "history");
  let history = [];
  try {
    history = readdirSync(historyDir, { recursive: true })
      .filter((n) => n.endsWith("ui-report.json"))
      .map((n) => JSON.parse(readFileSync(resolve(historyDir, n))))
      .sort((a, b) => a.started_at.localeCompare(b.started_at));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  for (const r of reports)
    for (const [name, metric] of Object.entries(r.metrics)) {
      if (!metric || typeof metric !== "object" || !("budget" in metric))
        continue;
      const previous = history
        .filter(
          (h) =>
            h.environment?.platform === process.platform &&
            h.size === r.size &&
            h.environment?.arch === os.arch() &&
            h.reports.some(
              (p) =>
                p.scenario === "generator" &&
                p.details?.rows === generator.rows,
            ),
        )
        .flatMap((h) =>
          h.reports
            .filter(
              (p) =>
                p.scenario === r.scenario &&
                p.engine === r.engine &&
                p.details?.seed === r.details?.seed &&
                p.details?.fullSoak === r.details?.fullSoak &&
                p.details?.configuredSeconds === r.details?.configuredSeconds,
            )
            .map((p) => p.metrics[name]?.value),
        )
        .filter(Number.isFinite);
      metric.regression = !Number.isFinite(metric.value)
        ? { status: "measurement-missing" }
        : name === "durationSeconds"
          ? { status: "observation-only" }
          : regression(metric.value, previous, metric.direction);
      if (metric.regression.status === "regression") r.passed = false;
    }
  const result = {
    size: reports[0].size,
    started_at: reports.map((r) => r.started_at).sort()[0],
    passed: reports.every((r) => r.passed),
    reports,
    environment: {
      platform: process.platform,
      arch: os.arch(),
      release: os.release(),
      cpus: os.cpus()[0]?.model,
      cores: os.cpus().length,
      memory_bytes: os.totalmem(),
      load_average: os.loadavg(),
      node: process.version,
      revision: execFileSync("git", ["rev-parse", "HEAD"], {
        encoding: "utf8",
      }).trim(),
      rust: execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim(),
      playwright: JSON.parse(
        readFileSync("node_modules/playwright/package.json"),
      ).version,
      reference: process.env.OS_PERF_REFERENCE === "1",
      browser_builds: JSON.parse(
        readFileSync("node_modules/playwright-core/browsers.json"),
      ).browsers.filter((b) => ["chromium", "webkit"].includes(b.name)),
      frontend_version: JSON.parse(readFileSync("apps/desktop/package.json"))
        .version,
    },
    notes: [
      "E2E-A browser indication; native evidence reported separately. Reference A: run apps/desktop/e2e/perf/run-reference.sh from a macOS GUI session.",
    ],
  };
  mkdirSync(directory, { recursive: true });
  writeFileSync(
    resolve(directory, "ui-report.json"),
    JSON.stringify(result, null, 2) + "\n",
  );
  for (const r of reports) {
    console.log(`${r.engine} ${r.scenario}: ${r.passed ? "ok" : "FAILED"}`);
    for (const [name, m] of Object.entries(r.metrics))
      if (
        m?.budget !== undefined &&
        (m.passed === false || m.regression?.status === "regression")
      )
        console.error(
          `${name}: ${m.value}; budget ${m.direction ?? "max"} ${m.budget}; ${JSON.stringify(m.regression)}`,
        );
  }
  return result;
}
if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  process.exitCode = report(process.argv[2]).passed ? 0 : 1;
