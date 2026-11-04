// UI-102 performance (indication; the reference measurement runs on reference hardware, D2/D5):
// - selection → drawn chart, median of 20 selections, on a gen-ui-large crawl (OS_V01_LARGE=<path to .ospider>);
// - redraw after a live Overview update (fake bridge with fx-ui-chart), from the arrival of the answer to the DOM change.
import { test, expect, type Page } from "@playwright/test";
import { startAlphaBridge } from "../alpha/support";
import { startFakeBridge } from "../support/fake-bridge";
import fxChart from "../../../../tests/fixtures/ui/fx-ui-chart/overview.json" with { type: "json" };
import tickFixture from "../../src/ipc/__fixtures__/Tick.json" with { type: "json" };
import type { Tick } from "../../src/ipc/gen/Tick";
import { area, treeTab } from "./support";

const tick = { ...tickFixture, notices: [] };

/** Click on a tab name in the tree until the chart of that entry is drawn (next animation frame after the commit). */
async function selections(page: Page, entries: readonly string[], n: number): Promise<{ median: number; samples: number[] }> {
  const samples: number[] = [];
  for (let i = 0; i < n; i++) {
    const tab = entries[i % entries.length]!;
    samples.push(await page.evaluate(async (name) => {
      const button = [...document.querySelectorAll<HTMLButtonElement>(".os-sidebar-overview [data-ovc-entry]")].find(b => b.dataset.ovcEntry === `tab:${name}`)!;
      const start = performance.now();
      button.click();
      await new Promise<void>(resolve => {
        const check = () => document.querySelector(`[data-ovc-entry-shown="tab:${name}"] .os-ovc-body > *`) ? requestAnimationFrame(() => resolve()) : requestAnimationFrame(check);
        check();
      });
      return performance.now() - start;
    }, tab));
  }
  const sorted = [...samples].sort((a, b) => a - b);
  return { median: (sorted[Math.floor((n - 1) / 2)]! + sorted[Math.ceil((n - 1) / 2)]!) / 2, samples };
}

// spec: UI-102
test("gen-ui-large: selection to drawn chart, median of 20 selections", async ({ page }) => {
  const fixture = process.env.OS_V01_LARGE;
  test.skip(!fixture, "Benchmark: OS_V01_LARGE=<gen-ui-large .ospider> (reference measurement on the Mac mini, D2/D5)");
  test.setTimeout(30 * 60_000);
  const bridge = await startAlphaBridge();
  try {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    const opened = Date.now();
    await page.evaluate(async path => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await crawl.open({ kind: "path", path });
    }, fixture!);
    await expect(treeTab(page, "Response Codes")).toBeVisible({ timeout: 25 * 60_000 });
    const overviewMs = Date.now() - opened;
    const encountered = await page.getByRole("table", { name: "Summary" }).getByRole("row").nth(1).getByRole("cell").first().innerText();
    const result = await selections(page, ["Response Codes", "Internal", "Page Titles", "External", "Canonicals", "Images", "Directives"], 20);
    await test.info().attach("large-selection", { body: JSON.stringify({ encountered, overviewMs, ...result }), contentType: "application/json" });
    expect(result.median).toBeLessThanOrEqual(200);
  } finally {
    await bridge.stop();
  }
});

// spec: UI-102
test("fx-ui-chart: redraw after a live Overview update (answer arrived → chart changed)", async ({ page }) => {
  const bridge = await startFakeBridge();
  try {
    let answer = structuredClone(fxChart) as typeof fxChart;
    bridge.core.settings.general.language = "en";
    bridge.core.handlers.set("sidebar.overview", () => structuredClone(answer));
    bridge.core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(`/?bridge=${encodeURIComponent(bridge.url)}`);
    await expect(page.locator('[data-sidebar-ready="true"]')).toBeAttached();
    bridge.core.emit("main", { kind: "tick", tick: tick as Tick });
    await treeTab(page, "Response Codes").click();
    await expect(area(page).locator("path.os-ovc-segment")).toHaveCount(6);
    await page.evaluate(async () => {
      const { sidebar } = await import(/* @vite-ignore */ String("/src/ipc/sidebar.ts"));
      const w = window as unknown as { __v01Arrived: number[]; __v01Changed: number[] };
      w.__v01Arrived = []; w.__v01Changed = [];
      const original = sidebar.overview;
      sidebar.overview = (async (...args: Parameters<typeof original>) => { const r = await original(...args); w.__v01Arrived.push(performance.now()); return r; }) as typeof original;
      const host = document.querySelector(".os-ovc-area")!;
      new MutationObserver(() => w.__v01Changed.push(performance.now())).observe(host, { subtree: true, childList: true, attributes: true, characterData: true });
    });
    const samples: number[] = [];
    for (let i = 1; i <= 10; i++) {
      const codes = answer.tabs.find(t => t.tab === "Response Codes")!;
      codes.filters.find(f => f.id === "Success (2XX)")!.count = 812 - i;
      codes.filters.find(f => f.id === "Other")!.count = i;
      answer = structuredClone(answer);
      const before = await page.evaluate(() => (window as unknown as { __v01Changed: number[] }).__v01Changed.length);
      bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: tick.seq + 100 + i, countsVersion: 100 + i, sidebarVersion: 100 + i } as Tick });
      await expect.poll(() => page.evaluate(() => (window as unknown as { __v01Changed: number[] }).__v01Changed.length)).toBeGreaterThan(before);
      samples.push(await page.evaluate(() => {
        const w = window as unknown as { __v01Arrived: number[]; __v01Changed: number[] };
        const arrived = w.__v01Arrived.at(-1)!;
        return (w.__v01Changed.find(t => t >= arrived) ?? NaN) - arrived;
      }));
    }
    const sorted = [...samples].sort((a, b) => a - b);
    await test.info().attach("redraw-ms", { body: JSON.stringify({ median: (sorted[4]! + sorted[5]!) / 2, samples }), contentType: "application/json" });
    expect(samples.every(Number.isFinite)).toBe(true);
  } finally {
    await bridge.stop();
  }
});
