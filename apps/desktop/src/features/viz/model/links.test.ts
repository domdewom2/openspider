import { describe, expect, it } from "vitest";
import type { GraphDto } from "../../../ipc/gen/GraphDto";
import { fxUiWordsGraph, selectGraph, selectLinks } from "../fixtures/synthetic";
import { linkMenuEntries, overlayCsvEdges, overlayFrom, sameStructure, treeEdges, withLinks } from "./links";
import { VizTree } from "./tree";

const base = { snapshotId: "s", kind: "crawlGraph" as const, root: null, budget: 10_000, metric: "crawl_depth" };
const opts = { linkNofollow: true, linkExcludeNavFooter: false };
const t = (k: string) => k;

function withOverlay(mode: string, nofollow = true, nav = false): GraphDto {
  const tree = fxUiWordsGraph();
  const dto = selectGraph(tree, { budget: 10_000, expandDepth: 1_000_000, metric: "crawl_depth" });
  if (mode === "none") return dto;
  const pairs = selectLinks(tree, 2, mode, nofollow, nav);
  return { ...dto, edges: [...dto.edges, ...pairs.map(([source, target]) => ({ source, target, edgeType: mode.toLowerCase().includes("inlinks") ? "inlink" : "outlink" }))], linkTotal: pairs.length };
}

describe("Links eines Knotens (UI-097)", () => {
  // spec: UI-097
  it("Anfrage an viz.graph: Knoten, Richtung und Optionen; ohne Anfrage unverändert", () => {
    expect(withLinks(base, null, opts)).toBe(base);
    expect(withLinks(base, { node: 2, mode: "childOutlinks" }, { linkNofollow: false, linkExcludeNavFooter: true })).toEqual({ ...base, linksNode: 2, linksMode: "childOutlinks", linksIncludeNofollow: false, linksExcludeNavFooter: true });
  });

  // spec: UI-097
  it("Kantenzahl = Unique Inlinks bzw. Outlinks gemäß Optionen (fx-ui-words: 23/18/22/17, 7 Outlinks)", () => {
    for (const [nofollow, nav, want] of [[true, false, 23], [false, false, 18], [true, true, 22], [false, true, 17]] as const) {
      const dto = withOverlay("inlinks", nofollow, nav);
      const tree = VizTree.from({ ...dto, edges: treeEdges(dto) });
      const o = overlayFrom(dto, { node: 2, mode: "inlinks" }, tree);
      expect(o.edges.length).toBe(want);
      expect(o.total).toBe(want);
      expect(o.edges.every((e) => e.type === "inlink" && e.target === 2)).toBe(true);
    }
    const out = withOverlay("outlinks");
    expect(overlayFrom(out, { node: 2, mode: "outlinks" }, null).edges.length).toBe(7);
    // Verbundene Knoten = Ausgangsknoten + Gegenstellen; Kinder-Varianten heben die Kinder hervor.
    const child = withOverlay("childInlinks");
    const tree = VizTree.from({ ...child, edges: treeEdges(child) });
    const o = overlayFrom(child, { node: 2, mode: "childInlinks" }, tree);
    expect([...o.connected].sort((a, b) => a - b)).toEqual([2, 3, 25, 26]);
  });

  // spec: UI-097
  it("eingeblendete Links ändern die Struktur nicht; Baumkanten bleiben, CSV-Kanten tragen die Art", () => {
    const plain = withOverlay("none");
    const shown = withOverlay("inlinks");
    expect(sameStructure(plain, shown)).toBe(true);
    expect(treeEdges(shown)).toEqual(plain.edges);
    expect(sameStructure(plain, { ...plain, nodes: plain.nodes.slice(1) })).toBe(false);
    const o = overlayFrom(shown, { node: 2, mode: "inlinks" }, null);
    expect(overlayCsvEdges(o).every((e) => e.edgeType === "inlink")).toBe(true);
    expect(overlayCsvEdges(null)).toEqual([]);
  });

  // spec: UI-097
  it("Kontextmenü: vier Richtungen und Zurücksetzen; Pfadknoten und Knoten ohne Kinder mit Begründung ausgegraut", () => {
    const shown: unknown[] = [];
    let reset = 0;
    const entries = linkMenuEntries(t, { id: 2, address: "https://x/", children: [5], hiddenChildren: 0 }, null, (r) => shown.push(r), () => reset++);
    expect(entries.map((e) => e.label)).toEqual(["viz-menu-links-inlinks", "viz-menu-links-childInlinks", "viz-menu-links-outlinks", "viz-menu-links-childOutlinks", "viz-menu-links-reset"]);
    expect(entries.at(-1)!.isDisabled).toBe(true);
    entries[2]!.onAction!();
    expect(shown).toEqual([{ node: 2, mode: "outlinks" }]);
    const path = linkMenuEntries(t, { id: -3, address: null, children: [], hiddenChildren: 0 }, null, () => undefined, () => undefined);
    expect(path[0]).toMatchObject({ isDisabled: true, disabledReason: "viz-reason-path-node" });
    expect(path[1]).toMatchObject({ isDisabled: true, disabledReason: "viz-reason-no-children" });
    const active = overlayFrom(withOverlay("inlinks"), { node: 2, mode: "inlinks" }, null);
    const withReset = linkMenuEntries(t, { id: 2, address: "x", children: [], hiddenChildren: 0 }, active, () => undefined, () => reset++);
    withReset.at(-1)!.onAction!();
    expect(reset).toBe(1);
  });
});
