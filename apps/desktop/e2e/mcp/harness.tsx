import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import type { McpClientPreview } from "../../src/ipc/gen/McpClientPreview";
import { McpHost, McpStatusField, openMcp, type McpDesktop } from "../../src/features/mcp";
import { createMcpFixture } from "../../src/features/mcp/fixtures/agent-m2-ui";
import { runAction } from "../../src/ui/actions";
import { Button } from "../../src/ui/primitives";
import "../../src/ui/ui.css";

// Test-only service adapter. No production build entry imports this file.
const fixture = createMcpFixture();
const runtime = bootWindow({ kind: "main", transport: fixture.core.transport("main") });
await runtime.ready;
// `?lang=de` for the German screenshots of the connection dialog (B3-MCP); default stays English.
await setLanguage(new URLSearchParams(location.search).get("lang") === "de" ? "de" : "en");
let destination: string | null = "fixture-home/openspider-mcp-tools.md";
const clipboard: string[] = [];
const desktop: McpDesktop = {
  copy: async value => { clipboard.push(value); },
  chooseMarkdown: async () => destination,
};
const api = {
  setStatus: fixture.setStatus,
  calls: () => fixture.core.calls,
  token: fixture.token,
  clipboard,
  setDestination: (value: string | null) => { destination = value; },
  recordCalls: () => {
    for (const tool of ["list_crawls", "query_urls", "url_details"]) fixture.record(tool, `{"crawl_id":"out-basic-site","token":"${fixture.token()}"}`);
  },
  action: (name: string) => runAction(name),
  setClientPreview: (value: McpClientPreview) => { fixture.core.handlers.set("mcp.clientConfigPreview", () => value); },
  connected: () => [...fixture.connected],
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
};
declare global {
  interface Window {
    mcpFixture: typeof api;
  }
}
window.mcpFixture = api;
createRoot(document.getElementById("root")!).render(<main>
  <Button onPress={() => openMcp()}>Open MCP</Button>
  <McpHost desktop={desktop} />
  <footer aria-label="Status"><McpStatusField /></footer>
</main>);
