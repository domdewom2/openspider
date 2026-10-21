import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ViewSource } from "./ViewSource";
import { tokenizeHtml } from "./highlight";
import { diffMarks, findMatches, findSnippet } from "./model";
import { showInSource, useSourceTarget } from "./navigation";
import { CHUNK_LINES } from "./loader";
import { mockHunks } from "../rendered/fixtures/core";
import { ORIGINAL_HTML, RENDERED_HTML } from "../rendered/fixtures/data";
import { runAction } from "../../../ui/actions";
import { Text } from "@codemirror/state";
import { callsOf, lang, selection, settle, setup, stubRanges, teardown } from "../rendered/testing";

vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = (rendering = true, htmlStored = true) => ({ selection, actions: ctx.actions, rendering, htmlStored, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { stubRanges(); vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); useSourceTarget.setState({ target: undefined }); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); vi.restoreAllMocks(); });
const pane = (side: "left" | "right") => document.querySelector(`[data-side="${side}"]`)!;
/** Ohne den unsichtbaren Platzhalter, mit dem CodeMirror die Breite der Randspalte bemisst. */
const markers = (side: "left" | "right", kind: string) => [...pane(side).querySelectorAll(`.os-src-marker-${kind}`)].filter(m => (m.parentElement as HTMLElement).style.visibility !== "hidden");
const content = (side: "left" | "right") => pane(side).querySelector(".cm-content")?.textContent ?? "";
async function open(rendering = true) { render(<ViewSource {...props(rendering)} />); await settle(500); }

describe("View source", () => {
  // spec: UI-041
  it("shows the stored-HTML prerequisite instead of a code view when nothing is stored", async () => {
    ctx.options.config = { extraction: { store_html: false, store_rendered_html: false } };
    render(<ViewSource {...props(true, false)} />); await settle(300);
    expect(document.querySelector('[data-empty-state="HTML_NOT_STORED"]')).not.toBeNull(); expect(callsOf(ctx.core, "detail.sourceOpen")).toEqual([]);
  });
  // spec: UI-041
  it("loads the original and the rendered HTML in blocks into two read-only code views with line numbers", async () => {
    await open();
    expect(content("left")).toContain("<title>Start</title>"); expect(content("right")).toContain("<title>Start - rendered</title>");
    expect(content("right")).toContain('<p id="late">Added by script today</p>');
    expect(callsOf(ctx.core, "detail.sourceOpen")).toEqual([{ urlId: 1, which: "original", mode: "html", pretty: false }, { urlId: 1, which: "rendered", mode: "html", pretty: false }]);
    expect(pane("left").querySelectorAll(".cm-lineNumbers .cm-gutterElement").length).toBeGreaterThan(5);
    expect(pane("left").querySelector(".cm-content")!.getAttribute("aria-readonly")).toBe("true");
    expect(pane("left").querySelector(".os-tok-tag")).not.toBeNull();
  });
  // spec: UI-041
  it("loads a 20 MB class document in bounded blocks without blocking", async () => {
    ctx.options.largeLines = 9000; ctx.options.config = { rendering: { mode: "text" } };
    render(<ViewSource {...props(false)} />); await settle(800);
    expect(callsOf(ctx.core, "detail.sourceLines").map(a => (a as { count: number }).count)).toEqual([CHUNK_LINES, CHUNK_LINES, 1000]);
    expect(pane("left").textContent).toContain("9,000 lines");
  });
  // spec: UI-041
  it("marks added and removed lines with colour, a +/- margin and a summary computed by the core", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show differences" })); await settle(300);
    expect(screen.getByTestId("diff-summary")).toHaveTextContent("+2 / −1 lines");
    expect(pane("left").querySelectorAll(".os-src-line-removed")).toHaveLength(1); expect(pane("right").querySelectorAll(".os-src-line-added")).toHaveLength(2);
    expect(markers("right", "added").map(m => m.textContent)).toEqual(["+", "+"]);
    expect(markers("left", "removed").map(m => m.textContent)).toEqual(["−"]);
    expect(callsOf(ctx.core, "detail.sourceDiff")).toEqual([{ urlId: 1, mode: "lines" }]);
  });
  // spec: UI-041
  it("compares the visible text word by word: four added words show as +4", async () => {
    await open();
    fireEvent.click(screen.getByRole("radio", { name: "Visible text" })); await settle(500);
    fireEvent.click(screen.getByRole("checkbox", { name: "Show differences" })); await settle(500);
    expect(callsOf(ctx.core, "detail.sourceDiff")).toEqual([{ urlId: 1, mode: "words" }]);
    expect(screen.getByTestId("diff-summary")).toHaveTextContent("+4 / −0 words");
    expect([...pane("right").querySelectorAll(".os-src-word-added")].map(e => e.textContent).join(" ")).toBe("Added by script today");
    expect(pane("left").querySelectorAll(".os-src-word-removed")).toHaveLength(0);
    expect(callsOf(ctx.core, "detail.sourceOpen").at(-1)).toMatchObject({ mode: "text" });
  });
  // spec: UI-041
  it("steps through the differences with F7 and Shift+F7", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show differences" })); await settle(300);
    expect(screen.getByTestId("change-position")).toHaveTextContent("2 differences");
    act(() => { runAction("source.nextDiff"); }); expect(screen.getByTestId("change-position")).toHaveTextContent("Difference 1 of 2");
    act(() => { runAction("source.nextDiff"); }); expect(screen.getByTestId("change-position")).toHaveTextContent("Difference 2 of 2");
    act(() => { runAction("source.nextDiff"); }); expect(screen.getByTestId("change-position")).toHaveTextContent("Difference 1 of 2");
    act(() => { runAction("source.prevDiff"); }); expect(screen.getByTestId("change-position")).toHaveTextContent("Difference 2 of 2");
  });
  // spec: UI-041
  it("searches the active side with match counter, Enter navigation and optional regular expressions", async () => {
    await open();
    const box = screen.getByRole("searchbox", { name: "Search in the active side" });
    fireEvent.change(box, { target: { value: "title" } }); await settle(300);
    expect(screen.getByTestId("match-count")).toHaveTextContent("Match 1 of 2");
    expect(pane("left").querySelectorAll(".os-src-match").length).toBeGreaterThan(0);
    fireEvent.keyDown(box, { key: "Enter" }); expect(screen.getByTestId("match-count")).toHaveTextContent("Match 2 of 2");
    fireEvent.keyDown(box, { key: "Enter" }); expect(screen.getByTestId("match-count")).toHaveTextContent("Match 1 of 2");
    fireEvent.keyDown(box, { key: "Enter", shiftKey: true }); expect(screen.getByTestId("match-count")).toHaveTextContent("Match 2 of 2");
    fireEvent.click(screen.getByRole("checkbox", { name: "Regular expression" })); fireEvent.change(box, { target: { value: "<h[0-9]>" } }); await settle(300);
    expect(screen.getByTestId("match-count")).toHaveTextContent("Match 1 of 1");
    fireEvent.change(box, { target: { value: "(" } }); await settle(300);
    expect(screen.getByTestId("match-count")).toHaveTextContent("Invalid regular expression");
    // Die rechte Seite wird durchsucht, sobald sie die aktive ist.
    fireEvent.focus(pane("right").querySelector(".cm-content")!); fireEvent.click(screen.getByRole("checkbox", { name: "Regular expression" })); fireEvent.change(box, { target: { value: "late" } }); await settle(300);
    expect(screen.getByTestId("match-count")).toHaveTextContent("Match 1 of 1");
  });
  // spec: UI-041
  it("passes the pretty-print display option to the core and keeps the text mode unformatted", async () => {
    await open();
    fireEvent.click(screen.getByRole("checkbox", { name: "Format HTML (display only)" })); await settle(500);
    expect(callsOf(ctx.core, "detail.sourceOpen").at(-1)).toMatchObject({ which: "rendered", mode: "html", pretty: true });
    fireEvent.click(screen.getByRole("radio", { name: "Visible text" })); await settle(300);
    expect(screen.getByRole("checkbox", { name: "Format HTML (display only)" })).toBeDisabled();
  });
  // spec: UI-041
  it("falls back to a single side without JavaScript rendering and explains a missing rendered copy", async () => {
    ctx.options.config = { rendering: { mode: "text" } };
    const { unmount } = render(<ViewSource {...props(false)} />); await settle(500);
    expect(document.querySelector('[data-side="right"]')).toBeNull(); expect(screen.getByRole("checkbox", { name: "Show differences" })).toBeDisabled(); unmount();
    ctx.options.config = { extraction: { store_rendered_html: false } };
    render(<ViewSource {...props()} />); await settle(500);
    expect(pane("right").querySelector('[data-empty-state="RENDERED_HTML_NOT_STORED"]')).not.toBeNull(); expect(content("left")).toContain("<title>Start</title>");
  });
  // spec: UI-048
  it("marks the element requested by the accessibility tab in the rendered HTML", async () => {
    await open();
    act(() => showInSource({ urlId: 1, crawlId: selection.crawlId, which: "rendered", snippet: '<p id="late">Added by script today</p>', selector: "#late" })); await settle(500);
    expect(pane("right").querySelector(".os-src-target")?.textContent).toBe('<p id="late">Added by script today</p>');
    expect(screen.getByText("Marked #late in the rendered HTML.")).toBeInTheDocument();
  });
  // spec: UI-041
  it("saves the active side as .html or .txt", async () => {
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    const blobs: Blob[] = []; Object.assign(URL, { createObjectURL: (b: Blob) => { blobs.push(b); return "blob:x"; }, revokeObjectURL: vi.fn() });
    await open();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Export .html" })); await vi.advanceTimersByTimeAsync(50); });
    expect(click).toHaveBeenCalled(); expect(await blobs[0]!.text()).toBe(ORIGINAL_HTML.join("\n"));
  });
  // spec: UI-041
  it("highlights tags, attributes, strings, comments and entities and ignores script bodies", () => {
    const text = '<a href="x" data-y>t &amp; u</a><!-- c --><script>if (a < b) { x = "<b>" }</script>';
    const kinds = tokenizeHtml(text).map(t => `${t.cls}:${text.slice(t.from, t.to)}`);
    expect(kinds).toEqual(expect.arrayContaining(["tag:a", "attr:href", 'string:"x"', "attr:data-y", "entity:&amp;", "comment:<!-- c -->", "tag:script"]));
    expect(kinds.some(k => k.includes("<b>"))).toBe(false);
  });
  // spec: UI-041
  it("maps diff hunks to line marks and change anchors and finds matches and snippets", () => {
    const marks = diffMarks(mockHunks(ORIGINAL_HTML, RENDERED_HTML));
    expect([...marks.leftLines.keys()]).toEqual([4]); expect([...marks.rightLines.keys()]).toEqual([4, 9]); expect(marks.changes).toEqual([{ left: 4, right: 4 }, { left: 9, right: 9 }]);
    expect(findMatches(Text.of(["abc ABC", "x"]), "abc", false).total).toBe(2); expect(findMatches(Text.of(["a(b"]), "(", true).error).toBe(true);
    expect(findSnippet("<div><p id=\"a\">x</p></div>", "<p id=\"a\">y</p>")).toEqual({ from: 5, to: 15 });
    expect(findSnippet("none", "<p>")).toBeNull();
  });
});
