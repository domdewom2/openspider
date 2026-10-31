import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-129 [P1]
test("MCP read tools remain visible while a real catalogue response is delayed", async ({ page }) => {
  const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async () => {
      const { mcp } = await import(/* @vite-ignore */ String("/src/ipc/mcp.ts"));
      const original = mcp.tools;
      mcp.tools = async (signal?: AbortSignal) => { const rows = await original(signal); await new Promise(r => setTimeout(r, 50)); return rows; };
      const { openMcp } = await import(/* @vite-ignore */ String("/src/features/mcp/model.ts"));
      openMcp("tools");
    });
    const dialog = page.getByRole("dialog", { name: "MCP", exact: true });
    await expect(dialog.getByRole("checkbox", { name: "query_urls", exact: true })).toBeChecked();
    const missing = await page.evaluate(async () => {
      const host = document.querySelector('section[aria-label="Tools"]')!;
      let missing = 0;
      const observer = new MutationObserver(() => { if (!host.querySelector("table")) missing++; });
      observer.observe(host, { subtree: true, childList: true });
      const button = [...host.querySelectorAll("button")].find(b => b.textContent === "Refresh")!;
      button.click();
      await new Promise(r => setTimeout(r, 200));
      observer.disconnect();
      return missing;
    });
    expect(missing).toBe(0);
    await expect(dialog.getByRole("checkbox", { name: "query_urls", exact: true })).toBeChecked();
  } finally { await bridge.stop(); }
});

// spec: UI-080 [M2]
// spec: UI-082 [M2]
test("starting analysis from the menu shows running and a persistent completion", async ({ page }) => {
  test.setTimeout(60_000);
  const site = await startSite(); const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async url => {
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      const dto = await config.get(); const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, analysis: { ...root.analysis, auto_at_end: false } } });
      await crawl.start({ source: { kind: "spider", url }, scope: "domain" });
    }, `${site.base}/`);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Completed", { timeout: 20_000 });
    await page.evaluate(async () => {
      const { analysis } = await import(/* @vite-ignore */ String("/src/ipc/analysis.ts"));
      const original = analysis.start;
      analysis.start = async (args: Parameters<typeof original>[0], signal?: AbortSignal) => { await new Promise(r => setTimeout(r, 50)); return original(args, signal); };
      const { runAction } = await import(/* @vite-ignore */ String("/src/ui/actions.ts"));
      runAction("analysis.start");
    });
    await expect(page.getByTestId("analysis-feedback")).toContainText(/Analysis (running|completed)/);
    await expect(page.getByTestId("analysis-feedback")).toHaveText("Analysis completed", { timeout: 20_000 });
    await expect(page.getByTestId("analysis-feedback")).toHaveText("Analysis completed");
  } finally { await bridge.stop(); await site.stop(); }
});
