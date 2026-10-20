import { afterEach, expect, it, vi } from "vitest";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { createTauriClient, setClient } from "../../ipc/client";
import { nativeMcpDesktop } from "./desktop";

afterEach(() => { setClient(null); clearMocks(); vi.unstubAllGlobals(); });

// spec: AGENT-002
it("uses the native clipboard plugin after delayed token IPC without browser clipboard access", async () => {
  setClient(createTauriClient());
  const invoke = vi.fn();
  mockIPC(invoke);
  // Production adapter, including the exact command and argument contract of the installed plugin.
  await new Promise<void>(resolve => setTimeout(resolve, 10));
  await nativeMcpDesktop.copy("fixture-delayed-token");
  expect(invoke).toHaveBeenCalledWith("plugin:clipboard-manager|write_text", { text: "fixture-delayed-token" });
});

// spec: AGENT-002
it("reports Unsupported outside Tauri and propagates native clipboard failures", async () => {
  setClient({ transportName: "mock", invoke: vi.fn(), subscribe: () => () => undefined });
  await expect(nativeMcpDesktop.copy("fixture-token")).rejects.toMatchObject({ code: "Unsupported" });
  setClient(createTauriClient());
  mockIPC(() => { throw new Error("native clipboard unavailable"); });
  await expect(nativeMcpDesktop.copy("fixture-token")).rejects.toThrow("native clipboard unavailable");
});
