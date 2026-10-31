import { test } from "node:test";
import assert from "node:assert/strict";
import { regression } from "./report.mjs";
// spec: UI-148 [P1]
test("regression gate uses median of exactly last seven and fails above ten percent", () => {
  assert.equal(
    regression(110, [900, 100, 100, 100, 100, 100, 100, 100]).status,
    "ok",
  );
  assert.equal(
    regression(110.01, [900, 100, 100, 100, 100, 100, 100, 100]).status,
    "regression",
  );
  assert.equal(regression(90, [100, 100, 100], "min").status, "regression");
  assert.equal(regression(100, []).status, "no-baseline");
  assert.equal(regression(-5, [-5, -5, -5]).status, "ok");
  assert.equal(regression(-4, [-5, -5, -5]).status, "regression");
});

import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { report } from "./report.mjs";
// spec: UI-148 [P1]
test("an absolute budget failure cannot be excused by a slower history", () => {
  const directory = mkdtempSync(join(tmpdir(), "u31-report-"));
  const r = {
    scenario: "ipc",
    engine: "chromium",
    size: "small",
    started_at: "2026-01-01T00:00:00Z",
    metrics: { ms: { value: 101, budget: 100 } },
    passed: true,
  };
  try {
    mkdirSync(join(directory, "raw"));
    mkdirSync(join(directory, "history"));
    writeFileSync(join(directory, "raw", "ipc.json"), JSON.stringify(r));
    writeFileSync(
      join(directory, "gen-ui-large.json"),
      JSON.stringify({ elapsed_ms: 100, rows: 100000 }),
    );
    writeFileSync(
      join(directory, "playwright.json"),
      JSON.stringify({ stats: { unexpected: 0, interrupted: 0 } }),
    );
    writeFileSync(
      join(directory, "history", "ui-report.json"),
      JSON.stringify({
        size: "small",
        started_at: r.started_at,
        environment: { platform: process.platform, arch: process.arch },
        reports: [
          { ...r, metrics: { ms: { value: 1000, budget: 100 } } },
          { scenario: "generator", details: { rows: 100000 } },
        ],
      }),
    );
    const result = report(directory);
    assert.equal(result.passed, false);
    assert.equal(result.reports[0].metrics.ms.regression.status, "ok");
    assert.equal(result.reports[0].metrics.ms.passed, false);
    r.metrics.ms.value = null;
    writeFileSync(join(directory, "raw", "ipc.json"), JSON.stringify(r));
    assert.equal(
      report(directory).passed,
      false,
      "missing/nonfinite measurements must fail closed",
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

import { rendererProcesses } from "./rss.mjs";
// spec: UI-144
test("RSS attribution excludes another workspace and the Rust backend", () => {
  const processes = [
    { pid: 102, parent: 101, command: "chrome --type=renderer", bytes: 200 },
    { pid: 202, parent: 201, command: "chrome --type=renderer", bytes: 900 },
    { pid: 103, parent: 100, command: "os-app-bridge", bytes: 1000 },
    { pid: 101, parent: 100, command: "chrome", bytes: 300 },
    { pid: 201, parent: 200, command: "chrome", bytes: 800 },
    { pid: 104, parent: 101, command: "WebKitWebProcess", bytes: 100 },
  ];
  assert.deepEqual(
    rendererProcesses(processes, 100).map((p) => p.pid),
    [102, 104],
  );
  assert.deepEqual(rendererProcesses(processes, 999), []);
});
