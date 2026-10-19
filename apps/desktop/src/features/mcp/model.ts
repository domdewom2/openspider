import { create } from "zustand";
import type { TFn } from "../../i18n";
import type { McpStatus } from "../../ipc/gen/McpStatus";
import { toCmdError } from "../../ipc/errors";

export const MCP_TABS = ["server", "tools", "access", "clients", "log"] as const;
export type McpTab = typeof MCP_TABS[number];
export const MCP_CLASSES = ["read", "crawl", "write", "cost"] as const;
export const useMcpUi = create<{
  tab: McpTab | null;
  status: McpStatus;
}>(() => ({ tab: null, status: { kind: "off" } }));
export const openMcp = (tab: McpTab = "server") => useMcpUi.setState({ tab });

export function statusText(status: McpStatus, t: TFn): string {
  if (status.kind === "off") return t("mcp-status-off");
  if (status.kind === "ready" || status.clients < 1) return t("mcp-status-ready", { port: String(status.port) });
  return t("mcp-status-connected", { clients: status.clients });
}

export function parsePort(value: string): number | null | undefined {
  if (value.trim() === "") return null;
  if (!/^\d+$/.test(value)) return undefined;
  const port = Number(value);
  return Number.isInteger(port) && port >= 1 && port <= 65535 ? port : undefined;
}

/** Error details/arguments may contain credentials; only fixed, translated UI messages are rendered. */
export function errorKey(error: unknown): string {
  switch (toCmdError(error).code) {
    case "Unsupported": return "mcp-unavailable";
    case "Busy": return "mcp-busy";
    default: return "mcp-command-failed";
  }
}

/** Defense in depth for redacted server summaries and third-party client configurations. */
export function redactText(text: string): string {
  return text
    .replace(/\bBearer\s+[^\s"'\\,}]+/gi, "Bearer [redacted]")
    .replace(/((?:["']?)(?:[\w.-]*(?:token|secret|password|authorization|api[_-]?key|cookie|credential)[\w.-]*)(?:["']?)\s*[:=]\s*)("(?:\\.|[^"\\])*"|'[^']*'|[^\s,;}&]+)/gi, "$1\"[redacted]\"")
    .replace(/(https?:\/\/)[^\s/@]+:[^\s/@]+@/gi, "$1[redacted]@");
}
