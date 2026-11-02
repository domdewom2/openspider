import { render, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { DataGrid, defaultColumns, type DataGridProps } from "../grid/DataGrid";
import { ViewDataSource } from "../data/ViewDataSource";
import { SelectionModel } from "../state/selection";
import { resolveTokens } from "./tokens/tokens";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import fixture from "../grid/fixtures/fx-ui-basic.json";

afterEach(() => vi.restoreAllMocks());

// spec: UI-144
// spec: UI-140
it("keeps fixture cell text and accessible names while the hidden row text stays out of layout", async () => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(800);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(400);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockReturnValue(1_000_000);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, width: 800, height: 400, top: 0, left: 0, bottom: 400, right: 800, toJSON() {} });
  const source = new ViewDataSource(fixture as ViewHandle, {
    rows: vi.fn(async a => ({ ...a, rowKeys: Array.from({ length: a.limit }, (_, i) => a.offset + i + 1),
      cells: Array.from({ length: a.limit }, (_, i) => [a.offset + i + 1, ...fixture.row.slice(1)]) })),
    locate: vi.fn(async a => a.urlId - 1), copy: vi.fn(), close: vi.fn(),
  });
  const props: DataGridProps = { source, columns: defaultColumns(source.columns), selection: SelectionModel.empty(),
    onSelectionChange: vi.fn(), onColumnsChange: vi.fn(), onSortChange: vi.fn(), onContextMenu: vi.fn(), onActivate: vi.fn(), onError: vi.fn(),
    confirmLargeCopy: vi.fn(), onExportSelection: vi.fn(), rowHeight: 26,
    theme: { tokens: resolveTokens("light", "#2f855a"), fontFamily: "system-ui", fontSize: 12, zebra: true }, ariaLabel: "Fixture", locale: "de" };
  const mounted = render(<DataGrid {...props} />);
  await waitFor(() => expect(mounted.getAllByRole("gridcell").some(c => c.textContent === "12.345")).toBe(true));
  const cells = mounted.getAllByRole("gridcell");
  expect(cells.length).toBeGreaterThan(0);
  for (const cell of cells) {
    // Text content is unchanged, but every text node lies in a hidden (non-rendered) element.
    const label = cell.querySelector<HTMLElement>(`[id="${cell.getAttribute("aria-labelledby")}"]`)!;
    expect(label.hidden).toBe(true);
    expect(label.textContent).toBe(cell.textContent);
    const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
    for (let node = walker.nextNode(); node; node = walker.nextNode()) expect(node.parentElement!.closest("[hidden]")).not.toBeNull();
  }
  // The accessible name of the cell remains its fixture value.
  expect(mounted.getAllByRole("gridcell", { name: "12.345" }).length).toBeGreaterThan(0);
  expect(mounted.getAllByRole("gridcell", { name: "12.345" }).every(c => c.textContent === "12.345")).toBe(true);
  mounted.unmount(); await source.dispose();
});
