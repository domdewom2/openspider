import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";
import type {} from "./harness";

// M3-K01 (K3-01): Chromium-Bereitstellung im Konfigurationsabschnitt Spider › Rendering, E2E-A mit Mock-Kern.
const open = async (page: Page, lang: "de" | "en", mode: "text" | "javascript") => {
  await page.goto(`/e2e/render-provision/index.html?transport=mock&lang=${lang}`);
  await page.waitForFunction(() => !!window.provFixture);
  await page.evaluate((m) => window.provFixture.setMode(m), mode);
  await page.evaluate(() => window.provFixture.open());
  const dialog = page.getByRole("dialog", { name: lang === "de" ? "Crawl-Konfiguration" : "Crawl configuration" });
  await expect(dialog).toBeVisible();
  return dialog.getByTestId("chromium-provision");
};

// spec: CRAWL-077 [P1]
// spec: OUT-260 [M3]
test("en: JavaScript mode without Chromium offers the download; progress, then the provisioned Chromium is used and can be removed", async ({ page }) => {
  const section = await open(page, "en", "javascript");
  const notice = section.getByTestId("chromium-prov-js-missing");
  await expect(notice).toContainText("JavaScript rendering needs Chromium, but none was found on this computer.");
  const axe = await new AxeBuilder({ page }).include('[role="dialog"]').analyze();
  expect(axe.violations.filter((v) => v.impact === "critical" || v.impact === "serious")).toEqual([]);
  await notice.getByRole("button", { name: /^Download Chromium \(/ }).click();
  await expect(section.getByRole("progressbar", { name: "Chromium download progress" })).toBeVisible();
  await expect(section.getByTestId("chromium-prov-active")).toContainText("Using the downloaded Chromium 156.0.8078.80.", { timeout: 15_000 });
  await expect(section.getByTestId("chromium-prov-js-missing")).toHaveCount(0);
  await expect(section.getByTestId("chromium-prov-installed")).toHaveText("Downloaded: version 156.0.8078.80");
  await section.getByRole("button", { name: "Remove downloaded Chromium" }).click();
  await expect(section.getByTestId("chromium-prov-active")).toHaveText("No Chromium found.");
  expect(await page.evaluate(() => window.provFixture.calls())).toEqual(expect.arrayContaining(["render_provision.download", "render_provision.remove"]));
});

// spec: OUT-260 [M3]
test("de: Abbruch lässt den Teil-Download liegen, Fortsetzen schließt ab", async ({ page }) => {
  const section = await open(page, "de", "text");
  await expect(section.getByRole("heading", { name: "Chromium für JavaScript-Rendering" })).toBeVisible();
  await expect(section.getByTestId("chromium-prov-js-missing")).toHaveCount(0);
  await page.evaluate(() => window.provFixture.hold(true));
  await section.getByRole("button", { name: /^Chromium herunterladen \(/ }).click();
  await expect(section.getByRole("status")).toContainText("Wird heruntergeladen: 93,0 MB von 186 MB");
  await section.getByRole("button", { name: "Abbrechen" }).click();
  await expect(section.getByTestId("chromium-prov-failure")).toContainText("Der Download wurde abgebrochen. Er kann fortgesetzt werden.");
  await page.evaluate(() => window.provFixture.hold(false));
  await section.getByRole("button", { name: /^Download fortsetzen \(/ }).click();
  await expect(section.getByTestId("chromium-prov-installed")).toHaveText("Heruntergeladen: Version 156.0.8078.80", { timeout: 15_000 });
  await expect(section.getByTestId("chromium-prov-active")).toContainText("Verwendet wird das heruntergeladene Chromium 156.0.8078.80.");
});
