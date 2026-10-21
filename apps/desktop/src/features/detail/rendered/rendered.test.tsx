import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Rendered, tileHeight } from "./Rendered";
import { fileStem } from "./common";
import { callsOf, lang, selection, settle, setup, teardown } from "./testing";

vi.mock("../../../grid/DataGrid", async importOriginal => (await import("./testing")).gridMock(importOriginal as never));
vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = () => ({ selection, actions: ctx.actions, rendering: true, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); vi.restoreAllMocks(); });

describe("Rendered page", () => {
  // spec: UI-039
  it("shows the screenshot as tiles from the ospider URI scheme and the blocked resources by default", async () => {
    render(<Rendered {...props()} />); await settle(300);
    const tiles = [...document.querySelectorAll<HTMLImageElement>(".os-rendered-tile img")];
    expect(tiles.map(img => img.getAttribute("src"))).toEqual(["ospider://shot/fixture/1/0", "ospider://shot/fixture/1/1", "ospider://shot/fixture/1/2"]);
    expect(tiles.every(img => img.getAttribute("loading") === "lazy")).toBe(true);
    expect(tileHeight({ height: 8192, tiles: 8 })).toBe(1024);
    const table = screen.getByRole("table", { name: "Resources loaded while rendering" });
    expect(within(table).getAllByRole("columnheader").map(c => c.textContent)).toEqual(["Address", "Type", "Status Code", "Blocked By"]);
    expect(within(table).getByText("robots.txt")).toBeInTheDocument(); expect(within(table).queryByText("https://site.test/app.js")).toBeNull();
    expect((callsOf(ctx.core, "view.open").at(-1) as { spec: { detail: string } }).spec.detail).toBe("blockedResources");
  });
  // spec: UI-039
  it("lists all resources when the filter changes", async () => {
    render(<Rendered {...props()} />); await settle(300);
    fireEvent.click(screen.getByRole("button", { name: /Resources/ })); await settle(0);
    fireEvent.click(screen.getByRole("option", { name: "All resources" })); await settle(300);
    expect((callsOf(ctx.core, "view.open").at(-1) as { spec: { detail: string } }).spec.detail).toBe("network");
    expect(screen.getByText("https://site.test/app.js")).toBeInTheDocument();
  });
  // spec: UI-039
  it("explains the missing JavaScript rendering mode and a disabled screenshot setting", async () => {
    ctx.options.config = { rendering: { mode: "text" } };
    const { unmount } = render(<Rendered {...props()} rendering={false} />); await settle(300);
    expect(document.querySelector('[data-empty-state="NEEDS_JS_RENDERING"]')).not.toBeNull(); expect(callsOf(ctx.core, "detail.screenshot")).toEqual([]);
    unmount(); ctx.options.config = { rendering: { screenshots: { enabled: false } } };
    render(<Rendered {...props()} />); await settle(300);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(rendering.screenshots.enabled)"]')).not.toBeNull();
  });
  // spec: UI-039
  it("saves the screenshot under a file name derived from the address", async () => {
    const blob = new Blob(["x"]);
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ blob: async () => blob }));
    const createObjectURL = vi.fn().mockReturnValue("blob:x"); Object.assign(URL, { createObjectURL, revokeObjectURL: vi.fn() });
    const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    HTMLCanvasElement.prototype.toBlob = function (callback) { callback(new Blob(["png"], { type: "image/png" })); };
    render(<Rendered {...props()} />); await settle(300);
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Save screenshot as…" })); await vi.advanceTimersByTimeAsync(50); });
    expect(vi.mocked(fetch).mock.calls.map(c => c[0])).toEqual(["ospider://shot/fixture/1/0", "ospider://shot/fixture/1/1", "ospider://shot/fixture/1/2"]);
    expect(ctx.actions.reportError).not.toHaveBeenCalled(); expect(click).toHaveBeenCalled();
    expect(fileStem("https://site.test/a b?x=1")).toBe("site.test_a_b_x_1");
    vi.unstubAllGlobals();
  });
});
