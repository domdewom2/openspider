// Links eines Knotens einblenden (UI-097): Anfrage an `viz.graph` (optionale Linkangaben), Auswertung der
// zusätzlichen Kanten (`edgeType` inlink/outlink), verbundene Knoten für Hervorhebung/Ausgrauen und die Einträge des
// Knoten-Kontextmenüs. Gemeinsam für 2D (GraphView) und 3D (Graph3DView).
import type { GraphDto } from "../../../ipc/gen/GraphDto";
import type { GraphEdge } from "../../../ipc/gen/GraphEdge";
import type { VizGraphArgs } from "../../../ipc/gen/VizGraphArgs";
import type { MenuEntry } from "../../../ui/primitives";
import type { VizTree } from "./tree";

export type LinkMode = "inlinks" | "childInlinks" | "outlinks" | "childOutlinks";
export const LINK_MODES: readonly LinkMode[] = ["inlinks", "childInlinks", "outlinks", "childOutlinks"];
export type LinkEdgeType = "inlink" | "outlink";

export interface LinkRequest {
  node: number;
  mode: LinkMode;
}

export interface OverlayEdge {
  source: number;
  target: number;
  type: LinkEdgeType;
}

export interface LinkOverlay {
  request: LinkRequest;
  edges: OverlayEdge[];
  /** Eindeutige Links gemäß Optionen (auch zu nicht dargestellten Knoten). */
  total: number;
  /** Hervorgehobene Knoten: Ausgangsknoten bzw. seine Kinder und alle Gegenstellen. */
  connected: Set<number>;
}

export interface LinkOptions {
  linkNofollow: boolean;
  linkExcludeNavFooter: boolean;
}

/** `viz.graph`-Argumente mit Linkangabe; ohne Anfrage unverändert (nur Baumkanten). */
export function withLinks(args: VizGraphArgs, req: LinkRequest | null, o: LinkOptions): VizGraphArgs {
  if (!req) return args;
  return { ...args, linksNode: req.node, linksMode: req.mode, linksIncludeNofollow: o.linkNofollow, linksExcludeNavFooter: o.linkExcludeNavFooter };
}

export function isLinkEdge(e: GraphEdge): boolean {
  return e.edgeType === "inlink" || e.edgeType === "outlink";
}

/** Nur die Baumkanten (für den Strukturvergleich und das Baummodell). */
export function treeEdges(dto: GraphDto): GraphEdge[] {
  return dto.edges.some(isLinkEdge) ? dto.edges.filter((e) => !isLinkEdge(e)) : dto.edges;
}

/** Ob zwei Antworten dieselbe Struktur haben (gleiche Knoten in gleicher Reihenfolge, gleiche Baumkanten). */
export function sameStructure(a: GraphDto, b: GraphDto): boolean {
  if (a.nodes.length !== b.nodes.length || a.truncated !== b.truncated) return false;
  for (let i = 0; i < a.nodes.length; i++) {
    const x = a.nodes[i] as GraphDto["nodes"][number];
    const y = b.nodes[i] as GraphDto["nodes"][number];
    if (x.id !== y.id || x.hiddenChildren !== y.hiddenChildren || x.metric !== y.metric || x.colorValue !== y.colorValue) return false;
  }
  const ea = treeEdges(a);
  const eb = treeEdges(b);
  if (ea.length !== eb.length) return false;
  for (let i = 0; i < ea.length; i++) {
    const x = ea[i] as GraphEdge;
    const y = eb[i] as GraphEdge;
    if (x.source !== y.source || x.target !== y.target) return false;
  }
  return true;
}

/** Ausgangsknoten der Anfrage: der Knoten selbst bzw. seine direkten Kinder (im geladenen Baum). */
export function focusNodes(req: LinkRequest, tree: VizTree | null): number[] {
  if (req.mode === "inlinks" || req.mode === "outlinks") return [req.node];
  return tree?.get(req.node)?.children ?? [];
}

/** Zusatzkanten und hervorgehobene Knoten aus einer Antwort mit Linkangabe. */
export function overlayFrom(dto: GraphDto, req: LinkRequest, tree: VizTree | null): LinkOverlay {
  const edges: OverlayEdge[] = [];
  const connected = new Set<number>(req.mode === "inlinks" || req.mode === "outlinks" ? [req.node] : [req.node, ...focusNodes(req, tree)]);
  for (const e of dto.edges) {
    if (!isLinkEdge(e)) continue;
    edges.push({ source: e.source, target: e.target, type: e.edgeType as LinkEdgeType });
    connected.add(e.source);
    connected.add(e.target);
  }
  return { request: req, edges, total: dto.linkTotal ?? edges.length, connected };
}

/** Kanten für den CSV-Export (UI-090: `Edge Type` tree/inlink/outlink). */
export function overlayCsvEdges(overlay: LinkOverlay | null): GraphEdge[] {
  return overlay ? overlay.edges.map((e) => ({ source: e.source, target: e.target, edgeType: e.type })) : [];
}

type T = (key: string, args?: Record<string, string | number>) => string;

/**
 * Kontextmenü-Einträge (UI-097): „Inlinks zeigen“, „Inlinks auf Kinder zeigen“, „Outlinks zeigen“, „Outlinks von Kindern
 * zeigen“, „Zurücksetzen“. Pfadknoten ohne URL haben keine eigenen Links; Knoten ohne Kinder keine Kinder-Links.
 */
export function linkMenuEntries(t: T, node: { id: number; address?: string | null; children: readonly number[]; hiddenChildren: number } | undefined, active: LinkOverlay | null, show: (req: LinkRequest) => void, reset: () => void): MenuEntry[] {
  if (!node) return [];
  const own = node.address != null;
  const kids = node.children.length + node.hiddenChildren > 0;
  const entry = (mode: LinkMode, first = false): MenuEntry => {
    const needsOwn = mode === "inlinks" || mode === "outlinks";
    const ok = needsOwn ? own : kids;
    return {
      id: `viz-links-${mode}`,
      label: t(`viz-menu-links-${mode}`),
      separatorBefore: first,
      isDisabled: !ok,
      disabledReason: ok ? undefined : t(needsOwn ? "viz-reason-path-node" : "viz-reason-no-children"),
      onAction: () => show({ node: node.id, mode }),
    };
  };
  return [
    entry("inlinks", true),
    entry("childInlinks"),
    entry("outlinks"),
    entry("childOutlinks"),
    { id: "viz-links-reset", label: t("viz-menu-links-reset"), isDisabled: !active, disabledReason: active ? undefined : t("viz-reason-no-links"), onAction: reset },
  ];
}
