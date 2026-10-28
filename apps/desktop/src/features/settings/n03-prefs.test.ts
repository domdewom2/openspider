import { afterEach, expect, it } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { usePrefs } from "../../state/prefs";

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
