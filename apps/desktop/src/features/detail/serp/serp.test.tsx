import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DetailFrame } from "../frame/DetailFrame";
import { Serp } from "./Serp";
import { boldSegments, codepoints, datePrefix, displayText, parseTerms } from "./model";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

let ctx: ReturnType<typeof setup>;
const panel = (focus = 1) => ({ selection: { ...selection, urlIds: [focus], focus }, actions: ctx.actions, rendering: true, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); vi.setSystemTime(new Date("2026-10-21T10:00:00Z")); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });
const metric = (id: string) => screen.getByRole("row", { name: new RegExp(`^${id}`, "i") });
const cells = (row: HTMLElement) => within(row).getAllByRole("cell").map(c => c.textContent);
async function open(focus = 1) { render(<Serp {...panel(focus)} />); await settle(250); }

describe("SERP snippet", () => {
  // spec: UI-037
  // spec: ONPAGE-194
  it("truncates by the core-provided cut position and shows the measurements", async () => {
    await open(3);
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("W".repeat(30));
    expect(metric("Title")).toBeTruthy();
    const title = document.querySelector(".os-serp-title")!;
    expect(title.getAttribute("data-truncated")).toBe("true"); expect(title.textContent!.endsWith(" …")).toBe(true);
    const [chars, px, max, remaining, truncated] = cells(screen.getByRole("row", { name: /^Title/ })).slice(0, 5);
    expect(chars).toBe("30"); expect(max).toBe("561"); expect(Number(remaining!.replace(/,/g, ""))).toBeLessThan(0); expect(truncated).toBe("Yes"); expect(px).not.toBe("–");
    cleanup(); ctx = setup(); await open(2);
    expect(document.querySelector(".os-serp-title")!.getAttribute("data-truncated")).toBe("false");
    expect(cells(screen.getByRole("row", { name: /^Title/ }))[4]).toBe("No");
  });
  // spec: ONPAGE-194
  // spec: UI-037
  it("applies the device limits: a description that fits on desktop is truncated on mobile", async () => {
    await open(2);
    expect(document.querySelector(".os-serp-description")!.getAttribute("data-truncated")).toBe("false");
    expect(cells(screen.getByRole("row", { name: /^Description/ }))[2]).toBe("985");
    fireEvent.click(screen.getByRole("radio", { name: "Mobile" })); await settle(100);
    expect(document.querySelector(".os-serp-description")!.getAttribute("data-truncated")).toBe("true");
    expect(cells(screen.getByRole("row", { name: /^Description/ }))[2]).toBe("680");
    expect(callsOf(ctx.core, "detail.serpMeasure").at(-1)).toMatchObject({ device: "mobile" });
  });
  // spec: UI-037
  it("measures on every keystroke, stores the edit per URL and resets to the crawled values", async () => {
    await open(1);
    const title = screen.getByRole("textbox", { name: "Title" });
    const before = callsOf(ctx.core, "detail.serpMeasure").length;
    fireEvent.change(title, { target: { value: "Start page" } }); await settle(50);
    fireEvent.change(title, { target: { value: "Start page 2" } }); await settle(50);
    expect(callsOf(ctx.core, "detail.serpMeasure").length).toBe(before + 2);
    expect(callsOf(ctx.core, "detail.serpMeasure").at(-1)).toMatchObject({ title: "Start page 2", boldTerms: [], prefix: "" });
    expect(cells(screen.getByRole("row", { name: /^Title/ }))[0]).toBe("12");
    expect(callsOf(ctx.core, "detail.serpSaveEdit")).toEqual([]);
    await settle(450);
    expect(callsOf(ctx.core, "detail.serpSaveEdit")).toEqual([{ urlId: 1, title: "Start page 2", description: "A short description of the start page." }]);
    expect(screen.getByText("Edited")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Reset" })); await settle(50);
    expect(title).toHaveValue("Start"); expect(callsOf(ctx.core, "detail.serpReset")).toEqual([{ urlId: 1 }]); expect(screen.queryByText("Edited")).toBeNull();
  });
  // spec: ONPAGE-194
  it("keeps an edit across reopening and drops it when the text equals the crawled value again", async () => {
    await open(1);
    fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Edited title" } }); await settle(500);
    cleanup(); await open(1);
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("Edited title"); expect(screen.getByText("Edited")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Title" }), { target: { value: "Start" } }); await settle(500);
    expect(callsOf(ctx.core, "detail.serpReset")).toEqual([{ urlId: 1 }]); expect(ctx.edits.size).toBe(0);
  });
  // spec: UI-037
  it("sends bold terms and the date prefix to the core and prints the prefix before the description", async () => {
    await open(1);
    fireEvent.change(screen.getByRole("textbox", { name: "Search terms to highlight" }), { target: { value: "short, start" } }); await settle(50);
    expect(callsOf(ctx.core, "detail.serpMeasure").at(-1)).toMatchObject({ boldTerms: ["short", "start"] });
    expect([...document.querySelectorAll(".os-serp-description b")].map(b => b.textContent)).toEqual(["short", "start"]);
    fireEvent.click(screen.getByRole("radio", { name: "Date" })); await settle(50);
    expect(callsOf(ctx.core, "detail.serpMeasure").at(-1)).toMatchObject({ prefix: "Oct 13, 2026 — " });
    expect(document.querySelector(".os-serp-prefix")!.textContent).toBe("Oct 13, 2026 — ");
    await lang("de"); await settle(50);
    expect(callsOf(ctx.core, "detail.serpMeasure").at(-1)).toMatchObject({ prefix: "13.10.2026 — " });
  });
  // spec: ONPAGE-194
  it("explains a missing meta description instead of inventing one", async () => {
    await open(4);
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue("");
    expect(screen.getByText("This page has no meta description. Search engines choose their own text.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("Tom & Jerry");
  });
  // spec: UI-037
  it("counts characters and cut positions in code points, not UTF-16 units", () => {
    const text = "Über uns 😀 Käse Straße";
    expect(codepoints(text)).toHaveLength(22);
    expect(displayText(text, { truncated: true, cutAt: 10 })).toBe("Über uns 😀 …");
    expect(displayText(text, { truncated: false, cutAt: null })).toBe(text);
    expect(parseTerms(" a, b ,a,, ")).toEqual(["a", "b"]);
    expect(boldSegments("Hello World hello", ["hello"])).toEqual([{ text: "Hello", bold: true }, { text: " World ", bold: false }, { text: "hello", bold: true }]);
    expect(datePrefix("2026-10-13", "de-AT")).toBe("13.10.2026 — "); expect(datePrefix("2026-10-13", "en")).toBe("Oct 13, 2026 — ");
  });
  // spec: ONPAGE-192
  it("is the SERP lower tab of a Page Titles row and shows Title 1 and Meta Description 1", async () => {
    render(<DetailFrame selection={{ ...selection, topTab: "Page Titles" }} actions={ctx.actions} />); await settle(250);
    await act(async () => { fireEvent.click(screen.getByRole("tab", { name: "SERP Snippet", hidden: true })); await vi.advanceTimersByTimeAsync(300); });
    expect(screen.getByRole("textbox", { name: "Title" })).toHaveValue("Start");
    expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue("A short description of the start page.");
  });
});
