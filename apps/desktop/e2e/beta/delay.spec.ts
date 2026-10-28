import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

for (const applied of [false, true]) {
  // spec: CRAWL-145
  // spec: UI-013
  test(`Crawl-delay status explains the active option (applied=${applied})`, async ({ page }) => {
    const site = await startSite(); const bridge = await startAlphaBridge();
    try {
      await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
      await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
      const defaultValue = await page.evaluate(async ({ base, applied }) => {
        const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
        const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
        const dto = await config.get(); const root = dto.config as Record<string, Record<string, unknown>>;
        const defaultValue = root.robots!.respect_crawl_delay;
        await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 2 }, robots: { ...root.robots, respect_crawl_delay: applied, custom_files: { [new URL(base).hostname]: "User-agent: *\nCrawl-delay: 1\n" } } } });
        await crawl.start({ source: { kind: "spider", url: `${base}/` }, scope: "domain" });
        return defaultValue;
      }, { base: site.base, applied });
      expect(defaultValue).toBe(false);
      const status = page.getByTestId("crawl-status");
      await expect(status).toContainText(applied ? /Rate limit: Crawl-delay 1.0 s in robots.txt – applied/ : /Crawl-delay 1.0 s in robots.txt – not applied/);
      await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    } finally { await bridge.stop(); await site.stop(); }
  });
}
