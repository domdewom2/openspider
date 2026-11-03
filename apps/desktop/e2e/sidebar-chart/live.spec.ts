// UI-102 live run: a real crawl of fx-ui-basic (50 ms delay) over the real bridge; the table view of the chart area
// must agree with the Overview rows at every Overview update and after the crawl, and selecting calls no command.
import { test, expect, type Page } from "@playwright/test";
import { startAlphaBridge, startSite } from "../alpha/support";
import { area, BACKGROUND, treeTab } from "./support";

interface Snapshot { all: number | null; rows: [string, string, string][]; overview: Record<string, [string, string]>; crawled: number }

/** Chart table and Overview rows of Response Codes from the same rendered frame. */
function snapshot(page: Page): Promise<Snapshot> {
  return page.evaluate(() => {
    const text = (el: Element | null | undefined) => (el?.textContent ?? "").replace(/\s+/g, " ").trim();
    const rows = [...document.querySelectorAll(".os-ovc-table tbody tr")].map(tr => [text(tr.querySelector("th")), text(tr.children[1]), text(tr.children[2])] as [string, string, string]);
    const overview: Record<string, [string, string]> = {};
    const table = document.querySelector('.os-sidebar-overview table[aria-label="Response Codes"]');
    for (const tr of table?.querySelectorAll("tbody tr") ?? []) overview[text(tr.querySelector("th button"))] = [text(tr.children[1]), text(tr.children[2])];
    const crawled = Number(text(document.querySelector(".os-sidebar-overview table tbody tr:nth-child(2) td")).replace(/\D/g, "")) || 0;
    const all = overview.All ? Number(overview.All[0].replace(/\D/g, "")) : null;
    return { all, rows, overview, crawled };
  });
}

function compare(s: Snapshot): string[] {
  const problems: string[] = [];
  let sum = 0;
  for (const [name, count, pct] of s.rows) {
    if (name === "Unassigned") continue;
    const row = s.overview[name];
    if (!row) { problems.push(`${name}: no Overview row`); continue; }
    if (row[0] !== count || row[1] !== pct) problems.push(`${name}: chart ${count}/${pct}, overview ${row[0]}/${row[1]}`);
    sum += Number(count.replace(/\D/g, "")) || 0;
  }
  const rest = s.rows.find(r => r[0] === "Unassigned");
  if (s.all != null && (rest ? Number(rest[1].replace(/\D/g, "")) : 0) !== s.all - sum) problems.push(`unassigned ≠ ${s.all} − ${sum}`);
  return problems;
}

// spec: UI-102
// spec: UI-018
test("live crawl of fx-ui-basic: chart table equals the Overview rows at every update and after the crawl; no IPC on selection", async ({ page }) => {
  test.setTimeout(120_000);
  const site = await startSite();
  const bridge = await startAlphaBridge();
  try {
    await page.goto(`/?${new URLSearchParams({ bridge: bridge.url })}`);
    await expect(page.locator("[data-shell-ready]")).toHaveAttribute("data-shell-ready", "true");
    await page.evaluate(async startUrl => {
      const { client } = await import(/* @vite-ignore */ String("/src/ipc/client.ts"));
      const { config } = await import(/* @vite-ignore */ String("/src/ipc/config.ts"));
      const dto = await config.get();
      const root = dto.config as Record<string, Record<string, unknown>>;
      await config.set({ ...dto, config: { ...root, speed: { ...root.speed, threads: 1, max_urls_per_sec: 1 }, analysis: { ...root.analysis, auto_at_end: false } } });
      // Count every command from here on (UI-102: selecting an entry calls none).
      const w = window as unknown as { __v01Calls: string[] };
      w.__v01Calls = [];
      const invoke = client.invoke.bind(client);
      client.invoke = ((cmd: string, ...rest: unknown[]) => { w.__v01Calls.push(cmd); return (invoke as (...a: unknown[]) => unknown)(cmd, ...rest); }) as typeof client.invoke;
      await client.invoke("crawl.start", { source: { kind: "spider", url: startUrl }, scope: "domain" });
    }, `${site.base}/`);
    await expect(page.locator(".os-sidebar-overview")).toBeVisible({ timeout: 15_000 });
    await expect(treeTab(page, "Response Codes")).toBeVisible({ timeout: 15_000 });
    const calls = () => page.evaluate(() => (window as unknown as { __v01Calls: string[] }).__v01Calls.slice());
    const before = await calls();
    await treeTab(page, "Response Codes").click();
    await area(page).getByRole("button", { name: "Table" }).click();
    await expect(area(page).locator(".os-ovc-table")).toBeVisible();
    const after = await calls();
    // Background refreshes of the running crawl may interleave; none of them is caused by the selection.
    const added = after.slice(before.length).filter(c => !BACKGROUND.test(c) && !["sidebar.overview", "sidebar.countStatus", "analysis.status", "uistate.load", "uistate.save", "catalog.filterCounts", "view.rows", "view.open", "view.update", "sidebar.issues"].includes(c));
    expect(added).toEqual([]);
    const states = new Map<string, Snapshot>();
    const problems: string[] = [];
    const deadline = Date.now() + 90_000;
    let finished = false;
    while (Date.now() < deadline) {
      const s = await snapshot(page);
      if (s.rows.length && Object.keys(s.overview).length) {
        const key = JSON.stringify(s.overview);
        if (!states.has(key)) { states.set(key, s); problems.push(...compare(s)); }
      }
      const phase = await page.getByTestId("crawl-phase").textContent().catch(() => "");
      if (/Completed|Finished|Abgeschlossen/i.test(phase ?? "") && states.size >= 10) { finished = true; break; }
      await page.waitForTimeout(100);
    }
    await test.info().attach("live-states", { body: JSON.stringify({ states: states.size, finished, problems }), contentType: "application/json" });
    expect(problems).toEqual([]);
    expect(states.size, "at least 10 consecutive Overview updates compared").toBeGreaterThanOrEqual(10);
    await expect(page.getByTestId("crawl-phase")).toHaveText(/Completed|Finished/, { timeout: 60_000 });
    await expect.poll(async () => compare(await snapshot(page)), { timeout: 10_000 }).toEqual([]);
    const last = await snapshot(page);
    expect(last.all).toBeGreaterThan(0);
    // The selection survived every live update.
    await expect(area(page).locator("[data-ovc-entry-shown]")).toHaveCount(0);
    await expect(area(page)).toHaveAttribute("data-ovc-entry-shown", "tab:Response Codes");
  } finally {
    await bridge.stop();
    await site.stop();
  }
});
