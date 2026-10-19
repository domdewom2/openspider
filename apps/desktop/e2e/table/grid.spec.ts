import { test, expect } from "@playwright/test";
const fixture = "/src/features/table/fixtures/u10-contract.html";
test.beforeEach(async ({ page }) => {
  await page.clock.install({ time: new Date("2026-10-18T10:00:00Z") });
  await page.goto(fixture);
  await expect(page.getByText("Showing: 3 of 3", { exact: true })).toBeVisible();
});

// spec: UI-010
test("cycles sort through actual canvas header gestures and sends Shift secondary sort to the core", async ({ page }) => {
  const canvas = page.locator(".os-data-grid");
  const sort = () => page.evaluate(() => {
    const calls = window.__osMock!.calls.filter(c => c.cmd === "view.open");
    return (calls.at(-1)?.args as { spec: { sort: unknown[] } })?.spec.sort;
  });
  await canvas.click({ position: { x: 525, y: 12 } });
  await expect.poll(sort).toEqual([{ column: "Word Count", dir: "asc" }]);
  await canvas.click({ position: { x: 525, y: 12 } });
  await expect.poll(sort).toEqual([{ column: "Word Count", dir: "desc" }]);
  await canvas.click({ position: { x: 525, y: 12 } });
  await expect.poll(sort).toEqual([]);
  await canvas.click({ position: { x: 375, y: 12 } });
  await canvas.click({ position: { x: 525, y: 12 }, modifiers: ["Shift"] });
  await expect.poll(sort).toEqual([{ column: "Status Code", dir: "asc" }, { column: "Word Count", dir: "asc" }]);
});

// spec: UI-110
// spec: UI-114
test("supports keyboard context menu, copies URL ids, confirms removal and cancels a running operation", async ({ page }) => {
  const canvas = page.locator(".os-data-grid");
  await canvas.click({ position: { x: 225, y: 69 } });
  await expect(page.getByText(/1 selected/)).toBeVisible();
  await page.keyboard.press("Shift+F10");
  await page.getByRole("menuitem", { name: "Copy URLs", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__osMock!.calls.filter(c => c.cmd === "view.copy").at(-1)?.args)).toMatchObject({ selection: { kind: "ids", ids: [2] }, columns: ["Address"] });
  await canvas.click({ position: { x: 225, y: 69 }, button: "right" });
  await page.getByRole("menuitem", { name: "Remove", exact: true }).click();
  await expect(page.getByText("Remove 1 selected URLs from this crawl?")).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__osMock!.calls.filter(c => c.cmd === "crawl.remove").length)).toBe(0);
  await canvas.click({ position: { x: 225, y: 69 }, button: "right" });
  await page.getByRole("menuitem", { name: "Recrawl", exact: true }).click();
  await expect(page.getByText("Crawl analysis outdated")).toBeVisible();
  await page.getByRole("button", { name: "Cancel", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__osMock!.calls.filter(c => c.cmd === "op.cancel").at(-1)?.args)).toEqual({ opId: "crawl.recrawl-1" });
});
