import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Accessibility } from "./Accessibility";
import { DEFAULT_RULE_SORT } from "./model";
import { useSourceTarget } from "../source/navigation";
import { DETAIL_SHOW_TAB_EVENT } from "../frame/selection";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

vi.mock("../../../grid/DataGrid", async importOriginal => (await import("../rendered/testing")).gridMock(importOriginal as never));
vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = (rendering = true) => ({ selection, actions: ctx.actions, rendering, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); useSourceTarget.setState({ target: undefined }); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });
const table = (name: string) => screen.getByRole("table", { name });
const rowOf = (name: string, text: string) => within(table(name)).getByText(text, { selector: "td" }).closest("tr")!;

describe("Accessibility details", () => {
  // spec: UI-048
  // spec: TECH-259
  it("starts on Violation with the TECH-259 columns and the specified impact sort", async () => {
    render(<Accessibility {...props()} />); await settle(400);
    expect(screen.getByRole("radio", { name: "Violation" })).toBeChecked();
    expect(within(table("Rules")).getAllByRole("columnheader").map(c => c.textContent)).toEqual(["Issue", "Result", "Guideline", "Impact", "Nodes", "Help URL"]);
    expect(within(table("Rules")).getAllByRole("row").slice(1).map(r => within(r).getAllByRole("cell")[0]!.textContent)).toEqual(["image-alt", "heading-order"]);
    expect(callsOf(ctx.core, "view.open").at(-1)).toMatchObject({ spec: { detail: "a11yRules", params: { result: "Violation" }, sort: DEFAULT_RULE_SORT } });
    expect(DEFAULT_RULE_SORT.map(s => `${s.column}:${s.dir}`)).toEqual(["Impact:asc", "Nodes:desc", "Issue:asc"]);
  });
  // spec: UI-048
  it("shows each result category and its rule once, and Incomplete is listed but separate from Violation", async () => {
    render(<Accessibility {...props()} />); await settle(400);
    let total = 0;
    for (const result of ["Violation", "Incomplete", "Inapplicable", "Passed"]) {
      fireEvent.click(screen.getByRole("radio", { name: result })); await settle(400);
      total += within(table("Rules")).getAllByRole("row").length - 1;
    }
    expect(total).toBe(5);
    fireEvent.click(screen.getByRole("radio", { name: "All" })); await settle(400);
    expect((callsOf(ctx.core, "view.open").at(-1) as { spec: { params: unknown } }).spec.params).toEqual({});
    expect(within(table("Rules")).getAllByRole("row")).toHaveLength(6);
  });
  // spec: UI-048
  it("loads the affected elements of the selected rule and explains rules without elements", async () => {
    render(<Accessibility {...props()} />); await settle(400);
    expect(screen.getByText("Select a rule to see its affected elements.")).toBeInTheDocument();
    fireEvent.click(rowOf("Rules", "image-alt")); await settle(400);
    expect(within(table("Affected elements")).getAllByRole("columnheader").map(c => c.textContent)).toEqual(["Address", "Location on Page", "HTML"]);
    expect(within(table("Affected elements")).getAllByRole("row")).toHaveLength(3);
    expect(callsOf(ctx.core, "view.open").at(-1)).toMatchObject({ spec: { detail: "a11yNodes", params: { rule: "image-alt", result: "Violation" } } });
    fireEvent.click(screen.getByRole("radio", { name: "Passed" })); await settle(400);
    fireEvent.click(rowOf("Rules", "html-has-lang")); await settle(400);
    expect(screen.getByText("No elements with findings")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Affected elements" })).toBeNull();
  });
  // spec: UI-048
  it("opens the rendered source with the marked element and the browser with the selector on the clipboard", async () => {
    const shown = vi.fn(); document.addEventListener(DETAIL_SHOW_TAB_EVENT, shown);
    render(<Accessibility {...props()} />); await settle(400);
    fireEvent.click(rowOf("Rules", "image-alt")); await settle(400);
    fireEvent.click(within(table("Affected elements")).getAllByRole("cell")[1]!.closest("tr")!); await settle(400);
    expect(callsOf(ctx.core, "detail.a11yNode")).toEqual([{ urlId: 1, nodeId: 101 }]);
    expect(screen.getByText(/a11y.summary.fixture/i)).toBeInTheDocument();
    const open = async () => { fireEvent.contextMenu(within(table("Affected elements")).getByText("img.hero")); await settle(0); return screen.getByRole("menu", { name: "Detail actions" }); };
    fireEvent.click(within(await open()).getByRole("menuitem", { name: "Show in rendered HTML" })); await settle(0);
    expect(useSourceTarget.getState().target).toMatchObject({ urlId: 1, which: "rendered", selector: "img.hero", snippet: '<img class="hero" src="a.png">' });
    expect((shown.mock.calls[0]![0] as CustomEvent).detail).toEqual({ tab: "source" });
    fireEvent.click(within(await open()).getByRole("menuitem", { name: "Show in browser" })); await settle(0);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("img.hero"); expect(ctx.actions.openBrowser).toHaveBeenCalledWith("https://site.test/");
    document.removeEventListener(DETAIL_SHOW_TAB_EVENT, shown);
  });
  // spec: UI-048
  it("disables the rendered-source action when the rendered HTML is not stored", async () => {
    ctx.options.config = { extraction: { store_rendered_html: false } };
    render(<Accessibility {...props()} />); await settle(400);
    fireEvent.click(rowOf("Rules", "image-alt")); await settle(400);
    fireEvent.contextMenu(within(table("Affected elements")).getByText("img.hero")); await settle(0);
    expect(within(screen.getByRole("menu", { name: "Detail actions" })).getByRole("menuitem", { name: "Show in rendered HTML" })).toHaveAttribute("aria-disabled", "true");
  });
  // spec: UI-048
  it("explains the prerequisites in rank order: rendering first, then the accessibility option", async () => {
    ctx.options.config = { rendering: { mode: "text" }, extraction: { accessibility: false } };
    const { unmount } = render(<Accessibility {...props(false)} />); await settle(300);
    expect(document.querySelector('[data-empty-state="NEEDS_JS_RENDERING"]')).not.toBeNull(); unmount();
    ctx.options.config = { extraction: { accessibility: false } };
    render(<Accessibility {...props()} />); await settle(300);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(accessibility)"]')).not.toBeNull();
    expect(callsOf(ctx.core, "view.open")).toEqual([]);
  });
});
