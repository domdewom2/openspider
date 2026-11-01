import { test, expect, type Page } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import { genUiLarge } from "../../src/features/viz/fixtures/synthetic";
import { openViz, layoutDone } from "../viz/support";
import { startVizBridge } from "../viz/viz-bridge";
import { rendererRss } from "./rss.mjs";

// spec: UI-144
test("visualization memory: 10000 nodes add at most 300 MB and closing releases RSS", async ({
  page,
  context,
}, info) => {
  const startedAt = new Date().toISOString();
  const bridge = await startVizBridge(
    new FakeVizCore(() => genUiLarge([1, 40, 2000, 7959])),
  );
  let graph: Page | undefined;
  try {
    await page.goto("about:blank");
    await page.setContent("<p>UI baseline</p>");
    const before = rendererRss();
    graph = await context.newPage();
    await openViz(graph, bridge, "crawlGraph");
    await layoutDone(graph, 30000);
    expect(await graph.evaluate(() => window.__osViz!.nodes)).toBe(10000);
    const opened = rendererRss();
    await graph.close();
    await page.waitForTimeout(2000);
    const closed = rendererRss();
    const additional = opened.bytes - before.bytes;
    const freed = opened.bytes - closed.bytes;
    const result = {
      scenario: "viz-memory",
      engine: info.project.name,
      size: process.env.OS_PERF_ROWS === "5000000" ? "large" : "small",
      started_at: startedAt,
      metrics: {
        additionalRssBytes: {
          value: additional,
          budget: 300_000_000,
          direction: "max",
          passed: additional <= 300_000_000,
        },
        freedBytes: {
          value: freed,
          budget: 1,
          direction: "min",
          passed: freed > 0,
        },
      },
      details: {
        before,
        opened,
        closed,
        nodes: 10000,
        scope:
          "E2E-A browser indication; another blank page remains alive for attributable baseline",
      },
      passed: additional <= 300_000_000 && freed > 0,
    };
    await mkdir(resolve("../../target/loadtest/raw"), { recursive: true });
    await writeFile(
      resolve(`../../target/loadtest/raw/${info.project.name}-viz-memory.json`),
      JSON.stringify(result, null, 2),
    );
    await info.attach("viz-memory", {
      body: JSON.stringify(result),
      contentType: "application/json",
    });
    expect(result.passed, JSON.stringify(result.metrics)).toBe(true);
  } finally {
    if (graph && !graph.isClosed()) await graph.close();
    await bridge.stop();
  }
});
