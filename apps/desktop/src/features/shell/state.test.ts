import { afterEach, describe, expect, it, vi } from "vitest";
import { createLayoutStore, normalizeUiState, persistLayout } from "../../state/layout";
import type { UiStateJson } from "../../ipc/gen/UiStateJson";
import fixture from "./__fixtures__/ui-state-v1.json";
import invalid from "./__fixtures__/invalid-state.txt?raw";

function memory(initial: unknown = fixture) {
  let disk = structuredClone(initial) as UiStateJson;
  const api = { load: vi.fn(async () => structuredClone(disk)), save: vi.fn(async (state: UiStateJson) => { disk = structuredClone(state); return disk; }) };
  return { api, read: () => disk };
}
afterEach(() => vi.useRealTimers());
describe("versioned shell persistence", () => {
  // spec: UI-125 [P1]
  it("migrates the previous-version fixture without mutating it, preserves other features and restores exact geometry", async () => {
    const before = JSON.stringify(fixture);
    const disk = memory();
    const store = createLayoutStore(disk.api);
    await store.getState().load(1440, 836);
    expect(store.getState().layout.sidebar.share * 1440).toBeCloseTo(500);
    expect(store.getState().layout.detail.share * 836).toBeCloseTo(300);
    expect(store.getState().layout.detail.visible).toBe(false);
    await store.getState().flush();
    const restarted = createLayoutStore(disk.api);
    await restarted.getState().load(1024, 576);
    expect(restarted.getState().sections).toEqual(store.getState().sections);
    expect(restarted.getState().layout).toEqual(store.getState().layout);
    expect(disk.read().version).toBe(2);
    expect(JSON.stringify(fixture)).toBe(before);
  });
  // spec: UI-125 [P1]
  it("falls back for corrupt, invalid and future schemas; ignores unknown layout fields", () => {
    for (const value of [invalid, null, { version: 99, state: fixture.state }, { version: 2, state: { layout: { sidebar: { share: -1 } } } }]) {
      expect(normalizeUiState(value, 1024, 576).layout.sidebar).toMatchObject({ share: 260 / 1024, collapsed: true });
    }
    expect(normalizeUiState({ version: 2, state: { layout: { ...normalizeUiState(fixture, 1440, 836).layout, extra: true } } }, 1440, 836).layout)
      .not.toHaveProperty("extra");
  });
  // spec: UI-125 [P1]
  it("loads only once under StrictMode; debounces writes for one second", async () => {
    vi.useFakeTimers();
    const disk = memory();
    const store = createLayoutStore(disk.api);
    await Promise.all([store.getState().load(1440, 836), store.getState().load(1440, 836)]);
    expect(disk.api.load).toHaveBeenCalledTimes(1);
    const off = persistLayout(store);
    store.getState().setSection("searchHistory", ["a"]);
    await vi.advanceTimersByTimeAsync(999);
    expect(disk.api.save).not.toHaveBeenCalled();
    store.getState().setSection("searchHistory", ["b"]);
    await vi.advanceTimersByTimeAsync(1000);
    expect(disk.api.save).toHaveBeenCalledTimes(1);
    expect(store.getState().sections.searchHistory).toEqual(["b"]);
    off();
  });
  // spec: UI-021 [P1]
  it("clears all per-table column properties, keeps named templates and every unrelated slice, and persists after restart", async () => {
    const disk = memory();
    const store = createLayoutStore(disk.api);
    await store.getState().load(1440, 836);
    const { tables: _tables, ...expected } = store.getState().sections;
    store.getState().resetColumns();
    expect(store.getState().sections).toEqual(expected);
    expect(store.getState().sections.columnTemplates).toEqual(fixture.state.columnTemplates);
    await store.getState().flush();
    const restarted = createLayoutStore(disk.api);
    await restarted.getState().load(1440, 836);
    expect(restarted.getState().sections).toEqual(expected);
  });
  // spec: UI-125 [P1]
  it("reset all clears every persisted slice and chooses defaults for the current viewport", async () => {
    const disk = memory();
    const store = createLayoutStore(disk.api);
    await store.getState().load(1440, 836);
    store.getState().resetAll(1024, 576);
    expect(Object.keys(store.getState().sections)).toEqual(["layout"]);
    expect(store.getState().layout.sidebar).toMatchObject({ collapsed: true, visible: true });
    await store.getState().flush();
    const restarted = createLayoutStore(disk.api);
    await restarted.getState().load(1440, 836);
    expect(restarted.getState().layout).toEqual(store.getState().layout);
  });
  // spec: UI-125 [P1]
  it("serializes concurrent saves and retains edits made while the writer is busy", async () => {
    const disk = memory();
    let release!: () => void;
    disk.api.save.mockImplementationOnce(async (state) => { await new Promise<void>((resolve) => { release = resolve; }); return state; });
    const store = createLayoutStore(disk.api);
    await store.getState().load(1440, 836);
    store.getState().setSection("searchHistory", ["first"]);
    const first = store.getState().flush();
    await vi.waitFor(() => expect(release).toBeDefined());
    store.getState().setSection("searchHistory", ["second"]);
    const second = store.getState().flush();
    release();
    await Promise.all([first, second]);
    expect(disk.read().state).toHaveProperty("searchHistory", ["second"]);
    expect(disk.api.save).toHaveBeenCalledTimes(2);
  });
  // spec: UI-125 [P1]
  it("does not overwrite unread state on transport errors and retries failed writes", async () => {
    const disk = memory();
    disk.api.load.mockRejectedValueOnce(new Error("disconnected"));
    const store = createLayoutStore(disk.api);
    await expect(store.getState().load(1440, 836)).rejects.toThrow("disconnected");
    await store.getState().flush();
    expect(disk.api.save).not.toHaveBeenCalled();
    await store.getState().load(1440, 836);
    disk.api.save.mockRejectedValueOnce(new Error("disk full"));
    await expect(store.getState().flush()).rejects.toThrow("disk full");
    expect(store.getState().error).not.toBeNull();
    await store.getState().flush();
    expect(store.getState().error).toBeNull();
  });
});
