import { expect, it } from "vitest";
import { fxUiBasic, selectGraph } from "../fixtures/synthetic";
import { ForceDriver, type ForceInput, type ForceReply, type ForceRequest } from "./force";
import { VizTree } from "./tree";
import { depthRadius } from "./encoding";

function input(): ForceInput {
  const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 1_000_000, metric: "crawl_depth" }));
  const ids = tree.visible();
  const at = new Map(ids.map((id, i) => [id, i]));
  const parent = new Int32Array(ids.map((id) => at.get(tree.get(id)!.parent ?? Number.NaN) ?? -1));
  const radius = new Float64Array(ids.map((id) => depthRadius(tree.get(id)!.level)));
  return { parent, radius, params: { repulsion: 60, springLength: 30, seed: 3, allowOverlap: false } };
}

/** Synchronous driver: one queue, constant clock, so slices end only at convergence (deterministic). */
function driver() {
  const queue: (() => void)[] = [];
  const replies: ForceReply[] = [];
  const d = new ForceDriver((r) => replies.push(r), (fn) => queue.push(fn), () => 0);
  const pump = () => { while (queue.length) queue.shift()!(); };
  const send = (...messages: ForceRequest[]) => { for (const m of messages) d.handle(m); pump(); };
  const done = () => replies.filter((r): r is Extract<ForceReply, { type: "done" }> => r.type === "done").at(-1)!;
  return { send, done, replies };
}

// spec: UI-146
// spec: UI-092
it("a drag after the settled worker was released reheats like the kept simulation (fixture, bit-identical)", () => {
  const base = input();
  const kept = driver();
  kept.send({ type: "start", input: base });
  const settled = kept.done();
  expect(settled.reason).toBe("converged");
  expect(settled.alpha).toBeLessThan(0.3);

  // Previous behaviour: the same simulation receives fix → reheat(0.3).
  kept.send({ type: "fix", index: 3, x: 120, y: -40 });
  const expected = kept.done();

  // Released worker: a fresh driver resumes the final state, then receives the same fix.
  const fixed = new Float64Array(base.parent.length * 2).fill(Number.NaN);
  fixed[6] = 120; fixed[7] = -40;
  const resumed = driver();
  resumed.send(
    { type: "start", input: { ...base, init: settled.positions, fixed, resume: { alpha: settled.alpha, steps: settled.steps, velocities: settled.velocities } } },
  );
  expect(resumed.replies).toHaveLength(0); // resumed state is finished: no recomputation and no alpha = 1 restart
  resumed.send({ type: "fix", index: 3, x: 120, y: -40 });
  const actual = resumed.done();
  expect(actual.steps).toBe(expected.steps);
  expect(Array.from(actual.positions)).toEqual(Array.from(expected.positions));

  // A fresh start (alpha = 1) would move the graph differently.
  const fresh = driver();
  fresh.send({ type: "start", input: { ...base, init: settled.positions, fixed } });
  expect(Array.from(fresh.done().positions)).not.toEqual(Array.from(expected.positions));
});
