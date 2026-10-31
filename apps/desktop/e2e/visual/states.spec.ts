import { test, expect } from "@playwright/test";
import { EMPTY_CODES } from "../../src/ui/emptyState/codes";
import { prepare } from "./support";
for (const lang of ["de", "en"]) {
  // spec: UI-016
  test(`${lang}: alle registrierten Leerzustände zeigen lesbare Erklärung und Code`, async ({ page }) => {
    await prepare(page, lang, "light", "compact", "empty");
    for (const code of EMPTY_CODES) {
      const state = page.locator(`[data-empty-state="${code}"], [data-empty-state^="${code}("]`);
      await expect(state).toHaveCount(1);
      await expect(state).toBeVisible();
      expect((await state.innerText()).trim().length).toBeGreaterThan(10);
    }
  });
}
// spec: UI-120
test("System-Theme folgt OS-Wechsel, explizite Themes und Dichte wirken ohne Neustart", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/?transport=mock");
  await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  for (const theme of ["light", "dark"]) for (const rowDensity of ["compact", "normal"]) {
    await page.evaluate(async ({ theme, rowDensity }) => {
      const path = "/src/state/prefs.ts";
      await (await import(/* @vite-ignore */ path)).usePrefs.getState().patch({ appearance: { theme, rowDensity } });
    }, { theme, rowDensity });
    await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
    expect(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue("--os-row-height").trim())).toBe(rowDensity === "compact" ? "28px" : "44px");
  }
});
