// Clientseitiges Modell der geladenen Struktur (Baum: jede URL genau einmal, UI-091): Eltern/Kinder in
// Entdeckungsreihenfolge, eingeklappte Knoten, sichtbare Menge. Budget und Kappung entscheidet der Kern (UI-096).
import type { GraphDto } from "../../../ipc/gen/GraphDto";
import type { GraphNode } from "../../../ipc/gen/GraphNode";

export interface VNode extends GraphNode {
  parent: number | null;
  children: number[];
  /** Tiefe relativ zur Wurzel der Ansicht (Wurzeln = 0). */
  level: number;
}

export interface HiddenCount {
  children: number;
  descendants: number;
}

export class VizTree {
  readonly nodes = new Map<number, VNode>();
  readonly roots: number[] = [];
  /** Alle geladenen Knoten in Breitensuche-Reihenfolge. */
  readonly order: number[] = [];
  /** Im Client eingeklappte Knoten (Klick auf einen aufgeklappten Knoten). */
  readonly collapsed = new Set<number>();
  private visibleCache: number[] | null = null;

  static from(dto: GraphDto): VizTree {
    const tree = new VizTree();
    for (const n of dto.nodes) tree.nodes.set(n.id, { ...n, parent: null, children: [], level: 0 });
    for (const e of dto.edges) {
      const p = tree.nodes.get(e.source);
      const c = tree.nodes.get(e.target);
      // Ein Baum: jeder Knoten hat höchstens einen Elternknoten, Doppelkanten werden verworfen.
      if (!p || !c || c.parent !== null || e.source === e.target) continue;
      c.parent = p.id;
      p.children.push(c.id);
    }
    for (const n of dto.nodes) {
      const v = tree.nodes.get(n.id) as VNode;
      if (v.parent === null) tree.roots.push(v.id);
    }
    const queue = [...tree.roots];
    for (let i = 0; i < queue.length; i++) {
      const v = tree.nodes.get(queue[i] as number) as VNode;
      tree.order.push(v.id);
      for (const c of v.children) {
        (tree.nodes.get(c) as VNode).level = v.level + 1;
        queue.push(c);
      }
    }
    return tree;
  }

  get size(): number {
    return this.nodes.size;
  }

  get(id: number): VNode | undefined {
    return this.nodes.get(id);
  }

  private invalidate(): void {
    this.visibleCache = null;
  }

  /** Sichtbare Knoten (Breitensuche, Nachfahren eingeklappter Knoten ausgelassen). */
  visible(): number[] {
    if (this.visibleCache) return this.visibleCache;
    const out: number[] = [];
    const queue = [...this.roots];
    for (let i = 0; i < queue.length; i++) {
      const id = queue[i] as number;
      out.push(id);
      if (this.collapsed.has(id)) continue;
      for (const c of (this.nodes.get(id) as VNode).children) queue.push(c);
    }
    this.visibleCache = out;
    return out;
  }

  /** Sichtbare Baumkanten (Eltern → Kind). */
  visibleEdges(): [number, number][] {
    const out: [number, number][] = [];
    for (const id of this.visible()) {
      const v = this.nodes.get(id) as VNode;
      if (v.parent !== null) out.push([v.parent, id]);
    }
    return out;
  }

  /** Knoten mit ausgeblendeten Kindern (vom Kern gekappt oder im Client eingeklappt). */
  isCapped(id: number): boolean {
    const v = this.nodes.get(id);
    if (!v) return false;
    return v.hiddenChildren > 0 || (this.collapsed.has(id) && v.children.length > 0);
  }

  /** Ausgeblendete direkte Kinder und Nachfahren (für Badge und Tooltip, UI-096). */
  hidden(id: number): HiddenCount {
    const v = this.nodes.get(id);
    if (!v) return { children: 0, descendants: 0 };
    if (!this.collapsed.has(id)) return { children: v.hiddenChildren, descendants: v.hiddenDescendants };
    let descendants = 0;
    const stack = [...v.children];
    while (stack.length) {
      const c = this.nodes.get(stack.pop() as number) as VNode;
      descendants += 1 + c.hiddenDescendants;
      stack.push(...c.children);
    }
    return { children: v.children.length + v.hiddenChildren, descendants: descendants + v.hiddenDescendants };
  }

  /**
   * Klick auf einen Knoten im Baum: geladene Kinder ein-/ausklappen; fehlen sie, muss der Kern sie nachladen
   * (`explore` = „Teilbaum öffnen“). Blätter ohne ausgeblendete Kinder bleiben unverändert (`none`).
   */
  toggle(id: number): "collapsed" | "expanded" | "explore" | "none" {
    const v = this.nodes.get(id);
    if (!v) return "none";
    if (this.collapsed.has(id)) {
      this.collapsed.delete(id);
      this.invalidate();
      return "expanded";
    }
    if (v.children.length > 0) {
      this.collapsed.add(id);
      this.invalidate();
      return "collapsed";
    }
    return v.hiddenChildren > 0 ? "explore" : "none";
  }

  /** Alles Geladene aufklappen. */
  expandAll(): void {
    this.collapsed.clear();
    this.invalidate();
  }

  /** Pfad Wurzel → Knoten (Knotenkennungen). */
  pathTo(id: number): number[] {
    const out: number[] = [];
    let cur = this.nodes.get(id);
    while (cur) {
      out.push(cur.id);
      cur = cur.parent === null ? undefined : this.nodes.get(cur.parent);
    }
    return out.reverse();
  }

  /** Öffnet alle eingeklappten Vorfahren (Suche, UI-090). Liefert `true`, wenn sich etwas geändert hat. */
  reveal(id: number): boolean {
    let changed = false;
    for (const p of this.pathTo(id).slice(0, -1)) {
      if (this.collapsed.delete(p)) changed = true;
    }
    if (changed) this.invalidate();
    return changed;
  }

  /** Ob der Client gerade weniger zeigt, als der Kern geliefert hat (CSV-Export verlangt den gelieferten Stand). */
  hasClientCollapse(): boolean {
    return this.visible().length !== this.nodes.size;
  }
}
