// Darstellbare Szene: sichtbare Knoten mit Position und Stil, Kanten als Indexpaare. Grundlage für Canvas, Treffertest,
// SVG- und CSV-Export – alle Ausgaben zeigen dadurch genau dieselben Knoten (UI-090).
import { encodeStats, nodeStyle, type EncodeOptions, type EncodeStats } from "./encoding";
import type { VizTree } from "./tree";

export type LabelMode = "path" | "url";

export interface SceneNode {
  id: number;
  x: number;
  y: number;
  r: number;
  fill: string;
  stroke: string | null;
  dashed: boolean;
  hollow: boolean;
  capped: boolean;
  /** Badge-Zahl (ausgeblendete direkte Kinder), 0 = kein Badge. */
  badge: number;
  label: string;
}

export interface Scene {
  nodes: SceneNode[];
  index: Map<number, number>;
  edges: [number, number][];
  stats: EncodeStats;
}

export function buildScene(tree: VizTree, positions: Map<number, [number, number]>, enc: EncodeOptions, labelMode: LabelMode): Scene {
  const ids = tree.visible();
  const stats = encodeStats(ids.map((id) => tree.get(id)).filter((n) => n !== undefined));
  const nodes: SceneNode[] = [];
  const index = new Map<number, number>();
  for (const id of ids) {
    const n = tree.get(id);
    if (!n) continue;
    const capped = tree.isCapped(id);
    const st = nodeStyle(n, n.level, capped, enc, stats);
    const [x, y] = positions.get(id) ?? [0, 0];
    index.set(id, nodes.length);
    nodes.push({
      id,
      x,
      y,
      r: st.r,
      fill: st.fill,
      stroke: st.stroke,
      dashed: st.dashed,
      hollow: st.hollow,
      capped,
      badge: capped ? tree.hidden(id).children : 0,
      label: labelMode === "url" ? (n.address ?? n.label) : n.label,
    });
  }
  const edges: [number, number][] = [];
  for (const [s, t] of tree.visibleEdges()) {
    const a = index.get(s);
    const b = index.get(t);
    if (a !== undefined && b !== undefined) edges.push([a, b]);
  }
  return { nodes, index, edges, stats };
}

/** Positionen aus einem Layout-Feld (x0,y0,x1,y1 … in Reihenfolge `ids`) übernehmen. */
export function applyPositions(scene: Scene, ids: readonly number[], pos: Float64Array): void {
  for (let i = 0; i < ids.length; i++) {
    const k = scene.index.get(ids[i] as number);
    if (k === undefined) continue;
    const n = scene.nodes[k] as SceneNode;
    n.x = pos[i * 2] as number;
    n.y = pos[i * 2 + 1] as number;
  }
}

/** Raster für den Treffertest (Weltkoordinaten); wird bei Bedarf neu aufgebaut. */
export class HitIndex {
  private readonly cell: number;
  private readonly grid = new Map<string, number[]>();

  constructor(private readonly scene: Scene) {
    let max = 1;
    for (const n of scene.nodes) if (n.r > max) max = n.r;
    this.cell = Math.max(8, max * 2);
    scene.nodes.forEach((n, i) => {
      const k = `${Math.floor(n.x / this.cell)}:${Math.floor(n.y / this.cell)}`;
      const list = this.grid.get(k);
      if (list) list.push(i);
      else this.grid.set(k, [i]);
    });
  }

  /** Oberster Knoten unter dem Weltpunkt (Toleranz `slop` in Weltkoordinaten); später gezeichnete liegen oben. */
  hit(wx: number, wy: number, slop = 0): SceneNode | null {
    const gx = Math.floor(wx / this.cell);
    const gy = Math.floor(wy / this.cell);
    const reach = Math.ceil(slop / this.cell);
    let best = -1;
    for (let ox = -1 - reach; ox <= 1 + reach; ox++) {
      for (let oy = -1 - reach; oy <= 1 + reach; oy++) {
        for (const i of this.grid.get(`${gx + ox}:${gy + oy}`) ?? []) {
          const n = this.scene.nodes[i] as SceneNode;
          const dx = n.x - wx;
          const dy = n.y - wy;
          const r = n.r + slop;
          if (dx * dx + dy * dy <= r * r && i > best) best = i;
        }
      }
    }
    return best >= 0 ? (this.scene.nodes[best] as SceneNode) : null;
  }
}

export function sceneBounds(scene: Scene): { minX: number; minY: number; maxX: number; maxY: number } {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const n of scene.nodes) {
    minX = Math.min(minX, n.x - n.r);
    minY = Math.min(minY, n.y - n.r);
    maxX = Math.max(maxX, n.x + n.r);
    maxY = Math.max(maxY, n.y + n.r);
  }
  if (!Number.isFinite(minX)) return { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  return { minX, minY, maxX, maxY };
}
