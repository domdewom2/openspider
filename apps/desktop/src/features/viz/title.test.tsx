import { act, cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { setLanguage } from "../../i18n";
import { SettingsWindow } from "../settings/window/SettingsWindow";
import { VizWindow } from "./VizWindow";

const native = vi.hoisted(() => ({ setTitle: vi.fn(async (_title: string) => undefined) }));
vi.mock("../../ipc/transport/tauri", async (original) => ({ ...(await original<typeof import("../../ipc/transport/tauri")>()), isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: () => native }));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => undefined }));
vi.mock("../settings/frame/SettingsFrame", () => ({ SettingsFrame: () => null }));

beforeEach(async () => { native.setTitle.mockClear(); await setLanguage("en"); });
afterEach(async () => { cleanup(); await setLanguage("en"); });

// spec: UI-121
// spec: UI-090
it("sets document and native title of a visualisation window in the UI language and follows a language change (B3-UI)", async () => {
  render(<VizWindow params={{ snapshotId: null, kind: "crawlTree", rootUrlId: null, label: "viz-test" }} />);
  await waitFor(() => expect(native.setTitle).toHaveBeenLastCalledWith("Crawl tree – OpenSpider"));
  expect(document.title).toBe("Crawl tree – OpenSpider");
  await act(async () => { await setLanguage("de"); });
  await waitFor(() => expect(native.setTitle).toHaveBeenLastCalledWith("Crawl-Baum – OpenSpider"));
  expect(document.title).toBe("Crawl-Baum – OpenSpider");
  cleanup();
  // Without a chosen kind the window still carries a translated title, never the neutral one of the shell.
  render(<VizWindow params={{ snapshotId: null, kind: null, rootUrlId: null, label: "viz-test" }} />);
  await waitFor(() => expect(native.setTitle).toHaveBeenLastCalledWith("Visualisierung – OpenSpider"));
});

// spec: UI-121
it("renames the settings window natively after a language change (B3-UI)", async () => {
  render(<SettingsWindow />);
  await waitFor(() => expect(native.setTitle).toHaveBeenLastCalledWith("Settings"));
  await act(async () => { await setLanguage("de"); });
  await waitFor(() => expect(native.setTitle).toHaveBeenLastCalledWith("Einstellungen"));
});
