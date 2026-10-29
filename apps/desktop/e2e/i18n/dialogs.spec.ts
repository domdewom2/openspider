import { expect, test, type Page, type TestInfo, type Locator } from "@playwright/test";
import type {} from "../config/harness";
import type {} from "../export/harness";
import type {} from "../integrations/harness";
import type {} from "../settings-sections/harness";

async function language(page: Page, lang: string) {
  await page.evaluate(async lang => {
    const path = "/src/i18n/index.ts";
    await (await import(/* @vite-ignore */ path)).setLanguage(lang);
  }, lang);
}

async function untranslated(root: Locator, page: Page) {
  const english = await page.evaluate(async () => {
    if (document.documentElement.lang !== "de") return [];
    const path = "/src/i18n/index.ts";
    const { loadFtlFiles } = await import(/* @vite-ignore */ path);
    const values = (lang: string) => new Map<string, string>(loadFtlFiles().filter((f: { lang: string }) => f.lang === lang).flatMap((f: { source: string }) =>
      [...f.source.matchAll(/^([\w-]+) = ([^\r\n{]+)$/gm)].map(m => [m[1], m[2].trim()] as [string, string])));
    const en = values("en"); const de = values("de");
    const equal = new Set([...en].filter(([k, v]) => de.get(k) === v).map(([, v]) => v));
    for (const value of ["Profile", "Domain"]) equal.add(value); // German plural/homograph, not untranslated English.
    return [...en].filter(([k, v]) => de.has(k) && de.get(k) !== v && !equal.has(v) && v.length > 1).map(([, v]) => v);
  });
  const hits = await root.evaluate((root, english) => {
    const known = new Set(english);
    const hits: string[] = [];
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const el = node.parentElement;
      if (!el || !el.getClientRects().length || getComputedStyle(el).visibility === "hidden" || el.closest("input, textarea, pre, code, td, [role=gridcell], [data-value], .os-mono")) continue;
      const text = node.textContent?.trim() ?? "";
      // User-defined names (including editable default link positions) are stored data.
      if (el.tagName === "LEGEND" && [...(el.parentElement?.querySelectorAll("input") ?? [])].some(input => input.value === text)) continue;
      if (known.has(text)) hits.push(text);
    }
    return hits;
  }, english);
  expect([...new Set(hits)], "untranslated English UI text").toEqual([]);
}

async function fit(dialog: Locator, page: Page, info: TestInfo, name: string) {
  await expect(dialog).toBeVisible();
  await untranslated(dialog, page);
  const box = await dialog.boundingBox();
  expect(box!.x).toBeGreaterThanOrEqual(0);
  expect(box!.y).toBeGreaterThanOrEqual(0);
  expect(box!.x + box!.width).toBeLessThanOrEqual(1025);
  expect(box!.y + box!.height).toBeLessThanOrEqual(641);
  const clipped = await dialog.locator("button, label, h1, h2, h3, [role=tab], .os-checkbox > span:last-child, .os-switch > span:last-child").evaluateAll(elements => elements.flatMap(el => {
    if (el.getClientRects().length === 0 || el.children.length > 0) return [];
    const e = el as HTMLElement;
    const range = document.createRange(); range.selectNodeContents(e);
    const text = range.getBoundingClientRect();
    if (!text.width || !text.height) return [];
    const own = getComputedStyle(e);
    const namedEllipsis = own.textOverflow === "ellipsis" && !!(e.title || e.getAttribute("aria-label"));
    // Overflow alone is not clipping: wrapped text can paint fully outside its inline/flex line box.
    // Compare the actual text rectangle only with ancestors that clip their content.
    for (let parent: HTMLElement | null = e; parent && parent !== document.body; parent = parent.parentElement) {
      const css = getComputedStyle(parent); const box = parent.getBoundingClientRect();
      const x = ["hidden", "clip"].includes(css.overflowX) && !namedEllipsis && (text.left < box.left - 1 || text.right > box.right + 1);
      const y = ["hidden", "clip"].includes(css.overflowY) && (text.top < box.top - 1 || text.bottom > box.bottom + 1);
      if (x || y) return [{ text: e.textContent, x, y }];
    }
    return [];
  }));
  expect(clipped, name).toEqual([]);
  await info.attach(name, { body: await page.screenshot(), contentType: "image/png" });
}

test.use({ viewport: { width: 1024, height: 640 } });
for (const lang of ["de", "en"]) {
  // spec: UI-121
  test(`${lang}: all configuration pages fit at 1024 × 640`, async ({ page }, info) => {
    test.setTimeout(120_000);
    await page.goto(`/e2e/config/index.html?transport=mock&lang=${lang}`);
    await page.waitForFunction(() => !!window.cfgFixture);
    await page.evaluate(() => window.cfgFixture.open());
    const dialog = page.getByRole("dialog").first();
    const buttons = dialog.getByRole("navigation").getByRole("button");
    await expect(buttons.first()).toBeVisible();
    for (let i = 0; i < await buttons.count(); i++) {
      await buttons.nth(i).click();
      await fit(dialog, page, info, `config-${lang}-${i}`);
    }
  });

  // spec: UI-121
  test(`${lang}: integration dialogs and their tabs fit at 1024 × 640`, async ({ page }, info) => {
    await page.goto("/e2e/integrations/index.html?transport=mock");
    await page.waitForFunction(() => !!window.integFixture);
    await language(page, lang);
    await page.evaluate(() => window.integFixture.open("ahrefs"));
    const dialog = page.getByRole("dialog").first();
    const tabs = dialog.getByRole("tab");
    await expect(tabs.first()).toBeVisible();
    for (let i = 0; i < await tabs.count(); i++) {
      await tabs.nth(i).click();
      await fit(dialog, page, info, `integrations-${lang}-${i}`);
    }
  });

  // spec: UI-121
  test(`${lang}: export dialogs use translated catalog names at 1024 × 640`, async ({ page }, info) => {
    await page.goto("/e2e/export/index.html?transport=mock");
    await page.waitForFunction(() => !!window.exportFixture);
    await language(page, lang);
    await page.evaluate(() => window.__osMock!.handlers.set("export.catalog", () => ({
      bulk: [{ key: "Links", labelKey: "Links", items: [{ name: "Links:All Inlinks", labelKey: "Links:All Inlinks", enabled: true }] }], reports: [], sitemaps: [],
    })));
    await page.evaluate(() => window.exportFixture.open({ kind: "multi" }));
    const dialog = page.getByRole("dialog").first();
    await expect(dialog.getByRole("checkbox", { name: lang === "de" ? "Links: Alle eingehenden Links" : "Links:All Inlinks", exact: true })).toBeVisible();
    await fit(dialog, page, info, `export-${lang}`);
    await page.keyboard.press("Escape");
    await page.evaluate(() => window.exportFixture.open({ kind: "sitemap", sitemapKind: "pages", listMode: true, hreflangAvailable: false }));
    await fit(page.getByRole("dialog").first(), page, info, `sitemap-${lang}`);
  });
}

for (const lang of ["de", "en"]) {
  // spec: UI-121
  test(`${lang}: settings, MCP and scheduling remain readable in a small window`, async ({ page }, info) => {
    test.setTimeout(120_000);
    await page.goto(`/e2e/settings-sections/index.html?transport=mock&lang=${lang}`);
    await page.waitForFunction(() => !!window.settingsFixture);
    const sections = page.getByRole("tablist").getByRole("tab");
    await expect(sections.first()).toBeVisible();
    for (let i = 0; i < await sections.count(); i++) {
      await sections.nth(i).click();
      await expect(sections.nth(i)).toHaveAttribute("aria-selected", "true");
      await untranslated(page.locator("main"), page);
      await info.attach(`settings-${lang}-${i}`, { body: await page.screenshot(), contentType: "image/png" });
    }
    await page.goto("/e2e/mcp/index.html?transport=mock");
    await page.getByRole("button", { name: "Open MCP", exact: true }).click();
    await language(page, lang);
    const mcp = page.getByRole("dialog").first();
    const tabs = mcp.getByRole("tab");
    await expect(tabs.first()).toBeVisible();
    for (let i = 0; i < await tabs.count(); i++) {
      await tabs.nth(i).click();
      await fit(mcp, page, info, `mcp-${lang}-${i}`);
    }
    await page.goto("/e2e/tasks/index.html?transport=mock");
    await page.getByRole("button", { name: "Scheduling", exact: true }).click();
    await language(page, lang);
    await fit(page.getByRole("dialog").first(), page, info, `tasks-${lang}`);
    await page.getByRole("button", { name: lang === "de" ? "Aufgabe hinzufügen" : "Add task", exact: true }).click();
    const editor = page.getByRole("dialog").last();
    const editorTabs = editor.getByRole("tab");
    await expect(editorTabs.first()).toBeVisible();
    for (let i = 0; i < await editorTabs.count(); i++) {
      await editorTabs.nth(i).click();
      await fit(editor, page, info, `task-editor-${lang}-${i}`);
    }
  });
}

for (const lang of ["de", "en"]) {
  // spec: UI-121
  test(`${lang}: visualisation labels follow the language without changing graph data`, async ({ page }, info) => {
    const { startVizBridge } = await import("../viz/viz-bridge");
    const { openViz } = await import("../viz/support");
    const bridge = await startVizBridge();
    try {
      await openViz(page, bridge, "crawlTree");
      await language(page, lang);
      await expect(page.getByTestId("viz-node-count")).toHaveText(lang === "de" ? "25 Knoten" : "25 nodes");
      await expect(page.getByRole("complementary", { name: lang === "de" ? "Einstellungen der Visualisierung" : "Visualisation settings" })).toBeVisible();
      await untranslated(page.locator("body"), page);
      await info.attach(`viz-${lang}`, { body: await page.screenshot(), contentType: "image/png" });
    } finally { await bridge.stop(); }
  });
}
