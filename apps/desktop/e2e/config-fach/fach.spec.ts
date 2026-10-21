import fachFixture from "../../src/features/config/pages/custom-search/fixtures/fach.json" with { type: "json" };
import { expect, test, type Page } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import { startBridge, type Bridge } from "../support/bridge";
import type {} from "../config/harness";
let bridge: Bridge | null = null;
test.beforeEach(async ({ page }) => {
  bridge = await startBridge();
  expect(bridge, "build os-app-bridge with e2e-bridge first").not.toBeNull();
  await page.goto(`/e2e/config/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
  await page.waitForFunction(() => !!window.cfgFixture);
});
test.afterEach(async () => { await bridge?.stop(); bridge = null; });
async function open(page: Page, anchor: string) {
  await page.evaluate(a => window.cfgFixture.open(a), anchor);
  return page.getByRole("dialog", { name: "Crawl configuration" });
}
const saved = (page: Page) => page.evaluate(() => window.cfgFixture.invoke("config.get")) as Promise<{ config: Record<string, Record<string, unknown>> }>;

// spec: INTEG-051
// spec: INTEG-040
// spec: INTEG-043
// spec: INTEG-048
test("the core validates XPath 3.1, CSS and regex and the dialog prevents saving invalid extractors", async ({ page }) => {
  const d = await open(page, "custom_extraction");
  await d.getByRole("button", { name: "Add extractor" }).click();
  const expression = d.getByRole("textbox", { name: "Expression" });
  await expression.fill("//div[");
  await expect(d.getByText("✗ Invalid", { exact: true })).toBeVisible();
  await expect(d.getByRole("button", { name: "OK", exact: true })).toBeDisabled();
  await expression.fill("for $x in //a return $x/@href");
  await expect(d.getByText("✓ Valid", { exact: true })).toBeVisible();
  await d.getByLabel("Output", { exact: true }).selectOption("function_value");
  await expression.fill("count(//h1)");
  await expect(d.getByRole("button", { name: "OK", exact: true })).toBeEnabled();
  await d.getByLabel("Mode", { exact: true }).selectOption("css");
  await expression.fill("div > ");
  await expect(d.getByText("✗ Invalid", { exact: true })).toBeVisible();
  await d.getByLabel("Mode", { exact: true }).selectOption("regex");
  await expression.fill("(");
  await expect(d.getByText("✗ Invalid", { exact: true })).toBeVisible();
  await d.getByLabel("Mode", { exact: true }).selectOption("css");
  await expression.fill("img.product");
  await d.getByRole("textbox", { name: "Attribute (optional)" }).fill("src");
  await expect(d.getByLabel("Output", { exact: true })).toBeDisabled();
  await d.getByRole("button", { name: "OK", exact: true }).click();
  await expect(d).toHaveCount(0);
  expect((await saved(page)).config.custom_extraction?.extractors).toEqual([{ name: "Extractor 1", mode: "css", expression: "img.product", attribute: "src", output: "text" }]);
});

// spec: INTEG-020
// spec: INTEG-026
// spec: INTEG-028
// spec: INTEG-032
test("search keeps the core regex dialect, XPath scope and bulk options across save and reload", async ({ page }) => {
  const d = await open(page, "custom_search");
  await d.getByRole("button", { name: "Add search filter", exact: true }).click();
  await d.getByLabel("Type", { exact: true }).selectOption("regex");
  await d.getByLabel("Search scope", { exact: true }).selectOption("xpath");
  await d.getByRole("textbox", { name: "XPath", exact: true }).fill("//main");
  await d.getByRole("textbox", { name: "Expression", exact: true }).fill("(?is)(?=.*rot)(?=.*blau)");
  await expect(d.getByText("✓ Valid", { exact: true })).toBeVisible();
  await d.getByRole("button", { name: "Bulk add search filters" }).click();
  const bulk = page.getByRole("dialog", { name: "Bulk add search filters" }).getByRole("group", { name: "Bulk add search filters" });
  await bulk.getByRole("textbox", { name: "One expression per line" }).fill("alpha\nalpha\nbeta");
  await bulk.getByRole("button", { name: "Import", exact: true }).click();
  await expect(d.getByText(/Skipped 1 empty or duplicate rows/)).toBeVisible();
  await d.getByRole("button", { name: "OK", exact: true }).click(); await expect(d).toHaveCount(0);
  const filters = (await saved(page)).config.custom_search?.filters as Array<Record<string, unknown>>;
  expect(filters).toHaveLength(3); expect(filters[0]).toMatchObject({ xpath: "//main", scope: "xpath", expression: "(?is)(?=.*rot)(?=.*blau)" });
  const again = await open(page, "custom_search");
  await expect(again.getByRole("textbox", { name: "Expression", exact: true }).first()).toHaveValue("(?is)(?=.*rot)(?=.*blau)");
});

// spec: CRAWL-134
// spec: INTEG-081
// spec: INTEG-082
// spec: TECH-300
test("German Fachseiten fit a small window, retain ordered defaults and have no serious accessibility findings", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 640 });
  await page.goto(`/e2e/config/index.html?bridge=${encodeURIComponent(bridge!.url)}&lang=de`);
  await page.waitForFunction(() => !!window.cfgFixture);
  await page.evaluate(async fixture => {
    const dto = await window.cfgFixture.invoke("config.get") as { config: Record<string, unknown> };
    dto.config = { ...dto.config, ...fixture };
    await window.cfgFixture.invoke("config.set", dto);
  }, fachFixture);
  await page.evaluate(() => window.cfgFixture.open("link_positions"));
  const d = page.getByRole("dialog");
  await expect(d.getByRole("group", { name: "Content", exact: true }).getByRole("button", { name: "Nach oben" })).toBeDisabled();
  for (const anchor of ["custom_search", "custom_extraction", "custom_js", "segments", "analysis", "compare.change_detection"]) {
    await page.evaluate(a => window.cfgFixture.open(a), anchor);
    await expect(d).toBeVisible();
    expect(await d.evaluate(el => el.scrollWidth <= el.clientWidth + 1)).toBe(true);
    const result = await new AxeBuilder({ page }).include('[role="dialog"]').analyze();
    expect(result.violations.filter(v => v.impact === "critical" || v.impact === "serious")).toEqual([]);
  }
  await expect(d.getByRole("checkbox", { name: "Content", exact: true })).toBeDisabled();
});

// spec: TECH-300
test("Content is selectable only when both real crawl files contain stored content", async ({ page }) => {
  const { createServer } = await import("node:http");
  const server = createServer((_, res) => { res.writeHead(200, { "content-type": "text/html" }); res.end("<!doctype html><html><head><title>Local content fixture</title></head><body><main><h1>Stored text</h1><p>A local page for comparison.</p></main></body></html>"); });
  await new Promise<void>(resolve => server.listen(0, "127.0.0.1", resolve));
  try {
    const address = server.address(); if (!address || typeof address === "string") throw new Error("missing local port");
    const url = `http://127.0.0.1:${address.port}/`;
    const crawl = async (store: boolean) => {
      const ref = await page.evaluate(async ({ url, store }) => {
        const dto = await window.cfgFixture.invoke("config.get") as { config: Record<string, Record<string, unknown>> };
        dto.config.extraction!.store_html = store;
        dto.config.duplicates!.near_enabled = false;
        dto.config.analysis!.auto_at_end = false;
        dto.config.robots!.mode = "ignore";
        dto.config.limits!.max_urls = 1;
        await window.cfgFixture.invoke("profile.save", { name: "capability-fixture", config: dto, overwrite: true });
        return await window.cfgFixture.invoke("crawl.start", { source: { kind: "spider", url }, scope: "domain", profile: "capability-fixture" }) as { crawlId: string; file: string };
      }, { url, store });
      await expect.poll(async () => page.evaluate(async id => {
        const list = await window.cfgFixture.invoke("crawls.list", { show: "all" }) as Array<{ crawl: { crawlId: string; phase: string } }>;
        return list.find(row => row.crawl.crawlId === id)?.crawl.phase;
      }, ref.crawlId)).toBe("completed");
      await page.evaluate(async () => {
        try { await window.cfgFixture.invoke("crawl.stop"); }
        catch (error) { if ((error as { cmdError?: { messageKey: string } }).cmdError?.messageKey !== "err-crawl-not-running") throw error; }
        await window.cfgFixture.invoke("crawl.clear");
      });
      return ref.file;
    };
    const without = await crawl(false); const first = await crawl(true); const second = await crawl(true);
    const compare = (current: string, previous: string) => page.evaluate(async ({ current, previous }) => {
      await window.cfgFixture.invoke("compare.open", { current, previous, mapping: [], elements: [], contentThresholdPct: null });
      return window.cfgFixture.invoke("compare.contentAvailable");
    }, { current, previous });
    expect(await compare(first, without)).toBe(false);
    expect(await compare(second, first)).toBe(true);
    const d = await open(page, "compare.change_detection");
    await expect(d.getByRole("checkbox", { name: "Content", exact: true })).toBeEnabled();
    await d.getByRole("button", { name: "Cancel", exact: true }).click();
    await page.evaluate(() => window.cfgFixture.invoke("compare.close"));
    expect(await page.evaluate(() => window.cfgFixture.invoke("compare.contentAvailable"))).toBe(false);
  } finally { await new Promise<void>((resolve, reject) => server.close(e => e ? reject(e) : resolve())); }
});
