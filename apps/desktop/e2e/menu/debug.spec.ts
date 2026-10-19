import { test, expect } from "@playwright/test";
import { debugFixture } from "../../src/features/help/__fixtures__/fx-ui-basic";

// spec: OUT-241 [P1], OUT-244 [P1], OUT-227 [M2]
test("Debug-Dialog: Pfade, sofortiger Log-Level, Aufbewahrung und bestätigte Dateivorschau", async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, "languages", { value: ["en"] }));
  await page.goto("/?transport=mock");
  await page.waitForFunction(() => window.__osMenu !== undefined);
  await page.evaluate(async (raw) => {
    const fixture = JSON.parse(raw);
    const core = window.__osMock!;
    core.handlers.set("diag.debugInfo", () => fixture);
    core.handlers.set("diag.previewBundle", () => ({ files: ["logs/openspider.log", "system.txt", "config-diff.json", "tasks.json"], warnings: [] }));
    core.handlers.set("diag.bundle", () => ({ opId: "bundle-e2e" }));
    // @ts-expect-error Vite-Quellmodul.
    const { runAction } = await import("/src/ui/actions.ts");
    runAction("help.debug", { source: "menu" });
  }, JSON.stringify(debugFixture));
  const dialog = page.getByRole("dialog", { name: "Debug information" });
  await expect(dialog).toBeVisible();
  for (const path of Object.values(debugFixture.app.paths)) if (path) await expect(dialog.getByText(path, { exact: true })).toBeVisible();
  await dialog.getByRole("combobox", { name: "Log level" }).selectOption("debug");
  await expect.poll(() => page.evaluate(() => window.__osMock!.settings.logging.level)).toBe("debug");
  await dialog.getByRole("spinbutton", { name: "Log retention in days" }).fill("14");
  await dialog.getByRole("button", { name: "Apply", exact: true }).click();
  await expect.poll(() => page.evaluate(() => window.__osMock!.settings.logging.retentionDays)).toBe(14);
  await dialog.getByRole("button", { name: "Save logs…" }).click();
  await expect(dialog.getByRole("region", { name: "Files in the diagnostic bundle" })).toBeVisible();
  await expect(dialog.getByText("system.txt")).toBeVisible();
  expect(await page.evaluate(() => window.__osMock!.calls.some((call) => call.cmd === "diag.bundle"))).toBe(false);
  await dialog.getByRole("textbox", { name: "Destination ZIP file" }).fill("/fixture/support.zip");
  await dialog.getByRole("button", { name: "Save these files" }).click();
  await expect.poll(() => page.evaluate(() => window.__osMock!.calls.find((call) => call.cmd === "diag.bundle")?.args)).toEqual({ dest: "/fixture/support.zip" });
});
