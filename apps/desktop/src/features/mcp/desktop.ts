import { client } from "../../ipc/client";
import { IpcError, localError } from "../../ipc/errors";

export interface McpDesktop {
  copy(text: string): Promise<void>;
  chooseMarkdown(): Promise<string | null>;
}

export const nativeMcpDesktop: McpDesktop = {
  copy: text => navigator.clipboard.writeText(text),
  chooseMarkdown: () => {
    if (client.transportName !== "tauri") return Promise.reject(new IpcError(localError("Unsupported", "mcp-native-required")));
    return client.invoke<string | null>("dialog.save", { options: {
      defaultPath: "openspider-mcp-tools.md",
      filters: [{ name: "Markdown", extensions: ["md"] }],
    } });
  },
};
