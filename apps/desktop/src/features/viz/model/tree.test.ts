import { describe, expect, it } from "vitest";
import { fxUiBasic, fxUiDepth, fxUiDirs, genUiLarge, selectGraph } from "../fixtures/synthetic";
import { buildScene } from "./scene";
import { search, step } from "./search";
import { VizTree } from "./tree";
import { layoutTree, TREE_LAYOUT_DEFAULTS } from "./treeLayout";
import { exploreDecision, RootTrail } from "./details";

const ENC = { layout: "tree" as const, colorMode: "indexability" as const, palette: "standard" as const, sizeByMetric: false, minRadius: 3, maxRadius: 30, r0: 14, f: 0.8 };

describe("Crawl-Baum", () => {
  // spec: UI-091
  it("jeder Knoten erscheint genau einmal; Pfad zur Tiefe 12 folgt dem Crawl-Pfad (fx-ui-depth)", () => {
    const tree = VizTree.from(selectGraph(fxUiDepth(), { budget: 10_000, expandDepth: 100, metric: "crawl_depth" }));
    const ids = tree.visible();
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids.length).toBe(25);
    const deepest = tree.order.find((id) => tree.get(id)?.address?.endsWith("/level-12/"))!;
    expect(tree.pathTo(deepest).map((id) => tree.get(id)!.label)).toEqual(["/", ...Array.from({ length: 12 }, (_, i) => `/level-${i + 1}/`)]);
  });

  // spec: UI-091
  it("Klick klappt ein und auf; eingeklappte Knoten sind gekappt (grau, Badge mit Kinderzahl)", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    expect(tree.toggle(1)).toBe("collapsed");
    expect(tree.visible()).toEqual([1]);
    expect(tree.isCapped(1)).toBe(true);
    expect(tree.hidden(1)).toEqual({ children: 24, descendants: 24 });
    const scene = buildScene(tree, layoutTree(tree, TREE_LAYOUT_DEFAULTS), ENC, "path");
    expect(scene.nodes[0]!.badge).toBe(24);
    expect(tree.toggle(1)).toBe("expanded");
    expect(tree.visible()).toHaveLength(25);
  });

  // spec: UI-091
  it("„Bis Tiefe n aufklappen“ wirkt global (Kern liefert bis Tiefe n, tiefere Knoten gekappt)", () => {
    const tree = VizTree.from(selectGraph(fxUiDepth(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    expect(Math.max(...tree.order.map((id) => tree.get(id)!.level))).toBe(3);
    const capped = tree.order.filter((id) => tree.isCapped(id));
    expect(capped).toHaveLength(1);
    expect(tree.get(capped[0]!)!.level).toBe(3);
  });

  // spec: UI-091
  it("Ausrichtung links→rechts bzw. oben→unten", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const lr = layoutTree(tree, { ...TREE_LAYOUT_DEFAULTS, orientation: "lr" });
    const tb = layoutTree(tree, { ...TREE_LAYOUT_DEFAULTS, orientation: "tb" });
    expect(lr.get(2)![0]).toBe(TREE_LAYOUT_DEFAULTS.levelGap);
    expect(tb.get(2)![1]).toBe(TREE_LAYOUT_DEFAULTS.levelGap);
    expect(lr.get(1)).toEqual([0, 0]);
  });
});

describe("Verzeichnisbaum", () => {
  // spec: UI-093
  it("zwei Verzeichnisse ohne Indexseite → genau zwei hohle Pfadknoten; Query-Varianten sind Kinder ihres Pfadknotens (fx-ui-dirs)", () => {
    const tree = VizTree.from(selectGraph(fxUiDirs(), { budget: 10_000, expandDepth: 100, metric: "crawl_depth" }));
    const scene = buildScene(tree, layoutTree(tree, TREE_LAYOUT_DEFAULTS), ENC, "path");
    const hollow = scene.nodes.filter((n) => n.hollow).map((n) => n.label);
    expect(hollow).toEqual(["http", "127.0.0.1:8000", "docs", "blog"]);
    expect(hollow.filter((l) => l === "docs" || l === "blog")).toHaveLength(2);
    expect(tree.get(5)!.parent).toBe(4);
    expect(scene.nodes.every((n) => n.label !== "")).toBe(true);
  });
});

describe("Knotenbudget, Teilbaum öffnen, Wurzel setzen", () => {
  const large = genUiLarge();

  // spec: UI-096
  it("Budget 10 000 bei 1/40/2 000/47 959: Ebenen 1–3 vollständig, Ebene 4 teilweise, gekappte Knoten mit Badge (gen-ui-large)", () => {
    const tree = VizTree.from(selectGraph(large, { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const byLevel = [0, 0, 0, 0];
    for (const id of tree.order) byLevel[tree.get(id)!.level]! += 1;
    expect(tree.size).toBe(10_000);
    expect(byLevel).toEqual([1, 40, 2000, 7959]);
    const capped = tree.order.filter((id) => tree.isCapped(id));
    expect(capped.length).toBeGreaterThan(0);
    expect(capped.every((id) => tree.get(id)!.level === 2)).toBe(true);
    const h = tree.hidden(capped[capped.length - 1]!);
    expect(h.children).toBeGreaterThanOrEqual(23);
    expect(h.children).toBeLessThanOrEqual(24);
  });

  // spec: UI-096
  it("„Teilbaum öffnen“ fügt die direkten Kinder hinzu (+≈24); über Budget + 1 000 → Rückfrage", () => {
    const first = selectGraph(large, { budget: 10_000, expandDepth: 3, metric: "crawl_depth" });
    const target = first.nodes.filter((n) => n.hiddenChildren > 0).at(-1)!;
    expect(exploreDecision(target.hiddenChildren, first.nodes.length, 10_000)).toBe("explore");
    const next = selectGraph(large, { budget: 10_000, expandDepth: 3, metric: "crawl_depth", explore: [target.id] });
    expect(next.nodes.length - first.nodes.length).toBe(target.hiddenChildren);
    expect(exploreDecision(1_500, 10_000, 10_000)).toBe("confirm");
    expect(exploreDecision(0, 10_000, 10_000)).toBe("none");
  });

  // spec: UI-096
  it("„Als Wurzel setzen“ auf einen Ebene-2-Knoten zeigt dessen gesamten Teilbaum; Pfadleiste mit 2 Einträgen und Rückkehr", () => {
    const first = VizTree.from(selectGraph(large, { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const level2 = first.order.find((id) => first.get(id)!.level === 1)!;
    const focused = VizTree.from(selectGraph(large, { root: level2, budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    expect(focused.roots).toEqual([level2]);
    expect(focused.order.every((id) => !focused.isCapped(id))).toBe(true);
    const subtree = [level2];
    for (let i = 0; i < subtree.length; i++) subtree.push(...(large.children.get(subtree[i]!) ?? []));
    expect(focused.size).toBe(subtree.length);
    expect(subtree.length).toBeGreaterThan(1_200);
    let trail = RootTrail.start("http://127.0.0.1:8000/");
    trail = trail.focus(level2, "/l1/p2.html", { k: 2, x: 10, y: 20 });
    expect(trail.entries.map((e) => e.label)).toEqual(["http://127.0.0.1:8000/", "/l1/p2.html"]);
    const back = trail.back(0);
    expect(back.view).toEqual({ k: 2, x: 10, y: 20 });
    expect(back.trail.entries).toHaveLength(1);
  });
});

describe("Suche", () => {
  // spec: UI-090
  it("Teilstring auf Address, Groß/Klein egal, „i von n“, Enter/Shift+Enter zyklisch, Pfad zu eingeklappten Treffern öffnet sich", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    let s = search(tree, "PAGE-1");
    expect(s.matches.map((id) => tree.get(id)!.label)).toEqual(["/page-1.html", "/page-10.html", "/page-11.html", "/page-12.html", "/page-13.html", "/page-14.html", "/page-15.html", "/page-16.html", "/page-17.html", "/page-18.html", "/page-19.html"]);
    expect(s.index).toBe(0);
    s = step(s, -1);
    expect(s.index).toBe(10);
    s = step(s, 1);
    expect(s.index).toBe(0);
    tree.toggle(1);
    expect(tree.visible()).toHaveLength(1);
    expect(tree.reveal(s.matches[0]!)).toBe(true);
    expect(tree.visible()).toHaveLength(25);
  });
});
