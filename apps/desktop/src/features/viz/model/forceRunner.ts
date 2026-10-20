// Fensterseite des Layouts: startet den Worker (oder ohne Worker-Unterstützung denselben Treiber im Hauptthread).
import { ForceDriver, type ForceInput, type ForceReply, type ForceRequest } from "./force";

export interface ForceRunner {
  start(input: ForceInput, maxMs?: number): void;
  fix(index: number, x: number, y: number): void;
  release(index: number): void;
  pause(): void;
  resume(): void;
  dispose(): void;
}

export type ForceListener = (reply: ForceReply) => void;

function workerRunner(listener: ForceListener): ForceRunner | null {
  if (typeof Worker === "undefined") return null;
  let worker: Worker;
  try {
    worker = new Worker(new URL("./force.worker.ts", import.meta.url), { type: "module" });
  } catch {
    return null;
  }
  worker.onmessage = (e: MessageEvent<ForceReply>) => listener(e.data);
  const send = (msg: ForceRequest) => worker.postMessage(msg);
  return {
    start: (input, maxMs) => send({ type: "start", input, maxMs }),
    fix: (index, x, y) => send({ type: "fix", index, x, y }),
    release: (index) => send({ type: "release", index }),
    pause: () => send({ type: "pause" }),
    resume: () => send({ type: "resume" }),
    dispose: () => {
      send({ type: "stop" });
      worker.terminate();
    },
  };
}

/** Rückfall ohne Worker (vitest/jsdom): gleicher Treiber in Zeitscheiben im Hauptthread. */
export function inlineRunner(listener: ForceListener, schedule?: (fn: () => void) => void): ForceRunner {
  let alive = true;
  const driver = new ForceDriver((r) => alive && listener(r), schedule);
  const send = (msg: ForceRequest) => driver.handle(msg);
  return {
    start: (input, maxMs) => send({ type: "start", input, maxMs }),
    fix: (index, x, y) => send({ type: "fix", index, x, y }),
    release: (index) => send({ type: "release", index }),
    pause: () => send({ type: "pause" }),
    resume: () => send({ type: "resume" }),
    dispose: () => {
      alive = false;
      send({ type: "stop" });
    },
  };
}

export function createForceRunner(listener: ForceListener): ForceRunner {
  return workerRunner(listener) ?? inlineRunner(listener);
}
