import { test, expect } from "@playwright/test";
import { startBridge } from "../support/bridge";
import type {} from "./harness";

// spec: OUT-222
test("real bridge persists settings and supplies live hardware values without a mock fallback", async ({ page }) => {
  const bridge = await startBridge();
  try {
    expect(bridge, "real os-app bridge required").not.toBeNull();
    await page.goto(`/e2e/settings-sections/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.settingsFixture);
    await expect(page.getByLabel("Maximum cache memory (MB)")).toBeEnabled();
    await page.getByRole("tab", { name: "Exports", exact: true }).click();
    await page.getByLabel("Export folder").fill("/fixture/exports-u14");
    await page.getByRole("button", { name: "Apply: Export folder", exact: true }).click();
    await expect.poll(() => page.evaluate(() => window.settingsFixture.settings()?.export.directory)).toBe("/fixture/exports-u14");
    const settings = await page.evaluate(() => window.settingsFixture.invoke("settings.get")) as { export: { directory: string } };
    expect(settings.export.directory).toBe("/fixture/exports-u14");
    await page.reload();
    await page.getByRole("tab", { name: "Exports", exact: true }).click();
    await expect(page.getByLabel("Export folder")).toHaveValue("/fixture/exports-u14");
  } finally { await bridge?.stop(); }
});

// spec: OUT-270
test("real bridge accepts the object-shaped certificate import and validates it before trust changes", async ({ page }) => {
  const bridge = await startBridge();
  try {
    expect(bridge, "real os-app bridge required").not.toBeNull();
    await page.goto(`/e2e/settings-sections/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.settingsFixture);
    const paths = await page.evaluate(() => window.settingsFixture.importCertificate());
    expect(paths).toHaveLength(1);
    expect(paths[0]).toMatch(/\.pem$/);
    const before = await page.evaluate(() => window.settingsFixture.invoke("settings.get")) as { network: { trustedCertificates: string[] } };
    expect(before.network.trustedCertificates).toEqual([]);
    const saved = await page.evaluate(certificates => window.settingsFixture.invoke("settings.set", { patch: { network: { trustedCertificates: certificates } } }), paths) as { network: { trustedCertificates: string[] } };
    expect(saved.network.trustedCertificates).toEqual(paths);
    const rejected = await page.evaluate(async () => {
      try { await window.settingsFixture.invoke("settings.importCertificates", { certificates: ["invalid"] }); return false; }
      catch { return true; }
    });
    expect(rejected).toBe(true);
    const after = await page.evaluate(() => window.settingsFixture.invoke("settings.get")) as { network: { trustedCertificates: string[] } };
    expect(after.network.trustedCertificates).toEqual(paths);
  } finally { await bridge?.stop(); }
});
