import { test, expect } from "@playwright/test";
const fixture = "/src/features/table/fixtures/fx-ui-basic.html";
test.beforeEach(async ({ page }) => {
  await page.clock.install({ time: new Date("2026-10-18T10:00:00Z") });
  await page.goto(fixture);
});

// spec: UI-004 [P1]
test("reorders tabs, persists hidden tabs through restart, and resets", async ({ page }) => {
  await expect(page.getByRole("tab", { name: "Internal", exact: true })).toBeVisible();
  await page.getByRole("tab", { name: "Images", exact: true }).dragTo(page.getByRole("tab", { name: "Internal", exact: true }));
  await page.getByLabel("Configure tabs", { exact: true }).first().click();
  await page.getByRole("checkbox", { name: "AMP", exact: true }).uncheck();
  await expect(page.getByRole("tab", { name: "AMP", exact: true })).toHaveCount(0);
  await expect.poll(() => page.evaluate(() => localStorage.getItem("u10.fixture.uiState"))).toContain("AMP");
  await page.reload();
  await expect(page.getByRole("tab").first()).toHaveText("Images");
  await expect(page.getByRole("tab", { name: "AMP", exact: true })).toHaveCount(0);
  await page.getByLabel("Configure tabs", { exact: true }).first().click();
  await page.getByRole("button", { name: "Reset tabs", exact: true }).click();
  await expect(page.getByRole("tab").first()).toHaveText("Internal");
  await expect(page.getByRole("tab", { name: "AMP", exact: true })).toBeVisible();
});

// spec: UI-006
// spec: UI-014
test("uses filter counts, searches only on Enter, and exposes analysis prerequisites", async ({ page }) => {
  await page.getByRole("combobox", { name: "Filter", exact: true }).selectOption("Client Error (4xx)");
  await expect(page.getByText("Showing: 1 of 3", { exact: true })).toBeVisible();
  const search = page.getByLabel("Search Address", { exact: true });
  await search.fill("missing");
  await expect(page.getByText("Showing: 1 of 3", { exact: true })).toBeVisible();
  await search.press("Enter");
  await expect(page.getByText(/Showing: 0 of 3.*Search active/)).toBeVisible();
  await search.press("Escape");
  await expect(page.getByText("Showing: 1 of 3", { exact: true })).toBeVisible();
  await page.getByRole("combobox", { name: "Filter", exact: true }).selectOption("Unlinked");
  await expect(page.locator('[data-empty-state="NEEDS_CRAWL_ANALYSIS"]')).toBeVisible();
});

// spec: UI-008 [P1]
// spec: OUT-009
test("persists column selection, keeps Address fixed, and resets", async ({ page }) => {
  await page.getByRole("button", { name: "Configure columns", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "Address", exact: true })).toBeDisabled();
  await page.getByRole("checkbox", { name: "Word Count", exact: true }).uncheck();
  await page.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => page.evaluate(() => localStorage.getItem("u10.fixture.config"))).not.toContain('"Word Count"');
  await page.reload();
  await page.getByRole("button", { name: "Configure columns", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "Word Count", exact: true })).not.toBeChecked();
  await page.getByRole("button", { name: "Reset columns", exact: true }).click();
  await page.getByRole("button", { name: "Configure columns", exact: true }).click();
  await expect(page.getByRole("checkbox", { name: "Word Count", exact: true })).toBeChecked();
});
