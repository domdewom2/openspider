import { test, expect } from "@playwright/test";
import { SHORTCUTS, parseCombo, findReservedCollisions, findDuplicateBindings } from "../../src/ui/shortcuts";

for (const platform of ["mac", "win"] as const) {
  test.describe(`UI-113 ${platform}`, () => {
    test.beforeEach(async ({ page }) => {
      await page.clock.setFixedTime(new Date("2026-10-01T12:00:00Z"));
      await page.addInitScript((platform) => {
        Object.defineProperty(navigator, "platform", { value: platform === "mac" ? "MacIntel" : "Win32" });
        Object.defineProperty(navigator, "userAgentData", { value: { platform: platform === "mac" ? "macOS" : "Windows" } });
      }, platform);
      await page.goto("/?transport=mock");
      await page.waitForFunction(() => window.__osMenu !== undefined);
      await page.evaluate(async (ids) => {
        // @ts-expect-error Vite-Quellmodul: Fixture-Handler verhindert Browser-Standardaktionen.
        const { registerAction } = await import("/src/ui/actions.ts");
        ids.forEach((id: string) => registerAction(id, () => {}));
        const grid = document.querySelector("main")!;
        grid.setAttribute("role", "grid"); grid.setAttribute("tabindex", "0");
        grid.addEventListener("os:test-focus", () => (grid as HTMLElement).focus());
        const input = document.createElement("input"); input.setAttribute("aria-label", "Fixture search"); grid.append(input);
        (window as unknown as { actions: string[] }).actions = [];
        document.addEventListener("os:action", (e) => (window as unknown as { actions: string[] }).actions.push((e as CustomEvent).detail.id));
      }, SHORTCUTS.map((entry) => entry.id));
    });
    for (const shortcut of SHORTCUTS) {
      // spec: UI-113 [P1]
      test(`${shortcut.id}: Registerbelegung sendet das Aktionsereignis`, async ({ page }) => {
        const combo = parseCombo(shortcut.keys[platform], platform);
        const key = ({ Left: "ArrowLeft", Right: "ArrowRight", Esc: "Escape", Del: "Delete" } as Record<string, string>)[combo.key] ?? combo.key;
        const press = [combo.ctrl && "Control", combo.alt && "Alt", combo.shift && "Shift", combo.cmd && "Meta", key].filter(Boolean).join("+");
        if (shortcut.id === "search.clear") await page.getByRole("textbox", { name: "Fixture search" }).focus();
        else await page.getByRole("grid").focus();
        await page.keyboard.press(press);
        await expect.poll(() => page.evaluate(() => (window as unknown as { actions: string[] }).actions)).toEqual([shortcut.id]);
      });
    }
    // spec: UI-113
    test("Textfeld behält Kopieren, Markieren, Löschen und Navigation; Dialog sperrt globale Kürzel", async ({ page }) => {
      const input = page.getByRole("textbox", { name: "Fixture search" });
      await input.fill("fixture text"); await input.focus();
      const mod = platform === "mac" ? "Meta" : "Control";
      for (const key of [`${mod}+a`, `${mod}+c`, "Backspace", "Delete", platform === "mac" ? "Meta+[" : "Alt+ArrowLeft"]) await page.keyboard.press(key);
      expect(await page.evaluate(() => (window as unknown as { actions: string[] }).actions)).toEqual([]);
      await page.evaluate(() => { const dialog = document.createElement("div"); dialog.setAttribute("aria-modal", "true"); document.body.append(dialog); });
      await page.getByRole("grid").focus();
      await page.keyboard.press(`${mod}+n`);
      expect(await page.evaluate(() => (window as unknown as { actions: string[] }).actions)).toEqual([]);
    });
    // spec: UI-113 [P1]
    test("Aliase Backspace und Kontextmenütaste senden dieselbe Aktion", async ({ page }) => {
      await page.getByRole("grid").focus();
      await page.keyboard.press("Backspace");
      await page.keyboard.press("ContextMenu");
      expect(await page.evaluate(() => (window as unknown as { actions: string[] }).actions)).toEqual(["table.remove", "context.menu"]);
    });
  });
}
// spec: UI-113 [P1]
test("Keine Kollision mit Systemkürzeln oder anderen Aktionen", () => {
  expect(findReservedCollisions()).toEqual([]);
  expect(findDuplicateBindings()).toEqual([]);
});

for (const platform of ["mac", "win"] as const) {
  // spec: UI-002
  // spec: UI-113 [P1]
  test(`${platform}: Hilfedialog und vollständiges Register stimmen überein`, async ({ page }) => {
    await page.addInitScript((platform) => {
      Object.defineProperty(navigator, "platform", { value: platform === "mac" ? "MacIntel" : "Win32" });
      Object.defineProperty(navigator, "userAgentData", { value: { platform: platform === "mac" ? "macOS" : "Windows" } });
    }, platform);
    await page.goto("/?transport=mock");
    await page.waitForFunction(() => window.__osMenu !== undefined);
    await page.evaluate(async () => {
      const path = "/src/ui/actions.ts";
      (await import(/* @vite-ignore */ path)).runAction("help.shortcuts", { source: "menu" });
    });
    await expect(page.getByRole("dialog")).toBeVisible();
    await expect(page.locator("[data-shortcut]")).toHaveCount(SHORTCUTS.length);
    expect(await page.locator("[data-shortcut]").evaluateAll((rows) => rows.map((row) => row.getAttribute("data-shortcut")))).toEqual(SHORTCUTS.map((s) => s.id));
    await page.keyboard.press("Escape");
    await expect(page.getByRole("dialog")).toHaveCount(0);
  });
}
