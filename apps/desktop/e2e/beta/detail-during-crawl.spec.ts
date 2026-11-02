import { test, expect } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";

// spec: UI-030
// spec: UI-032
// spec: UI-033
// spec: UI-035
// spec: UI-036
// spec: UI-145
// spec: UI-143
// spec: UI-142
test("detail pages survive writes between opening and reading; External stays responsive", async ({ page }) => {
  test.setTimeout(60_000);
  const site = await startSite();
  const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async url => {
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const { crawl } = await import(/* @vite-ignore */ String("/src/ipc/crawl.ts"));
      const dto = await config.get();
      const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 2 }, analysis: { ...root.analysis, auto_at_end: false } } });
      await crawl.start({ source: { kind: "spider", url }, scope: "domain" });
    }, `${site.base}/`);
    const result = await page.evaluate(async () => {
      const { view } = await import(/* @vite-ignore */ String("/src/ipc/view.ts"));
      const { useSession } = await import(/* @vite-ignore */ String("/src/state/session.ts"));
      const { setDetailSelection } = await import(/* @vite-ignore */ String("/src/features/detail/frame/selection.ts"));
      const { toCmdError } = await import(/* @vite-ignore */ String("/src/ipc/errors.ts"));
      const waitCount = async (count: number) => {
        const deadline = performance.now() + 15_000;
        while ((useSession.getState().crawl?.crawled ?? 0) < count) {
          if (performance.now() > deadline) throw new Error("crawl did not advance");
          await new Promise(r => setTimeout(r, 50));
        }
      };
      await waitCount(4);
      setDetailSelection({ urlIds: [1], focus: 1, topTab: "Internal" });
      const handles = [];
      for (const kind of ["inlinks", "outlinks", "images", "resources"] as const) {
        const spec = kind === "images" ? { kind, urlIds: [1], search: null, sort: [] } : { kind, urlIds: [1], filter: { id: "All" }, search: null, sort: [] };
        handles.push(await view.open({ spec, columns: ["From", "To"] }));
      }
      const before = useSession.getState().crawl!.crawled;
      await waitCount(before + 2);
      const sizes = [];
      for (const handle of handles) {
        // Artificial delay on a real handle, with confirmed concurrent writes.
        await new Promise(r => setTimeout(r, 50));
        try {
          const rows = await view.rows({ viewId: handle.viewId, version: handle.version, offset: 0, limit: 20, columns: ["From", "To"] });
          sizes.push(rows.cells.length);
        } finally { await view.close({ viewId: handle.viewId }); }
      }
      // UI-142: the crawl may give the fresh External view a new version between view.open and the first view.rows. Like the UI,
      // the stale answer is dropped: close, reopen, read again. externalMs runs from the first open to the first page actually
      // read (reopening included); a view that stays stale beyond MAX_STALE_REOPENS still fails.
      const MAX_STALE_REOPENS = 3;
      const started = performance.now();
      let externalMs: number | undefined;
      let externalReopens = 0;
      while (externalMs === undefined) {
        const external = await view.open({ spec: { kind: "tab", tab: "External", filter: "All", search: null, sort: [] }, columns: ["Address"] });
        try {
          if (external.state !== "ready") throw new Error("External was not ready within synchronous build budget");
          await view.rows({ viewId: external.viewId, version: external.version, offset: 0, limit: 20, columns: ["Address"] });
          externalMs = performance.now() - started;
        } catch (error) {
          if (toCmdError(error).code !== "Stale" || externalReopens >= MAX_STALE_REOPENS) throw error;
          externalReopens++;
        } finally { await view.close({ viewId: external.viewId }); }
      }
      return { sizes, externalMs, externalReopens, phase: useSession.getState().crawl?.phase };
    });
    expect(result.phase).toBe("running");
    expect(result.sizes).toHaveLength(4);
    for (const size of result.sizes) expect(size).toBeGreaterThan(0);
    expect(result.externalMs).toBeLessThan(300);
    await test.info().attach("live-detail-measurements", { body: JSON.stringify(result), contentType: "application/json" });
    const region = page.getByRole("region", { name: "Details", exact: true });
    for (const name of ["Inlinks", "Outlinks", "Image Details", "Resources"]) {
      const tab = region.getByRole("tab", { name, exact: true });
      if (await tab.count()) await tab.click();
      else { await region.getByRole("button", { name: "More detail tabs" }).click(); await page.getByRole("menuitem", { name, exact: true }).click(); }
      await expect(page.getByRole("tabpanel", { name, exact: true })).toContainText(site.base);
    }
  } finally { await bridge.stop(); await site.stop(); }
});
