// Protokollgleiche Test-Bridge für die Visualisierungs-E2E (docs/plan/M2.md 3.6): `viz.*` beantwortet der Test-Kern
// aus src/features/viz/fixtures (synthetische Schnappschüsse bis 50 000 Knoten), alles Übrige der Mock-Kern. Eigenes
// WebSocket (RFC 6455, Textrahmen, auch 64-Bit-Längen für große Graphen).
import { createHash } from "node:crypto";
import { createServer, type IncomingMessage, type Server } from "node:http";
import type { Socket } from "node:net";
import { FakeVizCore } from "../../src/features/viz/fixtures/fakeCore";
import type { UiEvent } from "../../src/ipc/gen/UiEvent";
import { MockCore } from "../../src/ipc/transport/mock";

export interface VizBridge {
  url: string;
  viz: FakeVizCore;
  stop(): Promise<void>;
}

const GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

function frame(text: string): Buffer {
  const payload = Buffer.from(text);
  const len = payload.length;
  let head: Buffer;
  if (len < 126) head = Buffer.from([0x81, len]);
  else if (len < 65536) head = Buffer.from([0x81, 126, len >> 8, len & 255]);
  else {
    head = Buffer.alloc(10);
    head[0] = 0x81;
    head[1] = 127;
    head.writeBigUInt64BE(BigInt(len), 2);
  }
  return Buffer.concat([head, payload]);
}

function parse(buf: Buffer): { messages: string[]; rest: Buffer; close: boolean } {
  const messages: string[] = [];
  let off = 0;
  let close = false;
  const parts: Buffer[] = [];
  for (;;) {
    if (buf.length - off < 2) break;
    const fin = ((buf[off] as number) & 0x80) !== 0;
    const op = (buf[off] as number) & 0x0f;
    let len = (buf[off + 1] as number) & 0x7f;
    let pos = off + 2;
    if (len === 126) {
      if (buf.length - pos < 2) break;
      len = buf.readUInt16BE(pos);
      pos += 2;
    } else if (len === 127) {
      if (buf.length - pos < 8) break;
      len = Number(buf.readBigUInt64BE(pos));
      pos += 8;
    }
    if (buf.length - pos < 4 + len) break;
    const mask = buf.subarray(pos, pos + 4);
    pos += 4;
    const data = Buffer.alloc(len);
    for (let i = 0; i < len; i++) data[i] = (buf[pos + i] as number) ^ (mask[i % 4] as number);
    off = pos + len;
    if (op === 0x8) close = true;
    else if (op === 0x1 || op === 0x0) {
      parts.push(data);
      if (fin) {
        messages.push(Buffer.concat(parts.splice(0)).toString("utf8"));
      }
    }
  }
  return { messages, rest: buf.subarray(off), close };
}

function wire(e: UiEvent): { event: string; data: unknown } {
  switch (e.kind) {
    case "tick":
      return { event: "tick", data: e.tick };
    case "settingsChanged":
      return { event: "settingsChanged", data: { keys: e.keys } };
    case "notice":
      return { event: "notice", data: e.notice };
    case "focusUrl":
      return { event: "focusUrl", data: { crawlId: e.crawlId, urlId: e.urlId, isInternal: e.isInternal } };
  }
}

export async function startVizBridge(viz: FakeVizCore = new FakeVizCore()): Promise<VizBridge> {
  const core = new MockCore();
  const token = createHash("sha256").update(String(process.hrtime.bigint())).digest("hex").slice(0, 32);
  const server: Server = createServer((_, res) => res.writeHead(426).end());
  const sockets = new Set<Socket>();

  server.on("upgrade", (req: IncomingMessage, socket: Socket) => {
    const url = new URL(req.url ?? "/", "http://127.0.0.1");
    const key = req.headers["sec-websocket-key"];
    if (url.searchParams.get("token") !== token || typeof key !== "string") {
      socket.end("HTTP/1.1 403 Forbidden\r\n\r\n");
      return;
    }
    const accept = createHash("sha1").update(key + GUID).digest("base64");
    socket.write(`HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`);
    sockets.add(socket);
    let buf: Buffer = Buffer.alloc(0);
    const offs: (() => void)[] = [];
    const send = (obj: unknown) => socket.writable && socket.write(frame(JSON.stringify(obj)));
    const forward = (window: string) => (e: UiEvent) => send({ ...wire(e), window });
    socket.on("data", (chunk: Buffer) => {
      buf = Buffer.concat([buf, chunk]);
      const { messages, rest, close } = parse(buf);
      buf = rest;
      for (const raw of messages) {
        const msg = JSON.parse(raw) as { id?: number; cmd: string; args?: unknown; window?: string };
        if (msg.cmd === "__subscribe" && msg.window) {
          offs.push(core.addSub(msg.window, forward(msg.window)));
          offs.push(viz.transport(msg.window).subscribe(msg.window, forward(msg.window)));
          continue;
        }
        const window = msg.window ?? "main";
        const target = msg.cmd.startsWith("viz.") ? viz.transport(window) : core.transport(window);
        target
          .invoke(msg.cmd, msg.args ?? {})
          .then((ok) => send({ id: msg.id, ok: ok ?? null }))
          .catch((err: { cmdError?: unknown }) => send({ id: msg.id, err: err.cmdError ?? err }));
      }
      if (close) socket.end();
    });
    socket.on("close", () => {
      sockets.delete(socket);
      offs.forEach((o) => o());
    });
    socket.on("error", () => undefined);
  });

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const port = (server.address() as { port: number }).port;
  return {
    url: `ws://127.0.0.1:${port}/?token=${token}`,
    viz,
    async stop() {
      for (const s of sockets) s.destroy();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}
