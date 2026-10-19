import { expect, it, vi } from "vitest";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import { ViewDataSource } from "../data/ViewDataSource";
import { SelectionModel } from "../state/selection";
import { resolveTokens } from "../ui/tokens/tokens";
import { copySelection } from "./copy";
import { cellTooltip, formatCell, gridCell } from "./cells";
import fixture from "./fixtures/fx-ui-basic.json";
const handle = fixture as ViewHandle;
// spec: UI-007
it("formats fx-ui-basic in de/en with raw bytes, precise seconds and empty null cells", () => {
  expect(handle.columns.map((c, i) => formatCell(fixture.row[i], c, "de"))).toEqual(["1", fixture.row[1], "12.345", "0,123", ""]);
  expect(handle.columns.map((c, i) => formatCell(fixture.row[i], c, "en"))).toEqual(["1", fixture.row[1], "12,345", "0.123", ""]);
  expect(cellTooltip(12345, handle.columns[2], "en")).toBe("12.06 KB / 0.01 MB");
  expect(cellTooltip("x".repeat(2100), handle.columns[4], "de")).toHaveLength(2001);
  const status = { ...handle.columns[0], key: "Status Code" };
  expect(gridCell(404, status, "en", resolveTokens("light", "#2f855a"))).toMatchObject({ displayData: "404", readonly: true, allowOverlay: false });
});
// spec: UI-012
it("selects URL ids, toggles, ranges, and all-except without materializing unloaded rows", () => {
  let selection = SelectionModel.empty().select(1).select(3, true);
  expect(selection.value).toEqual({ kind: "ids", ids: [1, 3] });
  selection = selection.range([3, 4, 5], 5);
  expect(selection.anchor).toBe(3);
  selection = selection.all(handle.viewId, 1).select(4, true);
  expect(selection.has(4)).toBe(false); expect(selection.has(4_999_999)).toBe(true);
  expect(selection.count(fixture.total)).toBe(4_999_999);
  expect(selection.forVersion(handle.viewId, 2).count(fixture.total)).toBe(0);
});
// spec: UI-012
it("delegates copy with visible column order, warns for large selections and offers export at cell limit", async () => {
  const copy = vi.fn(); const confirmLarge = vi.fn(async () => true); const offerExport = vi.fn();
  const source = new ViewDataSource(handle, { copy, rows: vi.fn(), locate: vi.fn(), close: vi.fn() });
  const selection = SelectionModel.empty().range([1, 2, 3], 3);
  await copySelection(source, selection, ["Address", "Row"], { confirmLarge, offerExport });
  expect(copy).toHaveBeenCalledWith({ viewId: handle.viewId, selection: selection.value, columns: ["Address", "Row"], withHeader: true });
  await copySelection(source, selection.all(handle.viewId, 1), ["Address"], { confirmLarge, offerExport });
  expect(offerExport).toHaveBeenCalledOnce(); expect(confirmLarge).not.toHaveBeenCalled();
  source.total = 100001;
  await copySelection(source, selection.all(handle.viewId, 1), ["Address"], { confirmLarge, offerExport });
  expect(confirmLarge).toHaveBeenCalledWith(100001);
  confirmLarge.mockResolvedValue(false);
  copy.mockClear();
  await copySelection(source, selection.all(handle.viewId, 1), ["Address"], { confirmLarge, offerExport });
  expect(copy).not.toHaveBeenCalled();
});
