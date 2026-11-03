// M3-V02 Optik: Beschriftungen ohne Überlappung, Farben aus Tokens (hell/dunkel), Kugelgrößen mit Ober-/Untergrenze,
// Häufigkeitsstufen und Einpassen der Wortwolke.
import { describe, expect, it } from "vitest";
import { leafPx, radius3d, R3_MAX, R3_MIN } from "../Graph3DView";
import { fxUiShowcase, directoryTreeOf, fxUiShowcaseWords } from "../fixtures/synthetic";
import { depthRadius, luminance, metricRadius, paletteFromTokens, SIZE_DEFAULTS } from "./encoding";
import { lerpCamera, initialCamera, focusOn } from "./camera3d";
import { placeLabels, type LabelBox } from "./render";
import { fitWords, layoutWords, tierOf, TIER_WEIGHT } from "./words";

const LIGHT = { surface: "#ffffff", success: "#14652f", danger: "#a82027", warning: "#744900", warningBg: "#fff0cc", accent: "#0964dc", muted: "#59626d", none: "#7b848e" };
const DARK = { surface: "#1b1d20", success: "#a2e8b7", danger: "#ffb1b5", warning: "#ffda8b", warningBg: "#463719", accent: "#58a6ff", muted: "#a3abb5", none: "#8c949e" };

describe("Optik der Visualisierungen", () => {
  // spec: UI-098
  it("Beschriftungen überlappen nicht: spätere (niedrigere Priorität) entfallen, freie bleiben", () => {
    const boxes: LabelBox[] = [
      { x: 0, y: 0, w: 50, h: 16 },
      { x: 20, y: 5, w: 50, h: 16 },
      { x: 60, y: 0, w: 40, h: 16 },
      { x: 0, y: 30, w: 80, h: 16 },
    ];
    const shown = placeLabels(boxes);
    expect(shown).toEqual([0, 2, 3]);
    const kept = shown.map((i) => boxes[i]!);
    for (let a = 0; a < kept.length; a++)
      for (let b = a + 1; b < kept.length; b++) {
        const p = kept[a]!;
        const q = kept[b]!;
        expect(p.x < q.x + q.w && p.x + p.w > q.x && p.y < q.y + q.h && p.y + p.h > q.y).toBe(false);
      }
  });

  // spec: UI-098
  it("Kugelradius 3D zwischen Unter- und Obergrenze (≤ 3,5×), auch bei Metrik-Skalierung", () => {
    expect(R3_MAX / R3_MIN).toBeLessThanOrEqual(3.5);
    const enc = { sizeByMetric: false, minRadius: SIZE_DEFAULTS.minRadius, maxRadius: SIZE_DEFAULTS.maxRadius };
    expect(radius3d(depthRadius(0), enc)).toBe(14);
    expect(radius3d(depthRadius(9), enc)).toBe(R3_MIN);
    const metric = { ...enc, sizeByMetric: true };
    expect(radius3d(metricRadius(2000, 2000, 3, 30), metric)).toBe(R3_MAX);
    expect(radius3d(metricRadius(null, 2000, 3, 30), metric)).toBe(R3_MIN);
    const mid = radius3d(metricRadius(500, 2000, 3, 30), metric);
    expect(mid).toBeGreaterThan(R3_MIN);
    expect(mid).toBeLessThan(R3_MAX);
    // Dichte Graphen zeichnen kleinere Punkte (gleichbleibende Bildpunktzahl), kleine Graphen die volle Größe.
    expect(leafPx(950)).toBe(5);
    expect(leafPx(100_000)).toBeLessThan(2.5);
    expect(leafPx(100_000)).toBeGreaterThanOrEqual(2);
  });

  // spec: UI-098
  it("Farbschema aus Tokens: hell und dunkel getrennt, nicht indexierbar blasser als das Signalrot, Verlauf aus Akzent", () => {
    const light = paletteFromTokens(LIGHT, false);
    const dark = paletteFromTokens(DARK, true);
    expect(light.indexable).toBe("#14652f");
    expect(dark.indexable).toBe("#a2e8b7");
    expect(luminance(light.nonIndexable)).toBeGreaterThan(luminance(LIGHT.danger));
    expect(light.gradient[1]).toBe(LIGHT.accent);
    expect(light.status["4xx"]).not.toBe(LIGHT.warning);
    expect(dark.status["4xx"]).toBe(DARK.warning);
  });

  // spec: UI-100
  it("Wortwolke: Häufigkeitsstufen bestimmen Schriftstärke, Platzierung deterministisch, Einpassen zentriert die Wolke", async () => {
    expect([tierOf(0, 100), tierOf(10, 100), tierOf(30, 100), tierOf(80, 100)]).toEqual([0, 1, 2, 3]);
    const words = fxUiShowcaseWords();
    const a = await layoutWords(words, 900, 500, "sans-serif");
    const b = await layoutWords(words, 900, 500, "sans-serif");
    expect(a).toEqual(b);
    expect(a[0]!.weight).toBe(TIER_WEIGHT[0]);
    for (const p of a) expect(p.weight).toBe(TIER_WEIGHT[p.tier]);
    expect(words.map((_, i) => TIER_WEIGHT[tierOf(i, words.length)])).toContain(400);
    const fit = fitWords(a, 900, 500);
    expect(fit.k).toBeGreaterThan(0);
    expect(fit.k).toBeLessThanOrEqual(2.5);
  });

  // spec: UI-098
  it("Kameraflug mit Ease-in-out: langsamer Start, weiches Ankommen, exaktes Ziel", () => {
    const a = initialCamera(500);
    const b = focusOn(a, [300, 0, 0]);
    const at = (t: number) => lerpCamera(a, b, t).target[0];
    expect(at(0.1)).toBeLessThan(300 * 0.1);
    expect(at(0.9)).toBeGreaterThan(300 * 0.9);
    expect(at(1)).toBe(300);
  });

  // spec: UI-094
  it("Vorführ-Fixture: rund 950 URLs in fünf Ebenen, gemischte Status/Indexierbarkeit; Verzeichnisbaum mit Pfadknoten", () => {
    const crawl = fxUiShowcase();
    const nodes = [...crawl.nodes.values()];
    expect(nodes.length).toBeGreaterThan(900);
    expect(new Set(nodes.map((n) => n.depth))).toEqual(new Set([0, 1, 2, 3, 4]));
    expect(new Set(nodes.map((n) => n.statusCode))).toEqual(new Set([200, 301, 404, 500]));
    expect(nodes.some((n) => n.indexability === "Non-Indexable")).toBe(true);
    const dirs = directoryTreeOf(crawl);
    expect(dirs.nodes.size).toBeGreaterThan(nodes.length);
    expect([...dirs.nodes.values()].filter((n) => n.address === null).length).toBeGreaterThan(3);
    expect([...dirs.nodes.values()].filter((n) => n.address !== null).length).toBe(nodes.length);
  });
});
