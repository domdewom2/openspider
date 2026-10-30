import { test, expect } from "@playwright/test";

// spec: UI-126
test("virtual grid announces the complete total and absolute row indices, with a single Tab stop", async ({ page, browserName }) => {
  await page.goto("/e2e/a11y/grid.html");
  const grid = page.getByRole("grid", { name: "Virtual fixture" });
  await expect(grid).toHaveAttribute("aria-rowcount", "5001");
  await page.keyboard.press("Tab");
  await expect(page.getByRole("button", { name: "Before grid" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(grid).toBeFocused();
  await grid.press("End");
  await expect(grid.getByRole("row", { selected: true })).toHaveAttribute("aria-rowindex", "5001");
  await expect.poll(() => grid.getByRole("row").count()).toBeLessThan(100);
  const active = await grid.getAttribute("aria-activedescendant");
  await expect(page.locator(`[id="${active}"]`)).toContainText("5,000");
  await page.keyboard.press("Tab");
  await test.info().attach("focus-after-grid-tab", { body: await page.evaluate(() => document.activeElement?.outerHTML ?? ""), contentType: "text/html" });
  await expect.soft(page.getByRole("button", { name: "After grid" })).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(grid).toBeFocused();
  // U30-F01 remains a failing assertion until N12; any other failure stays unexpected.
  const errors = test.info().errors;
  if (browserName === "chromium" && errors.every(error => error.message?.includes("name: 'After grid'"))) {
    test.fail(true, "U30-F01: additional Chromium grid tab stop; known red until N12");
  }
});
