import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { uistate } from "../../ipc/uistate";
import { flushUiState, saveUiState } from "../../state/uistate";
import { persistColumns } from "../../state/columns";
import { useTabs } from "../../state/tabs";
import { defaultColumns } from "../../grid/DataGrid";
import { fixtureColumns, fixtureTabs, tableFixture } from "./fixtures/core";
let fx: ReturnType<typeof tableFixture>;
beforeEach(() => { fx = tableFixture(); setClient(createClient(fx.core.transport())); });
afterEach(async () => { await flushUiState(); vi.useRealTimers(); vi.restoreAllMocks(); setClient(null); });

// spec: UI-004 [P1]
// spec: UI-008 [P1]
it("debounces shared tab, column and detail updates into one validated write", async () => {
  vi.useFakeTimers();
  fx.core.uiState.state = { unrelated: "preserved" };
  const writes = vi.spyOn(uistate, "save");
  const first = saveUiState(state => ({ ...state, topTabs: { hidden: ["AMP"] } }));
  const second = saveUiState(state => ({ ...state, tableColumns: { Internal: ["Address"] } }));
  const third = saveUiState(state => ({ ...state, detail: { active: "inlinks" } }));
  await vi.advanceTimersByTimeAsync(999); expect(writes).not.toHaveBeenCalled();
  await vi.advanceTimersByTimeAsync(1); await Promise.all([first, second, third]);
  expect(writes).toHaveBeenCalledTimes(1);
  expect(fx.core.uiState.state).toMatchObject({ unrelated: "preserved", topTabs: { hidden: ["AMP"] }, tableColumns: { Internal: ["Address"] }, detail: { active: "inlinks" } });
});

// spec: UI-008 [P1]
it("serializes simultaneous column profiles and preserves tab visibility and other features", async () => {
  vi.useFakeTimers();
  useTabs.setState({ catalog: fixtureTabs, order: fixtureTabs.map(t => t.id), hidden: [] });
  const tabs = useTabs.getState().show("AMP", false);
  const first = persistColumns("Internal", defaultColumns(fixtureColumns));
  const second = persistColumns("Images", defaultColumns(fixtureColumns));
  await vi.runAllTimersAsync(); await Promise.all([tabs, first, second]);
  expect(fx.core.uiState.state).toMatchObject({ topTabs: { hidden: ["AMP"] }, tableColumns: { Internal: expect.any(Array), Images: expect.any(Array) } });
});

// spec: UI-004 [P1]
it("rejects malformed persisted state without overwriting it and recovers on the next save", async () => {
  const load = vi.spyOn(uistate, "load").mockResolvedValueOnce({ version: 1, state: [] });
  const save = vi.spyOn(uistate, "save");
  const invalid = saveUiState(state => ({ ...state, topTabs: {} })).catch(error => error);
  await flushUiState(); expect(await invalid).toBeInstanceOf(Error); expect(save).not.toHaveBeenCalled();
  load.mockRestore();
  const valid = saveUiState(state => ({ ...state, detail: { active: "inlinks" } }));
  await flushUiState(); await valid;
  expect(fx.core.uiState.state).toMatchObject({ detail: { active: "inlinks" } });
});
