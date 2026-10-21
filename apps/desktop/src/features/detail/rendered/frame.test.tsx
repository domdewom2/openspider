import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DetailFrame } from "../frame/DetailFrame";
import { showDetailTab } from "../frame/selection";
import { normalizeDetailPreferences } from "../frame/preferences";
import { lang, selection, settle, setup, stubRanges, teardown } from "./testing";

vi.mock("../../../grid/DataGrid", async importOriginal => (await import("./testing")).gridMock(importOriginal as never));
vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
beforeEach(async () => { stubRanges(); vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });
const select = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("tab", { name, hidden: true })); }); await settle(500); };

describe("detail frame with the technical tabs", () => {
  // spec: ONPAGE-192 [M2]
  it("serves SERP Snippet, Rendered Page, View Source, Duplicate Details and Spelling & Grammar Details for any row of a content tab", async () => {
    render(<DetailFrame selection={{ ...selection, topTab: "Page Titles" }} actions={ctx.actions} />); await settle(300);
    for (const name of ["SERP Snippet", "Rendered Page", "View Source", "Duplicate Details", "Spelling & Grammar Details"]) {
      await select(name);
      expect(document.querySelector('[data-empty-state="NO_DATA_YET"]'), name).toBeNull();
      expect(screen.getByRole("tabpanel", { name }).textContent?.length, name).toBeGreaterThan(10);
    }
  });
  // spec: UI-048
  // spec: UI-045
  // spec: UI-040
  it("serves the remaining technical tabs from the same frame", async () => {
    render(<DetailFrame selection={selection} actions={ctx.actions} />); await settle(300);
    for (const name of ["Chrome Console Log", "Structured Data Details", "Accessibility Details"]) {
      await select(name);
      expect(document.querySelector('[data-empty-state="NO_DATA_YET"]'), name).toBeNull();
      expect(screen.getByRole("tabpanel", { name }).textContent?.length, name).toBeGreaterThan(10);
    }
  });
  // spec: UI-048
  it("shows a requested tab, also when the user had hidden it", async () => {
    render(<DetailFrame selection={selection} actions={ctx.actions} />); await settle(300);
    await act(async () => { showDetailTab("source"); }); await settle(500);
    expect(screen.getByRole("tab", { name: "View Source", hidden: true })).toHaveAttribute("aria-selected", "true");
    expect(normalizeDetailPreferences({ order: [], hidden: ["source"] }).hidden).toEqual(["source"]);
    ctx.core.uiState = { version: 1, state: { detail: { order: [], hidden: ["a11y"] } } };
    cleanup(); render(<DetailFrame selection={selection} actions={ctx.actions} />); await settle(300);
    await act(async () => { showDetailTab("a11y"); }); await settle(500);
    expect(screen.getByRole("tab", { name: "Accessibility Details", hidden: true })).toHaveAttribute("aria-selected", "true");
  });
});
