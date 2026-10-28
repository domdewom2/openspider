import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { setLanguage } from "../../i18n";
import { ComparisonTable } from "./ComparisonTable";
import { ComparisonLinks } from "./ComparisonLinks";
import { ComparisonDetail } from "./ComparisonDetail";
import { ComparisonPageSource } from "./ComparisonGrid";
import { comparisonFixture, session, snapshot } from "./fixtures/out-compare/core";
import { useComparison } from "./model";
let fixture: ReturnType<typeof comparisonFixture>;
beforeEach(async () => {
  fixture = comparisonFixture(); setClient(createClient(fixture.core.transport())); await setLanguage("en");
  useComparison.setState({ session, snapshot, revision: 0, query: { tab: "Internal", filter: "All", side: "Current" }, selected: null, selectedPartner: null });
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1800);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(1800);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(360);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ x: 0, y: 0, width: 1800, height: 360, top: 0, left: 0, right: 1800, bottom: 360, toJSON() {} });
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });
const lastRows = () => fixture.core.calls.filter(c => c.cmd === "view.rows").at(-1)?.args;
// spec: CRAWL-144
it("configures link columns with the main table menu and retains them across sorting", async () => {
  render(<ComparisonLinks direction="outlinks" urlId={1} />);
  const grid = await screen.findByRole("grid", { name: "Link comparison table" });
  await screen.findByRole("columnheader", { name: "Previous Anchor" });
  fireEvent.click(screen.getByRole("button", { name: "Configure columns" }));
  const dialog = screen.getByRole("dialog");
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "Previous Anchor" }));
  fireEvent.click(within(dialog).getByRole("checkbox", { name: "Select Anchor for moving" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "Down" }));
  fireEvent.click(within(dialog).getByRole("button", { name: "Apply" }));
  expect(within(grid).queryByRole("columnheader", { name: "Previous Anchor" })).toBeNull();
  const names = within(grid).getAllByRole("columnheader").map(c => c.textContent);
  expect(names.indexOf("Anchor")).toBeGreaterThan(names.indexOf("Link Change"));
  fireEvent.keyDown(grid, { key: "ArrowUp" }); fireEvent.keyDown(grid, { key: "Enter" });
  await waitFor(() => expect(lastRows()).toMatchObject({ comparison: { sort: { column: "Source", dir: "asc" } } }));
  await screen.findByRole("grid");
  expect(screen.queryByRole("columnheader", { name: "Previous Anchor" })).toBeNull();
});
// spec: OUT-140
it("sorts ordinary comparison URLs through the core in both directions", async () => {
  render(<ComparisonTable />);
  let grid = await screen.findByRole("grid");
  fireEvent.keyDown(grid, { key: "ArrowUp" }); fireEvent.keyDown(grid, { key: "Enter" });
  await waitFor(() => expect(lastRows()).toMatchObject({ offset: 0, comparison: { sort: { column: "Address", dir: "asc" } } }));
  grid = await screen.findByRole("grid");
  expect(within(grid).getByRole("columnheader", { name: "Address" })).toHaveAttribute("aria-sort", "ascending");
  fireEvent.keyDown(grid, { key: "ArrowUp" }); fireEvent.keyDown(grid, { key: "Enter" });
  await waitFor(() => expect(lastRows()).toMatchObject({ comparison: { sort: { column: "Address", dir: "desc" } } }));
});
// spec: TECH-301
it("selects a URL through DataGrid and opens its existing paired detail", async () => {
  render(<><ComparisonTable /><ComparisonDetail /></>);
  const grid = await screen.findByRole("grid", { name: "Comparison table" });
  fireEvent.keyDown(grid, { key: "Home" });
  await waitFor(() => expect(useComparison.getState().selected).toBe(1));
  expect(useComparison.getState().selectedPartner).toBe(101);
  await screen.findByRole("grid", { name: "Link comparison table" });
  expect(lastRows()).toMatchObject({ comparison: { tab: "inlinks", urlId: 1, side: "Current" } });
});
// spec: TECH-301
it("keeps stable link keys separate from URL ids and preserves exact numeric cell contents", async () => {
  const source = new ComparisonPageSource({ viewId: "comparison", version: 1, offset: 100, total: 101, columns: ["Address", "Change %"],
    cells: [["https://site.test/a", 1.23456], ["https://site.test/b", null]], rowKeys: [-5, 204], side: "Previous", filtersEnabled: true });
  expect(source.rowKey(0)).toBe(-5); expect(await source.locate(204)).toBe(1);
  expect(await source.keys(0, 1)).toEqual([-5, 204]);
  expect(source.getCell(1, 0)).toBe("1.23456"); expect(source.getCell(1, 1)).toBe("");
  expect(await source.locate(1000)).toBeNull();
  await act(() => source.dispose());
});

// spec: OUT-140
it("does not offer core sorting for columns appended after the ordinary URL query", () => {
  const source = new ComparisonPageSource({ viewId: "comparison", version: 1, offset: 0, total: 0, columns: ["Address", "Comparison Status", "Previous Address"],
    cells: [], rowKeys: [], side: "Current", filtersEnabled: true });
  expect(source.columns.map(c => c.sortable)).toEqual([true, false, false]);
});

// spec: CRAWL-144
it("hides and restores URL columns with the shared menu without changing their population", async () => {
  render(<ComparisonTable />);
  await screen.findByRole("grid", { name: "Comparison table" });
  fireEvent.click(screen.getByRole("button", { name: "Configure columns" }));
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("checkbox", { name: "Comparison Status" }));
  fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(screen.queryByRole("columnheader", { name: "Comparison Status" })).toBeNull();
  expect(screen.getByRole("grid")).toHaveAttribute("aria-rowcount", "2");
  fireEvent.click(screen.getByRole("button", { name: "Configure columns" }));
  fireEvent.click(screen.getByRole("button", { name: "Reset columns" }));
  expect(screen.getByRole("columnheader", { name: "Comparison Status" })).toBeInTheDocument();
});
