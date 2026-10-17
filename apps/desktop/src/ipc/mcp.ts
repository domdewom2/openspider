// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { BoolArgs } from "./gen/BoolArgs";
import type { CallLogRow } from "./gen/CallLogRow";
import type { McpClientArgs } from "./gen/McpClientArgs";
import type { McpClientPreview } from "./gen/McpClientPreview";
import type { McpClientWritten } from "./gen/McpClientWritten";
import type { McpStartArgs } from "./gen/McpStartArgs";
import type { McpStatus } from "./gen/McpStatus";
import type { McpToken } from "./gen/McpToken";
import type { McpTokenArgs } from "./gen/McpTokenArgs";
import type { PageArgs } from "./gen/PageArgs";
import type { PathArgs } from "./gen/PathArgs";
import type { SetClassesArgs } from "./gen/SetClassesArgs";
import type { SetToolEnabledArgs } from "./gen/SetToolEnabledArgs";
import type { ToolInfo } from "./gen/ToolInfo";
import { client } from "./client";

export const mcp = {
  /** `mcp.status` */
  status: (signal?: AbortSignal) => client.invoke<McpStatus>("mcp.status", undefined, signal),
  /** `mcp.start` */
  start: (args: McpStartArgs, signal?: AbortSignal) => client.invoke<McpStatus>("mcp.start", args, signal),
  /** `mcp.stop` */
  stop: (signal?: AbortSignal) => client.invoke<McpStatus>("mcp.stop", undefined, signal),
  /** `mcp.tools` */
  tools: (signal?: AbortSignal) => client.invoke<ToolInfo[]>("mcp.tools", undefined, signal),
  /** `mcp.setToolEnabled` */
  setToolEnabled: (args: SetToolEnabledArgs, signal?: AbortSignal) => client.invoke<void>("mcp.setToolEnabled", args, signal),
  /** `mcp.setClasses` */
  setClasses: (args: SetClassesArgs, signal?: AbortSignal) => client.invoke<void>("mcp.setClasses", args, signal),
  /** `mcp.token` */
  token: (args: McpTokenArgs, signal?: AbortSignal) => client.invoke<McpToken>("mcp.token", args, signal),
  /** `mcp.regenerateToken` */
  regenerateToken: (args: McpTokenArgs, signal?: AbortSignal) => client.invoke<McpToken>("mcp.regenerateToken", args, signal),
  /** `mcp.setAutostart` */
  setAutostart: (args: BoolArgs, signal?: AbortSignal) => client.invoke<void>("mcp.setAutostart", args, signal),
  /** `mcp.saveToolsMarkdown` */
  saveToolsMarkdown: (args: PathArgs, signal?: AbortSignal) => client.invoke<void>("mcp.saveToolsMarkdown", args, signal),
  /** `mcp.clientConfigPreview` */
  clientConfigPreview: (args: McpClientArgs, signal?: AbortSignal) => client.invoke<McpClientPreview>("mcp.clientConfigPreview", args, signal),
  /** `mcp.clientConfigWrite` */
  clientConfigWrite: (args: McpClientArgs, signal?: AbortSignal) => client.invoke<McpClientWritten>("mcp.clientConfigWrite", args, signal),
  /** `mcp.callLog` */
  callLog: (args: PageArgs, signal?: AbortSignal) => client.invoke<CallLogRow[]>("mcp.callLog", args, signal),
} as const;
