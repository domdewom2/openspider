import { expect, it, vi } from "vitest";
import { bindPageClose } from "./windowLifetime";
import { createClient, setClient } from "../ipc/client";
import { MockCore } from "../ipc/transport/mock";
import { bootWindow } from "./runtime";

// spec: UI-144
it("releases a fixture window's runtime and renderer effects exactly once on pagehide", async () => {
  const core = new MockCore();
  setClient(createClient(core.transport()));
  const runtime = bootWindow({ kind: "viz", transport: core.transport(), menu: false, shortcuts: false });
  await runtime.ready;
  const unmount = vi.fn();
  const dispose = vi.spyOn(runtime, "dispose");
  const close = bindPageClose(() => { unmount(); runtime.dispose(); });
  window.dispatchEvent(new Event("pagehide"));
  close(); window.dispatchEvent(new Event("pagehide"));
  expect(unmount).toHaveBeenCalledTimes(1);
  expect(dispose).toHaveBeenCalledTimes(1);
});
