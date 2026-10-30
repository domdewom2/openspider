import { test, expect } from "@playwright/test";
import { prepare } from "./support";
for (const lang of ["de", "en"]) for (const theme of ["light", "dark"]) for (const density of ["compact", "normal"]) {
  // spec: UI-120
  // spec: UI-016
  // spec: UI-121
  test(`${lang}/${theme}/${density}: Plattform-Baselines`, async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1024, height: 640 });
    for (const scene of ["shell", "empty", "settings", "primitives", "serp", "source", "viz"]) {
      await prepare(page, lang, theme, density, scene);
      if (scene === "serp") await expect(page.locator(".os-serp-title")).toContainText("Start");
      if (scene === "source") {
        const diff = page.getByRole("checkbox", { name: /^(Show differences|Unterschiede anzeigen)$/ });
        await diff.focus();
        await diff.press("Space");
        await expect(diff).toBeChecked();
        await expect(page.getByTestId("diff-summary")).toContainText("+");
        await expect(page.locator(".cm-content").first()).toContainText("doctype");
        await expect(page.locator(".cm-content")).toHaveCount(2);
      }
      if (scene === "viz") await expect.poll(() => page.locator("svg text").count()).toBeGreaterThan(0);
      await expect(page).toHaveScreenshot(`${scene}-${lang}-${theme}-${density}.png`, { animations: "disabled", fullPage: true });
    }
  });
}
