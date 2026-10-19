import { test, expect } from "../fixtures/app";
import tick from "../../src/features/header/fixtures/tick-running.json" with { type: "json" };

// spec: UI-013
test("keeps MCP visible without an active crawl and opens its menu", async ({ app, page }) => {
  await app.open(page, "main");
  await page.getByRole("button", { name: /MCP:/ }).click();
  await expect(page.getByRole("menu", { name: /MCP:/ })).toBeVisible();
});

// spec: CRAWL-130 [M2]
test("renders fixture rates and memory without recomputing progress", async ({ page }) => {
  // This test targets the contract transport; bridge progress is covered by header lifecycle after K04.
  await page.goto("/?transport=mock");
  await expect(page.getByLabel("Start URL")).toBeVisible();
  await page.evaluate((base) => window.__osMock!.emit("main", { kind: "tick", tick: base } as never), tick);
  await expect(page.getByText("Current: 42.5 URL/s")).toBeVisible();
  await expect(page.getByText("Done 1,200 of 2,000 (60 %)")).toBeVisible();
  await expect(page.getByText("Memory: 800 MB / 1.00 GB")).toBeVisible();
  await expect(page.getByRole("progressbar", { name: "Crawl progress" })).toHaveAttribute("aria-valuenow", "60");
  await expect(page.getByRole("button", { name: "MCP: connected (1 client)" })).toBeVisible();
});
