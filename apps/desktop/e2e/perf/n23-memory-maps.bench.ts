// Additive Linux diagnosis only: no budget assertions or U31 sampler changes.
import { test, expect, type Page } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { loadavg } from "node:os";
import { performance as clock } from "node:perf_hooks";
import { resolve } from "node:path";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import { genUiLarge } from "../../src/features/viz/fixtures/synthetic";
import { openViz, layoutDone } from "../viz/support";
import { startVizBridge } from "../viz/viz-bridge";
import { rendererRss } from "./rss.mjs";
import { processMemoryMappings, processProvenance } from "./n23-provenance";

test.skip(process.platform !== "linux" || !process.env.N23_PHASE, "Benchmark: opt-in Linux mapping diagnosis");

// spec: UI-144
// spec: UI-146
test("N23 diagnostic: native visualization mappings before opening and after closing", async ({ page, context }, info) => {
  const bridge = await startVizBridge(new FakeVizCore(() => genUiLarge([1, 40, 2000, 7959])));
  const capture = () => {
    const sampleStartedMs = clock.now();
    const rss = rendererRss();
    const memoryMaps = processMemoryMappings(rss);
    expect(Array.isArray(memoryMaps) && memoryMaps.some(m => "rssBytes" in m && typeof m.rssBytes === "number" && m.rssBytes > 0)).toBe(true);
    return { rss, provenance: processProvenance(rss), memoryMaps, load: loadavg(), sampleStartedMs, sampleFinishedMs: clock.now() };
  };
  let graph: Page | undefined;
  try {
    await page.goto("about:blank");
    await page.setContent("<p>UI baseline</p>");
    const before = capture();
    graph = await context.newPage();
    await openViz(graph, bridge, "crawlGraph");
    await layoutDone(graph, 30000);
    expect(await graph.evaluate(() => window.__osViz!.nodes)).toBe(10000);
    const opened = capture();
    await graph.close();
    const closedAtMs = clock.now();
    await page.waitForTimeout(2000);
    const closed = capture();
    await page.waitForTimeout(8000);
    const closedAfterTenSeconds = capture();
    await page.waitForTimeout(20000);
    const closedAfterThirtySeconds = capture();
    const out = resolve("../../target/loadtest/n23", process.env.N23_PHASE!, info.project.name);
    await mkdir(out, { recursive: true });
    await writeFile(resolve(out, "native-memory.json"), JSON.stringify({ before, opened, closed, closedAfterTenSeconds, closedAfterThirtySeconds, closedAtMs, scope: "Diagnostic smaps RSS/PSS includes all unchanged U31 sampler PIDs; snapshots taken sequentially. U31 closing measurement remains unchanged at two seconds." }, null, 2));
  } finally {
    if (graph && !graph.isClosed()) await graph.close();
    await bridge.stop();
  }
});
