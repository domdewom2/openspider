import { expect, test } from "../fixtures/app";
import { textFits } from "./text-fit";

test.use({ freshState: true, viewport: { width: 1024, height: 640 } });
for (const lang of ["de", "en"]) {
  // spec: UI-121
  test(`${lang}: help dialogs remain inside a 1024 × 640 window`, async ({ page, app }, info) => {
    await app.open(page, "main");
    await page.waitForFunction(() => !!window.__osMenu);
    await page.evaluate(async lang => {
      const path = "/src/i18n/index.ts";
      await (await import(/* @vite-ignore */ path)).setLanguage(lang);
    }, lang);
    for (const action of ["app.about", "help.shortcuts", "help.debug", "help.usage"]) {
      await page.evaluate(async action => {
        const path = "/src/ui/actions.ts";
        (await import(/* @vite-ignore */ path)).runAction(action, { source: "menu" });
      }, action);
      const dialog = page.getByRole("dialog").first();
      await expect(dialog).toBeVisible();
      const box = await dialog.boundingBox();
      expect(box!.x).toBeGreaterThanOrEqual(0);
      expect(box!.y).toBeGreaterThanOrEqual(0);
      expect(box!.x + box!.width).toBeLessThanOrEqual(1025);
      expect(box!.y + box!.height).toBeLessThanOrEqual(641);
      await textFits(dialog, `${action}-${lang}`);
      await info.attach(`${action}-${lang}`, { body: await page.screenshot(), contentType: "image/png" });
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
    }
  });
}
