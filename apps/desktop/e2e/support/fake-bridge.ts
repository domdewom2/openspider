// Protokollgleiche Bridge in Node (docs/plan/M2.md 3.6) mit dem Mock-Kern: prüft den WebSocket-Transport des Frontends im
// Browser, unabhängig von der Rust-Bridge `os-app-bridge`. Handgeschriebenes WebSocket (RFC 6455, nur Textrahmen).
import { createHash } from "node:crypto";
import { createServer, type IncomingMessage, type Server } from "node:http";
import type { Socket } from "node:net";
import { MockCore } from "../../src/ipc/transport/mock";
import type { UiEvent } from "../../src/ipc/gen/UiEvent";

export interface FakeBridge {
  url: string;
  core: MockCore;
  stop(): Promise<void>;
}

const GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

function frame(text: string): Buffer {
  const payload = Buffer.from(text);
  const len = payload.length;
  const head = len < 126 ? Buffer.from([0x81, len]) : len < 65536 ? Buffer.from([0x81, 126, len >> 8, len & 255]) : null;
  if (!head) throw new Error("Nachricht zu groß für den Test-Server");
  return Buffer.concat([head, payload]);
}

/** Liest vollständige, maskierte Textrahmen aus dem Puffer; liefert Nachrichten und den Rest. */
function parse(buf: Buffer): { messages: string[]; rest: Buffer; close: boolean } {
  const messages: string[] = [];
  let off = 0;
  let close = false;
  for (;;) {
    if (buf.length - off < 2) break;
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
    else if (op === 0x1) messages.push(data.toString("utf8"));
  }
  return { messages, rest: buf.subarray(off), close };
}

export async function startFakeBridge(): Promise<FakeBridge> {
  const core = new MockCore();
  const token = Math.random().toString(16).slice(2);
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
    const forward = (window: string) => (e: UiEvent) => {
      const data = e.kind === "tick" ? e.tick : e.kind === "settingsChanged" ? { keys: e.keys } : e.kind === "notice" ? e.notice : { crawlId: e.crawlId, urlId: e.urlId, isInternal: e.isInternal };
      send({ event: e.kind, window, data });
    };
    socket.on("data", (chunk: Buffer) => {
      buf = Buffer.concat([buf, chunk]);
      const { messages, rest, close } = parse(buf);
      buf = rest;
      for (const raw of messages) {
        const msg = JSON.parse(raw) as { id?: number; cmd: string; args?: unknown; window?: string };
        if (msg.cmd === "__subscribe" && msg.window) {
          offs.push(core.addSub(msg.window, forward(msg.window)));
          continue;
        }
        core
          .transport(msg.window ?? "main")
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
    core,
    async stop() {
      for (const s of sockets) s.destroy();
      await new Promise<void>((resolve) => server.close(() => resolve()));
    },
  };
}
