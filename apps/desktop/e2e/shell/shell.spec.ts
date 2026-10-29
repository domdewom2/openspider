import type { Page } from "@playwright/test";
import { test, expect } from "../fixtures/app";

async function ready(page: Page) {
  await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
}
async function resetState(page: Page) {
  await page.evaluate(async () => {
    const prefsPath = "/src/state/prefs.ts";
    await (await import(/* @vite-ignore */ prefsPath)).usePrefs.getState().patch({ appearance: { zoomPct: 100 }, general: { language: "en" } });
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    const path = "/src/state/layout.ts";
    const { useLayout } = await import(/* @vite-ignore */ path);
    const root = document.querySelector(".os-app") as HTMLElement;
    const header = root.querySelector("header") as HTMLElement;
    const footer = root.querySelector("footer") as HTMLElement;
    useLayout.getState().resetAll(root.clientWidth, root.clientHeight - header.offsetHeight - footer.offsetHeight);
    await useLayout.getState().flush();
  });
  await page.reload();
  await ready(page);
}
async function action(page: Page, id: string) {
  await page.evaluate(async (id) => {
    const path = "/src/ui/actions.ts";
    const { runAction } = await import(/* @vite-ignore */ path);
    runAction(id, { source: "menu" });
  }, id);
}
async function sizes(page: Page) {
  return page.evaluate(() => {
    const bounds = (selector: string) => document.querySelector(selector)!.getBoundingClientRect();
    const root = bounds(".os-app"); const header = bounds(".os-app-header"); const status = bounds(".os-app-status");
    return { sidebar: bounds(".os-app-sidebar").width, detail: bounds("#shell-detail").height,
      content: root.height - header.height - status.height, main: bounds("main").width };
  });
}
async function drag(page: Page, pane: "sidebar" | "detail", size: number) {
  const grip = page.locator(`.os-shell-divider-${pane}`);
  const box = (await grip.boundingBox())!;
  const current = (await sizes(page))[pane];
  // Integer pointer coordinates: WebKit rounds fractional ones, and the header height is fractional since M2-U03.
  const x = Math.round(box.x + box.width / 2); const y = Math.round(box.y + box.height / 2);
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + (pane === "sidebar" ? Math.round(current - size) : 0),
    y + (pane === "detail" ? Math.round(current - size) : 0), { steps: 8 });
  await page.mouse.up();
}

// spec: UI-001
test("1440×900 formulas, both splitters, snap/collapse/restore, hide and independent resets", async ({ page, app }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await app.open(page, "main"); await ready(page); await resetState(page);
  let actual = await sizes(page);
  expect(actual.sidebar).toBeCloseTo(360, 0);
  expect(actual.detail).toBeCloseTo(actual.content * .35, 0);
  await drag(page, "sidebar", 500); await drag(page, "detail", 300);
  actual = await sizes(page);
  expect(actual.sidebar).toBeCloseTo(500, 0); expect(actual.detail).toBeCloseTo(300, 0);
  await drag(page, "sidebar", 150); expect((await sizes(page)).sidebar).toBeCloseTo(200, 0);
  await drag(page, "sidebar", 500); await drag(page, "sidebar", 99);
  await expect(page.locator(".os-app-sidebar")).toBeHidden();
  expect((await page.locator(".os-shell-divider-sidebar").boundingBox())!.width).toBe(6);
  await page.locator(".os-shell-divider-sidebar").click();
  expect((await sizes(page)).sidebar).toBeCloseTo(500, 0);
  await drag(page, "detail", 700);
  expect((await page.getByRole("main").boundingBox())!.height).toBeGreaterThanOrEqual(200);
  await drag(page, "detail", 300);
  await drag(page, "detail", 80); expect((await sizes(page)).detail).toBeCloseTo(120, 0);
  await drag(page, "detail", 300); await drag(page, "detail", 50);
  await expect(page.locator("#shell-detail")).toBeHidden();
  expect((await page.locator(".os-shell-divider-detail").boundingBox())!.height).toBe(6);
  await page.locator(".os-shell-divider-detail").click();
  expect((await sizes(page)).detail).toBeCloseTo(300, 0);
  await page.locator(".os-shell-divider-sidebar").dblclick();
  actual = await sizes(page);
  expect(actual.sidebar).toBeCloseTo(360, 0); expect(actual.detail).toBeCloseTo(300, 0);
  await drag(page, "sidebar", 500);
  await page.locator(".os-shell-divider-detail").dblclick();
  actual = await sizes(page);
  expect(actual.sidebar).toBeCloseTo(500, 0); expect(actual.detail).toBeCloseTo(actual.content * .35, 0);
  await action(page, "view.toggleSidebar"); await action(page, "view.toggleDetail");
  expect((await sizes(page)).main).toBe(1440);
  await expect(page.getByRole("separator")).toHaveCount(0);
});

// spec: UI-125 [P1]
test("restart restores shares/visibility, and resize keeps the saved proportions", async ({ page, app }) => {
  expect(app.mode, "Neustart-Persistenz braucht die echte Bridge").toBe("bridge");
  await page.setViewportSize({ width: 1440, height: 900 });
  await app.open(page, "main"); await ready(page); await resetState(page);
  await drag(page, "sidebar", 500); await drag(page, "detail", 300);
  const content = (await sizes(page)).content;
  // Read-merge-write is asynchronous: verify persistence before destroying the page.
  await expect.poll(() => page.evaluate(async (content) => {
    const path = "/src/ipc/uistate.ts";
    const { uistate } = await import(/* @vite-ignore */ path);
    return (await uistate.load()).state.layout.detail.share * content;
  }, content)).toBeCloseTo(300, 0);
  await page.reload(); await ready(page);
  expect((await sizes(page)).sidebar).toBeCloseTo(500, 0);
  expect((await sizes(page)).detail).toBeCloseTo(300, 0);
  const before = await sizes(page);
  await page.setViewportSize({ width: 1600, height: 1000 });
  await expect.poll(async () => (await sizes(page)).sidebar).toBeCloseTo(1600 * 500 / 1440, 0);
  const resized = await sizes(page);
  expect(resized.detail / resized.content).toBeCloseTo(300 / before.content, 2);
  await action(page, "view.toggleDetail");
  // Wait for the shared one-second persistence debounce using the actual core state.
  await expect.poll(() => page.evaluate(async () => {
    const path = "/src/ipc/uistate.ts";
    const { uistate } = await import(/* @vite-ignore */ path);
    return (await uistate.load()).state.layout.detail.visible;
  })).toBe(false);
  await page.reload(); await ready(page);
  await expect(page.locator("#shell-detail")).toBeHidden();
});

for (const scale of [1, 1.25, 1.5, 2]) {
  // spec: UI-001
  test(`1024×640 collision check, DPR ${scale}`, async ({ browser, app }) => {
    const context = await browser.newContext({ viewport: { width: 1024, height: 640 }, deviceScaleFactor: scale });
    const page = await context.newPage();
    await app.open(page, "main"); await ready(page); await resetState(page);
    await expect(page.locator(".os-app-sidebar")).toBeHidden();
    await action(page, "view.toggleSidebar");
    const collisions = await page.evaluate(() => {
      const elements = [...document.querySelectorAll<HTMLElement>('button,input,select,textarea,a[href],[tabindex="0"]')]
        .filter((e) => e.getClientRects().length > 0 && getComputedStyle(e).visibility !== "hidden")
        .map((e) => ({ name: e.getAttribute("aria-label") ?? e.tagName, rect: e.getBoundingClientRect() }));
      const failures: string[] = [];
      for (let i = 0; i < elements.length; i++) {
        const a = elements[i]!;
        if (a.rect.x < -.5 || a.rect.y < -.5 || a.rect.right > innerWidth + .5 || a.rect.bottom > innerHeight + .5) failures.push(`${a.name}: outside viewport`);
        for (const b of elements.slice(i + 1)) {
          if (Math.min(a.rect.right, b.rect.right) - Math.max(a.rect.left, b.rect.left) > .5
            && Math.min(a.rect.bottom, b.rect.bottom) - Math.max(a.rect.top, b.rect.top) > .5) failures.push(`${a.name} intersects ${b.name}`);
        }
      }
      return failures;
    });
    expect(collisions).toEqual([]);
    await expect(page.getByRole("banner")).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Data views" })).toBeVisible();
    await expect(page.getByRole("main")).toBeVisible();
    await expect(page.getByRole("region", { name: "Details", exact: true })).toBeVisible();
    await expect(page.getByRole("complementary")).toBeVisible();
    await expect(page.getByRole("contentinfo")).toBeVisible();
    await context.close();
  });
}
