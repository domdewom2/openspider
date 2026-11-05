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
import { ClientsPanel, ConnectPanel } from "./ClientsPanel";
import { nativeMcpDesktop, type McpDesktop } from "./desktop";
import { openConnect, openMcp, parsePort, redactText, statusText, useMcpUi } from "./model";
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
  useMcpUi.setState({ tab: null, connect: null, status: { kind: "off" } });
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
it("keeps the passive K17 probe silent, reports unavailability after opening the dialog and cleans up", async () => {
  const listeners = eventBus.listenerCount();
  fixture.core.handlers.set("mcp.status", () => { throw new IpcError(localError("Unsupported", "err-unsupported")); });
  await act(async () => { render(<McpHost desktop={desktop} />); });
  await waitFor(() => expect(calls("mcp.status")).toHaveLength(1));
  await waitFor(() => expect(calls("settings.get")).toHaveLength(1));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  const start = allMenuContributions().flatMap(c => c.items).find(item => item.id === "mcp.start");
  expect(start?.enabled?.({})).toBe(false);
  act(() => openMcp());
  expect(await screen.findByRole("alert")).toHaveTextContent("not available in this build");
  expect(screen.getByRole("button", { name: "Start server" })).toBeDisabled();
  expect(eventBus.listenerCount()).toBe(listeners + 3);
  cleanup();
  expect(eventBus.listenerCount()).toBe(listeners);
  expect(allMenuContributions().filter(c => c.menu === "mcp")).toEqual([]);
});

// spec: UI-129
it("reports Unsupported from an explicit server start after a successful passive probe", async () => {
  fixture.core.handlers.set("mcp.start", () => { throw new IpcError(localError("Unsupported", "err-unsupported")); });
  await host();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  await click("Start server");
  expect(await screen.findByRole("alert")).toHaveTextContent("not available in this build");
  expect(calls("mcp.start")).toHaveLength(1);
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

// spec: AGENT-043
// spec: AGENT-003 [M2]
it("connects each program in one click from its own menu dialog, with crawl preset and a new-session instruction", async () => {
  render(<McpHost desktop={desktop} />);
  for (const [kind, program] of [["claudeDesktop", "Claude Desktop"], ["claudeCode", "Claude Code"], ["codex", "Codex"], ["cursor", "Cursor"]] as const) {
    act(() => { runAction(`mcp.${kind}`); });
    const dialog = await screen.findByRole("dialog", { name: `Connect ${program}` });
    expect(within(dialog).getByText("Not connected yet.")).toBeVisible();
    const crawl = within(dialog).getByRole("checkbox", { name: "Agent may start crawls" });
    expect(crawl).toBeChecked();
    // No review checkbox and no mandatory preview: one button does backup, entry and permission.
    expect(within(dialog).queryByRole("checkbox", { name: /reviewed/i })).not.toBeInTheDocument();
    expect(within(dialog).getByText("Show changes").closest("details")).not.toHaveAttribute("open");
    fireEvent.click(within(dialog).getByRole("button", { name: "Connect" }));
    // Cursor manages MCP servers in its settings: the instruction asks to reload them (AGENT-043, 2026-11-05).
    const done = kind === "cursor"
      ? "Connected. Reload the MCP servers in Cursor or restart Cursor so that OpenSpider appears there."
      : `Connected. Start a new session in ${program} so that OpenSpider appears there.`;
    expect(await within(dialog).findByText(done)).toBeVisible();
    expect(within(dialog).getByText(`Backup: ${kind}.backup`)).toBeVisible();
    expect(calls("mcp.clientConnect").at(-1)?.args).toEqual({ client: kind, allowCrawl: true });
    expect(await within(dialog).findByText("Already connected.")).toBeVisible();
    expect(within(dialog).getByRole("button", { name: "Update" })).toBeEnabled();
    expect(within(dialog).getByRole("button", { name: "Disconnect" })).toBeEnabled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  }
  expect(fixture.core.settings.mcp.classes).toEqual(["read", "crawl"]);
  expect(calls("mcp.clientConfigWrite")).toHaveLength(0);
});

// spec: AGENT-003 [M2]
// spec: AGENT-043
it("connects read-only when the crawl checkbox is cleared", async () => {
  fixture.core.settings.mcp.classes = ["read", "crawl", "write"];
  render(<ConnectPanel client="codex" desktop={desktop} />);
  const crawl = await screen.findByRole("checkbox", { name: "Agent may start crawls" });
  expect(crawl).toBeChecked();
  fireEvent.click(crawl);
  expect(crawl).not.toBeChecked();
  await click("Connect");
  await waitFor(() => expect(calls("mcp.clientConnect").at(-1)?.args).toEqual({ client: "codex", allowCrawl: false }));
  await waitFor(() => expect(fixture.core.settings.mcp.classes).toEqual(["read", "write"]));
});

// spec: AGENT-043
it("recognises an existing entry, updates without changes and disconnects only the own entry", async () => {
  fixture.connected.add("codex");
  fixture.core.settings.mcp.classes = ["read"];
  render(<ConnectPanel client="codex" desktop={desktop} />);
  expect(await screen.findByText("Already connected.")).toBeVisible();
  // An existing connection shows the current grant instead of the default.
  expect(screen.getByRole("checkbox", { name: "Agent may start crawls" })).not.toBeChecked();
  await click("Update");
  expect(await screen.findByText("Already connected; the file is unchanged. Start a new session in Codex if OpenSpider is still missing there.")).toBeVisible();
  expect(screen.queryByText(/^Backup:/)).not.toBeInTheDocument();
  await click("Disconnect");
  expect(await screen.findByText("Disconnected. Start a new session in Codex.")).toBeVisible();
  expect(screen.getByText("Backup: codex.disconnect-backup")).toBeVisible();
  expect(await screen.findByText("Not connected yet.")).toBeVisible();
  expect(screen.queryByRole("button", { name: "Disconnect" })).not.toBeInTheDocument();
});

// spec: AGENT-043
it("shows only masked changed lines on request and copies only the own entry", async () => {
  const value = clientPreview("claudeCode", "notConnected", {
    // Defence in depth: even an unmasked service value is redacted before rendering.
    connectDiff: `@@ -3 +3,2 @@\n-  "x": 1\n+  "x": 1,\n+  "authorization": "Bearer ${TEST_TOKEN}"\n`,
    path: `fixture-home/.claude.json?token=${TEST_TOKEN}`,
  });
  fixture.core.handlers.set("mcp.clientConfigPreview", () => value);
  render(<ConnectPanel client="claudeCode" desktop={desktop} />);
  fireEvent.click(await screen.findByText("Show changes"));
  const diff = screen.getByLabelText("Configuration diff");
  // Line structure is preserved exactly (pre-formatted); only the masked value differs.
  expect(diff.textContent).toContain('\n+  "x": 1,\n+  "authorization": "[redacted]"');
  expect(screen.getByText(/Changed lines only/)).toBeVisible();
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  await click("Copy entry");
  await waitFor(() => expect(desktop.copy).toHaveBeenLastCalledWith(value.entry));
  expect(value.entry).not.toContain('"x"');
  expect(await screen.findByText(/Entry copied to the clipboard/)).toBeVisible();
});

// spec: AGENT-043
it("blocks files that cannot be changed and reports only fixed messages", async () => {
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("claudeDesktop", "notConnected", { problem: "syntax", line: 7, connectDiff: "", disconnectDiff: "" }));
  render(<ConnectPanel client="claudeDesktop" desktop={desktop} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("The file is not valid JSON or TOML (line 7). OpenSpider does not change it.");
  expect(screen.getByText("The file cannot be changed.")).toBeVisible();
  expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: "Agent may start crawls" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Copy entry" })).toBeEnabled();
  expect(screen.queryByText("Show changes")).not.toBeInTheDocument();
  cleanup();
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("codex"));
  for (const [error, text] of [
    [{ code: "Stale", messageKey: "mcp-client-changed", args: {}, retryable: false, detail: TEST_TOKEN }, "Another program changed the file while connecting. Nothing was written; try again."],
    [{ code: "InvalidInput", messageKey: "mcp-client-problem-syntax", args: { line: 3 }, retryable: false, detail: TEST_TOKEN }, "The file is not valid JSON or TOML (line 3)."],
    [{ code: "Io", messageKey: `err-${TEST_TOKEN}`, args: {}, retryable: false, detail: TEST_TOKEN }, "The MCP operation failed."],
  ] as const) {
    cleanup();
    fixture.core.handlers.set("mcp.clientConnect", () => { throw new IpcError(error as never); });
    render(<ConnectPanel client="codex" desktop={desktop} />);
    await click("Connect");
    expect(await screen.findByRole("alert")).toHaveTextContent(text);
    expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  }
});

// spec: AGENT-043
it("tells Cursor users to reload the MCP servers after updating and disconnecting, in German too", async () => {
  fixture.connected.add("cursor");
  render(<ConnectPanel client="cursor" desktop={desktop} />);
  expect(await screen.findByText("Already connected.")).toBeVisible();
  await click("Update");
  expect(await screen.findByText("Already connected; the file is unchanged. Reload the MCP servers in Cursor if OpenSpider is still missing there.")).toBeVisible();
  await click("Disconnect");
  expect(await screen.findByText("Disconnected. Reload the MCP servers in Cursor.")).toBeVisible();
  expect(screen.queryByText(/new session/)).not.toBeInTheDocument();
  const de = makeT("de");
  expect(de("mcp-connect-cursor")).toBe("Mit Cursor verbinden…");
  expect(de("mcp-client-done-connected-reload", { program: "Cursor" })).toBe("Verbunden. Lade in Cursor die MCP-Server neu oder starte Cursor neu, damit OpenSpider dort erscheint.");
  expect(de("mcp-client-done-disconnected-reload", { program: "Cursor" })).toBe("Getrennt. Lade in Cursor die MCP-Server neu.");
});

// spec: AGENT-043
it("blocks a read-only configuration file with a fixed message and keeps copying", async () => {
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("cursor", "notConnected", { problem: "readOnly", connectDiff: "", disconnectDiff: "" }));
  render(<ConnectPanel client="cursor" desktop={desktop} />);
  expect(await screen.findByRole("alert")).toHaveTextContent("The configuration file is read-only. OpenSpider does not change it. Remove the write protection, or copy the entry and paste it by hand.");
  expect(screen.getByText("The file cannot be changed.")).toBeVisible();
  expect(screen.getByRole("button", { name: "Connect" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Copy entry" })).toBeEnabled();
  cleanup();
  // The service refuses as well (file became read-only after the preview); only the fixed text appears.
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("cursor"));
  fixture.core.handlers.set("mcp.clientConnect", () => { throw new IpcError({ code: "Permission", messageKey: "mcp-client-problem-readOnly", args: {}, retryable: false, detail: TEST_TOKEN } as never); });
  render(<ConnectPanel client="cursor" desktop={desktop} />);
  await click("Connect");
  expect(await screen.findByRole("alert")).toHaveTextContent("The configuration file is read-only.");
  expect(document.body.innerHTML).not.toContain(TEST_TOKEN);
  expect(makeT("de")("mcp-client-problem-readOnly")).toContain("schreibgeschützt");
});

// spec: AGENT-043
it("offers the same panel with a program selection in the Clients tab", async () => {
  await host();
  await selectTab("Clients");
  expect(await screen.findByRole("region", { name: "Connect Claude Desktop" })).toBeVisible();
  cleanup();
  render(<ClientsPanel initialClient="codex" desktop={desktop} />);
  expect(await screen.findByRole("region", { name: "Connect Codex" })).toBeVisible();
  act(() => { openConnect("claudeCode"); });
  expect(useMcpUi.getState()).toMatchObject({ tab: null, connect: "claudeCode" });
});

// spec: AGENT-003 [M2]
it("does not promise a cost estimate or user confirmation before AGENT-030", () => {
  const de = makeT("de")("mcp-classes-help");
  const en = makeT("en")("mcp-classes-help");
  expect(de).not.toMatch(/Kostenschätzung|Bestätigung/);
  expect(en).not.toMatch(/cost estimate|confirmation/i);
  expect(de).toContain("Linkmetriken");
});

// spec: AGENT-043
it("warns before an update replaces the user's additions in the own entry, naming keys only", async () => {
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("codex", "outdated", { extraKeys: ["env", "startup_timeout_sec"] }));
  render(<ConnectPanel client="codex" desktop={desktop} />);
  expect(await screen.findByText("Connected, but the entry is outdated (for example a different program path).")).toBeVisible();
  expect(screen.getByText("The existing entry contains your own additions (env, startup_timeout_sec). “Update” replaces the whole entry; afterwards the additions are only in the backup.")).toBeVisible();
  expect(screen.getByRole("button", { name: "Update" })).toBeEnabled();
  cleanup();
  fixture.core.handlers.set("mcp.clientConfigPreview", () => clientPreview("codex", "outdated"));
  render(<ConnectPanel client="codex" desktop={desktop} />);
  expect(await screen.findByText(/entry is outdated/)).toBeVisible();
  expect(screen.queryByText(/own additions/)).not.toBeInTheDocument();
});
