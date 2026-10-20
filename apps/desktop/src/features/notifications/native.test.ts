import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { sendDesktopNotification, copyDesktopText } from "../../ipc/transport/tauri";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class {} }));
beforeEach(() => vi.mocked(invoke).mockReset());

// spec: UI-019
it("uses the installed native plugin and respects a denied notification permission", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(false);
  await sendDesktopNotification("Crawl completed", "1200 URLs crawled.");
  expect(invoke).toHaveBeenCalledExactlyOnceWith("plugin:notification|is_permission_granted");
});

// spec: UI-019
it("requests permission before sending a native notification with the URL count", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(null).mockResolvedValueOnce("granted").mockResolvedValueOnce(undefined);
  await sendDesktopNotification("Crawl completed", "1200 URLs crawled.");
  expect(vi.mocked(invoke).mock.calls).toEqual([
    ["plugin:notification|is_permission_granted"], ["plugin:notification|request_permission"],
    ["plugin:notification|notify", { options: { title: "Crawl completed", body: "1200 URLs crawled." } }],
  ]);
});

// spec: UI-019
it("copies diagnostics with the installed desktop clipboard plugin", async () => {
  vi.mocked(invoke).mockResolvedValueOnce(undefined);
  await copyDesktopText('{"version":"fixture"}');
  expect(invoke).toHaveBeenCalledExactlyOnceWith("plugin:clipboard-manager|write_text", { text: '{"version":"fixture"}' });
});
