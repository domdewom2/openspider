import { describe, expect, it } from "vitest";
import { fxUiBasic, fxUiDepth, selectGraph } from "../fixtures/synthetic";
import { depthLightness, depthRadius, hexToHsl, metricRadius, nodeStyle, PALETTES, encodeStats, type EncodeOptions } from "./encoding";
import { VizTree } from "./tree";

const FORCE: EncodeOptions = { layout: "force", colorMode: "indexability", palette: "standard", sizeByMetric: false, minRadius: 3, maxRadius: 30, r0: 14, f: 0.8 };
const TREE: EncodeOptions = { ...FORCE, layout: "tree" };

describe("Tiefenkodierung im Graphen", () => {
  // spec: UI-092
  it("Radius max(3, r₀·fᵗ): Wurzel 14, Tiefe 1 11,2, Tiefe 2 8,96, ab Tiefe 7 Mindestradius 3 (fx-ui-depth)", () => {
    const tree = VizTree.from(selectGraph(fxUiDepth(), { budget: 10_000, expandDepth: 100, metric: "crawl_depth" }));
    const radiusAt = (level: number) => {
      const id = tree.order.find((i) => tree.get(i)?.level === level) as number;
      const n = tree.get(id)!;
      return nodeStyle(n, n.level, false, FORCE, encodeStats([n])).r;
    };
    expect(radiusAt(0)).toBeCloseTo(14, 2);
    expect(radiusAt(1)).toBeCloseTo(11.2, 2);
    expect(radiusAt(2)).toBeCloseTo(8.96, 2);
    expect(depthRadius(6)).toBeCloseTo(14 * 0.8 ** 6, 5);
    expect(radiusAt(7)).toBe(3);
    expect(radiusAt(12)).toBe(3);
  });

  // spec: UI-092
  it("Helligkeit min(85 %, 30 % + 8 %·t) bei konstantem Farbton der Indexierbarkeitsfarbe", () => {
    expect(depthLightness(0)).toBe(30);
    expect(depthLightness(5)).toBe(70);
    expect(depthLightness(12)).toBe(85);
    const base = hexToHsl(PALETTES.standard.indexable);
    for (const t of [0, 1, 4, 9]) {
      const fill = nodeStyle({ address: "x", indexability: "Indexable" }, t, false, FORCE, encodeStats([])).fill;
      const [h, , l] = hexToHsl(fill);
      expect(Math.abs(h - base[0])).toBeLessThan(2);
      expect(l).toBeCloseTo(depthLightness(t), 0);
    }
  });

  // spec: UI-092
  it("Größenskalierung nach Metrik ersetzt die Tiefenkodierung vollständig", () => {
    const o = { ...FORCE, sizeByMetric: true };
    const s = { maxMetric: 100, minColor: 0, maxColor: 0 };
    const a = nodeStyle({ address: "x", indexability: "Indexable", metric: 25 }, 0, false, o, s);
    const b = nodeStyle({ address: "x", indexability: "Indexable", metric: 25 }, 9, false, o, s);
    expect(a.r).toBe(b.r);
    expect(a.fill).toBe(PALETTES.standard.indexable);
  });
});

describe("Farben und Größe nach Metrik", () => {
  // spec: UI-095
  it("Fläche proportional zum Wert: Word Count 50 gegen 2 000 → Radienverhältnis √40 (fx-ui-basic)", () => {
    const dto = selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "word_count" });
    const nodes = dto.nodes;
    const stats = encodeStats(nodes);
    const o = { ...TREE, sizeByMetric: true, minRadius: 1, maxRadius: 40 };
    const small = nodes.find((n) => n.metric === 50)!;
    const big = nodes.find((n) => n.metric === 2000)!;
    const ratio = nodeStyle(big, 1, false, o, stats).r / nodeStyle(small, 1, false, o, stats).r;
    expect(ratio).toBeCloseTo(Math.sqrt(40), 5);
  });

  // spec: UI-095
  it("fehlende Werte: Minimalgröße und gestrichelter Rand; Min-/Max-Radius einstellbar", () => {
    const o = { ...TREE, sizeByMetric: true, minRadius: 4, maxRadius: 20 };
    const st = nodeStyle({ address: "x", indexability: "Indexable", metric: null }, 1, false, o, { maxMetric: 10, minColor: 0, maxColor: 0 });
    expect(st.r).toBe(4);
    expect(st.dashed).toBe(true);
    expect(metricRadius(10, 10, 4, 20)).toBe(20);
    expect(metricRadius(0.001, 10, 4, 20)).toBe(4);
  });

  // spec: UI-095
  it("Farbmodus Segment nutzt die Segmentfarbe, Metrik-Verlauf reicht von Min bis Max", () => {
    const seg = nodeStyle({ address: "x", segmentColor: "#7B61FF" }, 1, false, { ...TREE, colorMode: "segment" }, encodeStats([]));
    expect(seg.fill).toBe("#7b61ff");
    const s = { maxMetric: 0, minColor: 10, maxColor: 20 };
    const lo = nodeStyle({ address: "x", colorValue: 10 }, 1, false, { ...TREE, colorMode: "metric" }, s);
    const hi = nodeStyle({ address: "x", colorValue: 20 }, 1, false, { ...TREE, colorMode: "metric" }, s);
    expect(lo.fill).toBe(PALETTES.standard.gradient[0]);
    expect(hi.fill).toBe(PALETTES.standard.gradient[1]);
    const status = nodeStyle({ address: "x", statusCode: 404 }, 1, false, { ...TREE, colorMode: "statusClass" }, s);
    expect(status.fill).toBe(PALETTES.standard.status["4xx"]);
  });
});

describe("Knotenfarben im Baum", () => {
  // spec: UI-091
  it("indexierbar grün, nicht indexierbar blassrot, gekappt grau; Farbschema für Farbfehlsichtige wählbar", () => {
    const s = encodeStats([]);
    const green = hexToHsl(nodeStyle({ address: "x", indexability: "Indexable" }, 1, false, TREE, s).fill);
    const red = hexToHsl(nodeStyle({ address: "x", indexability: "Non-Indexable" }, 1, false, TREE, s).fill);
    expect(green[0]).toBeGreaterThan(90);
    expect(green[0]).toBeLessThan(160);
    expect(red[0] < 15 || red[0] > 345).toBe(true);
    expect(red[2]).toBeGreaterThan(65);
    expect(nodeStyle({ address: "x", indexability: "Indexable" }, 1, true, TREE, s).fill).toBe(PALETTES.standard.capped);
    const cb = { ...TREE, palette: "colorblind" as const };
    expect(nodeStyle({ address: "x", indexability: "Indexable" }, 1, false, cb, s).fill).toBe(PALETTES.colorblind.indexable);
    expect(PALETTES.colorblind.indexable).not.toBe(PALETTES.colorblind.nonIndexable);
  });

  // spec: UI-093
  it("Pfadknoten ohne URL sind hohl", () => {
    const st = nodeStyle({ address: null }, 2, false, TREE, encodeStats([]));
    expect(st.hollow).toBe(true);
  });
});
