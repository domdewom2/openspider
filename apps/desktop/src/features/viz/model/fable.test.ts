// M3-V02 Fable-Fixrunde: Teilselektion der Beschriftungskandidaten, vollständiges Neu-Hochladen nach Kontextverlust.
import { describe, expect, it } from "vitest";
import { mulberry32 } from "./force";
import { uploadScene, type SceneSink } from "./gl3d";
import { nearestK, viewDepth } from "./labels3d";
import { initialCamera, viewMatrix } from "./camera3d";

describe("3D-Beschriftung und Kontextverlust", () => {
  // spec: UI-098
  it("nearestK liefert genau die k nächsten zulässigen Knoten wie eine Vollsortierung, ohne alle zu prüfen", () => {
    const rnd = mulberry32(3);
    const n = 20_000;
    const depth = Array.from({ length: n }, () => (rnd() < 0.1 ? -1 : rnd() * 1000));
    const ok = (i: number) => i % 7 !== 0;
    let accepted = 0;
    const got = nearestK(n, 150, (i) => depth[i]!, (i) => (accepted++, ok(i)));
    const want = depth
      .map((d, i) => ({ d, i }))
      .filter((x) => x.d > 0 && ok(x.i))
      .sort((a, b) => a.d - b.d || a.i - b.i)
      .slice(0, 150)
      .map((x) => x.i);
    expect(got).toEqual(want);
    // Projektion/Prüfung nur für einen Bruchteil der Knoten (Heap-Schwelle).
    expect(accepted).toBeLessThan(n / 4);
    expect(nearestK(5, 0, () => 1, () => true)).toEqual([]);
    expect(nearestK(3, 10, (i) => [3, 1, 2][i]!, () => true)).toEqual([1, 2, 0]);
  });

  // spec: UI-098
  it("viewDepth = Abstand vor der Kamera entlang der Blickrichtung", () => {
    const c = initialCamera(100, [0, 0, 0]);
    const v = viewMatrix(c);
    expect(viewDepth(v, 0, 0, 0)).toBeCloseTo(c.base, 4);
  });

  // spec: UI-098
  it("nach wiederhergestelltem Kontext werden Knoten, Kanten, Farben, Zusatzkanten, Lichtschein und Positionen neu geladen", () => {
    const calls: string[] = [];
    const sink: SceneSink = {
      setNodes: (r, c) => void calls.push(`nodes:${r.length}:${c.length}`),
      setEdges: (e) => void calls.push(`edges:${e.length}`),
      setColors: (c) => void calls.push(`colors:${c.length}`),
      setOverlay: (a, b) => void calls.push(`overlay:${a.length}:${b.length}`),
      setHalo: (h) => void calls.push(`halo:${h.length}`),
      setPositions: (p) => void calls.push(`positions:${p.length}`),
    };
    uploadScene(sink, { radius: new Float32Array(3), colors: new Uint8Array(12), edges: new Uint32Array([0, 1, 0, 2]), inlinks: new Uint32Array([1, 0]), outlinks: new Uint32Array(0), halo: new Uint32Array([2]), positions: new Float32Array(9) });
    expect(calls).toEqual(["nodes:3:12", "edges:4", "colors:12", "overlay:2:0", "halo:1", "positions:9"]);
  });
});
