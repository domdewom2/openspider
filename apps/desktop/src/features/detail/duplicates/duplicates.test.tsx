import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DetailVisible } from "../frame/request";
import { Duplicates, sortPartners, textLines } from "./Duplicates";
import { diffWords, tokenize } from "./diff";
import { useSession } from "../../../state/session";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = (focus: number) => ({ selection: { ...selection, urlIds: [focus], focus }, actions: ctx.actions, rendering: true, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
const analysisDone = () => useSession.setState({ analysis: { running: false, steps: [{ step: "near_duplicates", state: "done", stale: false }] } });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });
const rows = () => within(screen.getByRole("grid", { name: "Duplicate partners" })).getAllByRole("row").slice(1);
const text = (row: HTMLElement) => within(row).getAllByRole("gridcell").map(c => c.textContent);

describe("Duplicate details", () => {
  // spec: ONPAGE-145
  // spec: UI-044
  it("lists exact duplicates at similarity 100 sorted by address, without a crawl analysis", async () => {
    render(<Duplicates {...props(10)} />); await settle(400);
    expect(rows().map(text)).toEqual([["https://site.test/a?ref=x", "100", "Exact"], ["https://site.test/b.html", "100", "Exact"]]);
    expect(callsOf(ctx.core, "detail.duplicates")).toEqual([{ urlId: 10, kind: "exact" }]);
    expect(sortPartners([{ urlId: 1, address: "b", similarity: 91 }, { urlId: 2, address: "a", similarity: 95 }, { urlId: 3, address: "a", similarity: 91 }]).map(p => p.urlId)).toEqual([2, 3, 1]);
  });
  // spec: ONPAGE-145
  it("lists near duplicates by similarity descending and marks the differing words on both sides", async () => {
    analysisDone();
    render(<Duplicates {...props(20)} />); await settle(100);
    fireEvent.click(screen.getByRole("radio", { name: "Near" })); await settle(400);
    expect(rows().map(text)).toEqual([["https://site.test/near-95", "95", "Near"], ["https://site.test/near-91", "91", "Near"]]);
    fireEvent.click(rows()[0]!); await settle(400);
    expect(callsOf(ctx.core, "detail.duplicates").at(-1)).toEqual({ urlId: 21, kind: "near" });
    expect([...document.querySelectorAll("del")].map(e => e.textContent)).toEqual(["jumps"]); expect([...document.querySelectorAll("ins")].map(e => e.textContent)).toEqual(["leaps"]);
    expect(document.querySelectorAll('.os-dup-line[data-diff="removed"] .os-dup-margin')[0]!.textContent).toBe("−");
    expect(document.querySelectorAll('.os-dup-line[data-diff="added"] .os-dup-margin')[0]!.textContent).toBe("+");
    expect(screen.getByText("1 words only on this page, 1 words only on the partner page.")).toBeInTheDocument();
  });
  // spec: UI-044
  it("keeps both text columns scrolling together", async () => {
    render(<Duplicates {...props(10)} />); await settle(400);
    fireEvent.click(rows()[0]!); await settle(400);
    const [a, b] = [...document.querySelectorAll<HTMLElement>(".os-dup-text .os-fach-pane-body")];
    Object.defineProperties(a!, { scrollHeight: { value: 1000, configurable: true }, clientHeight: { value: 200, configurable: true } });
    Object.defineProperties(b!, { scrollHeight: { value: 600, configurable: true }, clientHeight: { value: 200, configurable: true } });
    vi.stubGlobal("requestAnimationFrame", (cb: () => void) => { cb(); return 1; });
    a!.scrollTop = 400; fireEvent.scroll(a!);
    expect(b!.scrollTop).toBe(200);
    vi.unstubAllGlobals();
  });
  // spec: UI-044
  it("shows the prerequisite state for Near without the option, or before the crawl analysis, and offers the analysis", async () => {
    ctx.options.config = { duplicates: { near_enabled: false } };
    const { unmount } = render(<Duplicates {...props(10)} />); await settle(100);
    fireEvent.click(screen.getByRole("radio", { name: "Near" })); await settle(300);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(duplicates.near_enabled)"]')).not.toBeNull(); unmount();
    ctx.options.config = {}; render(<Duplicates {...props(10)} />); await settle(100);
    fireEvent.click(screen.getByRole("radio", { name: "Near" })); await settle(300);
    expect(document.querySelector('[data-empty-state="NEEDS_CRAWL_ANALYSIS(near_duplicates)"]')).not.toBeNull();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Start crawl analysis" })); await Promise.resolve(); });
    expect(callsOf(ctx.core, "analysis.start")).toEqual([{ steps: ["near_duplicates"] }]);
    fireEvent.click(screen.getByRole("radio", { name: "Exact" })); await settle(300);
    expect(rows()).toHaveLength(2);
  });
  // spec: UI-044
  it("keeps Semantic unavailable until embeddings exist and explains an empty list", async () => {
    render(<Duplicates {...props(11)} />); await settle(400);
    expect(screen.getByRole("radio", { name: "Semantic" })).toBeDisabled();
    expect(document.querySelector('[data-empty-state="NO_MATCH"]')).not.toBeNull();
  });
  // spec: UI-044
  it("exports the list through the detail export of the frame", async () => {
    render(<Duplicates {...props(10)} />); await settle(400);
    fireEvent.click(screen.getByRole("button", { name: "Export details" }));
    expect(ctx.actions.export).toHaveBeenCalledWith({ kind: "detail", detail: "duplicates", urlIds: [10], params: { kind: "exact" }, sort: [] }, { kind: "ids", ids: [10] });
  });
  // spec: ONPAGE-145
  it("computes a minimal word diff over whitespace-separated tokens", () => {
    const a = tokenize("The quick  brown fox\njumps"); expect(a.map(t => t.text)).toEqual(["The", "quick", "brown", "fox", "jumps"]);
    const d = diffWords(["a", "b", "c", "d", "e"], ["a", "x", "c", "e", "f"]);
    expect(d.removed).toEqual([false, true, false, true, false]); expect(d.added).toEqual([false, true, false, false, true]);
    expect([d.removedCount, d.addedCount]).toEqual([2, 2]);
    expect(diffWords(["a", "b"], ["a", "b"]).removedCount).toBe(0);
    const big = diffWords(Array.from({ length: 4000 }, (_, i) => `a${i}`), Array.from({ length: 4000 }, (_, i) => `b${i}`)); expect(big.removedCount).toBe(4000);
    const lines = textLines("one two\nthree", tokenize("one two\nthree"), [false, true, false]);
    expect(lines.map(l => l.changed)).toEqual([true, false]); expect(lines[0]!.parts.map(p => p.text)).toEqual(["one", " ", "two"]);
  });
});

// spec: UI-018
// spec: UI-030
it("keeps duplicate data on return and refreshes the final revision without loading hidden data", async () => {
  const { rerender } = render(<DetailVisible.Provider value={true}><Duplicates {...props(10)} /></DetailVisible.Provider>);
  await settle(400); expect(rows()).toHaveLength(2);
  const before = callsOf(ctx.core, "detail.duplicates").length;
  rerender(<DetailVisible.Provider value={false}><Duplicates {...props(10)} /></DetailVisible.Provider>);
  act(() => useSession.setState({ countsVersion: useSession.getState().countsVersion + 1 }));
  await settle(400); expect(callsOf(ctx.core, "detail.duplicates")).toHaveLength(before);
  rerender(<DetailVisible.Provider value={true}><Duplicates {...props(10)} /></DetailVisible.Provider>);
  expect(rows()).toHaveLength(2);
  await settle(400); expect(callsOf(ctx.core, "detail.duplicates")).toHaveLength(before + 1);
});
