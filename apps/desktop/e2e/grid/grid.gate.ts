import { writeFile } from "node:fs/promises";
import { test, expect, type TestInfo } from "@playwright/test";
import { startGridBridge } from "./bridge";
import AxeBuilder from "@axe-core/playwright";
import type { Bridge } from "../support/bridge";

async function saveEvidence(info: TestInfo, name: string, data: unknown) {
  const path = info.outputPath(name);
  await writeFile(path, JSON.stringify(data, null, 2));
  await info.attach(name, { path, contentType: "application/json" });
}
let bridge: Bridge;
test.beforeAll(async () => { bridge = await startGridBridge(); });
test.afterAll(async () => { await bridge?.stop(); });
// spec: UI-140
// spec: UI-007
// spec: UI-012
test("gen-ui-large: last row, bounded DOM, React 19, keyboard and copy delegation", async ({ page }, info) => {
  const warnings: string[] = [];
  page.on("console", message => { if (["warning", "error"].includes(message.type())) warnings.push(message.text()); });
  page.on("pageerror", error => warnings.push(error.message));
  await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  // The fallback must satisfy the functional gate, including StrictMode.
  const ariaRowCount = await page.locator('[role="grid"]').first().getAttribute("aria-rowcount", { timeout: 1000 }).catch(() => null);
  expect(ariaRowCount).toBe("5000001");
  // U35: the standalone entry needs the same physical tokens as the app shell.
  await expect(page.locator(".os-grid-a11y")).toHaveCSS("width", "1px");
  await expect(page.locator(".os-grid-a11y")).toHaveCSS("height", "1px");
  const a11y = await new AxeBuilder({ page }).include(".os-data-grid").analyze();
  expect(a11y.violations).toEqual([]);
  const jump = await page.evaluate(() => window.gridSpike.jump(4_999_999));
  expect(jump.last).toBe(5_000_000);
  expect(jump.visible!.y + jump.visible!.height).toBeGreaterThanOrEqual(5_000_000);
  expect(jump.domRows).toBeLessThan(300);
  expect(jump.cachedRows).toBeLessThanOrEqual(20_000);
  const bounds = await page.evaluate(() => window.gridSpike.bounds(1, 4_999_999));
  expect(bounds).toBeDefined();
  await page.mouse.click(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().focus)).toBe(5_000_000);
  await page.keyboard.press("Home");
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().focus)).toBe(1);
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().selection)).toEqual({ kind: "ids", ids: [1, 2] });
  await page.keyboard.press("ControlOrMeta+a");
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().selection.kind)).toBe("all");
  await page.keyboard.press("ControlOrMeta+c");
  await expect.poll(() => page.evaluate(() => window.gridSpike.failures)).toEqual(["export-required"]);
  expect(await page.evaluate(() => window.gridSpike.errors)).toEqual([]);
  expect(warnings).toEqual([]);
  await saveEvidence(info, "S-1-functional.json", { platform: info.project.name, jump, warnings, ariaRowCount, checks: { ariaTree: ariaRowCount === "5000001", lastRow: jump.last === 5_000_000, jumpBudget: jump.contentMs <= 150, reactWarnings: warnings.length === 0 } });
});

for (const dpr of [1, 2, 3]) {
  // spec: UI-140
  test(`S-2 evidence: zoom matrix at DPR ${dpr}`, async ({ browser }, info) => {
    const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: dpr });
    const page = await context.newPage();
    try {
      await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
      await page.waitForFunction(() => window.gridSpike?.ready());
      await page.evaluate(() => window.gridSpike.jump(400));
      const evidence = [];
      for (const zoom of [0.8, 1, 1.25, 1.5, 2]) {
        const before = await page.evaluate(() => window.gridSpike.snapshot());
        const after = await page.evaluate(factor => window.gridSpike.zoom(factor), zoom);
        const row = after.visible!.y + 2;
        const bounds = await page.evaluate(row => window.gridSpike.bounds(1, row), row);
        await page.mouse.click(bounds!.x + bounds!.width / 2, bounds!.y + bounds!.height / 2);
        await page.waitForTimeout(100);
        const clicked = await page.evaluate(() => window.gridSpike.snapshot());
        evidence.push({ zoom, dpr, before, after, clicked, checks: {
          backingStore: after.canvas.every(c => Math.abs(c.widthError) <= 1 && Math.abs(c.heightError) <= 1),
          cellWidth: Math.abs(after.cellWidth! - 320 * zoom) <= 1,
          anchor: before.visible!.y === after.visible!.y,
          header: after.bounds!.y + 1 >= after.headerBounds!.y + after.headerBounds!.height,
          selection: JSON.stringify(before.selection) === JSON.stringify(after.selection),
          hit: clicked.focus === row + 1,
        } });
        expect(after.canvas.length).toBeGreaterThan(0);
        expect(evidence.at(-1)!.checks, JSON.stringify({ zoom, before: before.visible, after: after.visible })).toEqual({ backingStore: true, cellWidth: true, anchor: true, header: true, selection: true, hit: true });
        await info.attach(`zoom-${zoom}.png`, { path: await (async () => { const path = info.outputPath(`zoom-${zoom}.png`); await page.screenshot({ path }); return path; })(), contentType: "image/png" });
      }
      await saveEvidence(info, `S-2-dpr-${dpr}.json`, { platform: info.project.name, evidence });
      expect(await page.evaluate(() => window.gridSpike.errors)).toEqual([]);
    } finally { await context.close(); }
  });
}
// spec: UI-140
test("S-1 frame recorder (browser indication only)", async ({ page }, info) => {
  test.setTimeout(60_000);
  await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  const results = [];
  for (const axis of ["vertical", "horizontal"] as const) {
    const result = await page.evaluate(axis => window.gridSpike.record(axis), axis);
    results.push({ ...result, checks: { fps: result.fps >= 55, maxFrame: result.maxFrameMs <= 50 } });
  }
  await saveEvidence(info, "S-1-frames.json", results);
  for (const result of results) {
    expect(result.durationMs).toBeGreaterThanOrEqual(10_000); expect(result.domRows).toBeLessThan(300);
    expect(result.cachedRows).toBeLessThanOrEqual(20_000);
    // Enable budgets for a quiet reference-hardware run; CI records indications.
    if (process.env.OS_GRID_STRICT_BUDGETS === "1") expect(result.checks).toEqual({ fps: true, maxFrame: true });
  }
});

// spec: UI-140
test("S-1 diagnostic control: ARIA tree without StrictMode", async ({ page }, info) => {
  await page.goto(`/src/grid/spike.html?strict=0&bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  await expect(page.locator('[role="grid"]')).toHaveAttribute("aria-rowcount", "5000001");
  await saveEvidence(info, "S-1-without-strict.json", await page.evaluate(() => window.gridSpike.snapshot()));
});

// spec: UI-140
test("S-1 arbitrary programmatic jumps", async ({ page }, info) => {
  test.setTimeout(30_000);
  await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  const results = [];
  for (const row of [400, 2_500_000]) results.push(await page.evaluate(row => window.gridSpike.jump(row), row));
  await saveEvidence(info, "S-1-arbitrary-jumps.json", { platform: info.project.name, results });
  for (const result of results) { expect(result.requestedVisible).toBe(true); expect(result.contentReady).toBe(true); expect(result.visible!.y).toBe(result.requestedRow); }
  expect(await page.evaluate(() => window.gridSpike.errors)).toEqual([]);
});

// spec: UI-007
// spec: UI-012
// spec: UI-140
test("native scrollbar, wheel, horizontal virtualization and controlled column gestures", async ({ page }) => {
  await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  await page.evaluate(() => window.gridSpike.jump(400));
  const grid = page.getByRole("grid");
  const canvas = grid.locator("canvas");
  const box = (await grid.boundingBox())!;
  // Real thumb drag, not only assigning scrollTop: the canvas cannot cover the rail.
  await page.mouse.move(box.x + box.width - 7, box.y + 5);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width - 7, box.y + box.height - 20, { steps: 10 });
  await page.mouse.up();
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().last)).toBe(5_000_000);
  await page.evaluate(() => window.gridSpike.jump(400));
  await canvas.hover({ position: { x: 500, y: 200 } });
  await page.mouse.wheel(0, 130);
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().visible!.y)).toBe(405);
  const expected = await page.locator(".os-grid-scroll").evaluate(e => {
    e.scrollTop = (e.scrollHeight - e.clientHeight) / 2;
    // Native scrollTop can round a fractional midpoint to a physical pixel.
    const fraction = e.scrollTop / (e.scrollHeight - e.clientHeight);
    return Math.round((5_000_000 - Math.floor((Number.parseFloat(e.parentElement!.querySelector("canvas")!.style.height) - 30) / 26)) * fraction);
  });
  await expect.poll(() => page.evaluate(() => window.gridSpike.snapshot().visible!.y)).toBe(expected);
  await page.evaluate(() => window.gridSpike.jump(400));
  const address = await page.evaluate(() => window.gridSpike.bounds(1, 400));
  await page.locator(".os-grid-scroll").evaluate(e => { e.scrollLeft = e.scrollWidth; });
  await expect.poll(() => grid.getByRole("columnheader").last().getAttribute("aria-colindex")).toBe("65");
  expect(await page.evaluate(() => window.gridSpike.bounds(1, 400))).toEqual(address);
  expect(await grid.getByRole("columnheader").count()).toBeLessThan(15);
  await page.locator(".os-grid-scroll").evaluate(e => { e.scrollLeft = 0; });
  await expect.poll(() => grid.getByRole("columnheader").nth(2).getAttribute("aria-colindex")).toBe("3");
  const before = await page.evaluate(() => window.gridSpike.bounds(2, -1));
  await page.mouse.move(before!.x + before!.width - 2, before!.y + before!.height / 2);
  await page.mouse.down(); await page.mouse.move(before!.x + before!.width + 48, before!.y + 12, { steps: 5 }); await page.mouse.up();
  await expect.poll(() => page.evaluate(() => window.gridSpike.bounds(2, -1)!.width)).toBe(before!.width + 50);
  const resized = await page.evaluate(() => window.gridSpike.bounds(2, -1));
  await page.mouse.dblclick(resized!.x + resized!.width - 2, resized!.y + resized!.height / 2);
  await expect.poll(() => page.evaluate(() => window.gridSpike.bounds(2, -1)!.width)).not.toBe(resized!.width);
  const headers = await grid.getByRole("columnheader").allTextContents();
  const from = await page.evaluate(() => window.gridSpike.bounds(2, -1));
  const to = await page.evaluate(() => window.gridSpike.bounds(3, -1));
  await page.mouse.move(from!.x + 20, from!.y + 12); await page.mouse.down();
  await page.mouse.move(to!.x + 20, to!.y + 12, { steps: 5 }); await page.mouse.up();
  await expect(grid.getByRole("columnheader").nth(2)).toHaveText(headers[3]);
  await expect(grid.getByRole("columnheader").nth(3)).toHaveText(headers[2]);
  expect(await page.evaluate(() => window.gridSpike.errors)).toEqual([]);
});
