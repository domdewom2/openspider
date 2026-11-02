import { afterEach, expect, it, vi } from "vitest";
import { createForceRunner } from "./forceRunner";
import { FORCE_DEFAULTS, type ForceInput, type ForceReply, type ForceRequest } from "./force";
import { fxUiBasic, selectGraph } from "../fixtures/synthetic";

class TestWorker {
  static all: TestWorker[] = [];
  onmessage: ((event: MessageEvent<ForceReply>) => void) | null = null;
  messages: ForceRequest[] = [];
  terminated = false;
  constructor() { TestWorker.all.push(this); }
  postMessage(message: ForceRequest) { this.messages.push(structuredClone(message)); }
  terminate() { this.terminated = true; }
}
afterEach(() => { vi.unstubAllGlobals(); TestWorker.all = []; });

// spec: UI-144
// spec: UI-146
it("releases the settled worker VM and restarts dragging from final fixture positions", () => {
  vi.stubGlobal("Worker", TestWorker);
  const dto = selectGraph(fxUiBasic(), { budget: 10000, expandDepth: 3, metric: "crawl_depth" });
  const input: ForceInput = { parent: new Int32Array(dto.nodes.length).fill(-1), radius: new Float64Array(dto.nodes.length).fill(3), params: FORCE_DEFAULTS };
  const listener = vi.fn();
  const runner = createForceRunner(listener);
  runner.start(input);
  const worker = TestWorker.all[0]!;
  runner.fix(2, 80, 90);
  const pos = new Float64Array(dto.nodes.length * 2).fill(20);
  const velocities = new Float64Array(dto.nodes.length * 2).fill(0.01);
  worker.onmessage!({ data: { type: "done", positions: pos, steps: 200, move: 0, reason: "converged", alpha: 0.002, velocities } } as MessageEvent<ForceReply>);
  expect(worker.terminated).toBe(true);
  expect(worker.onmessage).toBeNull();
  expect(listener).toHaveBeenCalledTimes(1);
  runner.fix(1, 100, 200);
  const next = TestWorker.all[1]!;
  const start = next.messages[0]!;
  expect(start.type).toBe("start");
  if (start.type !== "start") throw new Error("missing restart");
  expect(Array.from(start.input.init!)).toEqual(Array.from(pos));
  expect(Array.from(start.input.fixed!.slice(2, 6))).toEqual([100, 200, 80, 90]);
  // The finished simulation resumes (alpha, steps, velocities); the drag itself follows as fix → reheat(0.3).
  expect(start.input.resume).toMatchObject({ alpha: 0.002, steps: 200 });
  expect(Array.from(start.input.resume!.velocities)).toEqual(Array.from(velocities));
  expect(next.messages[1]).toEqual({ type: "fix", index: 1, x: 100, y: 200 });
  expect(input.fixed).toBeUndefined();
  runner.dispose(); runner.dispose();
  expect(next.terminated).toBe(true);
  runner.fix(0, 1, 2); runner.start(input); runner.resume();
  expect(TestWorker.all).toHaveLength(2);
});
