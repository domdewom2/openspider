import { writeFile } from "node:fs/promises";
import { test, expect, type TestInfo } from "@playwright/test";
import { startGridBridge } from "./bridge";
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
  // Record candidate failures without stopping the remaining gate measurements.
  const ariaRowCount = await page.locator('[role="grid"]').first().getAttribute("aria-rowcount", { timeout: 1000 }).catch(() => null);
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
  for (const axis of ["vertical", "horizontal"] as const) results.push(await page.evaluate(axis => window.gridSpike.record(axis), axis));
  await saveEvidence(info, "S-1-frames.json", results);
  for (const result of results) { expect(result.durationMs).toBeGreaterThanOrEqual(10_000); expect(result.domRows).toBeLessThan(300); }
});

// spec: UI-140
test("S-1 diagnostic control: ARIA tree without StrictMode", async ({ page }, info) => {
  await page.goto(`/src/grid/spike.html?strict=0&bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  await expect(page.locator('[role="grid"]')).toHaveAttribute("aria-rowcount", "5000001");
  await saveEvidence(info, "S-1-without-strict.json", await page.evaluate(() => window.gridSpike.snapshot()));
});

// spec: UI-140
test("S-1 arbitrary programmatic jumps (candidate evidence)", async ({ page }, info) => {
  test.setTimeout(30_000);
  await page.goto(`/src/grid/spike.html?bridge=${encodeURIComponent(bridge.url)}`);
  await page.waitForFunction(() => window.gridSpike?.ready());
  const results = [];
  for (const row of [400, 2_500_000]) results.push(await page.evaluate(row => window.gridSpike.jump(row), row));
  await saveEvidence(info, "S-1-arbitrary-jumps.json", { platform: info.project.name, results });
  expect(await page.evaluate(() => window.gridSpike.errors)).toEqual([]);
});
