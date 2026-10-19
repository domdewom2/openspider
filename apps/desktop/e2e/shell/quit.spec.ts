import { test, expect } from "@playwright/test";
import { startFakeBridge } from "../support/fake-bridge";

// spec: UI-001
test("quit confirmation uses the command contract, handles native notices, and has a default pause action", async ({ page }) => {
  const bridge = await startFakeBridge();
  bridge.core.handlers.set("app.requestQuit", () => ({ kind: "confirm", reasons: ["crawl", "analysis", "export", "mcpSessions"] }));
  bridge.core.handlers.set("app.confirmQuit", () => undefined);
  try {
    await page.goto(`/?bridge=${encodeURIComponent(bridge.url)}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async () => {
      const path = "/src/ui/actions.ts";
      (await import(/* @vite-ignore */ path)).runAction("app.quit", { source: "menu" });
    });
    const dialog = page.getByRole("dialog", { name: "Quit OpenSpider?" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByRole("button", { name: "Pause and quit" })).toBeFocused();
    await dialog.getByRole("button", { name: "Cancel" }).click();
    await expect(dialog).toBeHidden();
    expect(bridge.core.calls.filter((c) => c.cmd === "app.confirmQuit").map((c) => c.args)).toEqual([{ value: false }]);
    bridge.core.emit("main", { kind: "notice", notice: { level: "info", code: "app-quit-confirmation", args: { reasons: ["export"] } } });
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Enter");
    await expect(dialog).toBeHidden();
    expect(bridge.core.calls.filter((c) => c.cmd === "app.confirmQuit").map((c) => c.args)).toEqual([{ value: false }, { value: true }]);
  } finally { await bridge.stop(); }
});
