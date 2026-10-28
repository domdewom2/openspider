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
        crawl[action] = async (signal?: AbortSignal) => {
          await new Promise(r => setTimeout(r, 500));
          const timing = { requested: performance.now(), resolved: 0 };
          if (action === "pause") Object.assign(globalThis, { __n03PauseIpc: timing });
          try { return await original(signal); } finally { timing.resolved = performance.now(); }
        };
      }
      await crawl.start({ source: { kind: "spider", url }, scope: "domain" });
    }, `${site.base}/`);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Running");
    const header = page.getByTestId("crawl-header");
    await page.evaluate(() => {
      const header = document.querySelector('[data-testid="crawl-header"]')!;
      const timing = { clicked: 0, acknowledged: 0 };
      header.addEventListener("pointerup", () => { timing.clicked = performance.now(); }, { once: true, capture: true });
      const observer = new MutationObserver(() => {
        if ([...header.querySelectorAll("button")].some(b => b.textContent === "Resume" && !b.disabled)) {
          timing.acknowledged = performance.now(); observer.disconnect();
        }
      });
      observer.observe(header, { subtree: true, childList: true, attributes: true, characterData: true });
      Object.assign(globalThis, { __n03PauseTiming: timing });
    });
    await header.getByRole("button", { name: "Pause", exact: true }).click();
    await expect(header.getByRole("button", { name: "Pausing …", exact: true })).toBeDisabled();
    await expect(header.getByRole("button", { name: "Stop", exact: true })).toBeDisabled();
    // Let the matcher retrieve the result even if accessibility traversal is slow.
    // The observer's pointer-up -> DOM budget is independently enforced below.
    await expect(header.getByRole("button", { name: "Resume", exact: true })).toBeEnabled({ timeout: 15_000 });
    const latency = await page.evaluate(() => {
      const timing = (globalThis as unknown as { __n03PauseTiming: { clicked: number; acknowledged: number } }).__n03PauseTiming;
      return timing.acknowledged - timing.clicked;
    });
    // Measure the actual DOM acknowledgement, excluding a role query over the large Overview.
    const ipc = await page.evaluate(() => (globalThis as unknown as { __n03PauseIpc: { requested: number; resolved: number } }).__n03PauseIpc);
    await test.info().attach("pause-ack-ms", { body: JSON.stringify({ latency, ipcMs: ipc.resolved - ipc.requested }), contentType: "application/json" });
    expect(latency).toBeGreaterThan(0);
    expect(latency).toBeLessThanOrEqual(3000);
    await header.getByRole("button", { name: "Stop", exact: true }).click();
    await expect(header.getByRole("button", { name: "Stopping …", exact: true })).toBeDisabled();
    await expect(page.getByTestId("crawl-phase")).toHaveText("Stopped", { timeout: 15_000 });
    await expect(header.getByRole("button", { name: "Resume", exact: true })).toHaveCount(0);
    expect(await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      try { await crawl.resume(); return true; } catch { return false; }
    })).toBe(false);
  } finally { await bridge.stop(); await site.stop(); }
});
