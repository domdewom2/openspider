import { describe, expect, it } from "vitest";
import { fxUiBasic, genUiLarge, selectGraph } from "../fixtures/synthetic";
import { ForceDriver, ForceSim, mulberry32, MOVE_EPSILON, type ForceInput, type ForceReply } from "./force";
import { inlineRunner } from "./forceRunner";
import { VizTree } from "./tree";
import { depthRadius } from "./encoding";

function input(tree: VizTree, seed = 1): ForceInput {
  const ids = tree.visible();
  const at = new Map(ids.map((id, i) => [id, i]));
  const parent = new Int32Array(ids.map((id) => at.get(tree.get(id)!.parent ?? Number.NaN) ?? -1));
  const radius = new Float64Array(ids.map((id) => depthRadius(tree.get(id)!.level)));
  return { parent, radius, params: { repulsion: 60, springLength: 30, seed, allowOverlap: false } };
}

const basic = () => VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 1_000_000, metric: "crawl_depth" }));

describe("kraftgerichtetes Layout", () => {
  // spec: UI-092
  it("gleiche Eingabe und gleicher Startwert → bitgleiche Endkoordinaten; anderer Startwert → andere (fx-ui-basic)", () => {
    const a = new ForceSim(input(basic(), 7));
    const b = new ForceSim(input(basic(), 7));
    const c = new ForceSim(input(basic(), 8));
    a.run();
    b.run();
    c.run();
    expect(Array.from(a.pos)).toEqual(Array.from(b.pos));
    expect(Array.from(a.pos)).not.toEqual(Array.from(c.pos));
  });

  // spec: UI-092
  it("die Wurzel ist fixiert und liegt nach Konvergenz im Ursprung (= Mitte der Zeichenfläche)", () => {
    const sim = new ForceSim(input(basic()));
    sim.run();
    expect(sim.converged()).toBe(true);
    expect(Math.abs(sim.pos[0]!)).toBeLessThanOrEqual(1);
    expect(Math.abs(sim.pos[1]!)).toBeLessThanOrEqual(1);
  });

  // spec: UI-092
  it("Abbruch bei mittlerer Bewegung < 0,1 px je Schritt; ohne Überlappung berühren sich Knoten nach Konvergenz kaum", () => {
    const sim = new ForceSim(input(basic()));
    sim.run();
    expect(sim.lastMove < MOVE_EPSILON || sim.alpha < 0.001).toBe(true);
    let overlaps = 0;
    for (let i = 0; i < sim.n; i++)
      for (let j = i + 1; j < sim.n; j++) {
        const d = Math.hypot(sim.pos[i * 2]! - sim.pos[j * 2]!, sim.pos[i * 2 + 1]! - sim.pos[j * 2 + 1]!);
        if (d < (sim.radius[i]! + sim.radius[j]!) * 0.9) overlaps++;
      }
    expect(overlaps).toBe(0);
  });

  // spec: UI-090
  it("gezogene Knoten bleiben fixiert, Lösen gibt sie frei; die Wurzel bleibt immer fixiert", () => {
    const sim = new ForceSim(input(basic()));
    sim.fix(3, 500, -200);
    sim.run();
    expect([sim.pos[6], sim.pos[7]]).toEqual([500, -200]);
    sim.release(3);
    sim.reheat();
    for (let i = 0; i < 50; i++) sim.step();
    expect(sim.pos[6]).not.toBe(500);
    sim.release(0);
    sim.step();
    expect([sim.pos[0], sim.pos[1]]).toEqual([0, 0]);
  });

  // spec: UI-092
  it("der Treiber stoppt spätestens nach der Zeitgrenze und meldet danach nichts mehr (Leerlauf)", () => {
    const replies: ForceReply[] = [];
    const queue: (() => void)[] = [];
    let now = 0;
    const driver = new ForceDriver((r) => replies.push(r), (fn) => queue.push(fn), () => (now += 5), 10);
    driver.handle({ type: "start", input: input(VizTree.from(selectGraph(genUiLarge([1, 30, 300]), { budget: 10_000, expandDepth: 9, metric: "crawl_depth" }))), maxMs: 40 });
    while (queue.length) queue.shift()!();
    const last = replies.at(-1)!;
    expect(last.type).toBe("done");
    expect(replies.filter((r) => r.type === "done")).toHaveLength(1);
    expect(queue).toHaveLength(0);
  });

  // spec: UI-146
  it("Layout läuft in Zeitscheiben (blockiert nicht) und konvergiert für 2 000 Knoten deutlich unter 10 s", async () => {
    const tree = VizTree.from(selectGraph(genUiLarge([1, 40, 1959]), { budget: 10_000, expandDepth: 9, metric: "crawl_depth" }));
    const started = performance.now();
    const done = await new Promise<ForceReply>((resolve) => {
      const runner = inlineRunner((r) => r.type === "done" && resolve(r));
      runner.start(input(tree));
    });
    expect(done.type).toBe("done");
    expect(performance.now() - started).toBeLessThan(10_000);
  }, 20_000);

  // spec: UI-092
  it("Zufallsgenerator ist deterministisch", () => {
    const a = mulberry32(3);
    const b = mulberry32(3);
    expect([a(), a(), a()]).toEqual([b(), b(), b()]);
  });
});
