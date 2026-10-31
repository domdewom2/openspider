import { afterEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { startPrefsBroadcast, usePrefs } from "../../state/prefs";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((ok, fail) => { resolve = ok; reject = fail; });
  return { promise, resolve, reject };
}
afterEach(() => setClient(null));

// spec: UI-120
// spec: UI-121
it("rebases overlapping failed preference writes on the confirmed fixture state", async () => {
  const core = new MockCore();
  const gates = [deferred(), deferred()];
  const started = [deferred(), deferred()];
  let call = 0;
  core.handlers.set("settings.set", async () => {
    const i = call++;
    started[i]!.resolve();
    await gates[i]!.promise;
    return core.settings;
  });
  setClient(createClient(core.transport()));
  await usePrefs.getState().load();
  const before = usePrefs.getState().dto;
  const theme = usePrefs.getState().patch({ appearance: { theme: "dark" } });
  const language = usePrefs.getState().patch({ general: { language: "de" } });
  expect(usePrefs.getState().appearance.theme).toBe("dark");
  expect(usePrefs.getState().language).toBe("de");
  await started[0]!.promise;
  gates[0]!.reject({ code: "Io", messageKey: "err-settings-io" });
  await theme;
  await started[1]!.promise;
  expect(usePrefs.getState().appearance.theme).toBe("system");
  expect(usePrefs.getState().language).toBe("de");
  gates[1]!.reject({ code: "Io", messageKey: "err-settings-io" });
  await language;
  expect(usePrefs.getState().dto).toEqual(before);
  expect(usePrefs.getState().appearance.theme).toBe("system");
  expect(usePrefs.getState().languageSetting).toBe("system");
  expect(usePrefs.getState().error?.code).toBe("Io");
});

// spec: UI-120
// spec: UI-121
it("broadcasts before a blocked save and broadcasts a rollback with the persistence error", async () => {
  const messages: unknown[] = [];
  class Channel {
    onmessage: ((event: MessageEvent) => void) | null = null;
    postMessage(message: unknown) { messages.push(message); }
    close() { /* fixture */ }
  }
  vi.stubGlobal("BroadcastChannel", Channel);
  const stop = startPrefsBroadcast("fixture");
  try {
    const core = new MockCore();
    const gate = deferred();
    core.handlers.set("settings.set", async () => { await gate.promise; return core.settings; });
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    const before = usePrefs.getState().dto;
    const saving = usePrefs.getState().patch({ general: { language: "de" }, appearance: { theme: "dark" } });
    expect(messages).toHaveLength(1);
    expect(messages[0]).toMatchObject({ patch: { general: { language: "de" }, appearance: { theme: "dark" } } });
    expect(usePrefs.getState().language).toBe("de");
    gate.reject({ code: "Io", messageKey: "err-settings-io" });
    await saving;
    expect(messages[1]).toMatchObject({ dto: before, error: { code: "Io" } });
    expect(usePrefs.getState().dto).toEqual(before);
  } finally { stop(); vi.unstubAllGlobals(); }
});

// spec: UI-120
// spec: UI-121
it("applies a remote live edit without a read and protects it from a stale settingsChanged read", async () => {
  let receive!: (data: unknown) => void;
  class Channel {
    onmessage: ((event: MessageEvent) => void) | null = null;
    constructor() { receive = data => this.onmessage?.({ data } as MessageEvent); }
    postMessage() { /* fixture */ }
    close() { /* fixture */ }
  }
  vi.stubGlobal("BroadcastChannel", Channel);
  const stop = startPrefsBroadcast("fixture");
  try {
    const core = new MockCore();
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    const before = usePrefs.getState().dto;
    receive({ sender: "other", id: "other:1", patch: { general: { language: "de" }, appearance: { theme: "dark" } } });
    expect(usePrefs.getState().language).toBe("de");
    expect(usePrefs.getState().appearance.theme).toBe("dark");
    await usePrefs.getState().load();
    expect(usePrefs.getState().language).toBe("de");
    receive({ sender: "other", id: "other:1", dto: before, error: { code: "Io", messageKey: "err-settings-io" } });
    expect(usePrefs.getState().dto).toEqual(before);
    expect(usePrefs.getState().error?.code).toBe("Io");
  } finally { stop(); vi.unstubAllGlobals(); }
});
