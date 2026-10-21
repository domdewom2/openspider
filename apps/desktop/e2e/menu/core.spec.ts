import { test, expect } from "../fixtures/app";

// spec: OUT-241 [P1], OUT-244 [P1], OUT-227 [M2]
test("Echte Bridge: effektive Debug-Pfade, Live-Log-Level und Diagnosevorschau", async ({ app, page }) => {
  expect(app.mode).toBe("bridge");
  await page.addInitScript(() => Object.defineProperty(navigator, "languages", { value: ["en"] }));
  await app.open(page, "main");
  await page.waitForFunction(() => window.__osMenu !== undefined);
  const paths = await page.evaluate(async () => {
    // @ts-expect-error Vite-Quellmodul.
    const { diag } = await import("/src/ipc/diag.ts");
    // @ts-expect-error Vite-Quellmodul.
    const { runAction } = await import("/src/ui/actions.ts");
    const info = await diag.debugInfo();
    runAction("help.debug", { source: "menu" });
    return Object.values(info.app.paths).filter((path) => typeof path === "string") as string[];
  });
  const dialog = page.getByRole("dialog", { name: "Debug information" });
  for (const path of paths) await expect(dialog.getByText(path, { exact: true })).toBeVisible();
  await dialog.getByRole("combobox", { name: "Log level" }).selectOption("debug");
  await expect.poll(() => page.evaluate(async () => {
    // @ts-expect-error Vite-Quellmodul.
    const { settings } = await import("/src/ipc/settings.ts");
    return (await settings.get()).logging.level;
  })).toBe("debug");
  await dialog.getByRole("button", { name: "Save logs…" }).click();
  const preview = dialog.getByRole("region", { name: "Files in the diagnostic bundle" });
  await expect(preview).toBeVisible();
  expect(await preview.getByRole("listitem").count()).toBeGreaterThan(0);
  await expect(dialog.getByRole("button", { name: "Save these files" })).toBeDisabled();
});
