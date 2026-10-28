import { test } from '../alpha/fixture';
import { crawledWithSelection, showPanes } from './support';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
test.use({ locale: "de-DE" });
test.beforeEach(async ({ page }) => {
  await page.clock.setFixedTime(new Date("2026-10-28T12:00:00Z"));
});

// spec: UI-001
test('own fx-ui-basic screenshots', async ({ alpha }) => {
  await alpha.page.setViewportSize({ width: 1024, height: 640 });
  await crawledWithSelection(alpha);
  for (const theme of ['light', 'dark']) {
    await alpha.page.evaluate(async theme => {
      const path = '/src/state/prefs.ts';
      const { usePrefs } = await import(/* @vite-ignore */ path);
      await usePrefs.getState().patch({ appearance: { theme } });
    }, theme);
    const dir = resolve('../../docs/plan/M2-reports/u35');
    mkdirSync(dir, { recursive: true });
    await alpha.page.screenshot({ scale: "css", path: resolve(dir, `${process.env.U35_CAPTURE ?? 'nachher'}-${theme}-1024x640.png`) });
  }
  if (!process.env.U35_CAPTURE) {
    await alpha.page.setViewportSize({ width: 1440, height: 900 });
    await showPanes(alpha.page, "default");
    for (const theme of ['light', 'dark']) {
      await alpha.page.evaluate(async theme => {
        const path = '/src/state/prefs.ts';
        const { usePrefs } = await import(/* @vite-ignore */ path);
        await usePrefs.getState().patch({ appearance: { theme } });
      }, theme);
      const dir = resolve('../../docs/plan/M2-reports/u35');
      await alpha.page.screenshot({ scale: 'css', path: resolve(dir, `nachher-${theme}-1440x900.png`) });
      await alpha.page.locator('[data-group="overview"] button').click();
      await alpha.page.locator('[data-dashboard] h2').first().waitFor();
      await alpha.page.locator('[data-dashboard] .os-dashboard-sections > section').last().getByRole('button').first().waitFor();
      await alpha.page.screenshot({ scale: 'css', path: resolve(dir, `dashboard-${theme}-1440x900.png`) });
      await alpha.page.locator('[data-group="urls"] button').first().click();
    }
  }
});
