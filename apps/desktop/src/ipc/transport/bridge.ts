// Transport der E2E-Bridge (`os-app-bridge`, docs/plan/M2.md 3.6): WebSocket zum Dispatcher von os-app.
//   Anfrage  {"id":1,"cmd":"view.rows","args":{…}}   →  {"id":1,"ok":<Wert>} | {"id":1,"err":<CmdError>}
//   Ereignis {"event":"tick","window":"main","data":<Tick>}
//   Abo      {"cmd":"__subscribe","window":"main"}
import { IpcError, localError, toCmdError } from "../errors";
import type { Transport, UiEvent } from "./types";

export interface WebSocketLike {
  readyState: number;
  send(data: string): void;
  close(): void;
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
}

export interface BridgeOptions {
  /** `ws://127.0.0.1:<port>/?token=<hex>` */
  url: string;
  createSocket?: (url: string) => WebSocketLike;
}

type Pending = { resolve: (v: unknown) => void; reject: (e: unknown) => void };

export function createBridgeTransport(opts: BridgeOptions): Transport {
  const make = opts.createSocket ?? ((u: string) => new WebSocket(u) as unknown as WebSocketLike);
  const socket = make(opts.url);
  const pending = new Map<number, Pending>();
  const listeners = new Map<string, Set<(e: UiEvent) => void>>();
  const subscribed = new Set<string>();
  const queue: string[] = [];
  let nextId = 1;
  let closed = false;

  const OPEN = 1;
  const flush = () => {
    while (queue.length > 0 && socket.readyState === OPEN) socket.send(queue.shift() as string);
  };
  const post = (message: unknown) => {
    queue.push(JSON.stringify(message));
    flush();
  };
  const failAll = () => {
    closed = true;
    for (const p of pending.values()) p.reject(new IpcError(localError("Io", "err-bridge-closed")));
    pending.clear();
  };

  socket.onopen = () => flush();
  socket.onclose = failAll;
  socket.onerror = () => {
    /* onclose folgt */
  };
  socket.onmessage = (ev) => {
    let msg: Record<string, unknown>;
    try {
      msg = JSON.parse(String(ev.data)) as Record<string, unknown>;
    } catch {
      return;
    }
    if (typeof msg.id === "number") {
      const p = pending.get(msg.id);
      if (!p) return;
      pending.delete(msg.id);
      if ("err" in msg) p.reject(new IpcError(toCmdError(msg.err)));
      else p.resolve(msg.ok);
      return;
    }
    if (typeof msg.event === "string") {
      const ui = toUiEvent(msg.event, msg.data);
      const win = typeof msg.window === "string" ? msg.window : "main";
      if (ui) for (const l of listeners.get(win) ?? []) l(ui);
    }
  };

  return {
    name: "bridge",
    invoke<T>(cmd: string, args: unknown): Promise<T> {
      if (closed) return Promise.reject(new IpcError(localError("Io", "err-bridge-closed")));
      const id = nextId++;
      return new Promise<T>((resolve, reject) => {
        pending.set(id, { resolve: resolve as (v: unknown) => void, reject });
        post({ id, cmd, args: args ?? {} });
      });
    },
    subscribe(window: string, onEvent: (e: UiEvent) => void): () => void {
      let set = listeners.get(window);
      if (!set) listeners.set(window, (set = new Set()));
      set.add(onEvent);
      if (!subscribed.has(window)) {
        subscribed.add(window);
        post({ cmd: "__subscribe", window });
      }
      return () => {
        set?.delete(onEvent);
      };
    },
  };
}

/** Bridge-Ereignisname + Nutzdaten → `UiEvent` (gleiche Form wie der Tauri-Channel). */
export function toUiEvent(event: string, data: unknown): UiEvent | null {
  switch (event) {
    case "focusUrl":
      return { ...(data as Omit<Extract<UiEvent, { kind: "focusUrl" }>, "kind">), kind: "focusUrl" };
    case "tick":
      return { kind: "tick", tick: data as never };
    case "settingsChanged": {
      const keys = Array.isArray(data) ? data : (data as { keys?: string[] } | null)?.keys ?? [];
      return { kind: "settingsChanged", keys: keys as string[] };
    }
    case "notice":
      return { kind: "notice", notice: data as never };
    default:
      return null;
  }
}
