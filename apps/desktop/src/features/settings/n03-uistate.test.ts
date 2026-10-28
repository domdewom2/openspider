import { afterEach, expect, it } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { flushUiState, saveUiState } from "../../state/uistate";

afterEach(() => setClient(null));

// spec: UI-124
// spec: UI-125
it("preserves the write revision and reapplies a layout edit after a concurrent recent crawl", async () => {
  const core = new MockCore();
  let reads = 0;
  let writes = 0;
  core.handlers.set("uistate.load", () => ({
    version: 1, revision: ++reads,
    state: { recentCrawls: reads === 1 ? [] : [{ path: "/fixture/new.ospider" }] },
  }));
  core.handlers.set("uistate.save", args => {
    if (++writes === 1) {
      expect(args).toMatchObject({ revision: 1, state: { split: 0.4 } });
      throw { code: "Stale", messageKey: "err-uistate-conflict" };
    }
    expect(args).toMatchObject({ revision: 2, state: { split: 0.4, recentCrawls: [{ path: "/fixture/new.ospider" }] } });
    return args;
  });
  setClient(createClient(core.transport()));
  const saved = saveUiState(state => ({ ...state, split: 0.4 }));
  await flushUiState();
  await saved;
  expect(reads).toBe(2);
  expect(writes).toBe(2);
});

// spec: UI-125
it("reports a persistent conflict after three attempts instead of silently discarding a patch", async () => {
  const { updateUiState } = await import("../../state/uistate");
  const core = new MockCore();
  let attempts = 0;
  core.handlers.set("uistate.save", () => {
    attempts++;
    throw { code: "Stale", messageKey: "err-uistate-conflict" };
  });
  setClient(createClient(core.transport()));
  await expect(updateUiState(state => ({ ...state, split: 0.4 }))).rejects.toMatchObject({ code: "Stale" });
  expect(attempts).toBe(3);
});

// spec: UI-124
// spec: UI-125
it("reapplies a recent removal to the latest list, preserving a concurrently added crawl", async () => {
  const { updateRecent } = await import("../help/recent");
  const core = new MockCore();
  let reads = 0;
  let writes = 0;
  core.handlers.set("uistate.load", () => ({
    version: 1, revision: ++reads,
    state: { recentCrawls: reads === 1 ? [{ path: "/fixture/missing" }] : [{ path: "/fixture/new" }, { path: "/fixture/missing" }] },
  }));
  core.handlers.set("uistate.save", args => {
    if (++writes === 1) throw { code: "Stale", messageKey: "err-uistate-conflict" };
    expect(args).toMatchObject({ revision: 2, state: { recentCrawls: [{ path: "/fixture/new" }] } });
    return args;
  });
  setClient(createClient(core.transport()));
  await updateRecent(entries => entries.filter(entry => entry.path !== "/fixture/missing"));
  expect(writes).toBe(2);
});
