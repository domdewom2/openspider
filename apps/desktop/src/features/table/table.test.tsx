import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DataGridProps } from "../../grid/DataGrid";
import { defaultColumns } from "../../grid/DataGrid";
import { createClient, setClient } from "../../ipc/client";
import { localError } from "../../ipc/errors";
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
import { setLanguage } from "../../i18n";
import { ViewDataSource } from "../../data/ViewDataSource";
import { UpperTable } from "./UpperTable";
import { ExportHost } from "../export/ExportHost";
import { useDetailSelection } from "../detail/frame/selection";
import { useSidebarNavigation } from "../sidebar/frame/state";
import { TableController } from "./controller";
import { contextSelection, launchBrowser, openSelected } from "./actions";
import { fixtureColumns, fixtureTabs, tableFixture } from "./fixtures/core";
import { flushUiState } from "../../state/uistate";
import fixture from "./fixtures/u10-contract.json";
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
afterEach(async () => { await flushUiState(); cleanup(); setClient(null); vi.restoreAllMocks(); });
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
  fireEvent.click(screen.getByRole("button", { name: /Filter/ }));
  fireEvent.click(await screen.findByRole("option", { name: /Client Error/ }));
  await waitFor(() => expect(capture.grid!.source.total).toBe(1));
  const search = screen.getByLabelText("Search Address");
  fireEvent.change(search, { target: { value: "/b" } });
  expect(useTabs.getState().queries.Internal.search).toBe("");
  fireEvent.keyDown(search, { key: "Enter", code: "Enter", charCode: 13 });
  await waitFor(() => expect(screen.getByText(/Showing: 1 of 3/)).toHaveTextContent("Search active"));
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().select(2)));
  expect(screen.getByText(/Showing: 1 of 3/)).toHaveTextContent("1 selected");
  fireEvent.keyDown(search, { key: "Escape" });
  expect(useTabs.getState().queries.Internal.search).toBe("");
  fireEvent.click(screen.getByRole("button", { name: /Filter/ }));
  const unlinked = await screen.findByRole("option", { name: /Unlinked/ });
  expect(unlinked).toHaveTextContent("–");
  fireEvent.click(unlinked);
  await waitFor(() => expect(document.querySelector('[data-empty-state="NEEDS_CRAWL_ANALYSIS"]')).not.toBeNull());
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
  const old = controller.refreshCounts();
  const latest = controller.refreshCounts();
  // K2-63: overlapping refreshes are now serialized; the late response still cannot regress totals.
  resolve({ version: 0, filters: [] }); await Promise.all([old, latest]);
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
it("formats issue names and priorities, and formats counts according to the UI language", async () => {
  const filter = { ...fixtureTabs[0].filters[0], count: 1234, issue: { issueType: "warning" as const, priority: "high" as const } };
  const configured = structuredClone(fixtureTabs);
  configured[0].filters[0] = filter;
  useTabs.setState({ catalog: configured });
  vi.spyOn(catalog, "filterCounts").mockResolvedValue({ version: 1, filters: [{ tab: "Internal", filter }] });
  await mounted();
  fireEvent.click(screen.getByRole("button", { name: /Filter/ }));
  const option = screen.getByRole("option", { name: /All/ });
  expect(within(option).getByText("warning")).toHaveClass("os-badge");
  expect(within(option).getByText("high")).toHaveClass("os-badge-danger");
  expect(option).toHaveTextContent("1,234");
  await act(async () => { await setLanguage("de"); });
  expect(screen.getAllByRole("option")[0]).toHaveTextContent("1.234");
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
  await act(async () => { await flushUiState(); });
  await waitFor(async () => expect(await confirmsRemoval()).toBe(true));
});

// spec: OUT-009
// spec: OUT-006
it("exports visible columns in their configured order through the shared export dialog and keeps the all-columns option explicit", async () => {
  fx.core.handlers.set("export.suggestName", () => "fixture.csv");
  const desktop = { choose: vi.fn(async () => "/out/fixture.csv"), directory: vi.fn(async () => "/out"), reveal: vi.fn(async () => undefined) };
  render(<><UpperTable /><ExportHost desktop={desktop} /></>); await screen.findByTestId("grid"); await waitFor(() => expect(capture.grid?.source.rowKey(0)).toBe(1));
  const next = [...capture.grid!.columns];
  const title = next.splice(next.findIndex(c => c.key === "Title 1"), 1)[0]; next.splice(2, 0, title);
  next.find(c => c.key === "Meta Keywords 1")!.visible = false;
  await act(async () => { await useColumns.getState().change("Internal", next); });
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  const dialog = await screen.findByRole("dialog", { name: "Export options" });
  expect(within(dialog).getByRole("button", { name: /File format/ })).toBeInTheDocument();
  await waitFor(() => expect(within(dialog).getByText(/Suggested filename: fixture\.csv/)).toBeInTheDocument());
  fireEvent.click(within(dialog).getByRole("button", { name: "Save…" }));
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "export.view")?.args).toMatchObject({ allColumns: false, columns: ["Address", "Title 1", "Status Code", "Word Count"], path: "/out/fixture.csv", format: "csv" }));
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

// spec: UI-110
it("reports a localized browser error when the native command is unsupported", async () => {
  await mounted();
  const delegate = createClient(fx.core.transport());
  const native = async <T,>(command: string, args?: unknown, signal?: AbortSignal): Promise<T> => {
    if (command === "browser_open") throw localError("Unsupported", "err-desktop-platform");
    return delegate.invoke<T>(command, args, signal);
  };
  setClient({ ...delegate, transportName: "tauri", invoke: native });
  await expect(launchBrowser([fixture.rows[0].Address])).rejects.toMatchObject({ cmdError: { code: "Unsupported", messageKey: "u10-browser-error" } });
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().select(1)));
  act(() => capture.grid!.onContextMenu({ column: "Address", urlId: 1, x: 0, y: 0 }));
  fireEvent.click(await screen.findByRole("menuitem", { name: "Open in browser" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not open the browser");
  await act(async () => { await setLanguage("de"); });
  expect(screen.getByRole("alert")).toHaveTextContent("Browser konnte nicht geöffnet werden");
});

// spec: UI-110
it("uses an app dialog for large copies and cancellation does not write the clipboard", async () => {
  await mounted();
  const copy = vi.spyOn(capture.grid!.source.api, "copy");
  let result!: Promise<boolean>;
  act(() => { result = capture.grid!.confirmLargeCopy(100001); });
  const dialog = await screen.findByRole("dialog");
  fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
  await expect(result).resolves.toBe(false);
  expect(copy).not.toHaveBeenCalled();
  act(() => { result = capture.grid!.confirmLargeCopy(100001); });
  fireEvent.click(within(await screen.findByRole("dialog")).getByRole("button", { name: "Copy rows" }));
  await expect(result).resolves.toBe(true);
});

// spec: UI-030
// spec: UI-012
it("publishes the table selection and focus to the detail pane (A01-02)", async () => {
  await mounted();
  expect(useDetailSelection.getState().selection.urlIds).toEqual([]);
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().select(2)));
  await waitFor(() => expect(useDetailSelection.getState().selection).toMatchObject({ urlIds: [2], focus: 2, topTab: "Internal" }));
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty().range([1, 2, 3], 3)));
  await waitFor(() => expect(useDetailSelection.getState().selection).toMatchObject({ urlIds: [1, 2, 3], focus: 3 }));
  act(() => capture.grid!.onSelectionChange(SelectionModel.empty()));
  await waitFor(() => expect(useDetailSelection.getState().selection.urlIds).toEqual([]));
});

// spec: UI-006
it("applies a sidebar navigation request to tab and filter, once per revision (A01-03)", async () => {
  await mounted();
  const second = fixtureTabs.find(t => t.id !== "Internal")!;
  const filter = second.filters.find(f => f.id !== "All")!.id;
  act(() => useSidebarNavigation.getState().navigate(second.id, filter));
  await waitFor(() => expect(useTabs.getState().active).toBe(second.id));
  expect(useTabs.getState().queries[second.id].filter).toBe(filter);
  // The user changes the filter afterwards; the old request is not applied again by an unrelated re-render.
  act(() => useTabs.getState().query({ filter: "All" }));
  await waitFor(() => expect(useTabs.getState().queries[second.id].filter).toBe("All"));
  await new Promise(r => setTimeout(r, 30));
  expect(useTabs.getState().queries[second.id].filter).toBe("All");
});

// spec: UI-142
it("waits for a building view and continues with the columns and version of its build operation (A01-16, A01-09)", async () => {
  const ready = fx.core.handlers.get("view.open")!;
  let opened = 0;
  fx.core.handlers.set("view.open", async (args, ctx) => {
    const handle = await ready(args, ctx) as ViewHandle;
    if (opened++ > 0) return handle;
    // The core answers `building` after its synchronous window: no columns, version 0, no total.
    return { viewId: handle.viewId, version: 0, total: null, columns: [], state: "building", op: { opId: "build-1", labelKey: "view-build", cancellable: true } };
  });
  const controller = new TableController();
  const opening = controller.open("Internal", defaultQuery());
  await waitFor(() => expect(controller.pending).toBe(true));
  expect(controller.source).toBeUndefined();
  const handle = await (ready as (a: unknown, c: unknown) => Promise<ViewHandle>)({ spec: { kind: "tab", tab: "Internal", filter: "All", search: null, sort: [] }, columns: [] }, undefined);
  const viewId = [...fx.views.keys()][0]!;
  fx.views.set(viewId, fx.views.get(handle.viewId)!);
  act(() => useSession.getState().applyTick({ ...tick(50), ops: [{ opId: "build-1", labelKey: "view-build", done: 1, total: 1, state: "done", result: { ...handle, viewId } as never }] }));
  await opening;
  expect(controller.error).toBeNull();
  expect(controller.source?.viewId).toBe(viewId);
  expect(controller.source!.columns.length).toBeGreaterThan(0);
  await waitFor(() => expect(controller.source!.rowKey(0)).toBe(1));
  controller.dispose();
});

// spec: UI-142
it("applies a version announcement that reached the session store before the source existed (A01-21)", async () => {
  const controller = new TableController();
  const open = fx.core.handlers.get("view.open")!;
  fx.core.handlers.set("view.open", async (args, ctx) => {
    const handle = await open(args, ctx) as ViewHandle;
    // The core announces a newer version in a tick that overtakes the open reply.
    useSession.getState().applyTick({ ...tick(60, [{ viewId: handle.viewId, version: handle.version + 4, total: 2 }]) });
    return handle;
  });
  await controller.open("Internal", defaultQuery());
  await waitFor(() => expect(controller.source!.version).toBe(1 + 4));
  expect(controller.source!.total).toBe(2);
  controller.dispose();
});

// spec: UI-018
it("refreshes the filter counts for every new counts version of the tick, independent of the file's change sequence (A01-20)", async () => {
  const controller = new TableController();
  const counts = vi.spyOn(catalog, "filterCounts");
  // The file's change sequence (FilterCounts.version) is far ahead of the core's tick counter.
  counts.mockResolvedValue({ version: 500, filters: [] });
  await controller.tick(tick(1)); await controller.tick(tick(2)); await controller.tick(tick(2)); await controller.tick(tick(3));
  expect(counts).toHaveBeenCalledTimes(3);
  controller.dispose();
});

// spec: UI-018
it("coalesces ticks while a filter-count snapshot is still in flight", async () => {
  const controller = new TableController();
  let release!: (value: typeof controller.counts) => void;
  const counts = vi.spyOn(catalog, "filterCounts")
    .mockImplementationOnce(() => new Promise(resolve => { release = resolve; }))
    .mockResolvedValue({ version: 2, filters: [] });
  const first = controller.refreshCounts();
  const ticks = Array.from({ length: 20 }, (_, i) => controller.tick(tick(i + 1)));
  expect(counts).toHaveBeenCalledTimes(1);
  release({ version: 1, filters: [] });
  await Promise.all([first, ...ticks]);
  expect(counts).toHaveBeenCalledTimes(2);
  expect(controller.counts.version).toBe(2);
  controller.dispose();
});
