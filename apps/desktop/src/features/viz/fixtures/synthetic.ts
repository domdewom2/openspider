// Synthetische Schnappschüsse für Tests (vitest und die E2E-Bridge in e2e/viz): bilden die Spec-Fixtures
// `fx-ui-basic`, `fx-ui-depth`, `fx-ui-dirs`, `fx-ui-words` und `gen-ui-large` auf Graph-Ebene nach, und
// `selectGraph` liefert daraus Antworten mit derselben Budget-/Aufklapp-Semantik wie `viz.graph` des Kerns
// (ebenenweise Auswahl bis zum Budget, Kinder in Entdeckungsreihenfolge, gekappte Knoten mit Zählern, UI-096).
import type { GraphDto } from "../../../ipc/gen/GraphDto";
import type { GraphNode } from "../../../ipc/gen/GraphNode";
import type { WordFreq } from "../../../ipc/gen/WordFreq";
import { mulberry32 } from "../model/force";

export interface SynthNode {
  id: number;
  label: string;
  address: string | null;
  depth: number;
  statusCode: number | null;
  indexability: string | null;
  uniqueInlinks: number | null;
  linkScore: number | null;
  segmentId: string | null;
  segmentColor: string | null;
  metrics: Record<string, number>;
}

/** Ein Hyperlink-Vorkommen für „Links eines Knotens“ (UI-097): Quelle, Ziel, follow, Linkposition. */
export type SynthLink = [source: number, target: number, follow: boolean, position: "Content" | "Navigation" | "Footer" | "Header"];

export interface SynthTree {
  nodes: Map<number, SynthNode>;
  children: Map<number, number[]>;
  roots: number[];
  links?: SynthLink[];
}

const HOST = "http://127.0.0.1:8000";

function page(id: number, path: string, depth: number, extra: Partial<SynthNode> = {}): SynthNode {
  return {
    id,
    label: path,
    address: `${HOST}${path}`,
    depth,
    statusCode: 200,
    uniqueInlinks: 1,
    linkScore: null,
    segmentId: null,
    segmentColor: null,
    ...extra,
    indexability: extra.indexability ?? "Indexable",
    metrics: { crawl_depth: depth, unique_inlinks: 1, ...(extra.metrics ?? {}) },
  };
}

function add(tree: SynthTree, n: SynthNode, parent: number | null): void {
  tree.nodes.set(n.id, n);
  if (parent === null) tree.roots.push(n.id);
  else {
    const list = tree.children.get(parent);
    if (list) list.push(n.id);
    else tree.children.set(parent, [n.id]);
  }
}

function empty(): SynthTree {
  return { nodes: new Map(), children: new Map(), roots: [] };
}

/** `fx-ui-basic`: Start-URL + 24 Unterseiten (eine noindex, ein 404, ein Redirect), Word Count 50 und 2 000 vertreten. */
export function fxUiBasic(): SynthTree {
  const t = empty();
  add(t, page(1, "/", 0, { uniqueInlinks: 24, linkScore: 100, metrics: { crawl_depth: 0, unique_inlinks: 24, word_count: 300, link_score: 100 } }), null);
  for (let i = 1; i <= 24; i++) {
    const id = i + 1;
    const extra: Partial<SynthNode> = { linkScore: 10 + i, metrics: { crawl_depth: 1, unique_inlinks: 1, word_count: i === 1 ? 50 : i === 2 ? 2000 : 100 + i * 10, link_score: 10 + i } };
    if (i === 3) extra.indexability = "Non-Indexable";
    if (i === 4) Object.assign(extra, { statusCode: 404, indexability: "Non-Indexable" });
    if (i === 5) Object.assign(extra, { statusCode: 301, indexability: "Non-Indexable" });
    if (i <= 6) Object.assign(extra, { segmentId: "0", segmentColor: "#7b61ff" });
    add(t, page(id, `/page-${i}.html`, 1, extra), 1);
  }
  return t;
}

/** `fx-ui-depth`: Kette von der Start-URL bis Tiefe 12 (je Ebene ein Seitenzweig). */
export function fxUiDepth(depth = 12): SynthTree {
  const t = empty();
  add(t, page(1, "/", 0), null);
  let parent = 1;
  for (let d = 1; d <= depth; d++) {
    const id = d * 2;
    add(t, page(id, `/level-${d}/`, d), parent);
    add(t, page(id + 1, `/level-${d}/side.html`, d), parent);
    parent = id;
  }
  return t;
}

/**
 * `fx-ui-dirs`: Verzeichnisbaum wie im Kern (Schema → Host = Start-URL → Pfadsegmente) mit zwei Verzeichnissen ohne
 * Indexseite (hohle Pfadknoten); die Query-Variante hängt als Kind an ihrer Seite.
 */
export function fxUiDirs(): SynthTree {
  const t = empty();
  const path = (id: number, label: string, depth: number): SynthNode => ({ ...page(id, label, depth), address: null, label, statusCode: null, indexability: null, uniqueInlinks: null, metrics: {} });
  add(t, path(-1, "http", 0), null);
  add(t, page(1, "/", 1, { label: "127.0.0.1:8000" }), -1);
  add(t, path(-2, "docs", 2), 1);
  add(t, page(2, "/docs/a.html", 3, { label: "a.html" }), -2);
  add(t, page(3, "/docs/b.html", 3, { label: "b.html" }), -2);
  add(t, path(-3, "blog", 2), 1);
  add(t, page(4, "/blog/post", 3, { label: "post" }), -3);
  add(t, page(5, "/blog/post?page=2", 4, { label: "post?page=2" }), 4);
  return t;
}

/**
 * `gen-ui-large`: Ebenen mit den angegebenen Knotenzahlen (Standard 1 / 40 / 2 000 / 47 959), Kinder gleichmäßig
 * in Entdeckungsreihenfolge auf die Eltern verteilt; Eigenschaften deterministisch aus dem Startwert.
 */
export function genUiLarge(levels: readonly number[] = [1, 40, 2000, 47959], seed = 7): SynthTree {
  const t = empty();
  const rnd = mulberry32(seed);
  let id = 1;
  let previous: number[] = [];
  levels.forEach((count, depth) => {
    const current: number[] = [];
    for (let i = 0; i < count; i++) {
      const nid = id++;
      const parent = depth === 0 ? null : (previous[Math.floor((i * previous.length) / count)] as number);
      const r = rnd();
      const indexable = r > 0.12;
      add(
        t,
        page(nid, depth === 0 ? "/" : `/l${depth}/p${nid}.html`, depth, {
          indexability: indexable ? "Indexable" : "Non-Indexable",
          statusCode: r < 0.03 ? 404 : r < 0.06 ? 301 : 200,
          uniqueInlinks: 1 + Math.floor(rnd() * 50),
          linkScore: 1 + Math.floor(rnd() * 99),
          metrics: { crawl_depth: depth, word_count: 50 + Math.floor(rnd() * 2000) },
        }),
        parent,
      );
      current.push(nid);
    }
    previous = current;
  });
  return t;
}

/**
 * `fx-ui-words` auf Graph-Ebene (wie `tests/fixtures/ui/fx-ui-words`): Zielseite (Kennung 2) mit 30 Inlinks von 23
 * Quellseiten; ohne nofollow 18, ohne Navigation/Footer 22, beides 17; 7 eindeutige Outlinks; Kinder `deep-1`/`deep-2`.
 */
export function fxUiWordsGraph(): SynthTree {
  const t = empty();
  const pages = 22;
  add(t, page(1, "/", 0, { uniqueInlinks: 2 }), null);
  add(t, page(2, "/target.html", 1, { uniqueInlinks: 23 }), 1);
  for (let i = 1; i <= pages; i++) add(t, page(2 + i, `/p${i}.html`, 1, { uniqueInlinks: i <= 3 ? 2 : 1 }), 1);
  const deep1 = 3 + pages;
  const deep2 = deep1 + 1;
  add(t, page(deep1, "/deep-1.html", 2, { uniqueInlinks: 2 }), 2);
  add(t, page(deep2, "/deep-2.html", 2, { uniqueInlinks: 1 }), 2);
  const p = (i: number) => 2 + i;
  const links: SynthLink[] = [[1, 2, true, "Navigation"], [1, 2, true, "Content"]];
  for (let i = 1; i <= pages; i++) links.push([1, p(i), true, "Content"]);
  for (let i = 1; i <= 20; i++) links.push([p(i), 2, i < 16, "Content"]);
  for (let i = 1; i <= 6; i++) links.push([p(i), 2, true, "Footer"]);
  links.push([p(21), 2, true, "Footer"], [2, 2, true, "Content"]);
  links.push([2, 1, true, "Header"], [2, p(1), true, "Navigation"], [2, deep1, true, "Content"], [2, deep2, true, "Content"], [2, p(2), true, "Content"], [2, p(3), true, "Footer"]);
  links.push([p(1), deep1, true, "Content"], [deep1, 1, true, "Content"], [deep2, 1, true, "Content"]);
  t.links = links;
  return t;
}

/** Eindeutige Links eines Knotens bzw. seiner Kinder wie `Snapshot::link_overlay` im Kern. */
export function selectLinks(tree: SynthTree, node: number, mode: string, includeNofollow: boolean, excludeNavFooter: boolean): [number, number][] {
  const focus = mode === "childInlinks" || mode === "childOutlinks" ? (tree.children.get(node) ?? []) : [node];
  const inbound = mode === "inlinks" || mode === "childInlinks";
  const pairs = new Map<string, [number, number]>();
  for (const [s, tg, follow, position] of tree.links ?? []) {
    if (!includeNofollow && !follow) continue;
    if (excludeNavFooter && (position === "Navigation" || position === "Footer")) continue;
    if (inbound ? !focus.includes(tg) : !focus.includes(s)) continue;
    pairs.set(`${s}:${tg}`, [s, tg]);
  }
  return [...pairs.values()].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
}

/** Body-Text der Zielseite von `fx-ui-words` mit Standardoptionen (UI-100, wie `expected.json`). */
export function fxUiBodyWords(): WordFreq[] {
  return [
    ["pricing", 6], ["plan", 3], ["plans", 3], ["every", 2], ["includes", 2], ["budget", 1], ["choose", 1], ["compare", 1], ["comparison", 1],
    ["details", 1], ["fair", 1], ["fits", 1], ["more", 1], ["simple", 1], ["support", 1], ["team", 1], ["updates", 1],
  ].map(([word, count]) => ({ word: word as string, count: count as number }));
}

export interface SelectOptions {
  root?: number | null;
  budget: number;
  expandDepth: number;
  explore?: readonly number[];
  metric: string;
  colorMetric?: string | null;
  availableMetrics?: readonly string[];
}

/** Antwort wie `viz.graph`: ebenenweise bis zum Budget, danach gekappt; „Teilbaum öffnen“ hängt Kinder an (bis +1 000). */
export function selectGraph(tree: SynthTree, o: SelectOptions): GraphDto {
  const roots = o.root != null ? [o.root] : tree.roots;
  const kids = (id: number) => tree.children.get(id) ?? [];
  const descendants = new Map<number, number>();
  const order: number[] = [];
  const q0 = [...roots];
  for (let i = 0; i < q0.length; i++) {
    order.push(q0[i] as number);
    q0.push(...kids(q0[i] as number));
  }
  for (let i = order.length - 1; i >= 0; i--) {
    const id = order[i] as number;
    descendants.set(id, kids(id).reduce((s, c) => s + 1 + (descendants.get(c) ?? 0), 0));
  }
  const visible: number[] = [];
  const selected = new Set<number>();
  const queue: [number, number][] = roots.map((r) => [r, 0]);
  for (let i = 0; i < queue.length && visible.length < o.budget; i++) {
    const [id, depth] = queue[i] as [number, number];
    selected.add(id);
    visible.push(id);
    if (depth < o.expandDepth) for (const c of kids(id)) queue.push([c, depth + 1]);
  }
  for (const e of o.explore ?? []) {
    if (!selected.has(e)) throw new Error("explore node must be visible");
    const extra = kids(e).filter((c) => !selected.has(c));
    if (visible.length + extra.length > o.budget + 1000) throw new Error("explore exceeds budget by more than 1000; focus this node instead");
    for (const c of extra) {
      selected.add(c);
      visible.push(c);
    }
  }
  const visDesc = new Map<number, number>();
  for (let i = order.length - 1; i >= 0; i--) {
    const id = order[i] as number;
    visDesc.set(id, kids(id).filter((c) => selected.has(c)).reduce((s, c) => s + 1 + (visDesc.get(c) ?? 0), 0));
  }
  const nodes: GraphNode[] = [];
  const edges: GraphDto["edges"] = [];
  for (const id of visible) {
    const n = tree.nodes.get(id) as SynthNode;
    let hiddenChildren = 0;
    for (const c of kids(id)) {
      if (selected.has(c)) edges.push({ source: id, target: c });
      else hiddenChildren++;
    }
    nodes.push({
      id,
      label: n.label,
      address: n.address,
      depth: n.depth,
      metric: n.metrics[o.metric] ?? null,
      colorValue: o.colorMetric ? (n.metrics[o.colorMetric] ?? null) : null,
      x: null,
      y: null,
      hasMore: hiddenChildren > 0,
      statusCode: n.statusCode,
      indexability: n.indexability,
      uniqueInlinks: n.uniqueInlinks,
      linkScore: n.linkScore,
      segmentId: n.segmentId,
      segmentColor: n.segmentColor,
      hiddenChildren,
      hiddenDescendants: (descendants.get(id) ?? 0) - (visDesc.get(id) ?? 0),
    });
  }
  const available = o.availableMetrics ?? [...new Set([...tree.nodes.values()].flatMap((n) => Object.keys(n.metrics)))].sort();
  return { provenanceMissing: false, availableMetrics: [...available], nodes, edges, truncated: selected.size < order.length };
}

/** `fx-ui-words`: Ankertexte auf die Zielseite (30 Inlinks). */
export function fxUiWords(): WordFreq[] {
  return [
    { word: "pricing", count: 12 },
    { word: "see our prices", count: 8 },
    { word: "plans", count: 5 },
    { word: "compare plans", count: 3 },
    { word: "costs", count: 1 },
    { word: "image: price table", count: 1 },
  ];
}
