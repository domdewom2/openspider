// M3-K01 (K3-01): Abschnitt „Chromium-Bereitstellung“ in Spider › Rendering – Status, Download mit Fortschritt,
// Abbruch/Fortsetzen, Entfernen, Hinweis mit Download-Knopf im JavaScript-Modus (de/en).
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setLanguage } from "../../../../i18n";
import { createClient, setClient } from "../../../../ipc/client";
import { IpcError } from "../../../../ipc/errors";
import type { ChromiumStatus } from "../../../../ipc/gen/ChromiumStatus";
import { MockCore } from "../../../../ipc/transport/mock";
import { useSession } from "../../../../state/session";
import { ConfigHost } from "../../frame/ConfigHost";
import { closeConfig, openConfig } from "../../frame/store";
import { installConfigCore, type FakeConfigCore } from "../../frame/testing";

const MB = 1_000_000;
let core: MockCore;
let fake: FakeConfigCore;
let status: ChromiumStatus;

function base(): ChromiumStatus {
  return {
    supported: true, version: "156.0.8078.80", downloadBytes: 186 * MB, installed: null, installedPath: null,
    active: "none", activePath: null, partialBytes: 0, job: "idle",
  };
}
function rendering(mode: "text" | "javascript", path: string | null = null) {
  const config = fake.config as unknown as { rendering: Record<string, unknown> };
  config.rendering = { ...config.rendering, mode, chromium_path: path };
}
async function open() {
  await act(async () => openConfig({ anchor: "rendering.mode" }));
  const dialog = await screen.findByRole("dialog", { name: /^(Crawl configuration|Crawl-Konfiguration)$/ });
  return within(await within(dialog).findByTestId("chromium-provision"));
}
const calls = (cmd: string) => core.calls.filter((c) => c.cmd === cmd);

beforeEach(async () => {
  core = new MockCore();
  fake = installConfigCore(core);
  core.handlers.set("compare.contentAvailable", () => false);
  core.handlers.set("api.list", () => []);
  core.handlers.set("catalog.columns", () => []);
  status = base();
  core.handlers.set("render_provision.status", () => structuredClone(status));
  setClient(createClient(core.transport("main")));
  await setLanguage("en");
  useSession.setState({ crawl: null, ops: {}, analysis: null });
});
afterEach(async () => { act(() => closeConfig()); cleanup(); await setLanguage("en"); });

describe("Chromium provisioning section (K3-01)", () => {
  // spec: OUT-260 [M3]
  it("downloads on request with progress, then shows the provisioned Chromium and offers removal", async () => {
    core.handlers.set("render_provision.download", () => {
      status = { ...status, job: { downloading: { done: 93 * MB, total: 186 * MB } } };
      return structuredClone(status);
    });
    core.handlers.set("render_provision.remove", () => { status = base(); return structuredClone(status); });
    render(<ConfigHost />);
    const section = await open();
    expect(section.getByRole("heading", { name: "Chromium for JavaScript rendering" })).toBeInTheDocument();
    expect(section.getByText(/fixed, verified “Chrome for Testing” version \(156\.0\.8078\.80\)/)).toBeInTheDocument();
    expect(section.getByTestId("chromium-prov-active")).toHaveTextContent("No Chromium found.");
    fireEvent.click(section.getByRole("button", { name: "Download Chromium (186 MB)" }));
    const bar = await section.findByRole("progressbar", { name: "Chromium download progress" });
    expect(bar).toHaveAttribute("aria-valuenow", String(93 * MB));
    expect(section.getByRole("status")).toHaveTextContent("Downloading: 93.0 MB of 186 MB");
    expect(calls("render_provision.download")[0].args).toEqual({ configuredPath: null });
    expect(section.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    // Fertig: der Abschnitt fragt während des Downloads selbst nach und zeigt danach die Installation.
    status = {
      ...base(), installed: "156.0.8078.80", installedPath: "/data/browser/156.0.8078.80/chrome-win64/chrome.exe",
      active: "provisioned", activePath: "/data/browser/156.0.8078.80/chrome-win64/chrome.exe",
    };
    expect(await section.findByTestId("chromium-prov-installed", {}, { timeout: 3000 })).toHaveTextContent("Downloaded: version 156.0.8078.80");
    expect(section.getByTestId("chromium-prov-active")).toHaveTextContent("Using the downloaded Chromium 156.0.8078.80.");
    expect(section.getByTestId("chromium-prov-active")).toHaveTextContent("Path: /data/browser/156.0.8078.80/chrome-win64/chrome.exe");
    expect(section.queryByRole("progressbar")).toBeNull();
    fireEvent.click(section.getByRole("button", { name: "Remove downloaded Chromium" }));
    expect(await section.findByRole("button", { name: /^Download Chromium/ })).toBeInTheDocument();
    expect(calls("render_provision.remove")).toHaveLength(1);
  });

  // spec: CRAWL-077 [P1]
  it("de: JavaScript mode without Chromium shows the notice with the download button", async () => {
    await setLanguage("de");
    rendering("javascript");
    core.handlers.set("render_provision.download", () => {
      status = { ...status, job: "verifying" };
      return structuredClone(status);
    });
    render(<ConfigHost />);
    const section = await open();
    const notice = section.getByTestId("chromium-prov-js-missing");
    expect(notice).toHaveTextContent("JavaScript-Rendering braucht Chromium, auf diesem Rechner wurde aber keins gefunden.");
    fireEvent.click(within(notice).getByRole("button", { name: /^Chromium herunterladen \(186/ }));
    expect(await section.findByText("Prüfsumme wird geprüft …")).toBeInTheDocument();
    expect(calls("render_provision.download")).toHaveLength(1);
  });

  // spec: CRAWL-077 [P1]
  it("a configured path that does not exist is named instead of offering a silent fallback", async () => {
    rendering("javascript", "/opt/none/chrome");
    core.handlers.set("render_provision.status", (args) => {
      expect(args).toEqual({ configuredPath: "/opt/none/chrome" });
      return { ...base(), activePath: "/opt/none/chrome" };
    });
    render(<ConfigHost />);
    const section = await open();
    const notice = section.getByTestId("chromium-prov-js-missing");
    expect(notice).toHaveTextContent("the path you entered does not exist");
    expect(within(notice).queryByRole("button")).toBeNull();
    expect(section.getByTestId("chromium-prov-active")).toHaveTextContent("The Chromium path you entered does not exist.");
  });

  // spec: OUT-260 [M3]
  it("a cancelled download can be resumed or discarded", async () => {
    status = { ...base(), partialBytes: 93 * MB, job: { failed: { messageKey: "err-chromium-cancelled", detail: "x" } } };
    core.handlers.set("render_provision.remove", () => { status = base(); return structuredClone(status); });
    render(<ConfigHost />);
    const section = await open();
    expect(section.getByTestId("chromium-prov-failure")).toHaveTextContent("The download was cancelled. It can be resumed.");
    expect(section.getByRole("button", { name: "Resume download (93.0 MB of 186 MB)" })).toBeInTheDocument();
    fireEvent.click(section.getByRole("button", { name: "Discard partial download" }));
    expect(await section.findByRole("button", { name: /^Download Chromium \(186/ })).toBeInTheDocument();
  });

  // spec: OUT-265 [M3]
  it("de: a checksum failure is reported and nothing counts as installed", async () => {
    await setLanguage("de");
    status = { ...base(), job: { failed: { messageKey: "err-chromium-checksum", detail: "err-chromium-checksum: …" } } };
    render(<ConfigHost />);
    const section = await open();
    expect(section.getByTestId("chromium-prov-failure")).toHaveTextContent("Die Prüfsumme des heruntergeladenen Archivs stimmt nicht.");
    expect(section.queryByTestId("chromium-prov-installed")).toBeNull();
    expect(section.getByRole("button", { name: /^Chromium herunterladen/ })).toBeInTheDocument();
  });

  it("removal refused during a crawl shows the reason", async () => {
    status = { ...base(), installed: "156.0.8078.80", active: "provisioned", activePath: "/x/chrome" };
    core.handlers.set("render_provision.remove", () => {
      throw new IpcError({ code: "CrawlRunning", messageKey: "err-chromium-crawl-running", args: {}, field: null, detail: null, retryable: false } as never);
    });
    render(<ConfigHost />);
    const section = await open();
    fireEvent.click(section.getByRole("button", { name: "Remove downloaded Chromium" }));
    expect(await section.findByTestId("chromium-prov-action-error")).toHaveTextContent("cannot be removed while a crawl is running");
  });

  it("system Chrome and unsupported platforms are explained", async () => {
    status = { ...base(), supported: false, downloadBytes: null, active: "system", activePath: "/usr/bin/google-chrome" };
    render(<ConfigHost />);
    const section = await open();
    expect(section.getByTestId("chromium-prov-active")).toHaveTextContent("Using the Chrome/Chromium installed on this computer.");
    expect(section.getByRole("note")).toHaveTextContent("There is no download for this platform.");
    expect(section.queryByRole("button", { name: /^Download Chromium/ })).toBeNull();
  });

  it("an outdated provisioned version offers the update", async () => {
    status = { ...base(), installed: "155.0.1.2", active: "provisioned", activePath: "/x/chrome" };
    render(<ConfigHost />);
    const section = await open();
    expect(section.getByTestId("chromium-prov-installed")).toHaveTextContent("Version 156.0.8078.80 is available.");
    expect(section.getByRole("button", { name: "Update to version 156.0.8078.80" })).toBeInTheDocument();
  });
});
