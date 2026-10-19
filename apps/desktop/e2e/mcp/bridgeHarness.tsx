import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { createBridgeTransport } from "../../src/ipc/transport/bridge";
import { client } from "../../src/ipc/client";
import { setLanguage } from "../../src/i18n";
import { useSession } from "../../src/state/session";
import { McpHost, McpStatusField, openMcp, type McpDesktop } from "../../src/features/mcp";
import "../../src/ui/ui.css";

const url = new URLSearchParams(location.search).get("bridge");
if (!url) throw new Error("Real bridge URL required");
await bootWindow({ kind: "main", transport: createBridgeTransport({ url }) }).ready;
await setLanguage("en");
let destination: string | null = null;
const desktop: McpDesktop = { chooseMarkdown: async () => destination, copy: async () => { throw new Error("Use native clipboard test"); } };
window.mcpBridge = {
  invoke: <T,>(command: string, args?: unknown) => client.invoke<T>(command, args),
  transport: client.transportName,
  appearedAt: performance.now(),
  crawl: () => useSession.getState().crawl,
  destination: (path: string) => { destination = path; },
};
declare global { interface Window { mcpBridge: {
  invoke<T>(command: string, args?: unknown): Promise<T>;
  transport: string;
  appearedAt: number;
  crawl: () => ReturnType<typeof useSession.getState>["crawl"];
  destination(path: string): void;
} } }
createRoot(document.getElementById("root")!).render(<main>
  <button onClick={() => openMcp()}>Open MCP</button><McpHost desktop={desktop} /><McpStatusField />
</main>);
