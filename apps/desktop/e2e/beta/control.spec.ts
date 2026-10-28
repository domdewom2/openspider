import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-003
// spec: CRAWL-126 [M2]
test("pause and stop lock immediately and a stopped paused crawl cannot resume", async ({ page }) => {
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
      for (const action of ["pause", "stop"] as const) {
        const original = crawl[action];
        crawl[action] = async (signal?: AbortSignal) => { await new Promise(r => setTimeout(r, 500)); return original(signal); };
      }
      await crawl.start({ source: { kind: "spider", url }, scope: "domain" });
    }, `${site.base}/`);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    const pauseAt = Date.now();
    await page.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(page.getByRole("button", { name: "Pausing …", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Stop", exact: true })).toBeDisabled();
    await expect(page.getByRole("button", { name: "Resume", exact: true })).toBeEnabled({ timeout: 3000 });
    expect(Date.now() - pauseAt).toBeLessThan(3000);
    await page.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(page.getByRole("button", { name: "Stopping …", exact: true })).toBeDisabled();
    await expect(page.getByTestId("crawl-phase")).toHaveText("Stopped", { timeout: 15_000 });
    await expect(page.getByRole("button", { name: "Resume", exact: true })).toHaveCount(0);
    expect(await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      try { await crawl.resume(); return true; } catch { return false; }
    })).toBe(false);
  } finally { await bridge.stop(); await site.stop(); }
});
