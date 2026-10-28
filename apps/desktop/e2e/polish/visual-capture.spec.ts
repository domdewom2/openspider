import { test } from '../alpha/fixture';
import { crawledWithSelection } from './support';
import { mkdirSync } from 'node:fs';
import { resolve } from 'node:path';
test.use({ locale: "de-DE" });
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
    await alpha.page.screenshot({ path: resolve(dir, `${process.env.U35_CAPTURE ?? 'nachher'}-${theme}-1024x640.png`) });
  }
});
