/** Real Streamable HTTP test client; never manufactures sessions, tools or call logs. */
import assert from "node:assert/strict";
export type RpcTool = { name: string; description?: string; inputSchema: { properties?: Record<string, unknown>; [key: string]: unknown } };
type RpcResponse = { result?: { tools?: RpcTool[]; isError?: boolean; content?: { type: string; text?: string }[] }; error?: unknown };
export class HttpMcpClient {
  session: string | null = null;
  private id = 0;
  constructor(readonly port: number, readonly token: string, readonly name = "u25-http-client") {}
  async request(method: string, message?: unknown, headers: Record<string, string> = {}): Promise<Response> {
    return fetch(`http://127.0.0.1:${this.port}/mcp`, {
      method, headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", Authorization: `Bearer ${this.token}`, "MCP-Protocol-Version": "2025-06-18", ...(this.session ? { "Mcp-Session-Id": this.session } : {}), ...headers },
      ...(message === undefined ? {} : { body: JSON.stringify(message) }), signal: AbortSignal.timeout(10_000),
    });
  }
  async rpc(method: string, params: unknown = {}): Promise<RpcResponse["result"]> {
    const response = await this.request("POST", { jsonrpc: "2.0", id: ++this.id, method, params });
    assert.equal(response.status, 200);
    const text = await response.text();
    const body = response.headers.get("content-type")?.includes("text/event-stream")
      ? text.split("\n").filter(line => line.startsWith("data: ")).map(line => JSON.parse(line.slice(6)) as RpcResponse).find(item => item.result || item.error)
      : JSON.parse(text) as RpcResponse;
    assert.ok(body); assert.equal(body.error, undefined);
    return body.result;
  }
  async initialize(): Promise<void> {
    const response = await this.request("POST", { jsonrpc: "2.0", id: ++this.id, method: "initialize", params: { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: this.name, version: "1" } } });
    assert.equal(response.status, 200);
    this.session = response.headers.get("mcp-session-id"); assert.ok(this.session);
    assert.match(await response.text(), /2025-06-18/);
    const ready = await this.request("POST", { jsonrpc: "2.0", method: "notifications/initialized" });
    assert.equal(ready.status, 202); await ready.body?.cancel();
  }
  async close(): Promise<void> {
    if (!this.session) return;
    const response = await this.request("DELETE"); assert.ok(response.ok); await response.body?.cancel(); this.session = null;
  }
}
