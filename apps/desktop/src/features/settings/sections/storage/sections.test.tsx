import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import { localError } from "../../../../ipc/errors";
import { setLanguage } from "../../../../i18n";
import { usePrefs } from "../../../../state/prefs";
import { useSession } from "../../../../state/session";
import { SettingsFrame } from "../../frame/SettingsFrame";
import { Storage } from "./index";
import { ramMaximum, GiB } from "./resources";
import { Crawls } from "../crawls";
import { Exports } from "../exports";
import { Network } from "../network";
import { Accounts } from "../accounts";
import { McpReference } from "../mcp";
import fixture from "./fixture.json";
import pem from "../network/fixtures/ca.pem.txt?raw";
import { certificateIssuer } from "../network/certificates";

let core: MockCore;
beforeEach(async () => {
  core = new MockCore();
  core.restartKeys.clear();
  core.handlers.set("settings.resources", () => fixture);
  core.handlers.set("crawls.checkLocation", () => []);
  core.handlers.set("crawls.list", () => [{ crawl: { crawlId: "old", phase: "completed" } }, { crawl: { crawlId: "live", phase: "running" } }]);
  core.handlers.set("crawls.move", () => ({ opId: "move" }));
  core.handlers.set("op.cancel", () => null);
  core.handlers.set("settings.setProxyPassword", () => null);
  core.handlers.set("settings.probeCertificates", () => ({ certificates: [pem], trusted: false }));
  core.handlers.set("settings.importCertificates", () => ["/fixture/accepted.pem"]);
  core.handlers.set("api.list", () => [{ kind: "gsc", connected: true, account: "fixture@example.test", accounts: [], autoConnect: false }]);
  setClient(createClient(core.transport("settings")));
  usePrefs.setState({ dto: null, error: null, restartRequired: [], loaded: false });
  useSession.setState({ ops: {} });
  await usePrefs.getState().load();
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.useRealTimers(); });
async function select(label: string, option: string) {
  fireEvent.click(screen.getByRole("button", { name: new RegExp(label) }));
  fireEvent.click(await screen.findByRole("option", { name: option }));
}
function apply(field: string) { fireEvent.click(screen.getByRole("button", { name: `Apply: ${field}` })); }

// spec: UI-122
it("registers the six sections and offers profiles without a memory mode", async () => {
  render(<SettingsFrame anchor="storage" />);
  expect(screen.getByText(/saved continuously/)).toBeVisible();
  await select("Performance profile", "Minimum memory");
  await waitFor(() => expect(core.settings.storage.performanceProfile).toBe("min_memory"));
  expect(screen.queryByRole("radio", { name: /memory mode/i })).toBeNull();
  for (const name of ["Crawls", "Exports", "Network", "Accounts", "MCP"]) {
    fireEvent.click(screen.getByRole("tab", { name }));
    expect(document.querySelector(`[data-settings-section="${name.toLowerCase()}"]`)).not.toBeNull();
  }
}, 15_000);

// spec: UI-123
it("displays engine plus renderer usage and rejects invalid RAM limits without saving", async () => {
  render(<Storage />);
  expect(await screen.findByText(/Current memory use: 1 \/ 8 GB/)).toBeVisible();
  const input = screen.getByLabelText("RAM limit (GB)");
  for (const invalid of ["0.9", "14.1", "8.55", ""]) {
    fireEvent.change(input, { target: { value: invalid } });
    expect(screen.getByRole("button", { name: "Apply: RAM limit (GB)" })).toBeDisabled();
  }
  expect(core.calls.filter((c) => c.cmd === "settings.set")).toHaveLength(0);
  fireEvent.change(input, { target: { value: "6.5" } }); apply("RAM limit (GB)");
  await waitFor(() => expect(core.settings.storage.ramLimitGb).toBe(6.5));
  expect(screen.queryByText(/restart required/i)).toBeNull();
  expect(ramMaximum(3 * GiB)).toBe(1);
});

// spec: UI-123
it("refreshes the measured usage within a second and stops polling when unmounted", async () => {
  vi.useFakeTimers();
  const view = render(<Storage />);
  await act(async () => { await Promise.resolve(); });
  core.handlers.set("settings.resources", () => ({ ...fixture, usedBytes: 2 * GiB }));
  await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
  expect(screen.getByText(/Current memory use: 2 \/ 8 GB/)).toBeVisible();
  view.unmount();
  const count = core.calls.length;
  await vi.advanceTimersByTimeAsync(2000);
  expect(core.calls).toHaveLength(count);
});

// spec: OUT-267
it("shows hardware-derived cache and renderer defaults and persists validated overrides", async () => {
  render(<Storage />);
  await waitFor(() => expect(screen.getByLabelText("Maximum cache memory (MB)")).toHaveValue(4096));
  expect(screen.getByLabelText("Maximum parallel render processes")).toHaveValue(8);
  fireEvent.change(screen.getByLabelText("Maximum cache memory (MB)"), { target: { value: "2048" } }); apply("Maximum cache memory (MB)");
  await waitFor(() => expect(core.settings.storage.cacheLimitMb).toBe(2048));
  fireEvent.change(screen.getByLabelText("Maximum parallel render processes"), { target: { value: "33" } });
  expect(screen.getByRole("button", { name: /Apply: Maximum parallel/ })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Maximum parallel render processes"), { target: { value: "4" } }); apply("Maximum parallel render processes");
  await waitFor(() => expect(core.settings.storage.renderProcessLimit).toBe(4));
});

// spec: CRAWL-124 [M2]
it("warns before accepting risky storage, leaves files in place, then tracks and cancels an explicit move", async () => {
  core.handlers.set("crawls.checkLocation", () => ["warn-location-sync", "warn-location-network"]);
  render(<Storage />);
  fireEvent.change(screen.getByLabelText("Crawl folder"), { target: { value: "/fixture/sync" } }); apply("Crawl folder");
  const warning = await screen.findByRole("dialog", { name: "Check crawl folder" });
  expect(core.settings.storage.crawlsDir).toBeNull();
  fireEvent.click(within(warning).getByRole("button", { name: "Choose another folder" }));
  expect(core.settings.storage.crawlsDir).toBeNull();
  apply("Crawl folder");
  fireEvent.click(await screen.findByRole("button", { name: "Use anyway" }));
  await waitFor(() => expect(core.settings.storage.crawlsDir).toBe("/fixture/sync"));
  expect(core.calls.some((c) => c.cmd === "crawls.move")).toBe(false);
  fireEvent.click(await screen.findByRole("button", { name: "Move existing crawls" }));
  await waitFor(() => expect(core.calls.find((c) => c.cmd === "crawls.move")?.args).toEqual({ ids: ["old"], destinationDir: "/fixture/sync" }));
  act(() => useSession.setState({ ops: { move: { opId: "move", labelKey: "", done: 512, total: 1024, state: "running" } } }));
  expect(screen.getByRole("progressbar")).toHaveAttribute("value", "512");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(core.calls.find((c) => c.cmd === "op.cancel")?.args).toEqual({ opId: "move" }));
});

// spec: OUT-229
it("requires the saved-comparison warning before activating retention and validates custom days", async () => {
  render(<Crawls />);
  await select("Crawl retention", "1 week");
  expect(core.settings.retention.policy).toBe("never");
  expect(screen.getByText(/those comparisons become unavailable/)).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Enable retention" }));
  await waitFor(() => expect(core.settings.retention.policy).toBe("one_week"));
  await select("Crawl retention", "Custom (days)");
  const days = await screen.findByLabelText("Retention in days");
  fireEvent.change(days, { target: { value: "0" } });
  expect(screen.getByRole("button", { name: "Apply retention" })).toBeDisabled();
  expect(core.settings.retention.policy).toBe("one_week");
  fireEvent.change(days, { target: { value: "45" } });
  fireEvent.click(screen.getByRole("button", { name: "Apply retention" }));
  await waitFor(() => expect(core.settings.retention.customDays).toBe(45));
});

// spec: OUT-222
it("saves export defaults only as app settings, shows restart flags, and restores settings after an error", async () => {
  core.restartKeys.add("export.directory");
  render(<Exports />);
  fireEvent.change(screen.getByLabelText("Export folder"), { target: { value: "/fixture/exports" } }); apply("Export folder");
  await waitFor(() => expect(core.settings.export.directory).toBe("/fixture/exports"));
  expect(document.querySelector('[data-restart-required="export.directory"]')).not.toBeNull();
  expect(core.calls.every((c) => !c.cmd.startsWith("config."))).toBe(true);
  core.handlers.set("settings.set", () => { throw { ...localError("Permission", "err-permission"), detail: "/fixture/denied: read-only" }; });
  fireEvent.change(screen.getByLabelText("Export folder"), { target: { value: "/fixture/denied" } }); apply("Export folder");
  expect(await screen.findByRole("alert")).toHaveTextContent("/fixture/denied: read-only");
  expect(usePrefs.getState().dto?.export.directory).toBe("/fixture/exports");
});

// spec: CRAWL-122
it("validates manual proxy fields and sends credentials only to the keychain command", async () => {
  render(<Network />);
  await select("Proxy mode", "Manual proxy");
  expect(screen.getByRole("button", { name: "Save proxy" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Proxy host"), { target: { value: "proxy.fixture.test" } });
  fireEvent.change(screen.getByLabelText("Proxy exceptions"), { target: { value: "localhost, .fixture.test" } });
  fireEvent.click(screen.getByRole("button", { name: "Save proxy" }));
  await waitFor(() => expect(core.settings.network.proxy.bypass).toEqual(["localhost", ".fixture.test"]));
  fireEvent.change(screen.getByLabelText("Proxy password"), { target: { value: "fixture-secret" } });
  fireEvent.click(screen.getByRole("button", { name: "Save password in keychain" }));
  await waitFor(() => expect(core.calls.find((c) => c.cmd === "settings.setProxyPassword")?.args).toEqual({ password: "fixture-secret" }));
  expect(JSON.stringify(core.calls.filter((c) => c.cmd === "settings.set"))).not.toContain("fixture-secret");
  expect(screen.getByLabelText("Proxy password")).toHaveValue("");
});

// spec: OUT-269
it("offers one application-wide proxy without restart, with system and direct options", async () => {
  render(<Network />);
  expect(screen.getByText(/integrations, updates, Google connections and email/)).toBeVisible();
  await select("Proxy mode", "No proxy");
  await waitFor(() => expect(core.settings.network.proxy.mode).toBe("none"));
  await select("Proxy mode", "Use system proxy");
  await waitFor(() => expect(core.settings.network.proxy.mode).toBe("system"));
  expect(screen.queryByText(/Restart required/)).toBeNull();
});

// spec: OUT-270
it("displays certificate issuers and imports only after explicit acceptance", async () => {
  expect(certificateIssuer(pem)).toContain("CN=OpenSpider U14 Fixture");
  render(<Network />);
  fireEvent.change(screen.getByLabelText("HTTPS test URL"), { target: { value: "https://fixture.test" } });
  fireEvent.click(screen.getByRole("button", { name: "Discover certificates" }));
  expect(await screen.findByText(/Issuer: CN=OpenSpider U14 Fixture/)).toBeVisible();
  expect(core.settings.network.trustedCertificates).toEqual([]);
  fireEvent.click(screen.getByRole("button", { name: "Add discovered certificates" }));
  await waitFor(() => expect(core.settings.network.trustedCertificates).toEqual(["/fixture/accepted.pem"]));
  expect(core.calls.find((c) => c.cmd === "settings.importCertificates")?.args).toEqual({ certificates: [pem] });
});

// spec: OUT-222
it("lists connected accounts, persists autoconnect and refers MCP management to its menu", async () => {
  const view = render(<Accounts />);
  expect(await screen.findByText(/fixture@example.test/)).toBeVisible();
  fireEvent.click(screen.getByRole("switch", { name: "Connect GSC automatically at startup" }));
  await waitFor(() => expect(core.settings.accounts.autoConnect).toEqual(["gsc"]));
  view.unmount(); render(<McpReference />);
  expect(screen.getByText(/Open the MCP menu/)).toBeVisible();
});

// spec: OUT-222
it("marks the core's snake_case restart keys at the corresponding control", async () => {
  usePrefs.setState({ restartRequired: ["storage.crawls_dir"] });
  render(<Storage />);
  expect(document.querySelector('[data-restart-required="storage.crawls_dir"]')).not.toBeNull();
});

// spec: CRAWL-124 [M2]
it("moves more than 1000 existing crawls in sequential OpHandle batches", async () => {
  core.handlers.set("crawls.list", () => Array.from({ length: 1001 }, (_, i) => ({ crawl: { crawlId: `id-${i}`, phase: "completed" } })));
  let number = 0;
  core.handlers.set("crawls.move", () => ({ opId: `batch-${++number}` }));
  render(<Storage />);
  fireEvent.change(screen.getByLabelText("Crawl folder"), { target: { value: "/fixture/move" } }); apply("Crawl folder");
  fireEvent.click(await screen.findByRole("button", { name: "Move existing crawls" }));
  await waitFor(() => expect(number).toBe(1));
  expect((core.calls.find((c) => c.cmd === "crawls.move")?.args as { ids: string[] }).ids).toHaveLength(1000);
  act(() => useSession.setState({ ops: { "batch-1": { opId: "batch-1", labelKey: "", done: 1000, total: 1000, state: "done" } } }));
  await waitFor(() => expect(number).toBe(2));
  expect(core.calls.filter((c) => c.cmd === "crawls.move")[1]?.args).toEqual({ ids: ["id-1000"], destinationDir: "/fixture/move" });
  act(() => useSession.setState({ ops: { "batch-2": { opId: "batch-2", labelKey: "", done: 1, total: 1, state: "done" } } }));
  expect(screen.getByText("Crawls moved.")).toBeVisible();
});

// spec: CRAWL-124 [M2]
it("checks a folder selected with the native directory chooser before changing settings", async () => {
  core.handlers.set("dialog.open", () => "/fixture/native-folder");
  core.handlers.set("crawls.checkLocation", () => ["warn-location-network"]);
  setClient(createClient({ ...core.transport("settings"), name: "tauri" }));
  render(<Storage />);
  fireEvent.click(screen.getByRole("button", { name: "Choose…" }));
  expect(await screen.findByRole("dialog", { name: "Check crawl folder" })).toHaveTextContent("/fixture/native-folder");
  expect(core.settings.storage.crawlsDir).toBeNull();
  const options = (core.calls.find((c) => c.cmd === "dialog.open")?.args as { options: { directory: boolean; multiple: boolean } }).options;
  expect(options.directory).toBe(true);
  expect(options.multiple).toBe(false);
});
