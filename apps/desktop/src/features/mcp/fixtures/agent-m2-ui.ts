/** Test-only service fixture for U25. K17 owns the real server/file implementations. */
import { MockCore } from "../../../ipc/transport/mock";
import type { McpStatus } from "../../../ipc/gen/McpStatus";
import type { ToolInfo } from "../../../ipc/gen/ToolInfo";
import type { CallLogRow } from "../../../ipc/gen/CallLogRow";
import type { McpClientKind } from "../../../ipc/gen/McpClientKind";
import type { McpClientPreview } from "../../../ipc/gen/McpClientPreview";

export const TEST_TOKEN = "u25-fixture-token-0123456789abcdef";
export const fixtureTools: ToolInfo[] = [
  { name: "list_crawls", class: "read", enabled: true, description: "List local crawls.", schema: { type: "object", properties: {} } },
  { name: "query_urls", class: "read", enabled: true, description: "Query crawl URLs.", schema: { type: "object", properties: { crawl_id: { type: "string" } }, required: ["crawl_id"] } },
  { name: "crawl_start", class: "crawl", enabled: false, description: "Start a crawl.", schema: { type: "object", properties: { start_url: { type: "string" } } } },
  { name: "export_tab", class: "write", enabled: false, description: "Export a tab.", schema: { type: "object", properties: { path: { type: "string" } } } },
  { name: "psi", class: "cost", enabled: false, description: "Request a performance report.", schema: { type: "object", properties: { confirm: { type: "boolean" } } } },
];
const documents: Record<McpClientKind, string> = {
  claudeDesktop: '{"mcpServers":{"foreign":{"command":"foreign-helper"}}}',
  claudeCode: '{"mcpServers":{"foreign":{"command":"foreign-helper"}}}',
  codex: '[mcp_servers.foreign]\ncommand = "foreign-helper"\n',
};
export function clientPreview(kind: McpClientKind, before = documents[kind]): McpClientPreview {
  const server = { command: "openspider", args: ["mcp"] };
  const after = kind === "codex"
    ? `${before}\n[mcp_servers.openspider]\ncommand = "openspider"\nargs = ["mcp"]\n`
    : JSON.stringify({ ...JSON.parse(before), mcpServers: { ...JSON.parse(before).mcpServers, openspider: server } }, null, 2);
  return { path: `fixture-home/${kind}.${kind === "codex" ? "toml" : "json"}`, before, after, diff: `--- before\n+++ after\n-${before}\n+${after}` };
}
export function createMcpFixture() {
  const core = new MockCore();
  core.settings.mcp = { autostart: false, classes: ["read"], port: 51234 };
  let status: McpStatus = { kind: "off" };
  let token = TEST_TOKEN;
  let seq = 1;
  const disabled = new Set<string>();
  const written: string[] = [];
  const logs: CallLogRow[] = [];
  const markdown: { path: string; text: string }[] = [];
  const setStatus = (value: McpStatus) => {
    status = value;
    core.emit("*", { kind: "tick", tick: { seq: seq++, at: "2026-10-19T12:00:00Z", views: [], countsVersion: 0, issuesVersion: 0, sidebarVersion: 0, ops: [], notices: [], mcp: value } });
  };
  const tools = () => fixtureTools.map(tool => ({ ...tool, enabled: (core.settings.mcp.classes ?? ["read"]).includes(tool.class) && !disabled.has(tool.name) }));
  core.handlers.set("mcp.status", () => status);
  core.handlers.set("mcp.start", args => {
    const port = (args as { port?: number }).port ?? core.settings.mcp.port ?? 51234;
    core.settings.mcp.port = port; setStatus({ kind: "ready", port }); return status;
  });
  core.handlers.set("mcp.stop", () => { setStatus({ kind: "off" }); return status; });
  core.handlers.set("mcp.setAutostart", args => { core.settings.mcp.autostart = (args as { value: boolean }).value; });
  core.handlers.set("mcp.setClasses", args => { core.settings.mcp.classes = (args as { classes: string[] }).classes; });
  core.handlers.set("mcp.tools", tools);
  core.handlers.set("mcp.setToolEnabled", args => {
    const { name, enabled } = args as { name: string; enabled: boolean };
    if (enabled) disabled.delete(name); else disabled.add(name);
  });
  core.handlers.set("mcp.saveToolsMarkdown", args => {
    markdown.push({ path: (args as { path: string }).path, text: tools().filter(tool => tool.enabled).map(tool => `## ${tool.name}\n${tool.description}\n\n| Parameter | Schema |\n| --- | --- |\n${JSON.stringify(tool.schema)}`).join("\n") });
  });
  core.handlers.set("mcp.token", args => ({ masked: "••••••••", value: (args as { reveal: boolean }).reveal ? token : null }));
  core.handlers.set("mcp.regenerateToken", args => { token = `${TEST_TOKEN}-rotated`; return { masked: "••••••••", value: (args as { reveal: boolean }).reveal ? token : null }; });
  core.handlers.set("mcp.clientConfigPreview", args => clientPreview((args as { client: McpClientKind }).client));
  core.handlers.set("mcp.clientConfigWrite", args => { const { client } = args as { client: McpClientKind }; written.push(client); return { backupPath: `${client}.backup` }; });
  core.handlers.set("mcp.callLog", args => { const { offset, limit } = args as { offset: number; limit: number }; return logs.slice(offset, offset + limit); });
  function record(tool: string, params: string, client = "fixture-client") {
    logs.push({ at: "2026-10-19T12:00:00Z", tool, class: "read", client, durationMs: 12, ok: true, argsSummary: params, resultBytes: 2048 });
  }
  return { core, setStatus, tools, logs, written, markdown, record, token: () => token };
}
