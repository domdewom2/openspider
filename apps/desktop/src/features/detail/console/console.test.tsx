import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ConsoleLog } from "./ConsoleLog";
import { findBulk, findReport, MESSAGE_LIMIT, shortenMessage } from "./model";
import { useExportDialog } from "../../export/model";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

vi.mock("../../../grid/DataGrid", async importOriginal => (await import("../rendered/testing")).gridMock(importOriginal as never));
vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = (rendering = true) => ({ selection, actions: ctx.actions, rendering, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); useExportDialog.setState({ request: null }); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });

describe("Chrome console log", () => {
  // spec: UI-040
  // spec: TECH-114
  it("shows the TECH-114 columns in order and filters by level", async () => {
    render(<ConsoleLog {...props()} />); await settle(400);
    const table = screen.getByRole("table", { name: "Chrome Console Log" });
    expect(within(table).getAllByRole("columnheader").map(c => c.textContent)).toEqual(["Level", "Message", "Source URL", "Line", "Column", "Timestamp"]);
    expect(within(table).getByText("Uncaught Error: fixture-error")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Level/ })); await settle(0);
    fireEvent.click(screen.getByRole("option", { name: "Error" })); await settle(400);
    const spec = (callsOf(ctx.core, "view.open").at(-1) as { spec: { detail: string; params: unknown } }).spec;
    expect(spec).toMatchObject({ detail: "consoleLog", params: { level: "error" } });
    expect(within(screen.getByRole("table", { name: "Chrome Console Log" })).queryByText("deprecated call")).toBeNull();
  });
  // spec: UI-040
  it("shortens messages above 10 000 characters but copies them in full", async () => {
    expect(shortenMessage("a".repeat(MESSAGE_LIMIT)).truncated).toBe(false);
    const short = shortenMessage("😀".repeat(MESSAGE_LIMIT + 5)); expect(short.truncated).toBe(true); expect(short.total).toBe(MESSAGE_LIMIT + 5);
    render(<ConsoleLog {...props()} />); await settle(400);
    fireEvent.click(screen.getByText("x".repeat(10_050))); await settle(50);
    expect(screen.getByText(/Shown shortened; the complete message has 10,050 characters/)).toBeInTheDocument();
    expect(document.querySelector(".os-console-text")!.textContent).toHaveLength(MESSAGE_LIMIT + 1);
    fireEvent.click(screen.getByRole("button", { name: "Copy full message" })); await settle(0);
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("x".repeat(10_050));
  });
  // spec: UI-040
  it("links the cross-page exports to the bulk export and the report of the catalog", async () => {
    expect(findBulk(["JavaScript:All Console Messages", "JavaScript:Pages With JavaScript Issues"])).toBe("JavaScript:Pages With JavaScript Issues");
    expect(findReport(["JavaScript:JavaScript Console Log Summary"])).toBe("JavaScript:JavaScript Console Log Summary");
    render(<ConsoleLog {...props()} />); await settle(400);
    fireEvent.click(screen.getByRole("button", { name: "Export all pages with JavaScript issues" })); await settle(0);
    expect(useExportDialog.getState().request).toEqual({ kind: "bulk", name: "JavaScript:Pages With JavaScript Issues" });
    fireEvent.click(screen.getByRole("button", { name: "Console message summary of all pages" })); await settle(0);
    expect(useExportDialog.getState().request).toEqual({ kind: "report", name: "JavaScript:JavaScript Console Log Summary" });
  });
  // spec: UI-040
  it("shows the prerequisite states instead of an empty table", async () => {
    ctx.options.config = { rendering: { mode: "text" } };
    const { unmount } = render(<ConsoleLog {...props(false)} />); await settle(300);
    expect(document.querySelector('[data-empty-state="NEEDS_JS_RENDERING"]')).not.toBeNull(); unmount();
    ctx.options.config = { rendering: { js_error_reporting: false } };
    render(<ConsoleLog {...props()} />); await settle(300);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(rendering.js_error_reporting)"]')).not.toBeNull();
    expect(callsOf(ctx.core, "view.open")).toEqual([]);
  });
});
