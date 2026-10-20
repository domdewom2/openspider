import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { SiteStructureArgs } from "../../../ipc/gen/SiteStructureArgs";
import type { DepthRow } from "../../../ipc/gen/DepthRow";
import type { SiteStructureView } from "../../../ipc/gen/SiteStructureView";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fixture from "../../../../e2e/sidebar-fach/fach-dto.json";
import { SidebarFrame } from "../frame/SidebarFrame";
import { useSidebarNavigation } from "../frame/state";
import { depthRows, depthSearch, flattenPaths, NOTHING_LOADED, pathSearch, pct } from "./model";

const depths = fixture.siteStructure.depths as Record<SiteStructureView, DepthRow[]>;
let core: MockCore;

beforeEach(async () => {
  core = new MockCore();
  core.handlers.set("sidebar.overview", () => ({ summary: [], tabs: [] }));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("sidebar.siteStructure", args => {
    const a = args as SiteStructureArgs;
    return a.path ? { paths: fixture.siteStructure.moreChildren, depths: [] } : { paths: fixture.siteStructure.paths, depths: depths[a.view] };
  });
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });

async function open() {
  render(<Providers><SidebarFrame /></Providers>);
  fireEvent.click(await screen.findByRole("tab", { name: "Site Structure" }));
  return screen.findByRole("table", { name: "Crawl depth distribution" });
}
const search = () => {
  const spec = useSidebarNavigation.getState().target?.spec;
  return spec && spec.kind === "tab" ? spec : undefined;
};

// spec: UI-066
it("starts at depth 1, folds everything from depth 10 into 10+ and keeps the sum identical in all three views", () => {
  const sums = (["indexability", "responseCodes", "crawlDepth"] as const).map(view => {
    const rows = depthRows(depths[view], view);
    expect(rows.map(r => r.label)).toEqual(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10+"]);
    expect(rows.at(-1)?.count).toBe(3);
    expect(rows.reduce((s, r) => s + r.pct, 0)).toBeCloseTo(100, 0);
    if (view === "indexability") expect(rows[2]!.parts).toEqual([{ key: "Indexable", value: 0 }, { key: "Non-Indexable", value: 1 }]);
    if (view === "responseCodes") expect(rows[3]!.parts.find(p => p.key === "4xx")?.value).toBe(1);
    if (view !== "crawlDepth") for (const r of rows) expect(r.parts.reduce((s, p) => s + p.value, 0)).toBe(r.count);
    return rows.reduce((s, r) => s + r.count, 0);
  });
  expect(new Set(sums)).toEqual(new Set([12]));
  // The start URL (depth 0) is never a row; deeper raw depths 10, 11, 12 are one bucket.
  const raw = [0, 1, 10, 11, 12].map(depth => ({ depth, count: 1, byClass: {} }));
  expect(depthRows(raw, "crawlDepth").map(r => [r.label, r.count])).toEqual([["1", 1], ["10+", 3]]);
  expect(pct(1, 3)).toBe(33.3);
});

// spec: UI-066
it("opens the first three path levels and lets the rest be toggled", () => {
  const paths = fixture.siteStructure.paths as never;
  const rows = flattenPaths(paths, new Set(), NOTHING_LOADED);
  expect(rows.map(r => [r.level, r.node.name, r.expanded])).toEqual([[1, "site.test", true], [2, "a/", true], [3, "b/", true], [4, "c/", false], [2, "blog/", false]]);
  const folded = flattenPaths(paths, new Set(["https://site.test/a/"]), NOTHING_LOADED);
  expect(folded.map(r => r.node.name)).toEqual(["site.test", "a/", "blog/"]);
});

// spec: UI-066
it("renders both tables with the specified columns and sets the Address search from a path click", async () => {
  await open();
  const tree = await screen.findByRole("treegrid", { name: "Directory structure" });
  expect(within(tree).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Path", "URLs", "Indexable", "Non-Indexable"]);
  expect(within(tree).getByRole("row", { name: /a\// })).toHaveTextContent("7");
  fireEvent.click(within(tree).getByRole("button", { name: "a/" }));
  expect(search()).toMatchObject({ tab: "Internal", filter: "All", search: { kind: "rules", tree: { children: [{ column: "Address", op: "startsWith", value: "https://site.test/a/" }] } } });
  expect(pathSearch("https://x/")).toMatchObject({ kind: "rules" });
  const depth = screen.getByRole("table", { name: "Crawl depth distribution" });
  expect(within(depth).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Depth (Clicks from Start URL)", "Number of URLs", "% of Total"]);
  const ten = within(depth).getByRole("row", { name: /10\+/ });
  expect(ten).toHaveTextContent("3");
  expect(ten).toHaveTextContent("25.0 %");
});

// spec: UI-066
it("loads deeper directories on demand with the path argument", async () => {
  await open();
  const tree = await screen.findByRole("treegrid");
  fireEvent.click(within(tree).getByRole("button", { name: "Expand: https://site.test/a/b/c/" }));
  expect(await within(tree).findByRole("button", { name: "x.html" })).toBeVisible();
  expect(core.calls.filter(c => c.cmd === "sidebar.siteStructure").some(c => (c.args as SiteStructureArgs).path === "https://site.test/a/b/c/")).toBe(true);
  expect(within(tree).queryByRole("button", { name: /Load more/ })).toBeNull();
});

// spec: UI-066
it("switches the view with stacked bars, absolute-number tooltips and bar clicks that add the segment of the bar", async () => {
  await open();
  expect(await screen.findByRole("button", { name: "3 – Non-Indexable: 1" })).toHaveAttribute("title", "3 – Non-Indexable: 1");
  fireEvent.click(screen.getByRole("button", { name: "3 – Non-Indexable: 1" }));
  expect(search()?.search).toMatchObject({ tree: { children: [{ column: "Crawl Depth", op: "equals", value: "3" }, { column: "Indexability", op: "equals", value: "Non-Indexable" }] } });
  fireEvent.change(screen.getByLabelText("View"), { target: { value: "responseCodes" } });
  const bar = await screen.findByRole("button", { name: "4 – 4xx: 1" });
  expect(bar.style.background).toContain("--os-status-4xx");
  fireEvent.click(bar);
  expect(search()?.search).toMatchObject({ tree: { children: [{ column: "Crawl Depth", value: "4" }, { column: "Status Code", op: "greaterThan", value: "399" }, { column: "Status Code", op: "lessThan", value: "500" }] } });
  fireEvent.click(screen.getByRole("button", { name: "10+ – 2xx: 3" }));
  expect(search()?.search).toMatchObject({ tree: { children: [{ column: "Crawl Depth", op: "greaterThan", value: "9" }, expect.anything(), expect.anything()] } });
  fireEvent.change(screen.getByLabelText("View"), { target: { value: "crawlDepth" } });
  await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.siteStructure").some(c => (c.args as SiteStructureArgs).view === "crawlDepth")).toBe(true));
  expect(depthSearch(10, "crawlDepth", "ignored")).toMatchObject({ tree: { children: [{ op: "greaterThan", value: "9" }] } });
});

// spec: UI-066
it("passes the global segment filter to the aggregate", async () => {
  useSidebarNavigation.setState({ scope: { segment: { kind: "segment", name: "Blog" } } });
  await open();
  await waitFor(() => expect(core.calls.some(c => c.cmd === "sidebar.siteStructure" && (c.args as SiteStructureArgs).segment?.kind === "segment")).toBe(true));
});
