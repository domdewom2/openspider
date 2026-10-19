import { describe, expect, it } from "vitest";
import { fxUiBasic, fxUiDirs, selectGraph } from "../fixtures/synthetic";
import { csvPayload, directoryPath, metricOptions, MISSING, tooltipRows } from "./details";
import { buildScene } from "./scene";
import { VizTree } from "./tree";
import { layoutTree, TREE_LAYOUT_DEFAULTS } from "./treeLayout";

const ENC = { layout: "tree" as const, colorMode: "indexability" as const, palette: "standard" as const, sizeByMetric: false, minRadius: 3, maxRadius: 30, r0: 14, f: 0.8 };
const num = (v: number) => String(v);

describe("Tooltip", () => {
  // spec: UI-090
  it("genau diese Zeilen: Address, Status Code, Indexability, Crawl Depth, Unique Inlinks, Link Score, Skalierungsmetrik; fehlende Werte „–“", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "word_count" }));
    const rows = tooltipRows(tree.get(2)!, "", "word_count", num);
    expect(rows.map((r) => r.name)).toEqual(["Address", "Status Code", "Indexability", "Crawl Depth", "Unique Inlinks", "Link Score", "Word Count"]);
    expect(rows.map((r) => r.value)).toEqual(["http://127.0.0.1:8000/page-1.html", "200", "Indexable", "1", "1", "11", "50"]);
    const missing = tooltipRows({ ...tree.get(2)!, linkScore: null, metric: null, statusCode: null }, "", "ga4.sessions", num);
    expect(missing.find((r) => r.name === "Link Score")!.value).toBe(MISSING);
    expect(missing.find((r) => r.name === "Status Code")!.value).toBe(MISSING);
    expect(missing.at(-1)).toEqual({ name: "GA Sessions", value: MISSING });
  });

  // spec: UI-093
  it("Pfadknoten ohne gecrawlte URL zeigen im Tooltip nur den Pfad (fx-ui-dirs)", () => {
    const tree = VizTree.from(selectGraph(fxUiDirs(), { budget: 10_000, expandDepth: 100, metric: "crawl_depth" }));
    expect(tooltipRows(tree.get(-2)!, directoryPath(tree, -2), "crawl_depth", num)).toEqual([{ name: "Path", value: "/docs" }]);
  });
});

describe("Metrikauswahl", () => {
  // spec: UI-095
  it("Standardmetriken in fester Reihenfolge, ohne Daten ausgegraut; weitere numerische Spalten (Linkmetriken, Custom Extraction) wählbar", () => {
    const opts = metricOptions(["crawl_depth", "unique_inlinks", "word_count", "majestic.trust_flow", "custom_extraction.0"]);
    expect(opts.map((o) => o.name)).toEqual(["Crawl Depth", "Unique Inlinks", "Link Score", "Word Count", "Response Time", "GA Sessions", "GSC Clicks", "GSC Impressions", "custom_extraction.0", "majestic.trust_flow"]);
    expect(opts.filter((o) => o.disabled).map((o) => o.key)).toEqual(["link_score", "response_time", "ga4.sessions", "gsc.clicks", "gsc.impressions"]);
  });
});

describe("CSV-Export", () => {
  // spec: UI-090
  it("enthält genau die dargestellten Knoten und Kanten mit Koordinaten (Zeilenzahl = angezeigte Knoten)", () => {
    const tree = VizTree.from(selectGraph(fxUiBasic(), { budget: 10_000, expandDepth: 3, metric: "crawl_depth" }));
    const scene = buildScene(tree, layoutTree(tree, TREE_LAYOUT_DEFAULTS), ENC, "path");
    const payload = csvPayload(tree, scene);
    expect(payload.nodes).toHaveLength(25);
    expect(payload.edges).toHaveLength(24);
    expect(payload.nodes[1]).toMatchObject({ id: 2, x: TREE_LAYOUT_DEFAULTS.levelGap });
    expect(Object.keys(payload.nodes[0]!)).not.toContain("children");
    expect(tree.hasClientCollapse()).toBe(false);
    tree.toggle(1);
    expect(tree.hasClientCollapse()).toBe(true);
  });
});
