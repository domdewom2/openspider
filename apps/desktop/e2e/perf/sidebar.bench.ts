import { test, expect } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { performance as clock } from "node:perf_hooks";
import { startPerfBridge } from "./bridge";

// Each command gets its own bridge; a slow overview must not hide the other budgets.
for (const [name, budget] of [
  ["overview", 100],
  ["issues", 100],
  ["siteStructure", 1000],
  ["responseTimes", 1000],
] as const) {
  // spec: UI-145
  test(`sidebar ${name}: independent RPC latency`, async ({ page }, info) => {
    test.setTimeout(45_000);
    const bridge = await startPerfBridge();
    let started: number | undefined;
    const result = {
      scenario: `sidebar-${name}`,
      engine: info.project.name,
      size: process.env.OS_PERF_ROWS === "5000000" ? "large" : "small",
      started_at: new Date().toISOString(),
      metrics: {} as Record<
        string,
        { value: number; budget: number; passed: boolean; completed: boolean }
      >,
      details: { error: "", scope: "Core RPC, excluding UI paint" },
      passed: false,
    };
    try {
      const fixture = resolve(
        process.env.OS_PERF_FIXTURE ?? "../../target/ui-perf.ospider",
      );
      await page.goto(
        `/e2e/perf/index.html?${new URLSearchParams({ bridge: bridge.url, fixture })}`,
      );
      await page.waitForFunction(() => window.uiPerf?.ready(), undefined, {
        timeout: 30_000,
      });
      started = clock.now();
      const value = await page.evaluate(
        (name) => window.uiPerf.rpcLatency(name),
        name,
      );
      result.metrics.ms = {
        value,
        budget,
        completed: true,
        passed: value <= budget,
      };
      result.passed = value <= budget;
    } catch (error) {
      result.details.error = String(error);
      if (started !== undefined)
        result.metrics.ms = {
          value: clock.now() - started,
          budget,
          completed: false,
          passed: false,
        };
    } finally {
      await mkdir(resolve("../../target/loadtest/raw"), { recursive: true });
      await writeFile(
        resolve(
          `../../target/loadtest/raw/${info.project.name}-sidebar-${name}.json`,
        ),
        JSON.stringify(result, null, 2),
      );
      await bridge.stop();
    }
    expect(result.passed, JSON.stringify(result)).toBe(true);
  });
}
