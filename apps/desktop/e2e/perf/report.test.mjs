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
  assert.deepEqual(
    rendererProcesses(
      [{ pid: 105, parent: 100, command: "WPEWebProcess", bytes: 100 }],
      100,
    ).map((p) => p.pid),
    [105],
  );
});

import { webkitXpcRenderers } from "./rss.mjs";
// spec: UI-144
test("macOS WebKit XPC renderers are attributed via own browser bundle and start", () => {
  const root = "/Users/u/Library/Caches/ms-playwright/webkit-2370";
  const xpc = (name) =>
    `${root}/com.apple.WebKit.${name}.xpc/Contents/MacOS/com.apple.WebKit.${name}`;
  const ui = `${root}/Playwright.app/Contents/MacOS/Playwright`;
  const processes = [
    { pid: 101, parent: 100, started: -51, command: "/bin/sh" },
    { pid: 102, parent: 101, started: -50, command: ui },
    { pid: 103, parent: 100, started: -52, command: "os-app-bridge" },
    { pid: 201, parent: 1, started: -50, command: xpc("WebContent") },
    {
      pid: 202,
      parent: 1,
      started: -10,
      command: `${xpc("WebContent")}.Development`,
    },
    { pid: 203, parent: 1, started: -100, command: xpc("WebContent") },
    { pid: 204, parent: 1, started: -50, command: xpc("Networking") },
    {
      pid: 205,
      parent: 1,
      started: -5,
      command:
        "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent",
    },
    { pid: 206, parent: 102, started: -40, command: xpc("WebContent") },
  ].map((p) => ({ bytes: 1, ...p }));
  // Only launchd-started WebContent of the own bundle, not older than the browser.
  assert.deepEqual(
    webkitXpcRenderers(processes, 100).map((p) => p.pid),
    [201, 202],
  );
  // Another workspace's tree has no Playwright browser: nothing attributable.
  assert.deepEqual(webkitXpcRenderers(processes, 999), []);
  // A browser of another Playwright bundle does not interfere.
  const other = {
    pid: 301,
    parent: 300,
    started: -60,
    bytes: 1,
    command:
      "/opt/ms-playwright/webkit-9/Playwright.app/Contents/MacOS/Playwright",
  };
  assert.deepEqual(
    webkitXpcRenderers([...processes, other], 100).map((p) => p.pid),
    [201, 202],
  );
  // A foreign live browser of the same bundle that could own a candidate: no guess.
  for (const started of [-60, -50, -20])
    assert.throws(
      () =>
        webkitXpcRenderers(
          [...processes, { ...other, started, command: ui }],
          100,
        ),
      /Ambiguous WebKit renderer attribution/,
    );
  // Started after every candidate: it cannot own them.
  assert.deepEqual(
    webkitXpcRenderers(
      [...processes, { ...other, started: -5, command: ui }],
      100,
    ).map((p) => p.pid),
    [201, 202],
  );
});
