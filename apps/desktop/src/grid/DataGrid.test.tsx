import { StrictMode } from "react";
import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DataGrid, defaultColumns, type DataGridProps, type GridController } from "./DataGrid";
import { ViewDataSource } from "../data/ViewDataSource";
import { SelectionModel } from "../state/selection";
import { resolveTokens } from "../ui/tokens/tokens";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import fixture from "./fixtures/fx-ui-basic.json";

afterEach(() => vi.restoreAllMocks());
function setup() {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(1_000_000);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, width: 800, height: 400, top: 0, left: 0, bottom: 400, right: 800, toJSON() {} });
  const copy = vi.fn(async () => ({ rows: 1, bytes: 100 }));
  const source = new ViewDataSource(fixture as ViewHandle, {
    rows: vi.fn(async a => ({ ...a, rowKeys: Array.from({ length: a.limit }, (_, i) => a.offset + i + 1),
      cells: Array.from({ length: a.limit }, (_, i) => [a.offset + i + 1, ...fixture.row.slice(1)]) })),
    locate: vi.fn(async a => a.urlId - 1), copy, close: vi.fn(),
  });
  const props: DataGridProps = { source, columns: defaultColumns(source.columns), selection: SelectionModel.empty().select(1),
    onSelectionChange: vi.fn(), onColumnsChange: vi.fn(), onSortChange: vi.fn(), onContextMenu: vi.fn(), onActivate: vi.fn(), onError: vi.fn(),
    confirmLargeCopy: vi.fn(), onExportSelection: vi.fn(), rowHeight: 26,
    theme: { tokens: resolveTokens("light", "#2f855a"), fontFamily: "system-ui", fontSize: 12, zebra: true }, ariaLabel: "Fixture", locale: "de" };
  return { props, source, copy };
}
// spec: UI-007
// spec: UI-012
// spec: UI-140
it("exposes actual fixture cells under StrictMode and delegates selection and copy", async () => {
  const { props, copy, source } = setup();
  const mounted = render(<StrictMode><DataGrid {...props} /></StrictMode>);
  const grid = mounted.getByRole("grid");
  expect(grid).toHaveAttribute("aria-rowcount", "5000001");
  expect(grid).toHaveAttribute("aria-readonly", "true");
  await waitFor(() => expect(mounted.getAllByRole("gridcell").some(c => c.textContent === "12.345")).toBe(true));
  expect(mounted.getAllByRole("row").length).toBeLessThan(300);
  fireEvent.keyDown(grid, { key: "c", ctrlKey: true });
  await waitFor(() => expect(copy).toHaveBeenCalledWith({ viewId: source.viewId, selection: props.selection.value, columns: source.columns.map(c => c.key), withHeader: true }));
  fireEvent.keyDown(grid, { key: "a", metaKey: true });
  expect(vi.mocked(props.onSelectionChange).mock.lastCall![0].value).toEqual({ kind: "all", viewId: source.viewId, version: 1, except: [] });
  fireEvent.keyDown(grid, { key: "End", shiftKey: true });
  await waitFor(() => expect(vi.mocked(props.onSelectionChange).mock.lastCall![0].focus).toBe(5_000_000));
  expect(vi.mocked(props.onSelectionChange).mock.lastCall![0].value.kind).toBe("all");
  expect(vi.mocked(source.api.rows).mock.calls.every(([args]) => args.columns.length > 0)).toBe(true);
  await waitFor(() => expect(mounted.getAllByRole("row").at(-1)).toHaveAttribute("aria-rowindex", "5000001"));
  expect(document.getElementById(grid.getAttribute("aria-activedescendant")!)).toHaveTextContent("5.000.000");
  expect(props.onError).not.toHaveBeenCalled();
  mounted.unmount(); await source.dispose();
});
// spec: UI-140
it("keeps exact middle jumps despite compressed scrollbar rounding and reconnects after unmount", async () => {
  const { props, source } = setup(); let controller: GridController | null = null;
  props.controller = c => { controller = c; };
  const mounted = render(<StrictMode><DataGrid {...props} /></StrictMode>);
  await waitFor(() => expect(controller).not.toBeNull());
  for (const row of [400, 2_500_000, 4_999_999]) {
    act(() => controller!.scrollTo(row));
    await waitFor(() => expect(source.rowKey(row)).toBe(row + 1));
    const region = controller!.visible();
    expect(region.y).toBeLessThanOrEqual(row);
    expect(region.y + region.height).toBeGreaterThan(row);
    fireEvent.scroll(mounted.container.querySelector(".os-grid-scroll")!);
    expect(controller!.visible().y).toBe(region.y);
  }
  const scroll = mounted.container.querySelector(".os-grid-scroll")!;
  scroll.scrollLeft = 500; fireEvent.scroll(scroll);
  expect(controller!.bounds(2, controller!.visible().y)).toMatchObject({ x: -90, width: 120 });
  expect(controller!.bounds(999, 0)).toBeUndefined();
  mounted.unmount(); expect(controller).toBeNull(); expect(source.onScrollTo).toBeUndefined();
  await source.dispose();
});
// spec: UI-012
it("handles keyboard header sorting, horizontal navigation, context menus and activation", async () => {
  const { props, source } = setup(); const mounted = render(<DataGrid {...props} />);
  const grid = mounted.getByRole("grid");
  await waitFor(() => expect(source.rowKey(0)).toBe(1));
  fireEvent.keyDown(grid, { key: "ArrowRight" });
  fireEvent.keyDown(grid, { key: "Enter" });
  expect(props.onActivate).toHaveBeenCalledWith(1, "Address");
  fireEvent.keyDown(grid, { key: "F10", shiftKey: true });
  expect(props.onContextMenu).toHaveBeenCalledWith(expect.objectContaining({ column: "Address", row: 0, urlId: 1 }));
  fireEvent.keyDown(grid, { key: "ArrowUp" }); fireEvent.keyDown(grid, { key: "Enter" });
  expect(props.onSortChange).toHaveBeenCalledWith([{ column: "Address", dir: "asc" }]);
  mounted.unmount(); await source.dispose();
});
// spec: UI-140
// spec: UI-012
it("handles an empty fixture and cancels a pending row selection on unmount", async () => {
  const { props, source } = setup(); source.total = 0;
  const mounted = render(<DataGrid {...props} selection={SelectionModel.empty()} />);
  expect(mounted.getByRole("grid")).toHaveAttribute("aria-rowcount", "1");
  expect(mounted.queryAllByRole("gridcell")).toHaveLength(0);
  fireEvent.keyDown(mounted.getByRole("grid"), { key: "End" });
  expect(source.api.rows).not.toHaveBeenCalled();
  mounted.unmount(); await source.dispose();
  const pending = setup();
  let finish!: (window: Awaited<ReturnType<typeof source.api.rows>>) => void;
  vi.mocked(pending.source.api.rows).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const second = render(<DataGrid {...pending.props} />);
  fireEvent.keyDown(second.getByRole("grid"), { key: "ArrowDown" });
  second.unmount();
  await act(async () => finish({ viewId: pending.source.viewId, version: 1, offset: 0, columns: pending.source.columns.map(c => c.key),
    rowKeys: [1, 2], cells: [fixture.row, [2, ...fixture.row.slice(1)]] }));
  expect(pending.props.onSelectionChange).not.toHaveBeenCalled();
  await pending.source.dispose();
});
