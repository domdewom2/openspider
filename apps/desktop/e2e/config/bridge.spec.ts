import { expect, test, type Page } from "@playwright/test";
import { startBridge, type Bridge } from "../support/bridge";
import type {} from "./harness";

// E2E-A gegen den echten Kern (`os-app-bridge`): Konfiguration lesen/schreiben, Validierung durch den Kern, Regel-Tester mit
// der Funktion der Engine, robots-Tester und Profile.
let bridge: Bridge | null = null;
test.beforeEach(async ({ page }) => {
  bridge = await startBridge();
  expect(bridge, "real os-app bridge required (cargo build -p os-app --features e2e-bridge --bin os-app-bridge)").not.toBeNull();
  await page.goto(`/e2e/config/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
  await page.waitForFunction(() => !!window.cfgFixture);
});
test.afterEach(async () => {
  await bridge?.stop();
  bridge = null;
});

const dialogOf = (page: Page) => page.getByRole("dialog", { name: "Crawl configuration" });
const openDialog = async (page: Page, anchor?: string) => {
  await page.evaluate((a) => window.cfgFixture.open(a), anchor);
  const dialog = dialogOf(page);
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText("Loading configuration…")).toHaveCount(0);
  return dialog;
};
const get = async (page: Page) => (await page.evaluate(() => window.cfgFixture.invoke("config.get"))) as { config: Record<string, Record<string, unknown>> };

// spec: OUT-220
test("the dialog reads the core configuration, OK writes it and config.get returns it", async ({ page }) => {
  expect(await page.evaluate(() => window.cfgFixture.transport)).toBe("bridge");
  const dialog = await openDialog(page, "limits.max_urls");
  const urls = dialog.getByRole("textbox", { name: "Max. URLs", exact: true });
  await expect(urls).toHaveValue("5000000");
  await urls.fill("1234");
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await get(page)).config.limits?.max_urls).toBe(1234);
  const again = await openDialog(page, "limits.max_urls");
  await expect(again.getByRole("textbox", { name: "Max. URLs", exact: true })).toHaveValue("1234");
});

// spec: OUT-220
test("validation comes from the core: its message appears at the field and the draft is not written", async ({ page }) => {
  const dialog = await openDialog(page, "limits.max_urls");
  const urls = dialog.getByRole("textbox", { name: "Max. URLs", exact: true });
  await urls.fill("0");
  await expect(dialog.getByText(/muss zwischen 1 und/)).toBeVisible();
  await expect(urls).toHaveAttribute("aria-invalid", "true");
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(dialog).toBeVisible();
  expect((await get(page)).config.limits?.max_urls).toBe(5000000);
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await get(page)).config.limits?.max_urls).toBe(5000000);
});

// spec: CRAWL-077
test("the rendering mode is chosen in the dialog and kept by the core", async ({ page }) => {
  const dialog = await openDialog(page, "rendering.mode");
  await expect(dialog.getByRole("radio", { name: /Text only/ })).toBeChecked();
  await dialog.getByText("JavaScript (headless Chromium)", { exact: true }).click();
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await get(page)).config.rendering?.mode).toBe("javascript");
});

// spec: CRAWL-111
test("regex tester against the engine function: encoding, line number, partial-match hint, syntax errors", async ({ page }) => {
  const dialog = await openDialog(page, "scope.include");
  await dialog.getByRole("textbox", { name: "Include rules" }).fill(".*%20.*");
  await dialog.getByRole("textbox", { name: "Test URL" }).fill("https://site.test/a b");
  const result = dialog.getByTestId("regex-result");
  await expect(result).toHaveAttribute("data-verdict", "included");
  await expect(dialog.getByTestId("regex-match")).toContainText("Include line 1");
  await expect(result).toContainText("https://site.test/a%20b");

  await dialog.getByRole("textbox", { name: "Include rules" }).fill("https://site.test/blog");
  await dialog.getByRole("textbox", { name: "Test URL" }).fill("https://site.test/blog/post");
  await expect(result).toHaveAttribute("data-verdict", "notIncluded");
  await expect(dialog.getByTestId("regex-partial-hint")).toContainText("Include line 1 only matches part of the URL – add .* ?");
  await dialog.getByText("Rules also match parts of the URL", { exact: true }).click();
  await expect(result).toHaveAttribute("data-verdict", "included");

  await dialog.getByRole("textbox", { name: "Exclude rules" }).fill("https://site.test/blog/private\n(");
  await expect(dialog.getByTestId("regex-invalid")).toContainText("Exclude line 2");
  await expect(dialog.getByText(/Line 2 \(\(\)/)).toHaveCount(1);
  await dialog.getByRole("textbox", { name: "Test URL" }).fill("https://site.test/blog/private/x");
  await expect(result).toHaveAttribute("data-verdict", "excluded");
  await expect(dialog.getByTestId("regex-match")).toContainText("Exclude line 1");
});

// spec: CRAWL-107
test("rewrite test field uses the engine function and shows intermediate results", async ({ page }) => {
  const dialog = await openDialog(page, "rewrite.remove_parameters");
  await dialog.getByRole("textbox", { name: "Test URL" }).fill("https://site.test/old/page?utm_source=x&keep=1");
  await dialog.getByRole("textbox", { name: "Parameters to remove" }).fill("utm_source");
  await expect(dialog.getByTestId("rewrite-output")).toHaveText("https://site.test/old/page?keep=1");
  await dialog.getByRole("button", { name: "Add rule" }).click();
  await dialog.getByRole("textbox", { name: "Regular expression, rule 1" }).fill("/old/");
  await dialog.getByRole("textbox", { name: "Replacement, rule 1" }).fill("/new/");
  await expect(dialog.getByTestId("rewrite-output")).toHaveText("https://site.test/new/page?keep=1");
  await expect(dialog.getByRole("list", { name: "Intermediate results" }).getByRole("listitem")).toHaveCount(2);
  // Ohne Speichern: die gespeicherte Konfiguration bleibt unverändert.
  expect(((await get(page)).config.rewrite?.regex_replace as unknown[]).length).toBe(0);
});

// spec: CRAWL-102
test("custom robots.txt blocks /blog/ in the tester although the live file is not consulted; the file is stored in the configuration", async ({ page }) => {
  const dialog = await openDialog(page, "robots.mode");
  await dialog.getByRole("textbox", { name: "Subdomain" }).fill("site.test");
  await dialog.getByRole("button", { name: "Add", exact: true }).click();
  await dialog.getByRole("textbox", { name: "robots.txt for site.test" }).fill("User-agent: *\nAllow: /\nDisallow: /blog/\n");
  await dialog.getByRole("textbox", { name: "URL to test" }).fill("https://site.test/blog/post");
  await dialog.getByRole("button", { name: "Test", exact: true }).click();
  const result = dialog.getByTestId("robots-result");
  await expect(result).toHaveAttribute("data-allowed", "false");
  await expect(result).toContainText("Deciding line 3");
  await expect(result).toContainText("User agent: OpenSpider");
  await dialog.getByRole("textbox", { name: "URL to test" }).fill("https://site.test/ok");
  await dialog.getByRole("button", { name: "Test", exact: true }).click();
  await expect(result).toHaveAttribute("data-allowed", "true");
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await get(page)).config.robots?.custom_files).toEqual({ "site.test": "User-agent: *\nAllow: /\nDisallow: /blog/\n" });
});

// spec: OUT-220 [M2]
test("profiles round trip through the core: save, list, load as draft, default, delete", async ({ page }) => {
  let dialog = await openDialog(page, "limits.max_urls");
  await dialog.getByRole("textbox", { name: "Max. URLs", exact: true }).fill("4242");
  await dialog.getByRole("navigation").getByRole("button", { name: "Profiles", exact: true }).click();
  await dialog.getByRole("textbox", { name: "Profile name" }).fill("klein");
  await dialog.getByRole("button", { name: "Save", exact: true }).click();
  const row = dialog.locator('tr[data-profile="klein"]');
  await expect(row).toBeVisible();
  expect(await page.evaluate(() => window.cfgFixture.invoke("profile.list"))).toMatchObject([{ name: "klein", isDefault: false }]);
  // Abbrechen: die Sitzung bleibt unverändert, das Profil bleibt gespeichert.
  await dialog.getByRole("button", { name: "Cancel" }).click();
  expect((await get(page)).config.limits?.max_urls).toBe(5000000);

  dialog = await openDialog(page, "profiles");
  await dialog.getByRole("button", { name: "Load profile klein" }).click();
  await expect(dialog.getByText(/Profile "klein" loaded into the dialog/)).toBeVisible();
  expect((await get(page)).config.limits?.max_urls).toBe(5000000);
  await dialog.getByRole("button", { name: "OK", exact: true }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect((await get(page)).config.limits?.max_urls).toBe(4242);

  // Standard setzen und mit Rückfrage zurücksetzen (löscht default.osconfig).
  dialog = await openDialog(page, "profiles");
  await dialog.getByRole("button", { name: "Make profile klein the default" }).click();
  await expect(dialog.locator('tr[data-profile="klein"]')).toHaveAttribute("data-default", "true");
  await dialog.getByRole("button", { name: "Reset default" }).click();
  await page.getByRole("dialog", { name: "Reset default?" }).getByRole("button", { name: "Confirm" }).click();
  await expect(dialog.locator('tr[data-profile="klein"]')).not.toHaveAttribute("data-default", "true");
  expect(await page.evaluate(() => window.cfgFixture.invoke("profile.list"))).toMatchObject([{ name: "klein", isDefault: false }]);

  await dialog.getByRole("button", { name: "Delete profile klein" }).click();
  await page.getByRole("dialog", { name: "Delete profile?" }).getByRole("button", { name: "Confirm" }).click();
  await expect(dialog.getByTestId("profiles-empty")).toBeVisible();
  expect(await page.evaluate(() => window.cfgFixture.invoke("profile.list"))).toEqual([]);
});
