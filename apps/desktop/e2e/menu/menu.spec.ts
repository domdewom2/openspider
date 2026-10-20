import { test, expect } from "@playwright/test";
import { MENU_ORDER } from "../../src/ui/menu/types";
import { SHORTCUTS } from "../../src/ui/shortcuts";
for (const platform of ["mac", "win"] as const) for (const language of ["de", "en"]) {
  // spec: UI-002
  test(`${platform}/${language}: Menüfolge, Aktivierung und Hilfe-Dialog`, async ({ page }) => {
    await page.addInitScript(({ platform, language }) => {
      Object.defineProperty(navigator, "platform", { value: platform === "mac" ? "MacIntel" : "Win32" });
        Object.defineProperty(navigator, "userAgentData", { value: { platform: platform === "mac" ? "macOS" : "Windows" } });
      Object.defineProperty(navigator, "languages", { value: [language] });
    }, { platform, language });
    await page.goto("/?transport=mock");
    await expect.poll(() => page.evaluate(() => window.__osMenu?.model.lang)).toBe(language);
    const model = await page.evaluate(() => window.__osMenu!.model);
    expect(model.menus.map((m) => m.id)).toEqual(MENU_ORDER[platform]);
    const mode = model.menus.find((m) => m.id === "mode")!;
    expect(mode.items.map((i) => i.action)).toEqual(["mode.spider", "mode.list", "mode.serp", "mode.apis", "mode.compare"]);
    expect(mode.items.find((i) => i.id === "mode.spider")?.checked).toBe(true);
    const file = model.menus.find((m) => m.id === "file")!;
    expect(file.items.find((i) => i.id === "file.saveAs")?.enabled).toBe(false);
    expect(file.items.find((i) => i.id === "file.saveAs")?.disabledReason).toBeTruthy();
    // Das MCP-Menü gehört U25 (Start/Stopp erst bei verfügbarem Dienst); der Eintrag erscheint genau einmal.
    expect(model.menus.find((m) => m.id === "mcp")?.items.filter((i) => i.id === "mcp.start")).toHaveLength(1);
    await page.evaluate(async () => {
      // @ts-expect-error Vite lädt das Quellmodul im E2E-Browser.
      const { runAction } = await import("/src/ui/actions.ts");
      runAction("help.shortcuts", { source: "menu" });
    });
    const dialog = page.getByRole("dialog");
    await expect(dialog).toBeVisible();
    await expect(dialog.locator("[data-shortcut]")).toHaveCount(SHORTCUTS.length);
    await page.keyboard.press("Escape");
    await expect(dialog).not.toBeVisible();
  });
}
// spec: UI-002
test("Menüsprachwechsel aktualisiert auch einen geöffneten Hilfe-Dialog", async ({ page }) => {
  await page.goto("/?transport=mock");
  await expect(page.getByRole("main")).toBeVisible();
  await page.evaluate(async () => {
    // @ts-expect-error Vite-Quellmodul.
    const { runAction } = await import("/src/ui/actions.ts");
    // @ts-expect-error Vite-Quellmodul.
    const { usePrefs } = await import("/src/state/prefs.ts");
    runAction("help.shortcuts", { source: "menu" });
    await usePrefs.getState().patch({ general: { language: "de" } });
  });
  await expect(page.getByRole("dialog", { name: "Tastaturkürzel" })).toBeVisible();
  expect(await page.evaluate(() => window.__osMenu!.model.menus.find((m) => m.id === "file")?.label)).toBe("Datei");
});
