import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DataGridProps } from "../../grid/DataGrid";
import { defaultColumns } from "../../grid/DataGrid";
import { createClient, setClient } from "../../ipc/client";
import { eventBus } from "../../ipc/events";
import { crawl } from "../../ipc/crawl";
import { catalog } from "../../ipc/catalog";
import { view } from "../../ipc/view";
import type { ViewHandle } from "../../ipc/gen/ViewHandle";
import type { Tick } from "../../ipc/gen/Tick";
import { useTabs, cycleSort, defaultQuery } from "../../state/tabs";
import { normalizeColumns, useColumns, exportColumns } from "../../state/columns";
import { SelectionModel } from "../../state/selection";
import { useSession } from "../../state/session";
import { setLanguage, makeT } from "../../i18n";
import { ViewDataSource } from "../../data/ViewDataSource";
import { UpperTable, filterLabel } from "./UpperTable";
import { TableController } from "./controller";
import { contextSelection, launchBrowser, openSelected } from "./actions";
import { fixtureColumns, fixtureTabs, tableFixture } from "./fixtures/core";
import fixture from "./fixtures/fx-ui-basic.json";
const capture = vi.hoisted(() => ({ grid: undefined as DataGridProps | undefined }));
vi.mock("../../grid/DataGrid", async original => {
  const lib = await original<typeof import("../../grid/DataGrid")>();
  return { ...lib, DataGrid: (props: DataGridProps) => { capture.grid = props; return <div data-testid="grid" />; } };
});
let fx: ReturnType<typeof tableFixture>;
beforeEach(async () => {
  fx = tableFixture(); setClient(createClient(fx.core.transport()));
  useTabs.setState({ catalog: fixtureTabs, order: fixtureTabs.map(t => t.id), active: "Internal", hidden: [], queries: {}, loaded: true, focusId: undefined, focusRequest: 0, error: null });
  useColumns.setState({ metadata: {}, byTab: {} });
  useSession.setState({ ops: {} }); capture.grid = undefined;
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });
const tick = (seq: number, views: Tick["views"] = []): Tick => ({ seq, at: "2026-10-18T10:00:00Z", views, countsVersion: seq, issuesVersion: seq, sidebarVersion: seq, ops: [], notices: [] });
async function mounted() { render(<UpperTable />); await screen.findByTestId("grid"); await waitFor(() => expect(capture.grid?.source.rowKey(0)).toBe(1)); }

// spec: UI-010
it("cycles primary and secondary sorting without sorting a local page", async () => {
  await mounted();
  act(() => capture.grid!.onSortGesture!("Word Count", false));
  expect(useTabs.getState().queries.Internal.sort).toEqual([{ column: "Word Count", dir: "asc" }]);
  await waitFor(() => expect(capture.grid!.headerTitle!("Word Count")).toBe("Word Count ↑"));
  act(() => capture.grid!.onSortGesture!("Word Count", false));
  expect(useTabs.getState().queries.Internal.sort[0].dir).toBe("desc");
  await waitFor(() => expect(capture.grid!.headerTitle!("Word Count")).toBe("Word Count ↓"));
  act(() => capture.grid!.onSortGesture!("Word Count", false));
  expect(useTabs.getState().queries.Internal.sort).toEqual([]);
  let sort = cycleSort([], "Title 1"); sort = cycleSort(sort, "Status Code", true);
  expect(sort.map(s => s.column)).toEqual(["Title 1", "Status Code"]);
  expect(cycleSort(cycleSort(sort, "Title 1", true), "Title 1", true)).toEqual([{ column: "Status Code", dir: "asc" }]);
  await waitFor(() => expect(fx.core.calls.filter(c => c.cmd === "view.open").length).toBeGreaterThan(2));
  expect(fx.core.calls.filter(c => c.cmd === "view.open").some(c => JSON.stringify(c.args).includes('"dir":"desc"'))).toBe(true);
});

// spec: UI-006
// spec: UI-014
it("combines filter and Enter search, retaining the All denominator and selected count", async () => {
  await mounted();
  fireEvent.change(screen.getByLabelText("Filter"), { target: { value: "Client Error (4xx)" } });
  await waitFor(() => expect(capture.grid!.source.total).toBe(1));
  const search = screen.getByLabelText("Search Address");
  fireEvent.change(search, { target: { value: "/b" } });
  expect(useTabs.getState().queries.Internal.search).toBe("");
  fireEvent.submit(search.closest("form")!);
  await waitFor(() => expect(screen.getByText(/Showing: 1 of 3/)).toHaveTextContent("Search active"));
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().select(2)));
  expect(screen.getByText(/Showing: 1 of 3/)).toHaveTextContent("1 selected");
  fireEvent.keyDown(search, { key: "Escape" });
  expect(useTabs.getState().queries.Internal.search).toBe("");
  fireEvent.change(screen.getByLabelText("Filter"), { target: { value: "Unlinked" } });
  await waitFor(() => expect(document.querySelector('[data-empty-state="NEEDS_CRAWL_ANALYSIS"]')).not.toBeNull());
  expect(screen.getByRole("option", { name: /Unlinked \(–\)/ })).toBeInTheDocument();
});

// spec: UI-008 [P1]
// spec: OUT-009
it("keeps Address first and restores hidden columns at their saved positions, with profile-owned export order", async () => {
  const original = defaultColumns(fixtureColumns);
  const saved = original.map(c => ({ ...c, visible: c.key !== "Word Count", width: c.key === "Title 1" ? 400 : c.width }));
  const normalized = normalizeColumns(fixtureColumns, saved, exportColumns(saved));
  expect(normalized.map(c => c.key)).toEqual(original.map(c => c.key));
  expect(normalized.find(c => c.key === "Title 1")!.width).toBe(400);
  expect(normalizeColumns(fixtureColumns, [], ["Title 1", "Status Code"]).filter(c => c.visible).map(c => c.key)).toEqual(["Row", "Address", "Title 1", "Status Code"]);
  await useColumns.getState().load("Internal");
  await useColumns.getState().change("Internal", saved);
  expect(fx.config().config).toMatchObject({ speed: { threads: 2 }, ui: { columns: { Internal: ["Address", "Status Code", "Title 1", "Meta Keywords 1"] } } });
  useColumns.setState({ metadata: {}, byTab: {} }); await useColumns.getState().load("Internal");
  expect(useColumns.getState().byTab.Internal).toEqual(normalized);
});

// spec: UI-008
it("lists dynamic columns, toggles immediately, and protects Address", async () => {
  await mounted(); fireEvent.click(screen.getByRole("button", { name: "Configure columns" }));
  const address = await screen.findByRole("checkbox", { name: "Address" }); expect(address).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "Word Count" }));
  await waitFor(() => expect(capture.grid!.columns.find(c => c.key === "Word Count")!.visible).toBe(false));
  fireEvent.click(screen.getByRole("checkbox", { name: "Word Count" }));
  await waitFor(() => expect(capture.grid!.columns.find(c => c.key === "Word Count")!.visible).toBe(true));
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
});

// spec: UI-004
it("preserves a URL selection across a tab switch and locates it in the target view", async () => {
  const controller = new TableController();
  await controller.open("Internal", defaultQuery()); controller.setSelection(SelectionModel.empty().select(2));
  await controller.open("Images", defaultQuery());
  expect(await controller.focus(controller.selection.focus!)).toBe(1);
  expect(controller.selection.value).toEqual({ kind: "ids", ids: [2] }); controller.dispose();
});

// spec: UI-004
it("closes obsolete view-open responses after a rapid tab switch", async () => {
  const controller = new TableController(); let resolve!: (handle: ViewHandle) => void;
  const original = view.open;
  const spy = vi.spyOn(view, "open").mockImplementationOnce(() => new Promise(r => { resolve = r; })).mockImplementation(original);
  const first = controller.open("Internal", defaultQuery());
  await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
  await controller.open("Images", defaultQuery());
  resolve({ viewId: "old", version: 1, total: 3, state: "ready", columns: fixtureColumns }); await first;
  expect(controller.source!.viewId).not.toBe("old");
  expect(fx.core.calls.some(c => c.cmd === "view.close" && (c.args as { viewId: string }).viewId === "old")).toBe(true);
  controller.dispose();
});

// spec: UI-018
it("keeps focus at its screen offset when rows are inserted above it; tail stops after scrolling upward", async () => {
  const handle = { viewId: "live", version: 1, total: 100, state: "ready", columns: fixtureColumns } as ViewHandle;
  let shift = 0;
  const api = { rows: vi.fn(async (a: import("../../ipc/gen/RowsArgs").RowsArgs) => ({ ...a, rowKeys: Array.from({ length: a.limit }, (_, i) => a.offset + i - shift), cells: Array.from({ length: a.limit }, () => []) })),
    locate: vi.fn(async ({ urlId }: { urlId: number }) => urlId + shift), copy: view.copy, close: vi.fn() };
  const source = new ViewDataSource(handle, api); const scroll = vi.fn(); source.onScrollTo = scroll;
  await source.ensureRange(20, 29); source.setFocus(25, 25); shift = 5;
  await source.onViewChanged(2, 105); expect(scroll).toHaveBeenLastCalledWith(25);
  await source.ensureRange(95, 104); await source.onViewChanged(3, 110); expect(scroll).toHaveBeenLastCalledWith(100);
  await source.ensureRange(40, 49); shift = 10; await source.onViewChanged(4, 115);
  expect(scroll).not.toHaveBeenLastCalledWith(105);
  source.setSorted(true); await source.ensureRange(105, 114); await source.onViewChanged(5, 120);
  expect(scroll).not.toHaveBeenLastCalledWith(110); await source.dispose();
});

// spec: UI-018
it("updates exact paused/resumed totals from ticks and ignores a late counts response", async () => {
  const controller = new TableController(); await controller.open("Internal", defaultQuery()); await controller.refreshCounts();
  fx.replaceRows(fixture.rows.slice(0, 2));
  await controller.tick(tick(2, [{ viewId: controller.source!.viewId, version: 2, total: 2 }]));
  expect(controller.source!.total).toBe(2); expect(controller.counts.filters[0].filter.count).toBe(2);
  fx.replaceRows(fixture.rows);
  await controller.tick(tick(3, [{ viewId: controller.source!.viewId, version: 3, total: 3 }]));
  expect(controller.source!.total).toBe(3); expect(controller.counts.filters[0].filter.count).toBe(3);
  let resolve!: (value: typeof controller.counts) => void;
  vi.spyOn(catalog, "filterCounts").mockImplementationOnce(() => new Promise(r => { resolve = r; }));
  const old = controller.refreshCounts(); await controller.refreshCounts(); resolve({ version: 0, filters: [] }); await old;
  expect(controller.counts.version).toBe(3); controller.dispose();
});

// spec: UI-110
it("right-click retains a multi-selection inside it and replaces it outside; browser launch is bounded and confirmed", async () => {
  const selection = SelectionModel.empty().range([1, 2, 3], 2);
  expect(contextSelection(selection, 2)).toBe(selection);
  expect(contextSelection(selection, 9).value).toEqual({ kind: "ids", ids: [9] });
  const controller = new TableController(); await controller.open("Internal", defaultQuery());
  const open = vi.fn(); const confirm = vi.fn(async () => false);
  await openSelected(controller.source!, selection, confirm, open); expect(open).not.toHaveBeenCalled();
  confirm.mockResolvedValue(true); await openSelected(controller.source!, selection, confirm, open);
  expect(open).toHaveBeenCalledWith(fixture.rows.map(r => r.Address));
  const native = vi.fn(async (_cmd: string, _args?: unknown, _signal?: AbortSignal) => undefined);
  setClient({ transportName: "tauri", invoke: async <T,>(...args: Parameters<typeof native>) => { await native(...args); return undefined as T; }, subscribe: () => () => undefined });
  await launchBrowser(["javascript:alert(1)", ...Array.from({ length: 20 }, (_, i) => `https://fixture.invalid/${i}`)]);
  expect(native).toHaveBeenCalledWith("browser_open", { urls: Array.from({ length: 10 }, (_, i) => `https://fixture.invalid/${i}`) }, undefined);
  controller.dispose();
});

// spec: UI-110
it("opens context actions immediately and sends copy of the clicked cell to the core", async () => {
  await mounted();
  act(() => capture.grid!.onContextMenu({ column: "Title 1", row: 1, urlId: 2, x: 10, y: 10 }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Copy cell value" }));
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "view.copy")?.args).toMatchObject({ selection: { kind: "ids", ids: [2] }, columns: ["Title 1"], withHeader: false }));
});

// spec: UI-114
it("cancels removal without mutation, confirms the count, shows operation progress and supports cancellation", async () => {
  await mounted();
  act(() => { capture.grid!.onSelectionChange(SelectionModel.empty().range([1, 2], 2)); capture.grid!.onContextMenu({ column: "Address", urlId: 2, x: 1, y: 1 }); });
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
  expect(await screen.findByText("Remove 2 selected URLs from this crawl?")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(fx.core.calls.filter(c => c.cmd === "crawl.remove")).toEqual([]);
  act(() => capture.grid!.onContextMenu({ column: "Address", urlId: 2, x: 1, y: 1 }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
  fireEvent.click(await screen.findByRole("button", { name: "Remove" }));
  await screen.findByText("Crawl analysis outdated");
  expect(fx.core.calls.find(c => c.cmd === "crawl.remove")?.args).toEqual({ selection: { kind: "ids", ids: [1, 2] } });
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(fx.core.calls.some(c => c.cmd === "op.cancel")).toBe(true));
});

// spec: UI-114
it("submits recrawl as a URL selection and refreshes the view through subsequent ticks", async () => {
  await mounted(); const spy = vi.spyOn(crawl, "recrawl");
  act(() => capture.grid!.onContextMenu({ column: "Address", urlId: 2, x: 1, y: 1 }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Recrawl" }));
  await waitFor(() => expect(spy).toHaveBeenCalledWith({ selection: { kind: "ids", ids: [2] } }));
  const source = capture.grid!.source;
  act(() => eventBus.emit({ kind: "tick", tick: tick(2, [{ viewId: source.viewId, version: 2, total: 3 }]) }));
  await waitFor(() => expect(capture.grid!.source.version).toBe(2));
  expect(capture.grid!.selection.focus).toBe(2);
});

// spec: UI-006
// spec: UI-014
it("formats issue names and priorities, and formats counts according to the UI language", () => {
  const filter = { ...fixtureTabs[0].filters[0], count: 1234, issue: { issueType: "warning" as const, priority: "high" as const } };
  expect(filterLabel(filter, makeT("en"), "en")).toContain("1,234");
  expect(filterLabel(filter, makeT("de"), "de")).toContain("1.234");
  expect(filterLabel(filter, makeT("en"), "en")).toContain("▲ warning · high");
});

// spec: UI-114
it("handles 150-row select-all without expanding it, shows progress, and always confirms keyboard removal", async () => {
  fx.replaceRows(Array.from({ length: 150 }, (_, i) => ({ ...fixture.rows[0], id: i + 1, Address: `https://fixture.invalid/${i + 1}` })));
  fx.core.uiState.state = { tableConfirmRemove: false };
  await mounted();
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().all(capture.grid!.source.viewId, capture.grid!.source.version)));
  const { runAction } = await import("../../ui/actions");
  act(() => { runAction("table.remove", { source: "shortcut" }); });
  expect(await screen.findByText("Remove 150 selected URLs from this crawl?")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  act(() => capture.grid!.onContextMenu({ column: "Address", row: 0, urlId: 1, x: 1, y: 1 }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Remove" }));
  await screen.findByRole("progressbar");
  expect(fx.core.calls.find(c => c.cmd === "crawl.remove")?.args).toMatchObject({ selection: { kind: "all", except: [] } });
  act(() => useSession.setState({ ops: { "crawl.remove-1": { opId: "crawl.remove-1", labelKey: "op-remove", done: 60, total: 150, state: "running" } } }));
  expect(screen.getByRole("progressbar")).toHaveAttribute("value", "60");
  expect(screen.getByText("60 / 150")).toBeInTheDocument();
});

// spec: UI-114
it("lets settings restore the removal confirmation preference", async () => {
  const { RemovalConfirmationSettings, confirmsRemoval } = await import("./RemovalConfirmationSettings");
  fx.core.uiState.state = { tableConfirmRemove: false };
  render(<RemovalConfirmationSettings />);
  const setting = await screen.findByRole("checkbox", { name: "Confirm removal from the table context menu" });
  await waitFor(() => expect(setting).not.toBeChecked());
  fireEvent.click(setting);
  await waitFor(async () => expect(await confirmsRemoval()).toBe(true));
});

// spec: OUT-009
it("exports visible columns in their configured order and keeps the all-columns option explicit", async () => {
  await mounted();
  const next = [...capture.grid!.columns];
  const title = next.splice(next.findIndex(c => c.key === "Title 1"), 1)[0]; next.splice(2, 0, title);
  next.find(c => c.key === "Meta Keywords 1")!.visible = false;
  await act(async () => { await useColumns.getState().change("Internal", next); });
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  fireEvent.change(await screen.findByLabelText("Destination CSV file"), { target: { value: "fixture.csv" } });
  const dialog = screen.getByRole("dialog");
  fireEvent.click(dialog.querySelector('[data-role="ok"]')!);
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "export.view")?.args).toMatchObject({ allColumns: false, columns: ["Address", "Title 1", "Status Code", "Word Count"] }));
});

// spec: UI-004
it("publishes the active view to detail panes and clears snapshot selections on tab switches", async () => {
  const { useTableSelection } = await import("./selection");
  const controller = new TableController(); await controller.open("Internal", defaultQuery());
  controller.setSelection(SelectionModel.empty().all(controller.source!.viewId, controller.source!.version));
  expect(useTableSelection.getState().selection.value.kind).toBe("all");
  await controller.open("Images", defaultQuery());
  expect(useTableSelection.getState()).toMatchObject({ viewId: controller.source!.viewId, selection: { value: { kind: "ids", ids: [] } } });
  controller.dispose(); expect(useTableSelection.getState().viewId).toBeUndefined();
});

// spec: UI-014
// spec: UI-018
it("accepts a new crawl's lower change sequence and discards the previous crawl's counters", async () => {
  const controller = new TableController();
  vi.spyOn(catalog, "filterCounts").mockResolvedValueOnce({ version: 100, filters: [] });
  await controller.refreshCounts(); expect(controller.counts.version).toBe(100);
  controller.resetCrawl(); await controller.refreshCounts();
  expect(controller.counts.version).toBe(1);
  expect(controller.counts.filters[0].filter.count).toBe(3); controller.dispose();
});
