// Additive diagnostics only; U31 assertions and measurements remain unchanged.
import { test, expect, type Page } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import { genUiLarge } from "../../src/features/viz/fixtures/synthetic";
import { openViz, layoutDone } from "../viz/support";
import { startVizBridge } from "../viz/viz-bridge";
import { rendererRss } from "./rss.mjs";
import { startWebKitCapture } from "./n23-webkit";
import { processProvenance } from "./n23-provenance";

test.skip(!process.env.N23_PHASE, "Benchmark: opt-in renderer diagnostics via N23_PHASE");

async function installWidthProbe(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const stats = { count: 0, ms: 0, traceEvents: [] as { name: string; ph: string; ts: number; dur: number; pid: number; tid: number }[] };
    Object.assign(window, { n23Layout: stats });
    const property = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetWidth")!;
    Object.defineProperty(HTMLElement.prototype, "offsetWidth", { ...property, get() {
      const start = performance.now();
      try { return property.get!.call(this); }
      finally { const ms = performance.now() - start; stats.count++; stats.ms += ms; if (ms && stats.traceEvents.length < 100000) stats.traceEvents.push({ name: "HTMLElement.offsetWidth", ph: "X", ts: start * 1000, dur: ms * 1000, pid: 1, tid: 1 }); }
    } });
  });
}

// spec: UI-144
// spec: UI-146
test("N23 diagnostic: canvas costs and heap with 10000-node fixture", async ({ page, context }, info) => {
  const out = resolve("../../target/loadtest/n23", process.env.N23_PHASE ?? "before", info.project.name);
  await mkdir(out, { recursive: true });
  const bridge = await startVizBridge(new FakeVizCore(() => genUiLarge([1, 40, 2000, 7959])));
  const session = info.project.name === "chromium" ? await context.newCDPSession(page) : null;
  const events: unknown[] = [];
  if (session) {
    await session.send("HeapProfiler.enable");
    await session.send("Profiler.enable");
    await session.send("Profiler.start");
    session.on("Tracing.dataCollected", e => events.push(...e.value));
    await session.send("Tracing.start", { categories: "devtools.timeline,v8,blink,disabled-by-default-v8.cpu_profiler", transferMode: "ReportEvents" });
  }
  await page.addInitScript(() => {
    const calls: Record<string, { count: number; ms: number; maxMs: number }> = {};
    Object.assign(window, { n23Canvas: calls });
    for (const name of ["stroke", "fill", "fillText"] as const) {
      const original = CanvasRenderingContext2D.prototype[name];
      Object.defineProperty(CanvasRenderingContext2D.prototype, name, { value: function(this: CanvasRenderingContext2D, ...args: unknown[]) {
        const start = performance.now();
        try { return Reflect.apply(original, this, args); }
        finally { const ms = performance.now() - start; const stat = calls[name] ??= { count: 0, ms: 0, maxMs: 0 }; stat.count++; stat.ms += ms; stat.maxMs = Math.max(stat.maxMs, ms); }
      } });
    }
  });
  try {
    const before = rendererRss();
    await openViz(page, bridge, "crawlGraph");
    const stopWebKit = info.project.name === "webkit" ? await startWebKitCapture(page, out, "viz") : null;
    await layoutDone(page, 30000);
    expect(await page.evaluate(() => window.__osViz!.nodes)).toBe(10000);
    const opened = rendererRss();
    const provenance = processProvenance(opened);
    if (session) {
      const chunks: string[] = [];
      session.on("HeapProfiler.addHeapSnapshotChunk", e => chunks.push(e.chunk));
      await session.send("HeapProfiler.takeHeapSnapshot");
      await writeFile(resolve(out, "viz.heapsnapshot"), chunks.join(""));
      const profile = await session.send("Profiler.stop");
      await writeFile(resolve(out, "cpu.json"), JSON.stringify(profile));
      const done = new Promise<void>(resolveDone => session.once("Tracing.tracingComplete", () => resolveDone()));
      await session.send("Tracing.end");
      await done;
      await writeFile(resolve(out, "trace.json"), JSON.stringify({ traceEvents: events }));
    }
    await stopWebKit?.(true);
    const canvas = await page.evaluate(() => (window as unknown as { n23Canvas: unknown }).n23Canvas);
    await writeFile(resolve(out, "diagnostic.json"), JSON.stringify({ before, opened, provenance, canvas, webkitInspector: session ? "CDP trace + heap" : "WebKit Inspector timeline, script/CPU samples and heap via locked internal adapter", load: (await import("node:os")).loadavg() }, null, 2));
  } finally { await bridge.stop(); }
});

// spec: UI-140
// spec: UI-144
test("N23 diagnostic: scroll layout costs with real U31 fixture", async ({ page, context }, info) => {
  test.setTimeout(180000);
  const { startPerfBridge } = await import("./bridge");
  const bridge = await startPerfBridge();
  const out = resolve("../../target/loadtest/n23", process.env.N23_PHASE ?? "before", info.project.name);
  await mkdir(out, { recursive: true });
  await installWidthProbe(page);
  try {
    const fixture = resolve(process.env.OS_PERF_FIXTURE ?? "../../target/ui-perf.ospider");
    await page.goto(`/e2e/perf/index.html?bridge=${encodeURIComponent(bridge.url)}&fixture=${encodeURIComponent(fixture)}`);
    await page.waitForFunction(() => window.uiPerf?.ready(), undefined, { timeout: 120000 });
    const session = info.project.name === "chromium" ? await context.newCDPSession(page) : null;
    const events: unknown[] = [];
    if (session) {
      session.on("Tracing.dataCollected", e => events.push(...e.value));
      await session.send("Tracing.start", { categories: "devtools.timeline,v8,blink", transferMode: "ReportEvents" });
    }
    const stopWebKit = info.project.name === "webkit" ? await startWebKitCapture(page, out, "scroll") : null;
    const before = rendererRss();
    const provenance = processProvenance(before);
    const vertical = await page.evaluate(() => window.uiPerf.record("vertical"));
    const horizontal = await page.evaluate(() => window.uiPerf.record("horizontal"));
    const layout = await page.evaluate(() => (window as unknown as { n23Layout: unknown }).n23Layout);
    if (session) {
      const done = new Promise<void>(resolveDone => session.once("Tracing.tracingComplete", () => resolveDone()));
      await session.send("Tracing.end"); await done;
      await writeFile(resolve(out, "scroll-trace.json"), JSON.stringify({ traceEvents: events }));
      await session.send("HeapProfiler.enable");
      const chunks: string[] = [];
      session.on("HeapProfiler.addHeapSnapshotChunk", e => chunks.push(e.chunk));
      await session.send("HeapProfiler.takeHeapSnapshot");
      await writeFile(resolve(out, "grid.heapsnapshot"), chunks.join(""));
    }
    await stopWebKit?.(true);
    await writeFile(resolve(out, "scroll.json"), JSON.stringify({ vertical, horizontal, layout, provenance, before, after: rendererRss(), load: (await import("node:os")).loadavg() }, null, 2));
  } finally { await bridge.stop(); }
});

// spec: UI-143
test("N23 diagnostic: renderer trace under crawl load", async ({ page, context }, info) => {
  const { createServer } = await import("node:http");
  const { startPerfBridge } = await import("./bridge");
  const bridge = await startPerfBridge();
  const pending: (() => void)[] = [];
  const timer = setInterval(() => pending.shift()?.(), 1000 / 220);
  const site = createServer((req, res) => {
    if (req.url === "/robots.txt") { res.end("User-agent: *\nAllow: /\n"); return; }
    const n = Number(req.url?.slice(1)) || 0;
    pending.push(() => { res.setHeader("Content-Type", "text/html"); res.end(`<html><head><title>Load ${n}</title></head><body>${Array.from({ length: 30 }, (_, i) => `<a href="/${(n * 17 + i + 1) % 200000}">page</a>`).join("")}</body></html>`); });
  });
  await new Promise<void>(resolveSite => site.listen(0, "127.0.0.1", resolveSite));
  const address = site.address();
  if (!address || typeof address === "string") throw new Error("fixture port missing");
  const out = resolve("../../target/loadtest/n23", process.env.N23_PHASE ?? "before", info.project.name);
  await mkdir(out, { recursive: true });
  const session = info.project.name === "chromium" ? await context.newCDPSession(page) : null;
  const events: unknown[] = [];
  await installWidthProbe(page);
  try {
    await page.goto(`/e2e/perf/shell.html?bridge=${encodeURIComponent(bridge.url)}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(base => window.perfShell.start(base), `http://127.0.0.1:${address.port}/`);
    await page.waitForFunction(() => (window.perfShell.state()?.crawled ?? 0) > 1000);
    if (session) {
      session.on("Tracing.dataCollected", e => events.push(...e.value));
      await session.send("Tracing.start", { categories: "devtools.timeline,v8,blink,disabled-by-default-v8.cpu_profiler", transferMode: "ReportEvents" });
    }
    const stopWebKit = info.project.name === "webkit" ? await startWebKitCapture(page, out, "crawl") : null;
    const before = rendererRss();
    const provenance = processProvenance(before);
    await page.waitForTimeout(15000);
    const after = rendererRss();
    if (session) {
      const done = new Promise<void>(resolveDone => session.once("Tracing.tracingComplete", () => resolveDone()));
      await session.send("Tracing.end"); await done;
      await writeFile(resolve(out, "crawl-trace.json"), JSON.stringify({ traceEvents: events }));
    }
    await stopWebKit?.();
    await writeFile(resolve(out, "crawl.json"), JSON.stringify({ before, after, provenance, state: await page.evaluate(() => window.perfShell.state()), layout: await page.evaluate(() => (window as unknown as { n23Layout: unknown }).n23Layout), load: (await import("node:os")).loadavg() }, null, 2));
  } finally {
    await page.evaluate(() => window.perfShell?.stop()).catch(() => {});
    clearInterval(timer); site.closeAllConnections();
    await new Promise<void>(resolveSite => site.close(() => resolveSite()));
    await bridge.stop();
  }
});
