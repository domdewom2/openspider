import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Providers } from "../../../app/Providers";
import { loadFtlFiles, setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { useSession } from "../../../state/session";
import type { CrawlStatus } from "../../../ipc/gen/CrawlStatus";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { Tick } from "../../../ipc/gen/Tick";
import tick from "../../../ipc/__fixtures__/Tick.json";
import fxChart from "../../../../../../tests/fixtures/ui/fx-ui-chart/overview.json";
import fxUnassigned from "../../../../../../tests/fixtures/ui/fx-ui-chart/overview-unassigned.json";
import { useComparison } from "../../compare/model";
import { useSegments } from "../../table-advanced/stores";
import { SidebarFrame } from "../frame/SidebarFrame";
import { CHART_LAYOUT_KEY, useChartLayout } from "../frame/layout";
import { useSidebarNavigation } from "../frame/state";
import { useOverviewSelection } from "../overview/selection";
import { flushUiState } from "../../../state/uistate";
import { OverviewChartArea } from "./OverviewChart";

let core: MockCore;
let answer: OverviewDto;
const crawl = tick.crawl as CrawlStatus;
beforeEach(async () => {
  core = new MockCore();
  answer = structuredClone(fxChart) as unknown as OverviewDto;
  core.handlers.set("sidebar.overview", () => structuredClone(answer));
  core.handlers.set("analysis.status", () => ({ running: false, steps: [] }));
  core.handlers.set("analysis.start", () => ({ opId: "analysis-1", labelKey: "analysis", cancellable: true }));
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  useOverviewSelection.setState({ crawl: null, entry: null });
  useChartLayout.getState().reset();
  useSegments.setState({ segments: [], filter: null });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.useRealTimers(); });

const mount = () => render(<Providers><SidebarFrame /></Providers>);
const area = () => screen.getByRole("region", { name: "Chart of the selected entry" });
const ipcCalls = () => core.calls.filter(c => c.cmd !== "diag.logFrontend").length;
async function ready() {
  mount();
  await screen.findByRole("table", { name: "Summary" });
  await waitFor(() => expect(useChartLayout.getState().loaded).toBe(true));
}
const selectTab = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
function item(key: string): HTMLElement {
  const el = area().querySelector<HTMLElement>(`[data-ovc-key="${key}"]`);
  if (!el) throw new Error(`no chart item ${key}`);
  return el;
}
const newVersion = (seq: number) => act(() => useSession.getState().applyTick({ ...tick, seq, countsVersion: seq, sidebarVersion: seq } as unknown as Tick));

describe("UI-102 chart area in the Overview tab", () => {
  // spec: UI-102
  it("empty state without selection, chart of the selected entry, and no IPC call when selecting", async () => {
    await ready();
    expect(area()).toHaveTextContent("Select an entry in the overview to see its breakdown.");
    const before = ipcCalls();
    selectTab("Response Codes");
    const ring = await within(area()).findByRole("img", { name: "Response Codes: breakdown of 1,000 URLs" });
    expect(ring.querySelectorAll("path.os-ovc-segment")).toHaveLength(6);
    expect(within(ring).getByText("1,000")).toBeInTheDocument();
    // Description of the three largest categories (UI-126).
    const desc = document.getElementById(ring.getAttribute("aria-describedby")!)!;
    expect(desc).toHaveTextContent("Success (2XX): 812 (81.2 %); Redirection (3XX): 96 (9.6 %); Client Error (4XX): 61 (6.1 %)");
    fireEvent.click(screen.getByRole("button", { name: "Summary" }));
    expect(await within(area()).findByRole("img", { name: "Summary: breakdown of 2,500 URLs" })).toBeInTheDocument();
    selectTab("Page Titles");
    expect(await within(area()).findByText("A URL can be in several categories – the shares do not add up to 100 %.")).toBeInTheDocument();
    expect(ipcCalls()).toBe(before);
  });

  // spec: UI-102
  // spec: UI-061 [M3]
  it("a filter row selects and navigates, highlights its category; the arrow keeps collapsing; selection survives collapse and tab switch", async () => {
    await ready();
    fireEvent.click(screen.getByRole("button", { name: "Client Error (4XX)" }));
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Response Codes", filter: "Client Error (4XX)" });
    await waitFor(() => expect(item("f:Client Error (4XX)")).toHaveAttribute("data-highlight", "true"));
    expect(area().querySelector('.os-ovc-mark[data-kind="highlight"]')).not.toBeNull();
    // `All` shows the chart without highlight.
    fireEvent.click(within(screen.getByRole("table", { name: "Response Codes" })).getByRole("button", { name: "All" }));
    await waitFor(() => expect(area().querySelector("[data-highlight]")).toBeNull());
    // UI-061: the arrow collapses and expands; the selection of the block stays.
    fireEvent.click(screen.getByRole("button", { name: "▾ Response Codes" }));
    expect(screen.queryByRole("table", { name: "Response Codes" })).not.toBeInTheDocument();
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "filter", tab: "Response Codes", filter: "All" });
    expect(within(area()).getByRole("img", { name: /Response Codes/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Issues" }));
    fireEvent.click(screen.getByRole("tab", { name: "Overview" }));
    expect(await within(area()).findByRole("img", { name: /Response Codes/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "▸ Response Codes" }));
    expect(screen.getByRole("table", { name: "Response Codes" })).toBeInTheDocument();
  });

  // spec: UI-061 [M3]
  it("tree keyboard: arrow up/down moves the selection, right/left collapse and expand a tab block", async () => {
    await ready();
    const summary = screen.getByRole("button", { name: "Summary" });
    summary.focus();
    fireEvent.keyDown(summary, { key: "ArrowDown" });
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "tab", tab: "Internal" });
    const internal = screen.getByRole("button", { name: "Internal" });
    expect(internal).toHaveFocus();
    expect(internal).toHaveAttribute("aria-current", "true");
    fireEvent.keyDown(internal, { key: "ArrowDown" });
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "filter", tab: "Internal", filter: "All" });
    expect(useSidebarNavigation.getState().target).toBeNull();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowLeft" });
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "tab", tab: "Internal" });
    fireEvent.keyDown(document.activeElement!, { key: "ArrowLeft" });
    expect(screen.queryByRole("table", { name: "Internal" })).not.toBeInTheDocument();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowRight" });
    expect(screen.getByRole("table", { name: "Internal" })).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowUp" });
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "summary" });
  });

  // spec: UI-102
  it("click targets: segment and legend apply tab + filter, summary categories add the condition, residual categories do not navigate", async () => {
    await ready();
    selectTab("Response Codes");
    const segment = await waitFor(() => {
      const el = area().querySelector<SVGPathElement>('[data-ovc-segment="f:Client Error (4XX)"]');
      if (!el) throw new Error("segment");
      return el;
    });
    fireEvent.click(segment);
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Response Codes", filter: "Client Error (4XX)" });
    useSidebarNavigation.setState({ target: null });
    fireEvent.click(item("f:Success (2XX)"));
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Response Codes", filter: "Success (2XX)" });
    fireEvent.click(screen.getByRole("button", { name: "Summary" }));
    await waitFor(() => expect(item("nonIndexable")).toBeTruthy());
    fireEvent.click(item("nonIndexable"));
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Internal", filter: "HTML", search: { kind: "rules", tree: { children: [{ column: "Indexability", op: "equals", value: "Non-Indexable" }] } } });
    useSidebarNavigation.setState({ target: null });
    fireEvent.click(item("hidden"));
    expect(useSidebarNavigation.getState().target).toBeNull();
  });

  // spec: UI-102
  it("„Sonstige“ shows XML and Flash as extra bars and in its tooltip; Esc hides them; the table lists eleven rows", async () => {
    await ready();
    selectTab("Internal");
    await waitFor(() => expect(area().querySelectorAll(".os-ovc-bars > li")).toHaveLength(10));
    const others = item("others");
    fireEvent.focus(others);
    const tip = await within(area()).findByRole("tooltip");
    expect(tip).toHaveTextContent("Others");
    expect(tip).toHaveTextContent("XML: 10");
    expect(tip).toHaveTextContent("Flash: 6");
    fireEvent.click(others);
    expect(area().querySelectorAll(".os-ovc-bars > li")).toHaveLength(12);
    expect(item("f:XML")).toBeTruthy();
    expect(item("f:Flash")).toBeTruthy();
    fireEvent.keyDown(others, { key: "Escape" });
    expect(area().querySelectorAll(".os-ovc-bars > li")).toHaveLength(10);
    fireEvent.click(within(area()).getByRole("button", { name: "Table" }));
    const table = within(area()).getByRole("table", { name: "Internal: breakdown of 2,000 URLs" });
    expect(within(table).getAllByRole("row")).toHaveLength(12);
    expect(within(table).getByRole("row", { name: /XML/ })).toHaveTextContent("XML100.5 %");
    fireEvent.click(within(table).getByRole("button", { name: "XML" }));
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Internal", filter: "XML" });
  });

  // spec: UI-102
  it("keyboard focus group: arrows follow the legend, Home/End jump, Enter applies the filter, Esc returns to the tree", async () => {
    await ready();
    selectTab("Response Codes");
    await waitFor(() => expect(item("f:Success (2XX)")).toBeTruthy());
    const focusable = area().querySelectorAll('.os-ovc-legend [tabindex="0"]');
    expect(focusable).toHaveLength(1);
    const first = item("f:Success (2XX)");
    first.focus();
    expect(await within(area()).findByRole("tooltip")).toHaveTextContent("of 1,000 URLs");
    fireEvent.keyDown(first, { key: "ArrowDown" });
    expect(item("f:Redirection (3XX)")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "End" });
    expect(item("f:Other")).toHaveFocus();
    fireEvent.keyDown(document.activeElement!, { key: "Home" });
    expect(first).toHaveFocus();
    fireEvent.keyDown(first, { key: "ArrowDown" });
    fireEvent.keyDown(document.activeElement!, { key: "ArrowDown" });
    expect(item("f:Client Error (4XX)")).toHaveFocus();
    fireEvent.click(document.activeElement!);
    expect(useSidebarNavigation.getState().target?.spec).toMatchObject({ tab: "Response Codes", filter: "Client Error (4XX)" });
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.getByRole("button", { name: "Response Codes" })).toHaveFocus();
  });

  // spec: UI-102
  it("without counters: hint with „Start crawl analysis“; mixed block lists pending filters with „–“; Sitemaps action starts the analysis", async () => {
    await ready();
    selectTab("Sitemaps");
    expect(await within(area()).findByText("The counts of this entry are not available yet.")).toBeInTheDocument();
    fireEvent.click(within(area()).getByRole("button", { name: "Start crawl analysis" }));
    await waitFor(() => expect(core.calls.some(c => c.cmd === "analysis.start")).toBe(true));
    selectTab("Canonicals");
    const pending = await waitFor(() => item("f:Unlinked"));
    expect(pending).toHaveTextContent("Unlinked–");
    expect(pending.closest("li")).toHaveTextContent("Crawl analysis required");
    expect(area().querySelector('.os-ovc-bars [data-ovc-key="f:Unlinked"]')).toBeNull();
  });

  // spec: UI-102
  it("residual category „Unassigned“ explains itself and does not navigate", async () => {
    answer = structuredClone(fxUnassigned) as unknown as OverviewDto;
    await ready();
    selectTab("Response Codes");
    const rest = await waitFor(() => item("unassigned"));
    fireEvent.focus(rest);
    const tip = await within(area()).findByRole("tooltip");
    expect(tip).toHaveTextContent("10 · 1.0 %");
    expect(tip).toHaveTextContent("URLs of this entry that no category contains.");
    expect(tip).toHaveTextContent("This category has no table filter.");
    fireEvent.click(rest);
    expect(useSidebarNavigation.getState().target).toBeNull();
  });

  // spec: UI-102
  it("live updates redraw only when a value of the selected entry changes; selection, focus and tooltip stay", async () => {
    await ready();
    selectTab("Response Codes");
    await waitFor(() => expect(item("f:Success (2XX)")).toBeTruthy());
    const first = item("f:Success (2XX)");
    first.focus();
    await within(area()).findByRole("tooltip");
    const renders = { n: 0 };
    const spy = vi.spyOn(globalThis.console, "warn");
    const count = () => area().querySelector('[data-ovc-segment="f:Success (2XX)"]')!.getAttribute("d");
    const before = count();
    // An answer with other summary values but the same Response Codes: nothing in the chart changes.
    answer = { ...answer, summary: answer.summary.map(r => r.key === "Total URLs Crawled" ? { ...r, count: r.count + 1 } : r) };
    const observer = new MutationObserver(() => { renders.n++; });
    observer.observe(area(), { subtree: true, childList: true, characterData: true, attributes: true });
    newVersion(20);
    await waitFor(() => expect(screen.getByRole("table", { name: "Summary" })).toHaveTextContent("2,381"));
    expect(renders.n).toBe(0);
    // A changed value of the entry redraws once; selection, focus and tooltip survive.
    const changed = structuredClone(answer);
    const codes = changed.tabs.find(t => t.tab === "Response Codes")!;
    codes.filters.find(f => f.id === "Success (2XX)")!.count = 800;
    codes.filters.find(f => f.id === "Other")!.count = 12;
    answer = changed;
    newVersion(21);
    await waitFor(() => expect(count()).not.toBe(before));
    observer.disconnect();
    expect(useOverviewSelection.getState().entry).toEqual({ kind: "tab", tab: "Response Codes" });
    expect(item("f:Success (2XX)")).toHaveFocus();
    expect(within(area()).getByRole("tooltip")).toHaveTextContent("800");
    expect(spy).not.toHaveBeenCalled();
  });

  // spec: UI-102
  it("segment filter: segment-filtered counts and the segment chip; no chip without a segment filter", async () => {
    core.handlers.set("sidebar.overview", args => (args as { segment?: unknown }).segment
      ? { ...structuredClone(answer), summary: answer.summary.map(r => ({ ...r, count: Math.round(r.count / 10) })) } : structuredClone(answer));
    await ready();
    fireEvent.click(screen.getByRole("button", { name: "Summary" }));
    await within(area()).findByRole("img", { name: "Summary: breakdown of 2,500 URLs" });
    expect(area().querySelector("[data-ovc-chip]")).toBeNull();
    useSegments.setState({ segments: [{ id: "s1", name: "Shop", color: "#1E8F2D", order: 0, rules: [] } as never] });
    act(() => useSidebarNavigation.getState().setScope({ segment: { kind: "segment", name: "Shop" } }));
    expect(await screen.findByRole("img", { name: "Summary: breakdown of 250 URLs" })).toBeInTheDocument();
    const chip = area().querySelector<HTMLElement>("[data-ovc-chip]")!;
    expect(chip).toHaveTextContent("Segment Shop");
    expect(chip.style.background).not.toBe("");
  });

  // spec: UI-102
  it("another crawl drops the selection; comparison mode shows its hint", async () => {
    await ready();
    selectTab("Internal");
    await waitFor(() => expect(item("f:HTML")).toBeTruthy());
    act(() => useSession.setState({ crawl: { ...crawl, crawlId: "other-crawl" } }));
    await waitFor(() => expect(area()).toHaveTextContent("Select an entry in the overview to see its breakdown."));
    expect(useOverviewSelection.getState().entry).toBeNull();
    cleanup();
    const before = useComparison.getState();
    useComparison.setState({ session: {} as never });
    try {
      render(<Providers><OverviewChartArea data={answer} startAnalysis={() => undefined} /></Providers>);
      expect(screen.getByRole("region", { name: "Chart of the selected entry" })).toHaveTextContent("Not available in comparison mode");
    } finally {
      useComparison.setState(before, true);
    }
  });

  // spec: UI-102
  it("view „Table“ and the area height are stored in the ui-state and restored", async () => {
    await ready();
    selectTab("Internal");
    fireEvent.click(await within(area()).findByRole("button", { name: "Table" }));
    await act(async () => { await flushUiState(); });
    expect((core.uiState.state as Record<string, unknown>)[CHART_LAYOUT_KEY]).toEqual({ share: null, collapsed: false, view: "table" });
    const grip = screen.getByRole("separator", { name: "Resize chart area" });
    fireEvent.keyDown(grip, { key: "Home" });
    expect(area).toThrow();
    fireEvent.keyDown(grip, { key: "Enter" });
    expect(within(area()).getByRole("table", { name: /Internal/ })).toBeInTheDocument();
    await act(async () => { await flushUiState(); });
    cleanup();
    useChartLayout.getState().reset();
    await ready();
    expect(useChartLayout.getState().layout.view).toBe("table");
  });

  // spec: UI-102
  it("de and en contain every chart text", () => {
    const files = loadFtlFiles().filter(f => f.feature.includes("overview-chart"));
    const keys = (lang: string) => [...(files.find(f => f.lang === lang)?.source ?? "").matchAll(/^([a-z0-9-]+) =/gm)].map(m => m[1]).sort();
    expect(keys("de").length).toBeGreaterThan(30);
    expect(keys("de")).toEqual(keys("en"));
  });

  // spec: UI-102
  it("an Overview answer that changes nothing renders no chart part again (mutation observer)", async () => {
    mount();
    await screen.findByRole("table", { name: "Summary" });
    selectTab("Response Codes");
    await waitFor(() => expect(item("f:Success (2XX)")).toBeTruthy());
    const renders = vi.fn();
    const observer = new MutationObserver(renders);
    observer.observe(area(), { subtree: true, childList: true, attributes: true, characterData: true });
    answer = { ...answer, ageMs: 4_000 };
    newVersion(30);
    await waitFor(() => expect(core.calls.filter(c => c.cmd === "sidebar.overview").length).toBeGreaterThanOrEqual(2));
    await act(async () => { await new Promise(r => setTimeout(r, 50)); });
    observer.disconnect();
    expect(renders).not.toHaveBeenCalled();
  });
});
