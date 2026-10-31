import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-001
// spec: UI-002
test("quit confirmation can cancel or pause active work before exit", async ({ page }) => {
  test.setTimeout(60_000);
  const site = await startSite(); const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async url => {
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      const dto = await config.get(); const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 2 } } });
      await crawl.start({ source: { kind: "spider", url }, scope: "domain" });
      const { app } = await import(/* @vite-ignore */ String("/src/ipc/app.ts"));
      await new Promise(r => setTimeout(r, 50));
      await app.requestQuit(); // Same kernel entry as the native close/exit callbacks.
    }, `${site.base}/`);
    const dialog = page.getByRole("dialog", { name: "Quit OpenSpider?", exact: true });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    await page.evaluate(async () => {
      const { runAction } = await import(/* @vite-ignore */ String("/src/ui/actions.ts"));
      await runAction("app.quit");
    });
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Pause and quit", exact: true }).click();
    await expect(dialog).toHaveCount(0);
    const phases = await page.evaluate(async () => {
      const { crawls } = await import(/* @vite-ignore */ String("/src/ipc/crawls.ts"));
      return (await crawls.list({ show: "all" })).map((entry: { crawl: { phase: string } }) => entry.crawl.phase);
    });
    expect(phases).toEqual(["paused"]);
    const idle = await page.evaluate(async () => {
      const { app } = await import(/* @vite-ignore */ String("/src/ipc/app.ts"));
      return app.requestQuit();
    });
    expect(idle).toEqual({ kind: "quit" });
  } finally { await bridge.stop(); await site.stop(); }
});
