import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { DataEditorProps } from "@glideapps/glide-data-grid";
import { DataGrid, defaultColumns, type DataGridProps } from "./DataGrid";
import { ViewDataSource } from "../data/ViewDataSource";
import { SelectionModel } from "../state/selection";
import { resolveTokens } from "../ui/tokens/tokens";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import fixture from "./fixtures/fx-ui-basic.json";
const state = vi.hoisted(() => ({ props: undefined as DataEditorProps | undefined }));
vi.mock("@glideapps/glide-data-grid", async original => {
  const lib = await original<typeof import("@glideapps/glide-data-grid")>();
  const React = await import("react");
  return { ...lib, default: React.forwardRef((_props: DataEditorProps, ref) => {
    state.props = _props;
    React.useImperativeHandle(ref, () => ({ scrollTo: vi.fn(), getBounds: vi.fn() }));
    return <canvas tabIndex={0} data-testid="canvas" />;
  }) };
});
// spec: UI-007
// spec: UI-012
it("adapts the fixture with pinned columns, read-only cells and core-owned copy", async () => {
  const copy = vi.fn(async () => ({ rows: 1, bytes: 100 }));
  const source = new ViewDataSource(fixture as ViewHandle, { rows: vi.fn(async a => ({ ...a, rowKeys: [1], cells: [fixture.row] })),
    locate: vi.fn(async () => 0), copy, close: vi.fn() });
  await source.ensureRow(0);
  const onSelectionChange = vi.fn();
  const props: DataGridProps = { source, columns: defaultColumns(source.columns), selection: SelectionModel.empty().select(1),
    onSelectionChange, onColumnsChange: vi.fn(), onSortChange: vi.fn(), onContextMenu: vi.fn(), onActivate: vi.fn(), onError: vi.fn(),
    confirmLargeCopy: vi.fn(), onExportSelection: vi.fn(), rowHeight: 26, theme: { tokens: resolveTokens("light", "#2f855a"), fontFamily: "system-ui", fontSize: 12, zebra: true },
    ariaLabel: "Fixture", locale: "de" };
  const mounted = render(<DataGrid {...props} />);
  expect(state.props!.freezeColumns).toBe(2);
  expect(state.props!.keybindings).toMatchObject({ copy: false, cut: false, paste: false });
  expect(state.props!.onPaste).toBe(false);
  expect(state.props!.getCellContent([2, 0])).toMatchObject({ displayData: "12.345", readonly: true, allowOverlay: false });
  fireEvent.keyDown(mounted.getByTestId("canvas"), { key: "c", ctrlKey: true });
  await waitFor(() => expect(copy).toHaveBeenCalledWith({ viewId: source.viewId, selection: props.selection.value, columns: source.columns.map(c => c.key), withHeader: true }));
  fireEvent.keyDown(mounted.getByTestId("canvas"), { key: "a", metaKey: true });
  expect(onSelectionChange.mock.lastCall![0].value).toEqual({ kind: "all", viewId: source.viewId, version: 1, except: [] });
  act(() => state.props!.onColumnResize!( { title: "Address", width: 320 }, 440, 1, 440 ));
  expect(vi.mocked(props.onColumnsChange).mock.lastCall![0][1].width).toBe(440);
  mounted.unmount();
});
