// Tooltip (UI-090, UI-093, UI-096), Metrikauswahl (UI-095), „Teilbaum öffnen“-Entscheidung und Pfadleiste (UI-096),
// Nutzlast des CSV-Exports (UI-090).
import type { GraphEdge } from "../../../ipc/gen/GraphEdge";
import type { GraphNode } from "../../../ipc/gen/GraphNode";
import type { Scene, SceneNode } from "./scene";
import type { VizTree, VNode } from "./tree";
import type { Transform } from "./viewport";

export const MISSING = "–";

/** Feste Metriken in der Reihenfolge der Spec; Kennungen wie im Kern (`metric_key`). */
export const STANDARD_METRICS: readonly { key: string; name: string }[] = [
  { key: "crawl_depth", name: "Crawl Depth" },
  { key: "unique_inlinks", name: "Unique Inlinks" },
  { key: "link_score", name: "Link Score" },
  { key: "word_count", name: "Word Count" },
  { key: "response_time", name: "Response Time" },
  { key: "ga4.sessions", name: "GA Sessions" },
  { key: "gsc.clicks", name: "GSC Clicks" },
  { key: "gsc.impressions", name: "GSC Impressions" },
];

export interface MetricOption {
  key: string;
  name: string;
  /** Keine Daten im Schnappschuss (API nicht verbunden) → ausgegraut. */
  disabled: boolean;
}

/** Standardmetriken (ausgegraut ohne Daten) plus weitere numerische Spalten des Schnappschusses (Linkmetriken, Custom Extraction). */
export function metricOptions(available: readonly string[]): MetricOption[] {
  const have = new Set(available);
  const out: MetricOption[] = STANDARD_METRICS.map((m) => ({ ...m, disabled: !have.has(m.key) }));
  const known = new Set(STANDARD_METRICS.map((m) => m.key));
  for (const key of [...have].sort()) if (!known.has(key)) out.push({ key, name: key, disabled: false });
  return out;
}

export function metricName(key: string): string {
  return STANDARD_METRICS.find((m) => m.key === key)?.name ?? key;
}

export interface TooltipRow {
  name: string;
  value: string;
}

/**
 * Zeilen des Knoten-Tooltips: Address, Status Code, Indexability, Crawl Depth, Unique Inlinks, Link Score, gewählte
 * Skalierungsmetrik (Name + Wert); fehlende Werte „–“. Pfadknoten ohne URL zeigen nur ihren Pfad (UI-093).
 */
export function tooltipRows(n: VNode, path: string, metricKey: string, num: (v: number) => string): TooltipRow[] {
  const v = (x: number | null | undefined) => (x == null || !Number.isFinite(x) ? MISSING : num(x));
  const s = (x: string | null | undefined) => (x == null || x === "" ? MISSING : x);
  if (n.address == null) return [{ name: "Path", value: path }];
  return [
    { name: "Address", value: n.address },
    { name: "Status Code", value: v(n.statusCode) },
    { name: "Indexability", value: s(n.indexability) },
    { name: "Crawl Depth", value: v(n.depth) },
    { name: "Unique Inlinks", value: v(n.uniqueInlinks) },
    { name: "Link Score", value: v(n.linkScore) },
    { name: metricName(metricKey), value: v(n.metric) },
  ];
}

/** Pfad eines Knotens im Verzeichnisbaum („/a/b“) aus den Beschriftungen seiner Vorfahren unterhalb des Hosts. */
export function directoryPath(tree: VizTree, id: number): string {
  const labels = tree.pathTo(id).map((p) => tree.get(p)?.label ?? "");
  // Schema-Knoten („https“) und Host stehen vorne; der Rest ist der Pfad.
  const rest = labels.slice(2);
  return labels.length <= 2 ? labels.join("://") : `/${rest.join("/")}`;
}

/** Überschreitung, ab der „Teilbaum öffnen“ statt zu laden nach „Als Wurzel setzen“ fragt (UI-096). */
export const EXPLORE_OVERSHOOT = 1_000;

export function exploreDecision(hiddenChildren: number, shown: number, budget: number): "explore" | "confirm" | "none" {
  if (hiddenChildren <= 0) return "none";
  return shown + hiddenChildren > budget + EXPLORE_OVERSHOOT ? "confirm" : "explore";
}

export interface TrailEntry {
  /** `null` = Start-URL bzw. Wurzelverzeichnis. */
  id: number | null;
  label: string;
  /** Zoom und Position beim Verlassen dieser Wurzel. */
  view: Transform | null;
}

/** Pfadleiste „Start-URL › … › aktuelle Wurzel“: Rückkehr stellt Zoom und Position wieder her. */
export class RootTrail {
  constructor(readonly entries: readonly TrailEntry[]) {}

  static start(label: string): RootTrail {
    return new RootTrail([{ id: null, label, view: null }]);
  }

  get current(): TrailEntry {
    return this.entries[this.entries.length - 1] as TrailEntry;
  }

  /** Neue Wurzel; die aktuelle Ansicht wird am bisherigen Eintrag gemerkt. */
  focus(id: number, label: string, view: Transform): RootTrail {
    const head = this.entries.slice(0, -1);
    return new RootTrail([...head, { ...this.current, view }, { id, label, view: null }]);
  }

  /** Zurück zum Eintrag `index`; liefert neue Leiste und die zu restaurierende Ansicht. */
  back(index: number): { trail: RootTrail; view: Transform | null } {
    const i = Math.max(0, Math.min(index, this.entries.length - 1));
    const target = this.entries[i] as TrailEntry;
    return { trail: new RootTrail([...this.entries.slice(0, i), { ...target, view: null }]), view: target.view };
  }
}

/** Genau die dargestellten Knoten und Kanten mit aktuellen Koordinaten (UI-090: Zeilenzahl = angezeigte Knoten). */
export function csvPayload(tree: VizTree, scene: Scene): { nodes: GraphNode[]; edges: GraphEdge[] } {
  const nodes: GraphNode[] = scene.nodes.map((s: SceneNode) => {
    const { parent: _p, children: _c, level: _l, ...rest } = tree.get(s.id) as VNode;
    return { ...rest, x: s.x, y: s.y };
  });
  const edges: GraphEdge[] = scene.edges.map(([a, b]) => ({ source: (scene.nodes[a] as SceneNode).id, target: (scene.nodes[b] as SceneNode).id }));
  return { nodes, edges };
}
