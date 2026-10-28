import type { ScopeArgs } from "../../src/ipc/gen/ScopeArgs";
import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-018
// spec: UI-061
// spec: CRAWL-126
test("overview retains its last response during crawl updates and after resume", async ({ page }) => {
  test.setTimeout(60_000);
  const site = await startSite(); // fx-ui-basic includes 50 ms server delays.
  const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async startUrl => {
      const { client } = await import(/* @vite-ignore */ String("/src/ipc/client.ts"));
      const { sidebar } = await import(/* @vite-ignore */ String("/src/ipc/sidebar.ts"));
      const original = sidebar.overview;
      sidebar.overview = async (scope: ScopeArgs, signal?: AbortSignal) => {
        // Delay delivery of a REAL bridge response; do not replace its data.
        const response = await original(scope, signal);
        await new Promise(resolve => setTimeout(resolve, 50));
        if (signal?.aborted) throw new DOMException("Aborted", "AbortError");
        return response;
      };
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const dto = await config.get();
      const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 2 }, analysis: { ...root.analysis, auto_at_end: false } } });
      await client.invoke("crawl.start", { source: { kind: "spider", url: startUrl }, scope: "domain" });
    }, `${site.base}/`);
    const overview = page.locator(".os-sidebar-overview");
    await expect(overview).toBeVisible({ timeout: 15_000 });
    await page.evaluate(() => {
      const samples: { missing: number; updates: number } = { missing: 0, updates: 0 };
      const host = document.querySelector(".os-sidebar")!;
      const observer = new MutationObserver(() => {
        samples.updates++;
        if (!host.querySelector(".os-sidebar-overview")) samples.missing++;
      });
      observer.observe(host, { childList: true, subtree: true, characterData: true });
      Object.assign(window, { __n03Overview: { samples, observer } });
    });
    // Bound the sample to a live crawl: the fixture has 29 encountered URLs.
    await expect.poll(() => page.evaluate(async () => {
      const { useSession } = await import(/* @vite-ignore */ String("/src/state/session.ts"));
      return useSession.getState().crawl?.crawled ?? 0;
    })).toBeGreaterThanOrEqual(4);
    await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await crawl.pause();
    });
    await expect(page.getByTestId("crawl-phase")).toHaveText("Paused");
    await expect(overview).toBeVisible();
    await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await crawl.resume();
    });
    await expect.poll(() => page.evaluate(async () => {
      const { useSession } = await import(/* @vite-ignore */ String("/src/state/session.ts"));
      return useSession.getState().crawl?.crawled ?? 0;
    })).toBeGreaterThanOrEqual(8);
    await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await crawl.pause();
    });
    await expect(overview).toBeVisible();
    const result = await page.evaluate(() => {
      const state = (window as unknown as { __n03Overview: { samples: { missing: number; updates: number }; observer: MutationObserver } }).__n03Overview;
      state.observer.disconnect();
      return state.samples;
    });
    await test.info().attach("overview-mutations", { body: JSON.stringify(result), contentType: "application/json" });
    expect(result.updates).toBeGreaterThan(0);
    expect(result.missing, "No intermediate empty state after the first overview response").toBe(0);
  } finally {
    await bridge.stop();
    await site.stop();
  }
});
