import { describe, expect, it } from "vitest";
import { clampZoom, fitBounds, MAX_ZOOM, MIN_ZOOM, toScreen, toWorld, wheelFactor, zoomAt } from "./viewport";

describe("Zoom und Verschieben", () => {
  // spec: UI-090
  it("Zoom hart begrenzt auf 0,05× bis 8×", () => {
    expect(clampZoom(0.001)).toBe(MIN_ZOOM);
    expect(clampZoom(100)).toBe(MAX_ZOOM);
    let t = { k: 1, x: 0, y: 0 };
    for (let i = 0; i < 100; i++) t = zoomAt(t, 2, 10, 10);
    expect(t.k).toBe(8);
    for (let i = 0; i < 100; i++) t = zoomAt(t, 0.5, 10, 10);
    expect(t.k).toBe(0.05);
  });

  // spec: UI-090
  it("Zoom zentriert auf den Mauszeiger: der Weltpunkt unter dem Zeiger bleibt stehen", () => {
    const t0 = { k: 1.3, x: 40, y: -12 };
    const before = toWorld(t0, 321, 123);
    const t1 = zoomAt(t0, wheelFactor(-120), 321, 123);
    expect(t1.k).toBeGreaterThan(t0.k);
    const after = toScreen(t1, before[0], before[1]);
    expect(after[0]).toBeCloseTo(321, 9);
    expect(after[1]).toBeCloseTo(123, 9);
    expect(wheelFactor(120)).toBeLessThan(1);
    expect(wheelFactor(3, 1)).toBeCloseTo(wheelFactor(48), 9);
  });

  // spec: UI-090
  it("Einpassen bleibt innerhalb der Zoomgrenzen", () => {
    const t = fitBounds({ minX: -1e7, minY: -1e7, maxX: 1e7, maxY: 1e7 }, 800, 600);
    expect(t.k).toBe(MIN_ZOOM);
    const small = fitBounds({ minX: 0, minY: 0, maxX: 10, maxY: 10 }, 800, 600);
    expect(small.k).toBe(1);
  });
});
