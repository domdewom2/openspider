// UI-102 E2E over the real WebSocket IPC with the fake bridge: the core answers `sidebar.overview` with fx-ui-chart.
import { test, expect, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { startFakeBridge, type FakeBridge } from "../support/fake-bridge";
import fxChart from "../../../../tests/fixtures/ui/fx-ui-chart/overview.json" with { type: "json" };
import fxUnassigned from "../../../../tests/fixtures/ui/fx-ui-chart/overview-unassigned.json" with { type: "json" };
import fxOverflow from "../../../../tests/fixtures/ui/fx-ui-chart/overview-overflow.json" with { type: "json" };
import tickFixture from "../../src/ipc/__fixtures__/Tick.json" with { type: "json" };
import type { Tick } from "../../src/ipc/gen/Tick";
import { area, BACKGROUND, chartItem, overlaps, pngSize, readDownload, segment, settleUiState, treeTab, validateSvg } from "./support";

const tick = { ...tickFixture, notices: [] };
let bridge: FakeBridge;
let overview: unknown = fxChart;

async function open(page: Page, summary = true) {
  await page.goto(`/?bridge=${encodeURIComponent(bridge.url)}`);
  await expect(page.locator('[data-sidebar-ready="true"]')).toBeAttached();
  bridge.core.emit("main", { kind: "tick", tick: tick as Tick });
  if (summary) await expect(page.getByRole("table", { name: "Summary" })).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.clock.setFixedTime(new Date("2026-11-03T10:00:00Z"));
  bridge = await startFakeBridge();
  overview = fxChart;
  bridge.core.settings.general.language = "en";
  bridge.core.handlers.set("sidebar.overview", () => structuredClone(overview));
  bridge.core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  bridge.core.handlers.set("analysis.start", () => ({ opId: "analysis-1", labelKey: "analysis", cancellable: true }));
});
test.afterEach(async () => { await bridge.stop(); });

const navigation = (page: Page) => page.evaluate(async () => {
  const { useSidebarNavigation } = await import(/* @vite-ignore */ String("/src/features/sidebar/frame/state.ts"));
  return useSidebarNavigation.getState().target?.spec ?? null;
});
const ipc = () => bridge.core.calls.map(c => c.cmd);

// spec: UI-102
test("ring, bars, summary and overlap from fx-ui-chart; selecting entries calls no command", async ({ page }) => {
  await open(page);
  await expect(area(page)).toContainText("Select an entry in the overview to see its breakdown.");
  // The shell persists its startup layout (UI-001/UI-125) one second after loading; that save is not caused by a
  // selection, so it is written before the window opens. A save caused by a selection is flushed before it closes.
  await settleUiState(page);
  const before = ipc();
  await treeTab(page, "Response Codes").click();
  const ring = area(page).getByRole("img", { name: "Response Codes: breakdown of 1,000 URLs" });
  await expect(ring).toBeVisible();
  await expect(ring.locator("path.os-ovc-segment")).toHaveCount(6);
  await expect(ring.locator("text")).toHaveText(["1,000", "URLs"]);
  await expect(area(page).locator(".os-ovc-legend button")).toHaveText([
    /^Success \(2XX\)812\s*81\.2 %$/, /^Redirection \(3XX\)96\s*9\.6 %$/, /^Client Error \(4XX\)61\s*6\.1 %$/,
    /^Blocked by Robots\.txt17\s*1\.7 %$/, /^Server Error \(5XX\)9\s*0\.9 %$/, /^No Response5\s*0\.5 %$/, /^Other0\s*0\.0 %$/,
  ]);
  await expect(area(page)).not.toContainText("Redirect Chain");
  await treeTab(page, "Internal").click();
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(10);
  await expect(chartItem(page, "others")).toContainText("16");
  await page.getByRole("button", { name: "Summary", exact: true }).click();
  await expect(area(page).getByRole("img", { name: "Summary: breakdown of 2,500 URLs" }).locator("path.os-ovc-segment")).toHaveCount(5);
  await treeTab(page, "Page Titles").click();
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(5);
  await expect(area(page)).toContainText("A URL can be in several categories – the shares do not add up to 100 %.");
  await expect(area(page).getByRole("button", { name: /1 filter without matches/ })).toBeVisible();
  await settleUiState(page);
  expect(ipc().slice(before.length).filter(c => !BACKGROUND.test(c)), "selecting entries must not call the core").toEqual([]);
});

// spec: UI-102
// spec: UI-061 [M3]
test("a filter row highlights its category and navigates the table; segments, legend and „Remaining“ are click targets", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "Client Error (4XX)", exact: true }).click();
  await expect(chartItem(page, "f:Client Error (4XX)")).toHaveAttribute("data-highlight", "true");
  await expect(chartItem(page, "f:Client Error (4XX)")).toHaveCSS("font-weight", "700");
  await expect(area(page).locator('.os-ovc-mark[data-kind="highlight"]')).toHaveCount(1);
  expect(await navigation(page)).toMatchObject({ tab: "Response Codes", filter: "Client Error (4XX)" });
  await page.getByRole("button", { name: "Summary", exact: true }).click();
  await segment(page, "nonIndexable").dispatchEvent("click");
  expect(await navigation(page)).toMatchObject({ tab: "Internal", filter: "HTML", search: { kind: "rules", tree: { children: [{ column: "Indexability", op: "equals", value: "Non-Indexable" }] } } });
  await treeTab(page, "Response Codes").click();
  await segment(page, "f:Success (2XX)").dispatchEvent("click");
  expect(await navigation(page)).toMatchObject({ tab: "Response Codes", filter: "Success (2XX)" });
  // Enter on the legend entry applies the filter.
  await chartItem(page, "f:Client Error (4XX)").focus();
  await page.keyboard.press("Enter");
  expect(await navigation(page)).toMatchObject({ tab: "Response Codes", filter: "Client Error (4XX)" });
  await treeTab(page, "Internal").click();
  await chartItem(page, "others").click();
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(12);
  await expect(chartItem(page, "f:XML")).toBeVisible();
  await expect(chartItem(page, "f:Flash")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(10);
  // UI-061: the arrow still collapses the block, the name only selects it.
  await page.getByRole("button", { name: "Collapse Internal", exact: true }).click();
  await expect(page.getByRole("table", { name: "Internal", exact: true })).toHaveCount(0);
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(10);
});

// spec: UI-102
test("residual category, error case and blocks without counters", async ({ page }) => {
  overview = fxUnassigned;
  await open(page);
  await treeTab(page, "Response Codes").click();
  await expect(area(page).locator("path.os-ovc-segment")).toHaveCount(7);
  await chartItem(page, "unassigned").hover();
  await expect(area(page).getByRole("tooltip")).toContainText("URLs of this entry that no category contains.");
  await chartItem(page, "unassigned").click();
  expect(await navigation(page)).toBeNull();
  overview = fxOverflow;
  bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: tick.seq + 10, countsVersion: 50, sidebarVersion: 50 } as Tick });
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(6);
  await expect(area(page)).toContainText("The categories add up to more than the base");
  await expect.poll(() => bridge.core.calls.filter(c => c.cmd === "diag.logFrontend").map(c => (c.args as { level: string }).level)).toContain("warn");
  await treeTab(page, "Sitemaps").click();
  await expect(area(page)).toContainText("The counts of this entry are not available yet.");
  await area(page).getByRole("button", { name: "Start crawl analysis" }).click();
  await expect.poll(() => bridge.core.calls.some(c => c.cmd === "analysis.start")).toBe(true);
  await treeTab(page, "Canonicals").click();
  await expect(chartItem(page, "f:Unlinked")).toContainText("–");
  await expect(area(page).locator('.os-ovc-bars [data-ovc-key="f:Unlinked"]')).toHaveCount(0);
});

// spec: UI-102
test("exports: SVG 1.1 with texts and a hatch pattern for 5XX, PNG 2× doubles 1×, CSV and copied rows of Internal", async ({ page, context, browserName }) => {
  await open(page);
  await treeTab(page, "Response Codes").click();
  const exportMenu = area(page).getByRole("button", { name: "Export ▾" });
  const save = async (item: string) => {
    const download = page.waitForEvent("download");
    await exportMenu.click();
    await page.getByRole("menuitem", { name: item }).click();
    return download;
  };
  const svgDownload = await save("Save as SVG");
  expect(svgDownload.suggestedFilename()).toBe("overview-response-codes-2026-11-03.svg");
  const svg = await validateSvg(page, (await readDownload(svgDownload)).toString("utf8"));
  expect(svg.texts).toEqual(expect.arrayContaining(["Client Error (4XX)", "Server Error (5XX)", "61 (6.1 %)", "9 (0.9 %)", "1,000"]));
  const fills = svg.segments.map(s => s.fill);
  const client = fills[2]!;
  const server = fills[4]!;
  expect(client).toMatch(/^#[0-9a-f]{6}$/);
  expect(server).toMatch(/^url\(#hatch-\d+\)$/);
  expect(svg.patterns[server.slice(5, -1)]).toBe(client);
  const png1 = pngSize(await readDownload(await save("Save as PNG (1×)")));
  const png2 = pngSize(await readDownload(await save("Save as PNG (2×)")));
  expect([png2.width, png2.height]).toEqual([png1.width * 2, png1.height * 2]);
  await treeTab(page, "Internal").click();
  const csvDownload = await save("Save data as CSV");
  const csv = (await readDownload(csvDownload)).toString("utf8");
  const lines = csv.replace(/^﻿/, "").split("\r\n").filter(Boolean);
  expect(lines[0]).toBe("Entry,Category,Filter,URLs,% of Total,Base");
  expect(lines).toHaveLength(12);
  expect(lines.slice(1).every(l => l.startsWith("Internal,") && l.endsWith(",2000"))).toBe(true);
  // Values of the table view.
  await area(page).getByRole("button", { name: "Table" }).click();
  const table = area(page).getByRole("table");
  await expect(table.locator("tbody tr")).toHaveCount(11);
  await expect(table.locator("tbody tr").first()).toHaveText(/^HTML1,20060\.0 %$/);
  expect(lines[1]).toBe("Internal,HTML,HTML,1200,60.0,2000");
  expect(lines.slice(1).every(l => /,\d+\.\d,2000$/.test(l))).toBe(true);
  if (browserName === "chromium") {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    await exportMenu.click();
    await page.getByRole("menuitem", { name: "Copy data" }).click();
    const copied = await page.evaluate(() => navigator.clipboard.readText());
    // UI-102: „Daten kopieren“ gives the same rows as the CSV (header + 11 rows), tab separated. The app writes LF;
    // the Windows clipboard stores text with CRLF, so lines are split platform-neutrally.
    const rows = copied.replace(/\r?\n$/, "").split(/\r?\n/);
    expect(rows[0]).toBe("Entry\tCategory\tFilter\tURLs\t% of Total\tBase");
    expect(rows).toEqual(lines.map(l => l.split(",").join("\t")));
  }
});

// spec: UI-102
// spec: UI-126
test("accessibility: ring, bars and table without critical/serious axe findings; keyboard focus group", async ({ page }) => {
  await open(page);
  await treeTab(page, "Response Codes").click();
  const ring = area(page).getByRole("img", { name: "Response Codes: breakdown of 1,000 URLs" });
  await expect(ring).toBeVisible();
  const scan = async (what: string) => {
    const result = await new AxeBuilder({ page }).include('[aria-label="Chart of the selected entry"]').analyze();
    const severe = result.violations.filter(v => v.impact === "critical" || v.impact === "serious");
    expect(severe.map(v => `${what} ${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
  };
  await scan("ring");
  // Tab reaches the view switch, the export menu and the chart as one stop.
  await area(page).getByRole("button", { name: "Chart" }).focus();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Tab");
  await expect(area(page).getByRole("button", { name: "Export ▾" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(chartItem(page, "f:Success (2XX)")).toBeFocused();
  await expect(area(page).getByRole("tooltip")).toContainText("of 1,000 URLs");
  await page.keyboard.press("ArrowDown");
  await expect(chartItem(page, "f:Redirection (3XX)")).toBeFocused();
  await page.keyboard.press("End");
  await expect(chartItem(page, "f:Other")).toBeFocused();
  await page.keyboard.press("Home");
  await expect(chartItem(page, "f:Success (2XX)")).toBeFocused();
  await expect(chartItem(page, "f:Success (2XX)")).toHaveCSS("outline-style", "solid");
  await page.keyboard.press("Escape");
  await expect(treeTab(page, "Response Codes")).toBeFocused();
  await treeTab(page, "Internal").click();
  await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(10);
  await scan("bars");
  await area(page).getByRole("button", { name: "Table" }).click();
  await expect(area(page).getByRole("table")).toBeVisible();
  await scan("table");
  // No live region in the chart area: live updates make no announcements.
  await expect(area(page).locator("[aria-live], [role=status], [role=alert]")).toHaveCount(0);
});

// spec: UI-102
test("layout 1440×900: 40 % of the tab, collapse under 70 px, grip restores, height and table view survive a restart", async ({ page }) => {
  await open(page);
  const panel = page.locator("#sidebar-panel");
  const pane = page.locator(".os-ovc-pane");
  const content = (await panel.boundingBox())!.height;
  const height = (await pane.boundingBox())!.height;
  expect(Math.abs(height - Math.max(180, content * 0.4))).toBeLessThanOrEqual(2);
  const grip = page.getByRole("separator", { name: "Resize chart area" });
  const drag = async (to: number) => {
    const g = (await grip.boundingBox())!;
    const bottom = (await panel.boundingBox())!;
    await page.mouse.move(g.x + g.width / 2, g.y + g.height / 2);
    await page.mouse.down();
    await page.mouse.move(g.x + g.width / 2, bottom.y + bottom.height - to - g.height / 2, { steps: 4 });
    await page.mouse.up();
  };
  await drag(300);
  expect(Math.abs((await pane.boundingBox())!.height - 300)).toBeLessThanOrEqual(2);
  await drag(50);
  await expect(pane).toBeHidden();
  expect((await grip.boundingBox())!.height).toBe(6);
  await grip.click();
  await expect(pane).toBeVisible();
  expect(Math.abs((await pane.boundingBox())!.height - 300)).toBeLessThanOrEqual(2);
  await treeTab(page, "Internal").click();
  await area(page).getByRole("button", { name: "Table" }).click();
  await expect.poll(() => (bridge.core.uiState.state as Record<string, { view?: string }>).overviewChart?.view, { timeout: 5_000 }).toBe("table");
  await page.reload();
  await expect(page.locator('[data-sidebar-ready="true"]')).toBeVisible();
  let seq = tick.seq + 100;
  await expect(async () => {
    bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: ++seq } as Tick });
    await expect(page.getByRole("table", { name: "Summary" })).toBeVisible({ timeout: 1000 });
  }).toPass();
  expect(Math.abs((await pane.boundingBox())!.height - 300)).toBeLessThanOrEqual(2);
  await treeTab(page, "Internal").click();
  await expect(area(page).getByRole("table")).toBeVisible();
});

// spec: UI-102
test("layout collision check at 1024×640: no controls of the chart area overlap", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 640 });
  await open(page, false);
  if (!(await page.locator(".os-app-sidebar").isVisible())) await page.locator(".os-shell-divider-sidebar").click();
  await expect(page.getByRole("table", { name: "Summary" })).toBeVisible();
  for (const entry of ["Response Codes", "Internal", "Page Titles"]) {
    await treeTab(page, entry).click();
    await expect(area(page).locator(".os-ovc-body")).toBeVisible();
    const boxes = await area(page).locator('.os-ovc-head button, .os-ovc-body button').evaluateAll(els => els
      .filter(e => e.getClientRects().length > 0)
      .map(e => ({ name: e.getAttribute("aria-label") ?? e.textContent ?? "", ...e.getBoundingClientRect().toJSON() as { x: number; y: number; width: number; height: number } })));
    const host = (await area(page).boundingBox())!;
    const failures: string[] = [];
    boxes.forEach((a, i) => {
      if (a.x < host.x - 0.5 || a.x + a.width > host.x + host.width + 0.5) failures.push(`${entry}: ${a.name} outside the area`);
      for (const b of boxes.slice(i + 1)) if (overlaps(a, b)) failures.push(`${entry}: ${a.name} intersects ${b.name}`);
    });
    expect(failures).toEqual([]);
  }
});

// spec: UI-102
test("selection to drawn chart: median of 20 selections (indication, server)", async ({ page }) => {
  await open(page);
  const entries = ["Response Codes", "Internal", "Page Titles", "External", "Canonicals"];
  const samples: number[] = [];
  for (let i = 0; i < 20; i++) {
    const name = entries[i % entries.length]!;
    const ms = await page.evaluate(async (tab) => {
      const button = [...document.querySelectorAll<HTMLButtonElement>(".os-sidebar-overview [data-ovc-entry]")].find(b => b.dataset.ovcEntry === `tab:${tab}`)!;
      const start = performance.now();
      button.click();
      await new Promise<void>(resolve => {
        const check = () => document.querySelector(`[data-ovc-entry-shown="tab:${tab}"] .os-ovc-body > *`) ? requestAnimationFrame(() => resolve()) : requestAnimationFrame(check);
        check();
      });
      return performance.now() - start;
    }, name);
    samples.push(ms);
  }
  samples.sort((a, b) => a - b);
  const median = (samples[9]! + samples[10]!) / 2;
  await test.info().attach("selection-ms", { body: JSON.stringify({ median, samples }), contentType: "application/json" });
  expect(median).toBeLessThanOrEqual(200);
});

// spec: UI-102
test("screenshots light and dark: ring, bars and table (report)", async ({ page }) => {
  const dir = process.env.OS_V01_SHOTS ? resolve(process.env.OS_V01_SHOTS) : test.info().outputPath("shots");
  const tag = process.env.OS_V01_SHOT_TAG ? `${process.env.OS_V01_SHOT_TAG}-` : "";
  mkdirSync(dir, { recursive: true });
  for (const theme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: theme });
    await open(page);
    await page.getByRole("button", { name: "Client Error (4XX)", exact: true }).click();
    await expect(chartItem(page, "f:Client Error (4XX)")).toHaveAttribute("data-highlight", "true");
    await area(page).screenshot({ path: join(dir, `${tag}ring-response-codes-${theme}.png`), animations: "disabled" });
    await treeTab(page, "Internal").click();
    await expect(area(page).locator(".os-ovc-bars > li")).toHaveCount(10);
    await area(page).screenshot({ path: join(dir, `${tag}bars-internal-${theme}.png`), animations: "disabled" });
    await area(page).getByRole("button", { name: "Table" }).click();
    await expect(area(page).getByRole("table")).toBeVisible();
    await area(page).screenshot({ path: join(dir, `${tag}table-internal-${theme}.png`), animations: "disabled" });
    await area(page).getByRole("button", { name: "Chart" }).click();
  }
});
