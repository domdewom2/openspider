import { describe, expect, it } from "vitest";
import { genUiLarge, fxUiBasic, selectGraph } from "../fixtures/synthetic";
import { depthRadius } from "./encoding";
import { ForceDriver, type ForceInput, type ForceReply } from "./force";
import { ForceSim3D } from "./force3d";
import { inlineRunner } from "./forceRunner";
import { createSimulation } from "./simulation";
import { VizTree } from "./tree";

function input(tree: VizTree, seed = 1): ForceInput {
  const ids = tree.visible();
  const at = new Map(ids.map((id, i) => [id, i]));
  const parent = new Int32Array(ids.map((id) => at.get(tree.get(id)!.parent ?? Number.NaN) ?? -1));
  const radius = new Float64Array(ids.map((id) => depthRadius(tree.get(id)!.level)));
  return { parent, radius, params: { repulsion: 60, springLength: 30, seed, allowOverlap: true }, dims: 3 };
}
const basic = () => VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 1_000_000, metric: "crawl_depth" }));
const large = (n: number) => VizTree.from(selectGraph(genUiLarge([1, 40, n - 41]), { budget: 100_000, expandDepth: 1_000_000, metric: "crawl_depth" }));

describe("3D-Layout (Octree-Barnes-Hut)", () => {
  // spec: UI-098
  it("gleiche Eingabe und gleicher Startwert → bitgleiche Endkoordinaten; anderer Startwert → andere", () => {
    const a = new ForceSim3D(input(basic(), 7));
    const b = new ForceSim3D(input(basic(), 7));
    const c = new ForceSim3D(input(basic(), 8));
    a.run();
    b.run();
    c.run();
    expect(Array.from(a.pos)).toEqual(Array.from(b.pos));
    expect(Array.from(a.pos)).not.toEqual(Array.from(c.pos));
  });

  // spec: UI-098
  it("Wurzel fixiert im Ursprung, Knoten nutzen alle drei Achsen, Konvergenz nach Bewegung < 0,1 bzw. Abkühlung", () => {
    const sim = new ForceSim3D(input(basic()));
    sim.run();
    expect(sim.converged()).toBe(true);
    expect([sim.pos[0], sim.pos[1], sim.pos[2]]).toEqual([0, 0, 0]);
    const spread = [0, 1, 2].map((k) => {
      const v = Array.from({ length: sim.n }, (_, i) => sim.pos[i * 3 + k]!);
      return Math.max(...v) - Math.min(...v);
    });
    for (const s of spread) expect(s).toBeGreaterThan(20);
    // Kinder liegen ungefähr in Federlänge um ihre Eltern (Baumkanten wirken als Federn).
    const d = Math.hypot(sim.pos[3]!, sim.pos[4]!, sim.pos[5]!);
    expect(d).toBeGreaterThan(10);
    expect(d).toBeLessThan(400);
  });

  // spec: UI-098
  it("fixierte Knoten bleiben an ihrer Position, Lösen gibt sie frei; gleiche Positionen im Octree werden aufgelöst", () => {
    const inp = input(basic());
    const sim = new ForceSim3D(inp);
    sim.fix(3, 50, -20, 10);
    sim.run(200);
    expect([sim.pos[9], sim.pos[10], sim.pos[11]]).toEqual([50, -20, 10]);
    sim.release(3);
    sim.reheat();
    for (let i = 0; i < 30; i++) sim.step();
    expect([sim.pos[9], sim.pos[10], sim.pos[11]]).not.toEqual([50, -20, 10]);
    const same = new ForceSim3D({ ...inp, init: new Float64Array(inp.parent.length * 3).fill(5) });
    same.step();
    expect(Array.from(same.pos).every(Number.isFinite)).toBe(true);
  });

  // spec: UI-098
  // spec: UI-146 [M3]
  it("Treiber und Worker-Fabrik wählen nach Dimension; 3D endet mit „done“ und liefert n × 3 Koordinaten", () => {
    const tree = basic();
    expect(createSimulation(input(tree))).toBeInstanceOf(ForceSim3D);
    expect(createSimulation({ ...input(tree), dims: 2 })).not.toBeInstanceOf(ForceSim3D);
    const replies: ForceReply[] = [];
    let t = 0;
    const queue: (() => void)[] = [];
    const driver = new ForceDriver((r) => replies.push(r), (fn) => queue.push(fn), () => (t += 1), 50, createSimulation);
    driver.handle({ type: "start", input: input(tree) });
    while (queue.length) queue.shift()!();
    const last = replies.at(-1)!;
    expect(last.type).toBe("done");
    expect(last.positions.length).toBe(tree.visible().length * 3);
    const runner = inlineRunner(() => undefined, () => undefined);
    runner.start(input(tree));
    runner.fix(2, 1, 2, 3);
    runner.dispose();
  });

  // spec: UI-098
  // spec: UI-146 [M3]
  it("10 000 Knoten: ein Schritt bleibt im Richtwert (Leistungsmessung 100 000 Knoten siehe Bericht v02)", () => {
    const sim = new ForceSim3D(input(large(10_000)));
    const start = performance.now();
    sim.step();
    sim.step();
    const perStep = (performance.now() - start) / 2;
    expect(sim.n).toBe(10_000);
    expect(Array.from(sim.pos.subarray(0, 30)).every(Number.isFinite)).toBe(true);
    // Großzügige Grenze (CI-Rauschen); der Wert erscheint im Testprotokoll.
    console.info(`3D-Layout 10 000 Knoten: ${perStep.toFixed(1)} ms/Schritt`);
    expect(perStep).toBeLessThan(2_000);
  });
});
