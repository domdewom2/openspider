// Minimaler MCP-Client über STDIO (`openspider mcp`): Zeilen-JSON-RPC. Liefert die Katalogtexte der Issues für den Vergleich
// „Erklärung in der Seitenleiste = MCP-Text“ (Alpha-Kriterium 3).
import { spawn } from "node:child_process";
import { toolBinary } from "./support";

export interface McpIssue {
  id: string;
  type: string;
  priority: string;
  tab: string;
  filter: string;
  count: number;
  what: string;
  why: string;
  fix: string;
}

export async function mcpCall(home: string, name: string, args: Record<string, unknown>): Promise<any> {
  const child = spawn(toolBinary("openspider"), ["mcp"], { env: { ...process.env, OPENSPIDER_HOME: home, OPENSPIDER_SECRET_STORE: "memory" }, stdio: ["pipe", "pipe", "ignore"] });
  let buf = "";
  const pending = new Map<number, (m: any) => void>();
  child.stdout.on("data", (d: Buffer) => {
    buf += d.toString();
    for (let i = buf.indexOf("\n"); i >= 0; i = buf.indexOf("\n")) {
      const line = buf.slice(0, i).trim();
      buf = buf.slice(i + 1);
      if (!line) continue;
      const m = JSON.parse(line);
      if (m.id !== undefined) pending.get(m.id)?.(m);
    }
  });
  let id = 0;
  const rpc = (method: string, params: unknown) =>
    new Promise<any>((ok, fail) => {
      const i = ++id;
      const timer = setTimeout(() => fail(new Error(`MCP ${method} ohne Antwort`)), 30_000);
      pending.set(i, (m) => {
        clearTimeout(timer);
        ok(m);
      });
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: i, method, params })}\n`);
    });
  try {
    await rpc("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "alpha-e2e", version: "0" } });
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" })}\n`);
    const reply = await rpc("tools/call", { name, arguments: args });
    if (reply.error || reply.result?.isError) throw new Error(`MCP ${name}: ${JSON.stringify(reply.error ?? reply.result).slice(0, 400)}`);
    return JSON.parse(reply.result.content[0].text);
  } finally {
    child.kill();
  }
}
