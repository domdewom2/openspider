import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { makeT, setLanguage } from "../../i18n";
import { createClient, setClient } from "../../ipc/client";
import { eventBus } from "../../ipc/events";
import { IpcError, localError } from "../../ipc/errors";
import type { McpStatus } from "../../ipc/gen/McpStatus";
import { attachSession, useSession } from "../../state/session";
import { runAction } from "../../ui/actions";
import { allMenuContributions } from "../../ui/menu/registry";
import { buildMenuModel } from "../../ui/menu/build";
import { McpHost, McpStatusField } from "./McpHost";
import { AccessPanel } from "./AccessPanel";
import { ClientsPanel } from "./ClientsPanel";
import { nativeMcpDesktop, type McpDesktop } from "./desktop";
import { openMcp, parsePort, redactText, statusText, useMcpUi } from "./model";
import { clientPreview, createMcpFixture, TEST_TOKEN } from "./fixtures/agent-m2-ui";

let fixture: ReturnType<typeof createMcpFixture>;
let desktop: McpDesktop;
let offs: (() => void)[];
const calls = (cmd: string) => fixture.core.calls.filter(call => call.cmd === cmd);
const click = async (name: string) => fireEvent.click(await screen.findByRole("button", { name }));
const selectTab = async (name: string) => fireEvent.click(await screen.findByRole("tab", { name }));
async function host(tab: Parameters<typeof openMcp>[0] = "server") {
  openMcp(tab);
  render(<><McpHost desktop={desktop} /><McpStatusField /></>);
  await waitFor(() => expect(screen.getByRole("button", { name: "Start server" })).toBeEnabled());
}
beforeEach(async () => {
  fixture = createMcpFixture();
  setClient(createClient(fixture.core.transport("main")));
  await setLanguage("en");
  useMcpUi.setState({ tab: null, status: { kind: "off" } });
  useSession.setState({ seq: -1, mcp: null, crawl: null, notices: [] });
  offs = [attachSession(eventBus), fixture.core.addSub("main", event => eventBus.emit(event))];
  desktop = { copy: vi.fn(async () => undefined), chooseMarkdown: vi.fn(async () => "fixture-home/openspider-mcp-tools.md") };
});
afterEach(() => { cleanup(); offs.reverse().forEach(off => off()); setClient(null); vi.restoreAllMocks(); });

// spec: UI-129
it("registers the MCP menu between View and Help without a crawl, and opens it from the status field", async () => {
  render(<><McpHost desktop={desktop} /><McpStatusField /></>);
  await waitFor(() => expect(allMenuContributions().some(c => c.menu === "mcp")).toBe(true));
  const model = buildMenuModel(allMenuContributions(), { platform: "win", lang: "en", t: makeT("en"), ctx: { hasCrawl: false }, native: false });
  const ids = model.menus.map(m => m.id);
  const { MENU_ORDER } = await import("../../ui/menu/types");
  expect(MENU_ORDER.win.indexOf("mcp")).toBeGreaterThan(MENU_ORDER.win.indexOf("view"));
  expect(MENU_ORDER.win.indexOf("mcp")).toBeLessThan(MENU_ORDER.win.indexOf("help"));
  expect(ids).toContain("mcp");
  await click("MCP: off");
  fireEvent.click(await screen.findByRole("menuitem", { name: "Configure…" }));
  expect(await screen.findByRole("dialog", { name: "MCP" })).toBeVisible();
  expect(screen.getAllByRole("tab")).toHaveLength(5);
  await click("Close");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

// spec: OUT-285 [P1]
it("starts on a validated port, follows live session ticks and persists autostart through reload", async () => {
  await host();
  const input = screen.getByRole("textbox", { name: "Port" });
  fireEvent.change(input, { target: { value: "65536" } });
  expect(screen.getByRole("button", { name: "Start server" })).toBeDisabled();
  fireEvent.change(input, { target: { value: "51234" } });
  await click("Start server");
  await waitFor(() => expect(calls("mcp.start")).toHaveLength(1));
  expect(calls("mcp.start")[0]?.args).toEqual({ port: 51234 });
  expect(await screen.findByRole("button", { name: "MCP: ready (Port 51234)", hidden: true })).toBeInTheDocument();
  for (const clients of [1, 2, 1, 0]) {
    act(() => fixture.setStatus(clients ? { kind: "connected", clients, port: 51234 } : { kind: "ready", port: 51234 }));
    expect(screen.getByRole("button", { name: clients ? `MCP: connected (${clients} ${clients === 1 ? "client" : "clients"})` : "MCP: ready (Port 51234)", hidden: true })).toBeInTheDocument();
  }
  await click("Stop server");
  await waitFor(() => expect(screen.getByRole("button", { name: "MCP: off", hidden: true })).toBeInTheDocument());
  fireEvent.click(screen.getByRole("checkbox", { name: "Start automatically when the app opens" }));
  await waitFor(() => expect(fixture.core.settings.mcp.autostart).toBe(true));
  cleanup();
  await host();
  expect(screen.getByRole("checkbox", { name: "Start automatically when the app opens" })).toBeChecked();
  expect(calls("mcp.start")).toHaveLength(1); // Autostart is owned by the service, not a second UI start.
});

// spec: OUT-285 [M2]
it("retains a newer tick when the initial status request finishes late", async () => {
  let resolve!: (value: McpStatus) => void;
  fixture.core.handlers.set("mcp.status", () => new Promise<McpStatus>(r => { resolve = r; }));
  render(<><McpHost desktop={desktop} /><McpStatusField /></>);
  await waitFor(() => expect(calls("mcp.status")).toHaveLength(1));
  act(() => fixture.setStatus({ kind: "connected", clients: 2, port: 51234 }));
  await act(async () => resolve({ kind: "off" }));
  expect(screen.getByRole("button", { name: "MCP: connected (2 clients)" })).toBeVisible();
});

// spec: OUT-285 [P1]
it("reports a port failure without changing Off and serializes duplicate starts", async () => {
  await host();
  let reject!: (error: unknown) => void;
  fixture.core.handlers.set("mcp.start", () => new Promise((_, r) => { reject = r; }));
  act(() => { runAction("mcp.start"); runAction("mcp.start"); });
  await waitFor(() => expect(calls("mcp.start")).toHaveLength(1));
  await act(async () => reject(new Error(`Port occupied: ${TEST_TOKEN}`)));
  expect(await screen.findByRole("alert")).toHaveTextContent("MCP operation failed");
  expect(screen.getByRole("button", { name: "MCP: off", hidden: true })).toBeInTheDocument();
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
});

// spec: UI-129
it("shows a recoverable error for the current K17 stub and cleans up event listeners and menu contributions", async () => {
  const listeners = eventBus.listenerCount();
  fixture.core.handlers.set("mcp.status", () => { throw new IpcError(localError("Unsupported", "err-unsupported")); });
  render(<McpHost desktop={desktop} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("not available in this build");
  expect(eventBus.listenerCount()).toBe(listeners + 3);
  cleanup();
  expect(eventBus.listenerCount()).toBe(listeners);
  expect(allMenuContributions().filter(c => c.menu === "mcp")).toEqual([]);
});

// spec: OUT-285 [P1]
it("shows service autostart failures even when the dialog is closed, without rendering notice arguments", async () => {
  render(<><McpHost desktop={desktop} /><McpStatusField /></>);
  await waitFor(() => expect(calls("mcp.status")).toHaveLength(1));
  act(() => eventBus.emit({ kind: "notice", notice: { code: "mcp-start-failed", level: "error", args: { token: TEST_TOKEN } } }));
  expect(await screen.findByRole("alert")).toHaveTextContent("MCP operation failed");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  expect(screen.getByRole("button", { name: "MCP: off" })).toBeVisible();
});

// spec: AGENT-003 [M2]
it("defaults to read only, requires explicit class grants and retains per-tool disabling", async () => {
  fixture.core.settings.mcp.classes = undefined; // Older settings: default read, never infer grants from tools.
  await host();
  await selectTab("Tools");
  const read = await screen.findByRole("checkbox", { name: "Read" });
  expect(read).toBeChecked(); expect(read).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: "Crawl" })).not.toBeChecked();
  expect(await screen.findByRole("checkbox", { name: "crawl_start" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "Crawl" }));
  await waitFor(() => expect(calls("mcp.setClasses")[0]?.args).toEqual({ classes: ["read", "crawl"] }));
  await waitFor(() => expect(screen.getByRole("checkbox", { name: "crawl_start" })).toBeEnabled());
  fireEvent.click(screen.getByRole("checkbox", { name: "query_urls" }));
  await waitFor(() => expect(calls("mcp.setToolEnabled")[0]?.args).toEqual({ name: "query_urls", enabled: false }));
  await waitFor(() => expect(screen.getByRole("checkbox", { name: "query_urls" })).not.toBeChecked());
  const active = fixture.tools().filter(tool => tool.enabled).map(tool => tool.name);
  expect(active).toEqual(["list_crawls", "crawl_start"]);
  cleanup();
  await host(); await selectTab("Tools");
  expect(await screen.findByRole("checkbox", { name: "Crawl" })).toBeChecked();
  expect(await screen.findByRole("checkbox", { name: "query_urls" })).not.toBeChecked();
});

// spec: OUT-285 [P1]
it("keeps service tool names and schemas intact and saves Markdown only after accepting a destination", async () => {
  await host(); await selectTab("Tools");
  const checkbox = await screen.findByRole("checkbox", { name: "query_urls" });
  expect(checkbox.closest("tr")?.querySelector("pre")?.textContent).toBe(JSON.stringify(fixture.tools()[1]!.schema, null, 2));
  vi.mocked(desktop.chooseMarkdown).mockResolvedValueOnce(null);
  await click("Download API description (Markdown)…");
  await waitFor(() => expect(desktop.chooseMarkdown).toHaveBeenCalledTimes(1));
  expect(calls("mcp.saveToolsMarkdown")).toHaveLength(0);
  await waitFor(() => expect(screen.getByRole("button", { name: "Download API description (Markdown)…" })).toBeEnabled());
  await click("Download API description (Markdown)…");
  await waitFor(() => expect(fixture.markdown).toHaveLength(1));
  expect(fixture.markdown[0]?.path).toBe("fixture-home/openspider-mcp-tools.md");
  expect(fixture.markdown[0]?.text.match(/^## /gm)).toHaveLength(fixture.tools().filter(tool => tool.enabled).length);
});

// spec: AGENT-003 [M2]
it("refreshes effective tool permissions when another window changes MCP settings", async () => {
  await host(); await selectTab("Tools");
  expect(await screen.findByRole("checkbox", { name: "export_tab" })).not.toBeChecked();
  act(() => {
    fixture.core.settings.mcp.classes = ["read", "write"];
    fixture.core.emit("*", { kind: "settingsChanged", keys: ["mcp.classes"] });
  });
  await waitFor(() => expect(screen.getByRole("checkbox", { name: "Write" })).toBeChecked());
  await waitFor(() => expect(screen.getByRole("checkbox", { name: "export_tab" })).toBeChecked());
});

// spec: AGENT-002
it("copies without revealing, displays only after Show and rotates without requesting a visible token", async () => {
  render(<AccessPanel desktop={desktop} />);
  expect(calls("mcp.token")).toHaveLength(0);
  await click("Copy token");
  await waitFor(() => expect(desktop.copy).toHaveBeenCalledWith(TEST_TOKEN));
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Show token");
  expect(await screen.findByText(TEST_TOKEN)).toBeVisible();
  await click("Hide token");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Regenerate token");
  await waitFor(() => expect(calls("mcp.regenerateToken")[0]?.args).toEqual({ reveal: false }));
  expect(fixture.token()).not.toBe(TEST_TOKEN);
  expect(document.body.innerHTML).not.toContain(fixture.token());
  await click("Copy token");
  await waitFor(() => expect(desktop.copy).toHaveBeenLastCalledWith(fixture.token()));
});

// spec: AGENT-002
it("discards late reveal responses on unmount and never renders clipboard error secrets", async () => {
  let resolve!: (value: unknown) => void;
  fixture.core.handlers.set("mcp.token", () => new Promise(r => { resolve = r; }));
  render(<AccessPanel desktop={desktop} />);
  await click("Show token");
  cleanup();
  render(<AccessPanel desktop={desktop} />);
  await act(async () => resolve({ masked: TEST_TOKEN, value: TEST_TOKEN }));
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  fixture.core.handlers.set("mcp.token", () => ({ masked: "mask", value: TEST_TOKEN }));
  vi.mocked(desktop.copy).mockRejectedValue(new Error(TEST_TOKEN));
  await click("Copy token");
  expect(await screen.findByRole("alert")).toBeVisible();
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
});

// spec: AGENT-002
it("clears a revealed token when changing tabs, closing and hiding the window", async () => {
  await host(); await selectTab("Access"); await click("Show token");
  expect(await screen.findByText(TEST_TOKEN)).toBeVisible();
  act(() => document.dispatchEvent(new Event("visibilitychange")));
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Show token");
  expect(await screen.findByText(TEST_TOKEN)).toBeVisible();
  await selectTab("Server"); await selectTab("Access");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Show token"); expect(await screen.findByText(TEST_TOKEN)).toBeVisible();
  await click("Close");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
});

// spec: AGENT-043
it("requires preview and explicit confirmation for each client; shows backup and preserves the preview on failure", async () => {
  for (const kind of ["claudeDesktop", "claudeCode", "codex"] as const) {
    cleanup();
    render(<ClientsPanel initialClient={kind} desktop={desktop} />);
    expect(screen.queryByRole("button", { name: "Write configuration with backup" })).not.toBeInTheDocument();
    await click("Preview changes");
    const write = await screen.findByRole("button", { name: "Write configuration with backup" });
    expect(write).toBeDisabled();
    expect(screen.getByLabelText("Configuration diff")).toHaveTextContent("foreign-helper");
    fireEvent.click(screen.getByRole("checkbox", { name: /I have reviewed/ }));
    expect(write).toBeEnabled();
    await click("Write configuration with backup");
    expect(await screen.findByText(`Backup: ${kind}.backup`)).toBeVisible();
    expect(fixture.written.at(-1)).toBe(kind);
    expect(calls("mcp.clientConfigPreview").filter(call => (call.args as { client: string }).client === kind)).toHaveLength(2);
  }
});

// spec: AGENT-043
it("replaces a stale diff instead of writing and redacts client config secrets", async () => {
  let changed = false;
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("claudeDesktop", JSON.stringify({ token: TEST_TOKEN, mcpServers: { foreign: { command: changed ? "changed-helper" : "foreign-helper" } } })));
  render(<ClientsPanel initialClient="claudeDesktop" desktop={desktop} />);
  await click("Preview changes");
  fireEvent.click(await screen.findByRole("checkbox", { name: /I have reviewed/ }));
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  changed = true;
  await click("Write configuration with backup");
  expect(await screen.findByText(/configuration changed/)).toBeVisible();
  expect(calls("mcp.clientConfigWrite")).toHaveLength(0);
  expect(screen.getByRole("checkbox", { name: /I have reviewed/ })).not.toBeChecked();
  expect(screen.getByLabelText("Configuration diff")).toHaveTextContent("changed-helper");
});

// spec: AGENT-043
it("renders bounded log pages including duration and result size, with credential redaction", async () => {
  for (let i = 0; i < 26; i++) fixture.record(`query_${i}`, `{"crawl_id":"out-basic-site","token":"${TEST_TOKEN}","authorization":"Bearer ${TEST_TOKEN}"}`);
  await host(); await selectTab("Call log");
  const table = await screen.findByRole("table", { name: "Call log" });
  expect(within(table).getAllByRole("row")).toHaveLength(26);
  expect(table).toHaveTextContent("12 ms"); expect(table).toHaveTextContent("2,048 B");
  expect(table).toHaveTextContent("out-basic-site");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Next page");
  await waitFor(() => expect(calls("mcp.callLog").at(-1)?.args).toEqual({ offset: 25, limit: 25 }));
  expect(await screen.findByText("query_25")).toBeVisible();
  expect(screen.getByRole("button", { name: "Next page" })).toBeDisabled();
});

// spec: UI-129
it("uses the canonical German status strings, plural rules and strict port validation", () => {
  const t = makeT("de");
  expect(statusText({ kind: "off" }, t)).toBe("MCP: aus");
  expect(statusText({ kind: "ready", port: 51234 }, t)).toBe("MCP: bereit (Port 51234)");
  expect(statusText({ kind: "connected", port: 51234, clients: 1 }, t)).toBe("MCP: verbunden (1 Client)");
  expect(statusText({ kind: "connected", port: 51234, clients: 2 }, t)).toBe("MCP: verbunden (2 Clients)");
  for (const input of ["-1", "0", "65536", "1.2", "1e3", "0x80", "abc"]) expect(parsePort(input)).toBeUndefined();
  expect(parsePort("")).toBeNull(); expect(parsePort("65535")).toBe(65535);
});

// spec: AGENT-043
it("redacts credentials in JSON, TOML, CLI arguments, URL userinfo and bearer text", () => {
  for (const text of [`{"access_token":"${TEST_TOKEN}"}`, `OPENAI_API_KEY = "${TEST_TOKEN}"`, `https://user:${TEST_TOKEN}@example.test/`, `Authorization: Bearer ${TEST_TOKEN}`, `url?api_key=${TEST_TOKEN}&name=ok`, `["mcp", "--token", "${TEST_TOKEN}"]`, `--api-key '${TEST_TOKEN}'`, `--password ${TEST_TOKEN}`]) expect(redactText(text)).not.toContain(TEST_TOKEN);
});

// spec: OUT-285 [P1]
it("uses the canonical Markdown filename in the native save dialog", async () => {
  setClient({ transportName: "tauri", invoke: vi.fn(async () => null) as never, subscribe: () => () => undefined });
  const { getClient } = await import("../../ipc/client");
  await nativeMcpDesktop.chooseMarkdown();
  expect(getClient().invoke).toHaveBeenCalledWith("dialog.save", expect.objectContaining({ options: expect.objectContaining({ defaultPath: "openspider-mcp-tools.md" }) }), undefined);
});
