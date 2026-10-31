/** Real Streamable HTTP test client; never manufactures sessions, tools or call logs. */
import assert from "node:assert/strict";
import { request as httpRequest } from "node:http";
export type RpcTool = { name: string; description?: string; inputSchema: { properties?: Record<string, unknown>; [key: string]: unknown } };
type RpcResponse = { result?: { tools?: RpcTool[]; isError?: boolean; content?: { type: string; text?: string }[] }; error?: unknown };
export class HttpMcpClient {
  session: string | null = null;
  private id = 0;
  constructor(readonly port: number, readonly token: string, readonly name = "u25-http-client") {}
  async request(method: string, message?: unknown, headers: Record<string, string> = {}): Promise<Response> {
    // Fetch normalizes the Host header. Use HTTP directly for the DNS-rebinding probe.
    if (headers.Host) {
      return new Promise<Response>((resolve, reject) => {
        const request = httpRequest(`http://127.0.0.1:${this.port}/mcp`, {
          method, headers: { Accept: "application/json, text/event-stream", "Content-Type": "application/json", Authorization: `Bearer ${this.token}`, ...headers },
        }, response => {
          const chunks: Buffer[] = [];
          response.on("data", chunk => chunks.push(Buffer.from(chunk)));
          response.on("error", reject);
          response.on("end", () => resolve(new Response(Buffer.concat(chunks), { status: response.statusCode!, headers: Object.fromEntries(Object.entries(response.headers).filter((entry): entry is [string, string | string[]] => entry[1] !== undefined).map(([key, value]) => [key, Array.isArray(value) ? value.join(", ") : value])) })));
        });
        request.on("error", reject);
        request.setTimeout(10_000, () => request.destroy(new Error("HTTP probe timeout")));
        request.end(message === undefined ? undefined : JSON.stringify(message));
      });
    }
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
      ? text.split(/\r?\n\r?\n/).map(event => event.split(/\r?\n/).filter(line => line.startsWith("data:")).map(line => line.slice(5).replace(/^ /, "")).join("\n")).filter(data => data.trim()).map(data => JSON.parse(data) as RpcResponse).find(item => item.result || item.error)
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
