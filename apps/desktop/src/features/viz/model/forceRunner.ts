// Fensterseite des Layouts: startet den Worker (oder ohne Worker-Unterstützung denselben Treiber im Hauptthread).
import { ForceDriver, type ForceInput, type ForceReply, type ForceRequest, type ForceResume } from "./force";

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
  let current: Worker | null = worker;
  let input: ForceInput | null = null;
  let resume: ForceResume | undefined;
  let maxMs: number | undefined;
  let disposed = false;
  const terminate = () => {
    if (!current) return;
    current.onmessage = null;
    current.terminate();
    current = null;
  };
  const listen = (target: Worker) => {
    target.onmessage = (e: MessageEvent<ForceReply>) => {
      if (target !== current || disposed) return;
      // A settled layout needs only its positions for the next interaction, not a live VM.
      if (input) input.init = e.data.positions;
      if (e.data.type === "done") {
        resume = { alpha: e.data.alpha, steps: e.data.steps, velocities: e.data.velocities };
        terminate();
      }
      listener(e.data);
    };
  };
  listen(worker);
  const send = (msg: ForceRequest) => {
    if (disposed) return;
    if (!current && input && (msg.type === "fix" || msg.type === "release")) {
      current = new Worker(new URL("./force.worker.ts", import.meta.url), { type: "module" });
      listen(current);
      // Resume the finished simulation, then apply the interaction exactly as before (fix/release + reheat 0.3).
      current.postMessage({ type: "start", input: { ...input, resume }, maxMs } satisfies ForceRequest);
    }
    current?.postMessage(msg);
  };
  return {
    start: (next, limit) => {
      if (disposed) return;
      // Keep compact restart data; never mutate caller-owned arrays.
      input = { ...next, fixed: next.fixed?.slice() ?? new Float64Array(next.parent.length * 2).fill(Number.NaN) };
      resume = undefined;
      maxMs = limit;
      if (!current) {
        current = new Worker(new URL("./force.worker.ts", import.meta.url), { type: "module" });
        listen(current);
      }
      send({ type: "start", input, maxMs });
    },
    fix: (index, x, y) => {
      if (input?.fixed) { input.fixed[index * 2] = x; input.fixed[index * 2 + 1] = y; }
      send({ type: "fix", index, x, y });
    },
    release: (index) => {
      if (input?.fixed) { input.fixed[index * 2] = Number.NaN; input.fixed[index * 2 + 1] = Number.NaN; }
      send({ type: "release", index });
    },
    pause: () => send({ type: "pause" }),
    resume: () => send({ type: "resume" }),
    dispose: () => {
      disposed = true;
      terminate();
      input = null;
      resume = undefined;
    },
  };
}

/** Rückfall ohne Worker (vitest/jsdom): gleicher Treiber in Zeitscheiben im Hauptthread. */
export function inlineRunner(listener: ForceListener, schedule?: (fn: () => void) => void): ForceRunner {
  let alive = true;
  const driver = new ForceDriver((r) => alive && listener(r), schedule);
  const send = (msg: ForceRequest) => { if (alive) driver.handle(msg); };
  return {
    start: (input, maxMs) => send({ type: "start", input, maxMs }),
    fix: (index, x, y) => send({ type: "fix", index, x, y }),
    release: (index) => send({ type: "release", index }),
    pause: () => send({ type: "pause" }),
    resume: () => send({ type: "resume" }),
    dispose: () => {
      alive = false;
      driver.handle({ type: "stop" });
    },
  };
}

export function createForceRunner(listener: ForceListener): ForceRunner {
  return workerRunner(listener) ?? inlineRunner(listener);
}
