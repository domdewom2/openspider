import { expect, it } from "vitest";
import { columnLayout, rowToScroll, scrollExtent, scrollToRow } from "./geometry";
import { defaultColumns } from "./DataGrid";
import fixture from "./fixtures/fx-ui-basic.json";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
// spec: UI-140
it("maps the full five-million-row fixture to a bounded native scrollbar including exact endpoints", () => {
  const max = fixture.total - 25, range = scrollExtent(fixture.total, 800, 26) - 800;
  expect(range).toBeLessThan(1_000_000);
  for (const row of [0, 1, 400, 2_500_000, max]) expect(scrollToRow(rowToScroll(row, max, range), max, range)).toBe(row);
  expect(scrollToRow(range, max, range)).toBe(max);
  expect(rowToScroll(1, 0, 0)).toBe(0);
  expect(scrollToRow(0, 0, 0)).toBe(0);
});
// spec: UI-007
// spec: UI-140
it("clips scrolled columns behind Row and Address and virtualizes hidden columns", () => {
  const columns = defaultColumns((fixture as ViewHandle).columns);
  const layout = columnLayout(columns, 200, 600);
  expect(layout.slice(0, 2).map(c => [c.key, c.x])).toEqual([["Row", 0], ["Address", 90]]);
  expect(layout.some(c => c.key === "Size (bytes)")).toBe(false);
  expect(layout.find(c => c.key === "Response Time")!.clipLeft).toBe(410);
  expect(layout.every(c => c.clipRight <= 600)).toBe(true);
});
