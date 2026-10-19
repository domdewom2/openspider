import { expect, test } from "@playwright/test";
import { startBridge, type Bridge } from "../support/bridge";
import type {} from "./harness";

// E2E-A gegen den echten Kern (`os-app-bridge`, API-Rahmen M2-I01). Keine Verbindung zu einem echten Anbieter: geprüft
// werden nur Liste, Status, Einstellungen und der APIs-Modus ohne verbundene Integration.
let bridge: Bridge | null = null;
test.beforeEach(async ({ page }) => {
  bridge = await startBridge();
  expect(bridge, "real os-app bridge required (cargo build -p os-app --features e2e-bridge --bin os-app-bridge)").not.toBeNull();
  await page.goto(`/e2e/integrations/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
  await page.waitForFunction(() => !!window.integFixture);
});
test.afterEach(async () => {
  await bridge?.stop();
  bridge = null;
});

// spec: INTEG-001
test("the core lists six integrations, all not connected; options are saved without secrets", async ({ page }) => {
  await page.evaluate(() => window.integFixture.open("majestic"));
  const dialog = page.getByRole("dialog", { name: "API access" });
  await expect(dialog.getByRole("tablist", { name: "Integrations" }).getByRole("tab")).toHaveText([
    "Google Analytics 4Not connected",
    "Search ConsoleNot connected",
    "PageSpeed InsightsNot connected",
    "MajesticNot connected",
    "AhrefsNot connected",
    "MozNot connected",
  ]);
  await dialog.getByLabel("API key").fill("sk-TEST-INTEG-002");
  await dialog.getByText("Fetch data from this source during crawls").click();
  await expect(dialog.getByRole("switch", { name: "Fetch data from this source during crawls" })).toBeChecked();
  await dialog.getByRole("tab", { name: "Metrics" }).click();
  await dialog.getByText("Domain", { exact: true }).click();
  await expect(dialog.getByRole("checkbox", { name: "Domain", exact: true })).toBeChecked();
  await dialog.getByRole("button", { name: "OK" }).click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  const saved = (await page.evaluate(() => window.integFixture.invoke("api.getSettings", { kind: "majestic" }))) as { values: Record<string, unknown> };
  expect(saved.values).toMatchObject({ enabled: true, levels: ["domain"] });
  expect(JSON.stringify(saved)).not.toContain("sk-TEST-INTEG-002");
});

// spec: INTEG-012
test("APIs mode without a connected integration starts no run and adds no crawl", async ({ page }) => {
  await expect(page.getByTestId("api-progress")).toHaveCount(0);
  await page.evaluate(() => window.integFixture.action("mode.apis"));
  const bar = page.getByRole("region", { name: "APIs mode" });
  await bar.getByRole("textbox", { name: "URLs (one per line)" }).fill("https://integ.test/a\nhttps://integ.test/a\nhttps://integ.test/b\n");
  await expect(bar.getByText("2 valid, 0 invalid, 1 duplicates")).toBeVisible();
  await bar.getByRole("button", { name: "Fetch API data" }).click();
  await expect(bar.getByText("Connect at least one integration first. Nothing was started.")).toBeVisible();
  expect(await page.evaluate(() => window.integFixture.crawl())).toBeNull();
  expect(await page.evaluate(() => window.integFixture.invoke("crawls.list", { show: "all" }))).toEqual([]);
});
