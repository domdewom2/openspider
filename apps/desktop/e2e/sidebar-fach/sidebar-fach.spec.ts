import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { startFakeBridge, type FakeBridge } from "../support/fake-bridge";
import fixture from "./fach-dto.json" with { type: "json" };
import tickFixture from "../../src/ipc/__fixtures__/Tick.json" with { type: "json" };
import type { Tick } from "../../src/ipc/gen/Tick";
import type { SegmentDef } from "../../src/ipc/gen/SegmentDef";
import type { SegmentPreviewArgs } from "../../src/ipc/gen/SegmentPreviewArgs";
import type { SiteStructureArgs } from "../../src/ipc/gen/SiteStructureArgs";
import type { ResponseTimesArgs } from "../../src/ipc/gen/ResponseTimesArgs";

// E2E-A for the sidebar tabs Site Structure, Segments (with segment dialog), Response Times and API over the real
// WebSocket IPC; the core side answers with the fixture of fach-dto.json (fx-ui-depth, fx-ui-slow, fx-ui-dirs).
const tick = { ...tickFixture, notices: [] };
let bridge: FakeBridge;
let seq = 100;
let opSeq = 0;

test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-10-18T10:00:00Z"));
  bridge = await startFakeBridge();
  const core = bridge.core;
  core.settings.general.language = "en";
  core.handlers.set("sidebar.overview", () => ({ summary: [], tabs: [] }));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("sidebar.siteStructure", args => {
    const a = args as SiteStructureArgs;
    const depths = fixture.siteStructure.depths as Record<string, unknown>;
    return a.path ? { paths: fixture.siteStructure.moreChildren, depths: [] } : { paths: fixture.siteStructure.paths, depths: depths[a.view] };
  });
  core.handlers.set("sidebar.responseTimes", args => fixture.responseTimes[(args as ResponseTimesArgs).scope]);
  core.handlers.set("sidebar.segments", args => (fixture.segmentRows as Record<string, unknown>)[(args as { view: string }).view] ?? fixture.segmentRows.indexability);
  core.handlers.set("sidebar.apiStatus", () => fixture.api);
  core.handlers.set("api.errors", () => fixture.apiErrors);
  core.handlers.set("api.list", () => []);
  core.handlers.set("segment.list", () => structuredClone(fixture.segmentList));
  core.handlers.set("catalog.columns", () => fixture.columns);
  core.handlers.set("segment.validate", () => []);
  core.handlers.set("segment.save", () => ({ invalidatedSteps: [] }));
  core.handlers.set("segment.reassign", () => {
    const opId = `reassign-${++opSeq}`;
    setTimeout(() => core.emit("main", { kind: "tick", tick: { ...tick, seq: ++seq, ops: [{ opId, labelKey: "op-segment-reassign", done: 1, total: 1, state: "done" }] } as Tick }), 30);
    return { opId, labelKey: "op-segment-reassign", cancellable: false };
  });
  core.handlers.set("op.cancel", () => undefined);
  // The long-running preview answers through a tick, exactly like the real core: counters follow the order of the list.
  core.handlers.set("segment.preview", args => {
    const opId = `preview-${++opSeq}`;
    const order = (args as SegmentPreviewArgs).segments.map(s => s.id);
    const first = order[0];
    const result = first === "seg-rest"
      ? { counts: [{ id: "seg-rest", urls: 6 }, { id: "seg-blog", urls: 0 }, { id: "seg-thin", urls: 0 }], unassigned: 0 }
      : fixture.preview;
    setTimeout(() => core.emit("main", { kind: "tick", tick: { ...tick, seq: ++seq, ops: [{ opId, labelKey: "op-segment-preview", done: 1, total: 1, state: "done", result }] } as Tick }), 30);
    return { opId, labelKey: "op-segment-preview", cancellable: true };
  });
  await page.goto(`/?bridge=${encodeURIComponent(bridge.url)}`);
  await expect(page.locator('[data-sidebar-ready="true"]')).toBeVisible();
  core.emit("main", { kind: "tick", tick: { ...tick, seq: ++seq } as Tick });
});
test.afterEach(async () => { await bridge.stop(); });

const sidebar = (page: import("@playwright/test").Page) => page.getByRole("complementary", { name: "Sidebar" });
// M2-U34 (K2-72): tabs that do not fit the sidebar width are opened through its overflow menu (UI-060, UI-004).
const openTab = async (page: import("@playwright/test").Page, name: string) => {
  const tab = sidebar(page).getByRole("tab", { name, exact: true });
  if (await tab.count()) await tab.click();
  else { await sidebar(page).getByRole("button", { name: "More sidebar tabs" }).click(); await page.getByRole("menuitem", { name, exact: true }).click(); }
};

// spec: UI-066
test("Site Structure shows directories and the depth distribution; views switch without changing the sum", async ({ page }) => {
  await openTab(page, "Site Structure");
  const bar = sidebar(page);
  await expect(bar.getByRole("treegrid", { name: "Directory structure" }).getByRole("button", { name: "a/", exact: true })).toBeVisible();
  const depth = bar.getByRole("table", { name: "Crawl depth distribution" });
  await expect(depth.getByRole("row", { name: /10\+/ })).toContainText("3");
  await expect(bar.getByRole("button", { name: "3 – Non-Indexable: 1" })).toBeVisible();
  await bar.getByLabel("View", { exact: true }).selectOption("responseCodes");
  await expect(bar.getByRole("button", { name: "4 – 4xx: 1" })).toBeVisible();
  await bar.getByLabel("View", { exact: true }).selectOption("crawlDepth");
  await expect(bar.getByRole("button", { name: "10+ – Crawl Depth: 3" })).toBeVisible();
  await expect.poll(() => bridge.core.calls.filter(c => c.cmd === "sidebar.siteStructure").map(c => (c.args as SiteStructureArgs).view)).toEqual(expect.arrayContaining(["indexability", "responseCodes", "crawlDepth"]));
  const axe = await new AxeBuilder({ page }).include('[aria-label="Sidebar"]').analyze();
  expect(axe.violations.map(v => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
});

// spec: UI-066
test("Site Structure loads deeper directories on demand", async ({ page }) => {
  await openTab(page, "Site Structure");
  await sidebar(page).getByRole("button", { name: "Expand: https://site.test/a/b/c/" }).click();
  await expect(sidebar(page).getByRole("button", { name: "x.html" })).toBeVisible();
  await expect.poll(() => bridge.core.calls.some(c => (c.args as SiteStructureArgs | undefined)?.path === "https://site.test/a/b/c/")).toBe(true);
});

// spec: UI-069
test("Response Times lists the fx-ui-slow buckets, the no-response footer and the scope filter", async ({ page }) => {
  await openTab(page, "Response Times");
  const table = sidebar(page).getByRole("table", { name: "Response time distribution" });
  await expect(table.getByRole("row", { name: /^0–1/ })).toContainText("2");
  await expect(table.getByRole("row", { name: /^10\+/ })).toContainText("1");
  await expect(table).toContainText("No response: 2");
  await sidebar(page).getByLabel("URLs", { exact: true }).selectOption("internalHtml");
  await expect(table.getByRole("row", { name: /^0–1/ })).toContainText("50.0 %");
  const axe = await new AxeBuilder({ page }).include('[aria-label="Sidebar"]').analyze();
  expect(axe.violations.map(v => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
});

// spec: UI-067, UI-068
test("Segments tab opens the segment dialog; moving a segment updates the preview and OK saves in the new order", async ({ page }) => {
  await openTab(page, "Segments");
  const table = sidebar(page).getByRole("table", { name: "Key figures per segment" });
  await expect(table.getByRole("row", { name: /Blog/ })).toContainText("25.0 %");
  await table.getByRole("button", { name: "Filter by segment: Blog" }).click();
  await expect.poll(() => bridge.core.calls.filter(c => c.cmd === "sidebar.segments").length).toBeGreaterThan(0);
  await sidebar(page).getByRole("button", { name: "Configure segments" }).click();
  const dialog = page.getByRole("dialog", { name: "Configure segments" });
  const list = dialog.getByRole("list", { name: "Segments in priority order" });
  await expect(list.getByRole("listitem")).toHaveCount(3);
  await expect(list.getByRole("listitem").first()).toContainText("3 URLs");
  await list.getByRole("button", { name: "Move Rest up" }).click();
  await list.getByRole("button", { name: "Move Rest up" }).click();
  await expect(list.getByRole("listitem").first()).toContainText("Rest");
  await expect(list.getByRole("listitem").first()).toContainText("6 URLs");
  const axe = await new AxeBuilder({ page }).include('[role="dialog"]').analyze();
  expect(axe.violations.map(v => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
  await expect(dialog.getByRole("button", { name: "OK" })).toBeEnabled();
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(dialog).toHaveCount(0);
  const save = bridge.core.calls.find(c => c.cmd === "segment.save")?.args as { segments: SegmentDef[] };
  expect(save.segments.map(s => [s.name, s.order])).toEqual([["Rest", 0], ["Blog", 1], ["Thin pages", 2]]);
  expect(bridge.core.calls.some(c => c.cmd === "segment.reassign")).toBe(true);
});

// spec: UI-068
test("segment dialog blocks OK for a reference to a segment further down and for duplicate names", async ({ page }) => {
  await openTab(page, "Segments");
  await sidebar(page).getByRole("button", { name: "Configure segments" }).click();
  const dialog = page.getByRole("dialog", { name: "Configure segments" });
  await dialog.getByRole("textbox", { name: "Name" }).fill("rest");
  await expect(dialog.getByRole("alert").filter({ hasText: "already has this name" })).toBeVisible();
  await expect(dialog.getByRole("button", { name: "OK" })).toBeDisabled();
  await dialog.getByRole("textbox", { name: "Name" }).fill("Blog");
  await dialog.getByRole("button", { name: "Add condition (AND)" }).click();
  const cond = dialog.getByRole("group", { name: "Condition 2 of group 1" });
  await cond.getByRole("combobox", { name: "Column" }).fill("Segment");
  await cond.getByRole("combobox", { name: "Operator" }).selectOption("equals");
  await cond.getByRole("textbox", { name: "Value" }).fill("Rest");
  await expect(cond.getByRole("alert")).toContainText("must be listed above this segment");
  await expect(dialog.getByRole("button", { name: "OK" })).toBeDisabled();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toHaveCount(0);
  expect(bridge.core.calls.some(c => c.cmd === "segment.save")).toBe(false);
});

// spec: UI-070
test("API tab shows all six sources in fixed order, the error list without secrets and opens the source dialog", async ({ page }) => {
  await openTab(page, "API");
  const table = sidebar(page).getByRole("table", { name: "API data sources" });
  const rows = table.locator("tbody tr[data-api-kind]");
  await expect(rows).toHaveCount(6);
  await expect(rows.nth(3)).toContainText("Not connected");
  await expect(rows.nth(4)).toContainText("Not connected");
  await expect(rows.nth(1)).toContainText("Quota exhausted");
  await table.getByRole("button", { name: "Show 2 errors of PageSpeed Insights" }).click();
  const errors = sidebar(page).getByRole("region", { name: "Errors of PageSpeed Insights" });
  await expect(errors.getByText("Quota exceeded")).toBeVisible();
  await expect(errors).not.toContainText("AIzaSyDUMMYSECRET123");
  await table.getByRole("button", { name: "Configure Ahrefs" }).click();
  await expect(page.getByRole("dialog", { name: "API access" })).toBeVisible();
  const axe = await new AxeBuilder({ page }).include('[aria-label="Sidebar"]').analyze();
  expect(axe.violations.map(v => `${v.id}: ${v.nodes[0]?.target}`)).toEqual([]);
});
