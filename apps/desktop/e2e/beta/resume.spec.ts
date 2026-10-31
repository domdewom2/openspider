import { test, expect } from "@playwright/test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startSite } from "../alpha/support";
import { persistentBridge } from "./persistent-bridge";

// spec: CRAWL-126 [M2]
// spec: UI-124
test("a paused crawl survives a real process restart, appears in Recent and resumes without duplicate URLs", async ({ page }) => {
  test.setTimeout(90_000);
  const home = mkdtempSync(join(tmpdir(), "os-n03-resume-"));
  const site = await startSite(); let bridge = await persistentBridge(home);
  try {
    const open = async () => {
      await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
      await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    };
    await open();
    const reference = await page.evaluate(async url => {
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      const dto = await config.get(); const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 2 }, analysis: { ...root.analysis, auto_at_end: false } } });
      return crawl.start({ source: { kind: "spider", url }, scope: "domain" });
    }, `${site.base}/`);
    await expect.poll(() => page.evaluate(async () => {
      const { useSession } = await import(/* @vite-ignore */ String("/src/state/session.ts"));
      return useSession.getState().crawl?.crawled ?? 0;
    })).toBeGreaterThanOrEqual(4);
    await page.evaluate(async () => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await new Promise(r => setTimeout(r, 50));
      await crawl.pause();
    });
    await expect(page.getByTestId("crawl-phase")).toHaveText("Paused");
    await page.goto("about:blank"); await bridge.stop(); bridge = await persistentBridge(home);
    await open();
    await expect.poll(() => page.evaluate(() => {
      const menu = window.__osMenu?.model;
      return JSON.stringify(menu?.menus.find(m => m.id === "file"));
    })).toContain(JSON.stringify(reference.file).slice(1, -1)); // Pfad im JSON-Text: Windows-Backslashes sind dort maskiert
    const listing = await page.evaluate(async () => {
      const { crawls } = await import(/* @vite-ignore */ String("/src/ipc/crawls.ts"));
      return crawls.list({ show: "all" });
    });
    expect(listing).toHaveLength(1); expect(listing[0]!.canResume).toBe(true);
    expect(listing[0]!.crawl.phase).toBe("paused");
    await page.evaluate(async () => {
      const { openCrawls } = await import(/* @vite-ignore */ String("/src/features/crawls/model.ts"));
      openCrawls();
    });
    const management = page.getByRole("dialog", { name: "Manage crawls…", exact: true });
    await expect(management).toBeVisible();
    await expect(management).toContainText(reference.crawlId);
    await management.getByRole("button", { name: "Close", exact: true }).click();
    await page.evaluate(async crawlId => {
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      await crawl.open({ kind: "id", crawlId }); await crawl.resume();
    }, reference.crawlId);
    await expect(page.getByTestId("crawl-phase")).toHaveText("Completed", { timeout: 30_000 });
    let addresses: string[] = [];
    await expect.poll(async () => {
      const rows = await page.evaluate(async () => {
      const { view } = await import(/* @vite-ignore */ String("/src/ipc/view.ts"));
      const handle = await view.open({ spec: { kind: "tab", tab: "Response Codes", filter: "All", search: null, sort: [] }, columns: ["Address"] });
      try { return (await view.rows({ viewId: handle.viewId, version: handle.version, offset: 0, limit: 100, columns: ["Address"] })).cells.map((row: unknown[]) => String(row[0])); }
      catch (error) {
        const { toCmdError } = await import(/* @vite-ignore */ String("/src/ipc/errors.ts"));
        // A tab generation can legitimately advance between open and rows when
        // the final change feed publishes. Retry only that documented condition.
        if (toCmdError(error).code === "Stale") return null;
        throw error;
      } finally { await view.close({ viewId: handle.viewId }); }
      });
      if (rows) addresses = rows;
      return rows?.length ?? 0;
    }, { timeout: 5000 }).toBe(27);
    // The table has 25 internal + 2 external URLs; robots/sitemap bookkeeping is excluded.
    expect(addresses).toHaveLength(27);
    expect(new Set(addresses).size).toBe(addresses.length);
  } finally { await bridge.stop(); await site.stop(); rmSync(home, { recursive: true, force: true }); }
});
