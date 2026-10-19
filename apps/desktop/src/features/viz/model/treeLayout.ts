// Baumlayout (UI-091, UI-093) über d3-hierarchy auf den sichtbaren Knoten; Ausrichtung links→rechts oder oben→unten.
import { hierarchy, tree as d3tree } from "d3-hierarchy";
import type { VizTree } from "./tree";

export type Orientation = "lr" | "tb";

export interface TreeLayoutOptions {
  orientation: Orientation;
  /** Abstand zwischen Ebenen (px). */
  levelGap: number;
  /** Abstand zwischen Nachbarknoten (px). */
  nodeGap: number;
}

export const TREE_LAYOUT_DEFAULTS: TreeLayoutOptions = { orientation: "lr", levelGap: 180, nodeGap: 18 };

interface H {
  id: number;
  children: H[];
}

/** Positionen der sichtbaren Knoten; mehrere Wurzeln (Verzeichnisbaum: Schemata) hängen an einer unsichtbaren Wurzel. */
export function layoutTree(tree: VizTree, o: TreeLayoutOptions): Map<number, [number, number]> {
  const visible = new Set(tree.visible());
  const out = new Map<number, [number, number]>();
  if (!tree.roots.length) return out;
  // Iterativ (Breitensuche) statt rekursiv aufbauen: auch sehr tiefe Pfade sprengen keinen Aufrufstapel.
  const virtual = tree.roots.length > 1;
  const rootData: H = virtual ? { id: Number.NaN, children: [] } : { id: tree.roots[0] as number, children: [] };
  const stack: [H, number][] = virtual ? tree.roots.map((r) => [rootData, r]) : [];
  if (!virtual) {
    const n = tree.get(rootData.id);
    if (n && !tree.collapsed.has(rootData.id)) for (const c of n.children) if (visible.has(c)) stack.push([rootData, c]);
  }
  // Reihenfolge der Kinder bleibt die Entdeckungsreihenfolge: Stapel in Vorwärtsrichtung abarbeiten.
  for (let i = 0; i < stack.length; i++) {
    const [parent, id] = stack[i] as [H, number];
    const h: H = { id, children: [] };
    parent.children.push(h);
    const n = tree.get(id);
    if (n && !tree.collapsed.has(id)) for (const c of n.children) if (visible.has(c)) stack.push([h, c]);
  }
  const root = hierarchy<H>(rootData, (d) => d.children);
  d3tree<H>().nodeSize([o.nodeGap, o.levelGap]).separation((a, b) => (a.parent === b.parent ? 1 : 1.5))(root);
  const shift = virtual ? o.levelGap : 0;
  root.each((d) => {
    if (Number.isNaN(d.data.id)) return;
    const breadth = d.x ?? 0;
    const depth = (d.y ?? 0) - shift;
    out.set(d.data.id, o.orientation === "lr" ? [depth, breadth] : [breadth, depth]);
  });
  return out;
}
