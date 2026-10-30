import { test, expect } from "@playwright/test";
import { audit, prepare, sizes } from "./support";
import observed from "../../../../docs/plan/M2-acceptance/e2e/layout-erster-lauf.json" with { type: "json" };
for (const lang of ["de", "en"]) for (const theme of ["light", "dark"]) for (const size of sizes) {
  test.describe(`${lang}/${theme}/${size.width}x${size.height}`, () => {
    test.use({ viewport: size });
    // spec: UI-001
    // spec: UI-121
    for (const check of ["Kollisionen", "abgeschnittener Text"] as const) {
    // spec: UI-001
    // spec: UI-121
    test(`K2-75: ${check}, drei Tab-Leisten, Überlaufmenüs und Dialog`, async ({ page }, info) => {
      await prepare(page, lang, theme);
      const reports: { scene: string; result: Awaited<ReturnType<typeof audit>> }[] = [];
      const inspect = async (scene: string) => reports.push({ scene, result: await audit(page) });
      await inspect("main");
      for (const group of ["content", "technical", "links", "performance", "custom"]) {
        const button = page.locator(`.u34-nav [data-group="${group}"] .u34-nav-button`);
        if (await button.isEnabled()) {
          await button.click();
          await expect(button).toHaveAttribute("aria-current", "page");
          await inspect(`group-${group}`);
        }
      }
      await page.locator('.u34-nav [data-group="urls"] .u34-nav-button').click();
      for (const trigger of await page.locator('[data-tabbar-overflow]').all()) {
        await trigger.click();
        await expect(page.getByRole("menu").last()).toBeVisible();
        await inspect("overflow");
        await page.keyboard.press("Escape");
      }
      await page.evaluate(async () => {
        const path = "/src/ui/actions.ts";
        (await import(/* @vite-ignore */ path)).runAction("help.shortcuts", { source: "menu" });
      });
      await expect(page.getByRole("dialog")).toBeVisible();
      await inspect("shortcuts-dialog");
      await info.attach("layout", { body: JSON.stringify({ project: info.project.name, lang, theme, size, reports }, null, 2), contentType: "application/json" });
      expect(reports[0]!.result.controls).toBeGreaterThan(20);
      if (check === "Kollisionen") {
        expect(reports.flatMap(({ scene, result }) => [...result.outside, ...result.collisions].map((hit) => `${scene}: ${hit}`))).toEqual([]);
      } else {
        const hits = reports.flatMap(({ scene, result }) => result.text.map(hit => `${scene}: ${hit}`));
        if (process.platform === "linux") {
          const known = observed.find(r => r.project === info.project.name && r.lang === lang && r.theme === theme && r.size.width === size.width)!;
          const knownHits = known.reports.flatMap(({ scene, result }) => result.text.map(hit => `${scene}: ${hit}`));
          expect(hits.filter(hit => !knownHits.includes(hit)), "Neue Textbefunde zusätzlich zu U32-002/U32-003").toEqual([]);
          // K2-63: U30 fixed all 48 Chromium label cases; only the 12 WebKit select cases remain.
          if (info.project.name.startsWith("webkit")) test.fail(true, "U32-003: natives Select scrollHeight/clientHeight");
        }
        expect(hits).toEqual([]);
      }
    });
    }
  });
}
