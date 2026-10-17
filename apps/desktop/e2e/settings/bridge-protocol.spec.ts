import { expect, test } from "@playwright/test";
import { startFakeBridge, type FakeBridge } from "../support/fake-bridge";

// Der WebSocket-Transport des Frontends gegen ein protokollgleiches Bridge-Gerüst (docs/plan/M2.md 3.6). Unabhängig von
// `os-app-bridge`: sichert, dass Anfrage/Antwort, Fehlerformat und Ereignisse je Fenster im echten Browser funktionieren.
let bridge: FakeBridge;
test.beforeEach(async () => {
  bridge = await startFakeBridge();
});
test.afterEach(async () => {
  await bridge.stop();
});

const q = (b: FakeBridge, extra = "") => `?bridge=${encodeURIComponent(b.url)}${extra}`;

test.describe("Bridge-Transport im Browser", () => {
  // spec: UI-121
  test("Sprachwechsel läuft über die Bridge und erreicht das zweite Fenster als Ereignis", async ({ page, context }) => {
    await page.goto(`/${q(bridge)}`);
    await expect(page.getByRole("navigation", { name: "Data views" })).toBeVisible();
    await expect.poll(() => page.evaluate(() => window.__osRuntime?.transport)).toBe("bridge");
    await expect(page.getByTestId("version")).toHaveText("Version 0.0.0-mock");

    const settings = await context.newPage();
    await settings.goto(`/settings.html${q(bridge)}`);
    await settings.getByRole("button", { name: /Language/ }).click();
    await settings.getByRole("option", { name: "Deutsch" }).click();

    await expect(page.getByRole("navigation", { name: "Datenansichten" })).toBeVisible();
    expect(bridge.core.settings.general.language).toBe("de");
    expect(bridge.core.calls.some((c) => c.cmd === "settings.set")).toBe(true);
  });

  // spec: UI-120
  test("Theme-Wechsel im Kern erreicht ein geöffnetes Fenster ohne Neuladen (settingsChanged)", async ({ page }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await page.goto(`/${q(bridge)}`);
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    // Änderung von außen (z. B. anderes Fenster oder MCP): der Kern meldet sie als Ereignis
    bridge.core.settings = { ...bridge.core.settings, appearance: { ...bridge.core.settings.appearance, theme: "dark" } };
    bridge.core.emit("main", { kind: "settingsChanged", keys: ["appearance.theme"] });
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark", { timeout: 1000 });
  });
});
