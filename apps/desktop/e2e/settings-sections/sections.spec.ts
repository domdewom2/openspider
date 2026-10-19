import { expect, test } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
import type {} from "./harness";

async function open(page: import("@playwright/test").Page, lang = "en") {
  await page.goto(`/e2e/settings-sections/index.html?transport=mock&lang=${lang}`);
  await page.waitForFunction(() => !!window.settingsFixture);
}

// spec: UI-123
test("RAM validation and persistence through a window reload", async ({ page }) => {
  await open(page);
  await expect(page.getByText(/Current memory use: 1 \/ 8 GB/)).toBeVisible();
  await page.getByLabel("RAM limit (GB)").fill("15");
  await expect(page.getByRole("button", { name: "Apply: RAM limit (GB)", exact: true })).toBeDisabled();
  await page.getByLabel("RAM limit (GB)").fill("6.5");
  await page.getByRole("button", { name: "Apply: RAM limit (GB)", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.settingsFixture.settings()?.storage.ramLimitGb)).toBe(6.5);
  await page.reload();
  await expect(page.getByLabel("RAM limit (GB)")).toHaveValue("6.5");
});

// spec: CRAWL-124 [M2]
test("risky-folder confirmation precedes save and an explicit move reports completion", async ({ page }) => {
  await open(page);
  await expect(page.getByLabel("Crawl folder")).toHaveValue("/mock/crawls");
  await page.getByLabel("Crawl folder").fill("/fixture/synced");
  await page.getByRole("button", { name: "Apply: Crawl folder", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Check crawl folder" })).toBeVisible();
  expect(await page.evaluate(() => window.settingsFixture.settings()?.storage.crawlsDir)).toBeNull();
  await page.getByRole("button", { name: "Use anyway" }).click();
  await page.getByRole("button", { name: "Move existing crawls", exact: true }).click();
  await expect(page.getByText("Moving existing crawls…", { exact: true })).toBeVisible();
  await page.evaluate(() => window.settingsFixture.completeMove());
  await expect(page.getByText("Crawls moved.", { exact: true })).toBeVisible();
});

// spec: OUT-229
test("retention activation explains the impact on saved comparisons", async ({ page }) => {
  await open(page);
  await page.getByRole("tab", { name: "Crawls", exact: true }).click();
  await page.getByRole("button", { name: /Crawl retention/ }).click();
  await page.getByRole("option", { name: "1 month", exact: true }).click();
  await expect(page.getByText(/those comparisons become unavailable/)).toBeVisible();
  await page.getByRole("button", { name: "Enable retention", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.settingsFixture.settings()?.retention.policy)).toBe("one_month");
  await page.getByRole("button", { name: /Crawl retention/ }).click();
  await page.getByRole("option", { name: "Custom (days)", exact: true }).click();
  await page.getByLabel("Retention in days").fill("365");
  expect(await page.evaluate(() => window.settingsFixture.settings()?.retention.policy)).toBe("one_month");
  await page.getByRole("button", { name: "Apply retention", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.settingsFixture.settings()?.retention.customDays)).toBe(365);
});

// spec: OUT-270
test("certificate discovery shows the issuer and waits for explicit trust", async ({ page }) => {
  await open(page);
  await page.getByRole("tab", { name: "Network", exact: true }).click();
  await page.getByLabel("HTTPS test URL").fill("https://fixture.test");
  await page.getByRole("button", { name: "Discover certificates" }).click();
  await expect(page.getByText(/Issuer: CN=OpenSpider U14 Fixture/)).toBeVisible();
  expect(await page.evaluate(() => window.settingsFixture.settings()?.network.trustedCertificates)).toEqual([]);
  await page.getByRole("button", { name: "Add discovered certificates" }).click();
  await expect.poll(() => page.evaluate(() => window.settingsFixture.settings()?.network.trustedCertificates)).toEqual(["/fixture/certificate.pem"]);
});

// spec: UI-122
test("all six sections remain accessible in German at 1024 by 640", async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 640 });
  await open(page, "de");
  for (const title of ["Speicher", "Crawls", "Exporte", "Netzwerk", "Konten", "MCP"]) {
    await page.getByRole("tab", { name: title, exact: true }).click();
    await expect(page.getByRole("heading", { name: title, exact: true })).toBeVisible();
    const issues = await new AxeBuilder({ page }).analyze();
    expect(issues.violations.filter((v) => v.impact === "critical" || v.impact === "serious").map((v) => v.id)).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
});
