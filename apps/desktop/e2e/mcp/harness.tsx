import { createRoot } from "react-dom/client";
import { bootWindow } from "../../src/app/runtime";
import { setLanguage } from "../../src/i18n";
import { client } from "../../src/ipc/client";
import type { McpClientKind } from "../../src/ipc/gen/McpClientKind";
import type { McpClientPreview } from "../../src/ipc/gen/McpClientPreview";
import type { McpClientWritten } from "../../src/ipc/gen/McpClientWritten";
import { McpHost, McpStatusField, openMcp, type McpDesktop } from "../../src/features/mcp";
import { createMcpFixture } from "../../src/features/mcp/fixtures/agent-m2-ui";
import { runAction } from "../../src/ui/actions";
import { Button } from "../../src/ui/primitives";
import "../../src/ui/ui.css";

// Test-only service adapter. No production build entry imports this file.
const fixture = createMcpFixture();
const runtime = bootWindow({ kind: "main", transport: fixture.core.transport("main") });
await runtime.ready;
await setLanguage("en");
const previewFile = window.mcpFilePreview;
const writeFile = window.mcpFileWrite;
if (previewFile) fixture.core.handlers.set("mcp.clientConfigPreview", args => previewFile((args as { client: McpClientKind }).client));
if (writeFile) fixture.core.handlers.set("mcp.clientConfigWrite", args => writeFile((args as { client: McpClientKind }).client));
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
  invoke: (command: string, args?: unknown) => client.invoke(command, args),
};
declare global {
  interface Window {
    mcpFixture: typeof api;
    mcpFilePreview?: (kind: McpClientKind) => Promise<McpClientPreview>;
    mcpFileWrite?: (kind: McpClientKind) => Promise<McpClientWritten>;
  }
}
window.mcpFixture = api;
createRoot(document.getElementById("root")!).render(<main>
  <Button onPress={() => openMcp()}>Open MCP</Button>
  <McpHost desktop={desktop} />
  <footer aria-label="Status"><McpStatusField /></footer>
</main>);
