import { describe, expect, it } from "vitest";
import * as cam from "./camera3d";

const H = 600;
const W = 800;

describe("Orbit-Kamera der 3D-Graphen", () => {
  // spec: UI-098
  it("Ziel liegt in der Bildmitte; Rotation ändert Azimut/Polarwinkel, Polarwinkel ohne Überschlag", () => {
    const c = cam.initialCamera(500);
    const view = cam.viewMatrix(c);
    const proj = cam.projMatrix(c, W / H, 500);
    const p = cam.project(view, proj, c.target, W, H)!;
    expect(p.x).toBeCloseTo(W / 2, 5);
    expect(p.y).toBeCloseTo(H / 2, 5);
    const r = cam.rotate(c, 100, 0);
    expect(r.theta).not.toBe(c.theta);
    expect(r.phi).toBe(c.phi);
    expect(cam.rotate(c, 0, -100_000).phi).toBeCloseTo(Math.PI - 0.01, 5);
    expect(cam.rotate(c, 0, 100_000).phi).toBeCloseTo(0.01, 5);
  });

  // spec: UI-098
  // spec: UI-090 [M3]
  it("Mausrad zoomt um den Zeiger und ist hart auf 0,05× … 8× begrenzt", () => {
    let c = cam.initialCamera(500);
    for (let i = 0; i < 100; i++) c = cam.zoomAt(c, 1.5, 0, 0, H);
    expect(c.k).toBe(8);
    for (let i = 0; i < 200; i++) c = cam.zoomAt(c, 0.5, 0, 0, H);
    expect(c.k).toBe(0.05);
    // Punkt unter dem Zeiger (in der Zielebene) bleibt stehen.
    const a = cam.initialCamera(500);
    const sx = 120;
    const sy = -80;
    const world = (k: cam.Camera3D) => {
      const w = cam.worldPerPixel(k, H);
      const { right, up } = cam.basis(k);
      return [0, 1, 2].map((i) => k.target[i]! + (right[i]! * sx - up[i]! * sy) * w);
    };
    const before = world(a);
    const after = world(cam.zoomAt(a, 2, sx, sy, H));
    for (let i = 0; i < 3; i++) expect(after[i]!).toBeCloseTo(before[i]!, 6);
  });

  // spec: UI-098
  it("Rechtsziehen verschiebt: die Szene folgt dem Zeiger", () => {
    const c = cam.initialCamera(500);
    const moved = cam.pan(c, 50, 0, H);
    const view = cam.viewMatrix(moved);
    const proj = cam.projMatrix(moved, W / H, 500);
    const p = cam.project(view, proj, c.target, W, H)!;
    expect(p.x).toBeCloseTo(W / 2 + 50, 3);
    expect(p.y).toBeCloseTo(H / 2, 3);
  });

  // spec: UI-098
  it("Suche fliegt zum Knoten: Ziel = Knoten, mindestens Zoom 3×; Zwischenstände weich, Ende exakt", () => {
    const c = cam.initialCamera(500);
    const to = cam.focusOn(c, [100, -50, 20]);
    expect(to.target).toEqual([100, -50, 20]);
    expect(to.k).toBe(3);
    expect(cam.lerpCamera(c, to, 1).target).toEqual([100, -50, 20]);
    const mid = cam.lerpCamera(c, to, 0.5);
    expect(mid.k).toBeGreaterThan(1);
    expect(mid.k).toBeLessThan(3);
    const view = cam.viewMatrix(to);
    const proj = cam.projMatrix(to, W / H, 500);
    const p = cam.project(view, proj, [100, -50, 20], W, H)!;
    expect(p.x).toBeCloseTo(W / 2, 4);
    expect(p.y).toBeCloseTo(H / 2, 4);
    // Punkte hinter der Kamera sind nicht sichtbar.
    const behind = cam.eye(c).map((v, i) => v + (v - c.target[i]!)) as cam.Vec3;
    expect(cam.project(cam.viewMatrix(c), cam.projMatrix(c, W / H, 500), behind, W, H)).toBeNull();
  });
});
