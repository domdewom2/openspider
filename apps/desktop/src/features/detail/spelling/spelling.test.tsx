import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Spelling } from "./Spelling";
import { setSpellingActions } from "./actions";
import { markPieces } from "./context";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

vi.mock("../../../grid/DataGrid", async importOriginal => (await import("../rendered/testing")).gridMock(importOriginal as never));
vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = () => ({ selection, actions: ctx.actions, rendering: true, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); setSpellingActions(undefined); await teardown(); vi.useRealTimers(); });
const rowOf = (error: string) => screen.getByText(error, { selector: "td" }).closest("tr")!;
async function menuFor(error: string) {
  fireEvent.contextMenu(within(rowOf(error)).getAllByRole("cell")[0]!); await settle(0);
  return screen.getByRole("menu", { name: "Detail actions" });
}

describe("Spelling & grammar details", () => {
  // spec: UI-047
  // spec: ONPAGE-157
  it("shows exactly Error, Type, Detail and Suggestion by default and keeps the extension columns out", async () => {
    render(<Spelling {...props()} />); await settle(400);
    const table = screen.getByRole("table", { name: "Spelling & Grammar Details" });
    expect(within(table).getAllByRole("columnheader").map(c => c.textContent)).toEqual(["Error", "Type", "Detail", "Suggestion"]);
    expect(within(table).getAllByRole("row")).toHaveLength(4);
    expect(callsOf(ctx.core, "view.open").at(-1)).toMatchObject({ columns: ["Error", "Type", "Detail", "Suggestion", "Page Section", "Rule ID"], spec: { detail: "spelling", params: {} } });
  });
  // spec: UI-047
  it("marks all occurrences of the selected error in the excerpt and scrolls to the first", async () => {
    const scroll = vi.spyOn(Element.prototype, "scrollIntoView");
    render(<Spelling {...props()} />); await settle(400);
    fireEvent.click(rowOf("Rechtschreibfeler")); await settle(300);
    expect(callsOf(ctx.core, "detail.spellingContext")).toEqual([{ urlId: 1, findingId: 2 }]);
    const marks = [...document.querySelectorAll("mark")]; expect(marks.map(m => m.textContent)).toEqual(["Rechtschreibfeler", "Rechtschreibfeler"]);
    expect(marks[0]!.getAttribute("data-first")).toBe("true"); expect(marks[1]!.getAttribute("data-first")).toBeNull();
    expect(scroll).toHaveBeenCalled(); expect(scroll.mock.contexts[scroll.mock.contexts.length - 1]).toBe(marks[0]);
    expect(screen.getByText("Suggestions: Rechtschreibfehler")).toBeInTheDocument();
    expect(markPieces("Kontackt", 0, 8).map(p => p.mark)).toEqual([true]);
    expect(markPieces("😀 Fehler und Fehlerhaft", 2, 8).filter(p => p.mark)).toHaveLength(1);
  });
  // spec: ONPAGE-157
  it("filters by page section and type through the core parameters", async () => {
    render(<Spelling {...props()} />); await settle(400);
    fireEvent.click(screen.getByRole("button", { name: /Page section/ })); await settle(0);
    fireEvent.click(screen.getByRole("option", { name: "Body" })); await settle(400);
    expect(callsOf(ctx.core, "view.open").at(-1)).toMatchObject({ spec: { params: { section: "Body" } } });
    expect(within(screen.getByRole("table", { name: "Spelling & Grammar Details" })).getAllByRole("row")).toHaveLength(3);
  });
  // spec: UI-047
  // spec: ONPAGE-157
  it("offers the three context actions by right click with the right availability and reports completion", async () => {
    const actions = { ignoreWord: vi.fn().mockResolvedValue(undefined), ignoreRule: vi.fn().mockResolvedValue(undefined), addToDictionary: vi.fn().mockResolvedValue(undefined) };
    setSpellingActions(actions);
    render(<Spelling {...props()} />); await settle(400);
    let menu = await menuFor("Kontackt");
    expect(within(menu).getAllByRole("menuitem").map(i => i.textContent).slice(0, 5)).toEqual(["Copy cell", "Copy row", "Ignore (all)", "Ignore rule", "Add to dictionary"]);
    expect(within(menu).getByRole("menuitem", { name: "Ignore rule" })).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Ignore (all)" })); await settle(0);
    expect(actions.ignoreWord).toHaveBeenCalledWith("Kontackt", undefined);
    expect(screen.getByText(/"Kontackt" is ignored in this crawl after the next check/)).toBeInTheDocument();
    menu = await menuFor("Kontackt"); fireEvent.click(within(menu).getByRole("menuitem", { name: "Add to dictionary" })); await settle(0);
    expect(actions.addToDictionary).toHaveBeenCalledWith("Kontackt", undefined);
    menu = await menuFor("ein ein");
    expect(within(menu).getByRole("menuitem", { name: "Add to dictionary" })).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Ignore rule" })); await settle(0);
    expect(actions.ignoreRule).toHaveBeenCalledWith("de.word-repeat");
  });
  // spec: ONPAGE-157
  it("turns a grammar rule off through the crawl configuration and reports the missing commands for the other actions", async () => {
    const saved: unknown[] = []; ctx.core.handlers.set("config.set", raw => { saved.push(raw); return { invalidatedSteps: [] }; });
    render(<Spelling {...props()} />); await settle(400);
    const menu = await menuFor("ein ein"); fireEvent.click(within(menu).getByRole("menuitem", { name: "Ignore rule" })); await settle(0);
    expect((saved[0] as { config: { grammar: { disabled_rules: string[] } } }).config.grammar.disabled_rules).toEqual(["de.word-repeat"]);
    const second = await menuFor("Kontackt"); fireEvent.click(within(second).getByRole("menuitem", { name: "Ignore (all)" })); await settle(0);
    expect(ctx.actions.reportError).toHaveBeenCalledWith(expect.objectContaining({ code: "Unsupported" }));
  });
  // spec: UI-047
  it("shows the core prerequisite state when spelling and grammar are off", async () => {
    ctx.options.config = { spelling: { enabled: false } };
    render(<Spelling {...props()} />); await settle(400);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(spelling.enabled)"]')).not.toBeNull();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Open setting" })); await Promise.resolve(); });
    const { openSettingsWindow } = await import("../../../app/windows"); expect(openSettingsWindow).toHaveBeenCalledWith("spelling.enabled");
  });
});
