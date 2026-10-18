import { test, expect } from "@playwright/test";
import { startFakeBridge, type FakeBridge } from "../support/fake-bridge";
import fixture from "./fx-ui-basic.json" with { type: "json" };
import tick from "../../src/ipc/__fixtures__/Tick.json" with { type: "json" };
import type { Tick } from "../../src/ipc/gen/Tick";

// Browser contract tests over real WebSocket IPC; the core commands are still K04/K06 stubs on this branch.
let bridge: FakeBridge;
test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-10-18T10:00:00Z"));
  bridge = await startFakeBridge();
  bridge.core.settings.general.language = "en";
  bridge.core.handlers.set("sidebar.overview", () => fixture.overview);
  bridge.core.handlers.set("sidebar.issues", () => fixture.issues);
  bridge.core.handlers.set("sidebar.issueExplain", () => fixture.explanation);
  bridge.core.handlers.set("analysis.status", () => fixture.analysis);
  bridge.core.handlers.set("analysis.start", () => ({ opId: "analysis-1", labelKey: "analysis", cancellable: true }));
  await page.goto(`/?bridge=${encodeURIComponent(bridge.url)}`);
  await expect(page.locator('[data-sidebar-ready="true"]')).toBeVisible();
  bridge.core.emit("main", { kind: "tick", tick: tick as Tick });
});
test.afterEach(async () => { await bridge.stop(); });

// spec: UI-060
test("sidebar mounts in its landmark, supports keyboard tabs and sends scoped navigation", async ({ page }) => {
  const sidebar = page.getByRole("complementary", { name: "Sidebar" });
  await expect(sidebar.getByRole("table", { name: "Summary" })).toBeVisible();
  await sidebar.getByRole("tab", { name: "Issues", exact: true }).click();
  await sidebar.getByRole("button", { name: "Page Titles: Duplicate", exact: true }).click();
  await expect(sidebar.getByRole("region", { name: "Issue explanation" })).toContainText("65");
  await sidebar.getByRole("button", { name: "Shop: 2", exact: true }).click();
  await expect.poll(() => bridge.core.calls.filter(c => c.cmd === "sidebar.issues").at(-1)?.args).toMatchObject({ segment: { kind: "segment", name: "Shop" } });
  await sidebar.getByRole("tab", { name: "Overview", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(sidebar.getByRole("tab", { name: "Issues", exact: true })).toBeFocused();
});

// spec: UI-061
test("shows prerequisites without zero counts, remembers collapse and refreshes live counts", async ({ page }) => {
  const summary = page.getByRole("table", { name: "Summary" });
  await expect(summary.getByRole("row", { name: /Total URLs Displayed/ })).toContainText("27");
  await expect(page.getByRole("row", { name: /Outside <head>/ })).toContainText("–");
  await expect(page.getByRole("row", { name: /Lorem Ipsum Placeholder/ })).toContainText("7 pages unchecked");
  await page.getByRole("button", { name: "▾ Page Titles", exact: true }).click();
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await page.getByRole("tab", { name: "Overview", exact: true }).click();
  await expect(page.getByRole("table", { name: "Page Titles", exact: true })).toHaveCount(0);
  bridge.core.handlers.set("sidebar.overview", () => ({ ...fixture.overview, summary: fixture.overview.summary.map(r => r.key === "Total URLs Encountered" ? { ...r, count: 30 } : r) }));
  bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: 18, countsVersion: 10 } as Tick });
  await expect(summary.getByRole("row", { name: /Total URLs Encountered/ })).toContainText("30");
});

// spec: UI-062
test("filters issues and uses the common exporter", async ({ page }) => {
  bridge.core.handlers.set("export.issues", () => ({ opId: "export-1", labelKey: "export", cancellable: true }));
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await page.getByLabel("Issue Type", { exact: true }).selectOption("opportunity");
  await page.getByLabel("Issue Priority", { exact: true }).selectOption("medium");
  await page.getByLabel("Search issues", { exact: true }).fill("duplicate");
  await expect(page.getByRole("table").getByRole("row")).toHaveCount(2);
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await dialog.getByLabel("Destination file").fill("issues.csv");
  await dialog.getByRole("button", { name: "Export", exact: true }).click();
  await expect.poll(() => bridge.core.calls.find(c => c.cmd === "export.issues")?.args).toEqual({ which: "issues", path: "issues.csv", format: "csv" });
});

// spec: UI-080
// spec: UI-082 [M2]
// spec: ONPAGE-140 [M2]
test("shows stale analysis, saved HTML notice and live named step progress in the status bar", async ({ page }) => {
  await expect(page.getByRole("status").filter({ hasText: "Analysis outdated" })).toBeVisible();
  await page.getByRole("tab", { name: "Analysis", exact: true }).click();
  await expect(page.getByText("Reanalysis needs saved HTML. Recrawl required.")).toBeVisible();
  await expect(page.getByRole("button", { name: "HTML storage setting" })).toBeVisible();
  await page.getByRole("button", { name: "Analyze again", exact: true }).click();
  await expect.poll(() => bridge.core.calls.find(c => c.cmd === "analysis.start")?.args).toEqual({ steps: null });
  bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: 19, analysis: { ...fixture.analysis, running: true, current: "link_score", percent: 42 } } as Tick });
  await expect(page.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "42");
  await expect(page.getByRole("contentinfo")).toContainText("42 %");
});

// spec: UI-065 [M2]
test("keeps the selected issue and refreshes its core explanation when the title threshold changes from 60 to 65", async ({ page }) => {
  let max = 60;
  bridge.core.handlers.set("sidebar.issues", () => [...fixture.issues, {
    id: "page-titles-over-max-chars", tab: "Page Titles", filter: `Over ${max} Characters`, type: "opportunity", priority: "medium", count: 3, pct: 12, segmentShares: [],
  }]);
  bridge.core.handlers.set("sidebar.issueExplain", () => ({ ...fixture.explanation, what: fixture.explanation.what.replaceAll("65", String(max)), fix: fixture.explanation.fix.replaceAll("65", String(max)) }));
  await page.getByRole("tab", { name: "Issues", exact: true }).click();
  await page.getByRole("button", { name: "Page Titles: Over 60 Characters", exact: true }).click();
  const explanation = page.getByRole("region", { name: "Issue explanation" });
  await expect(explanation).toContainText("limit of 60 characters");
  max = 65;
  bridge.core.emit("main", { kind: "tick", tick: { ...tick, seq: 20, issuesVersion: 11, countsVersion: 11 } as Tick });
  await expect(page.getByRole("button", { name: "Page Titles: Over 65 Characters", exact: true })).toBeVisible();
  await expect(explanation).toContainText("limit of 65 characters");
  await expect(explanation).not.toContainText("limit of 60 characters");
});
