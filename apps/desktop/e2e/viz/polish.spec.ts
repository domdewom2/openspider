// Vorher/Nachher-Screenshots der Optik (M3-V02, Feinschliff) in hell und dunkel mit der Vorführ-Fixture
// `fxUiShowcase` (~950 URLs, fünf Ebenen, gemischte Indexierbarkeit/Statuscodes). Nur auf Anforderung:
//   OS_V02_POLISH=nachher npx playwright test e2e/viz/polish.spec.ts --project=chromium --workers=1
// legt `docs/plan/M3-reports/v02/polish/<phase>-<ansicht>-<theme>.png` ab.
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import { directoryTreeOf, fxUiShowcase, fxUiShowcaseWords, SHOWCASE_SHOP } from "../../src/features/viz/fixtures/synthetic";
import { layoutDone, nodePoint, openViz } from "./support";
import { startVizBridge, type VizBridge } from "./viz-bridge";

const PHASE = process.env.OS_V02_POLISH ?? "";
const DIR = fileURLToPath(new URL("../../../../docs/plan/M3-reports/v02/polish/", import.meta.url));
let bridge: VizBridge;

test.skip(!PHASE, "Screenshots nur mit OS_V02_POLISH=<vorher|nachher>");
test.describe.configure({ timeout: 180_000 });

test.beforeEach(async () => {
  const core = new FakeVizCore(fxUiShowcase, () => directoryTreeOf(fxUiShowcase()), fxUiShowcaseWords);
  core.bodyWords = fxUiShowcaseWords;
  bridge = await startVizBridge(core);
});
test.afterEach(async () => {
  await bridge.stop();
});

async function save(page: Page, name: string, theme: string) {
  mkdirSync(DIR, { recursive: true });
  await page.waitForTimeout(400);
  await page.screenshot({ path: join(DIR, `${PHASE}-${name}-${theme}.png`) });
}

for (const theme of ["hell", "dunkel"] as const) {
  test.describe(theme, () => {
    test.beforeEach(async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme === "dunkel" ? "dark" : "light", reducedMotion: "reduce" });
    });

    test(`3D-Crawl-Graph (${theme})`, async ({ page }) => {
      await openViz(page, bridge, "crawlGraph3d");
      await page.waitForFunction(() => window.__osViz3d?.layout.phase === "done", undefined, { timeout: 60_000 });
      await save(page, "3d-crawl-graph", theme);
    });

    test(`3D-Verzeichnisgraph (${theme})`, async ({ page }) => {
      await openViz(page, bridge, "directoryGraph3d");
      await page.waitForFunction(() => window.__osViz3d?.layout.phase === "done", undefined, { timeout: 60_000 });
      await save(page, "3d-verzeichnisgraph", theme);
    });

    test(`2D-Verzeichnisgraph mit eingeblendeten Links (${theme})`, async ({ page }) => {
      await openViz(page, bridge, "directoryGraph");
      await layoutDone(page, 60_000);
      // Ganze Struktur einpassen (gleicher Ausschnitt vorher/nachher).
      await page.evaluate(() => {
        const v = window.__osViz!;
        const ps = v.positions();
        const xs = ps.map((q) => q.x);
        const ys = ps.map((q) => q.y);
        const c = document.querySelector("canvas.os-viz-canvas")!.getBoundingClientRect();
        const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
        const k = Math.min(1.2, (c.width * 0.9) / Math.max(1, x1 - x0), (c.height * 0.9) / Math.max(1, y1 - y0));
        v.setTransform({ k, x: c.width / 2 - ((x0 + x1) / 2) * k, y: c.height / 2 - ((y0 + y1) / 2) * k });
      });
      const p = await nodePoint(page, SHOWCASE_SHOP);
      await page.mouse.click(p.x, p.y, { button: "right" });
      await page.getByRole("menuitem", { name: "Show inlinks", exact: true }).click();
      await expect.poll(() => page.evaluate(() => window.__osViz!.links?.edges ?? 0)).toBeGreaterThan(50);
      await page.mouse.move(5, 5);
      await save(page, "2d-verzeichnisgraph-links", theme);
    });

    test(`Wortwolke (${theme})`, async ({ page }) => {
      await openViz(page, bridge, "bodyWords", { root: 1 });
      await expect(page.getByTestId("viz-terms").locator("tbody tr").first()).toBeVisible();
      await save(page, "wortwolke", theme);
    });
  });
}
