import { describe, expect, it } from "vitest";
import { fxUiBasic, fxUiDepth, genUiLarge, selectGraph } from "../fixtures/synthetic";
import { DEFAULT_STYLE, drawScene, toSvg, type RenderState } from "./render";
import { buildScene, HitIndex, sceneBounds } from "./scene";
import { VizTree } from "./tree";
import { layoutTree, TREE_LAYOUT_DEFAULTS } from "./treeLayout";
import type { EncodeOptions } from "./encoding";

const FORCE: EncodeOptions = { layout: "force", colorMode: "indexability", palette: "standard", sizeByMetric: false, minRadius: 3, maxRadius: 30, r0: 14, f: 0.8 };
const TREE: EncodeOptions = { ...FORCE, layout: "tree" };
const ALLOWED = new Set(["svg", "rect", "g", "path", "circle", "text"]);

function state(k = 1, showLabels = false, width = 1200, height = 800): RenderState {
  return { t: { k, x: width / 2, y: height / 2 }, width, height, showLabels, highlights: new Set(), current: null, hovered: null };
}

/** Strukturprüfung gegen SVG 1.1: wohlgeformt, Namensraum, Version, nur bekannte Elemente, Farben als Hex. */
function checkSvg(svg: string): Document {
  const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
  expect(doc.getElementsByTagName("parsererror")).toHaveLength(0);
  const root = doc.documentElement;
  expect(root.namespaceURI).toBe("http://www.w3.org/2000/svg");
  expect(root.getAttribute("version")).toBe("1.1");
  for (const el of Array.from(doc.getElementsByTagName("*"))) {
    expect(ALLOWED.has(el.localName)).toBe(true);
    for (const attr of ["fill", "stroke"]) {
      const v = el.getAttribute(attr);
      if (v !== null) expect(v === "none" || /^#[0-9a-f]{6}$/.test(v)).toBe(true);
    }
  }
  return doc;
}

describe("SVG-Export", () => {
  // spec: UI-092
  it("Radien prüfbar über den Export: Wurzel 14, Tiefe 1 11,2, Tiefe 2 8,96 (fx-ui-depth)", () => {
    const tree = VizTree.from(selectGraph(fxUiDepth(), { budget: 10_000, expandDepth: 100, metric: "crawl_depth" }));
    const pos = new Map(tree.order.map((id, i) => [id, [i * 30 - 300, 0] as [number, number]]));
    const doc = checkSvg(toSvg(buildScene(tree, pos, FORCE, "path"), state(), DEFAULT_STYLE));
    const r = (id: number) => Number(doc.getElementById(`n${id}`)!.getAttribute("r"));
    expect(r(1)).toBeCloseTo(14, 2);
    expect(r(2)).toBeCloseTo(11.2, 2);
    expect(r(4)).toBeCloseTo(8.96, 2);
  });

  // spec: UI-092
  it("Knotenbeschriftungen erst ab der eingestellten Zoomstufe; darunter keine <text>-Elemente für Beschriftungen", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const pos = layoutTree(tree, TREE_LAYOUT_DEFAULTS);
    const scene = buildScene(tree, pos, FORCE, "path");
    const below = checkSvg(toSvg(scene, state(1, false), DEFAULT_STYLE));
    expect(below.querySelectorAll("text.label")).toHaveLength(0);
    const above = checkSvg(toSvg(scene, { ...state(1.5, true), t: { k: 1.5, x: 100, y: 400 } }, DEFAULT_STYLE));
    expect(above.querySelectorAll("text.label").length).toBeGreaterThan(0);
  });

  // spec: UI-090
  it("valides SVG 1.1: Texte als <text>, Farben als Hex, nur der aktuelle Ausschnitt; Sonderzeichen maskiert", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    tree.get(2)!.label = '<a & "b">';
    const pos = layoutTree(tree, TREE_LAYOUT_DEFAULTS);
    const scene = buildScene(tree, pos, TREE, "path");
    const all = checkSvg(toSvg(scene, { ...state(1, true), t: { k: 1, x: 40, y: 400 } }, DEFAULT_STYLE));
    expect(all.querySelectorAll("circle.node")).toHaveLength(25);
    expect(Array.from(all.querySelectorAll("text.label")).some((t) => t.textContent === '<a & "b">')).toBe(true);
    const part = checkSvg(toSvg(scene, { ...state(1, false, 300, 100), t: { k: 1, x: 40, y: 50 } }, DEFAULT_STYLE));
    expect(part.querySelectorAll("circle.node").length).toBeLessThan(25);
  });

  // spec: UI-146
  it("SVG-Export von 10 000 Knoten ≤ 5 s und ≤ 20 MB (gen-ui-large)", () => {
    const tree = VizTree.from(selectGraph(genUiLarge(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const pos = layoutTree(tree, TREE_LAYOUT_DEFAULTS);
    const scene = buildScene(tree, pos, TREE, "path");
    const started = performance.now();
    const b = sceneBounds(scene);
    const k = 0.05;
    const view = { ...state(k, false, Math.ceil((b.maxX - b.minX) * k) + 200, Math.ceil((b.maxY - b.minY) * k) + 200), t: { k, x: 100 - b.minX * k, y: 100 - b.minY * k } };
    const svg = toSvg(scene, view, DEFAULT_STYLE);
    expect(performance.now() - started).toBeLessThan(5_000);
    expect(new Blob([svg]).size).toBeLessThan(20 * 1024 * 1024);
    expect((svg.match(/class="node/g) ?? []).length).toBe(10_000);
  });
});

describe("Canvas-Zeichnung und Treffertest", () => {
  // spec: UI-146
  it("zeichnet auf Canvas mit DPR-Skalierung und findet Knoten unter dem Zeiger", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const scene = buildScene(tree, layoutTree(tree, TREE_LAYOUT_DEFAULTS), TREE, "path");
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d")!;
    drawScene(ctx, scene, { ...state(1, true), highlights: new Set([2]), current: 2 }, DEFAULT_STYLE, 2);
    const hit = new HitIndex(scene);
    const n = scene.nodes[3]!;
    expect(hit.hit(n.x + 1, n.y)?.id).toBe(n.id);
    expect(hit.hit(n.x + 50, n.y + 9)).toBeNull();
  });
});
