// Einstellungen der Seitenleiste (UI-090 … UI-096) mit den Standardwerten der Spec und Abbildung auf `viz.graph`.
import type { VizGraphArgs } from "../../ipc/gen/VizGraphArgs";
import { DEPTH_DEFAULTS, SIZE_DEFAULTS, type ColorMode, type EncodeOptions, type Palette } from "./model/encoding";
import { FORCE_DEFAULTS } from "./model/force";
import type { LabelMode } from "./model/scene";
import { TREE_LAYOUT_DEFAULTS, type Orientation } from "./model/treeLayout";
import { coreKind, layoutOf, type VizWindowKind } from "./open";

export const DEFAULT_BUDGET = 10_000;
export const MAX_BUDGET = 50_000;
/** Der Graph kennt kein Aufklappen: alle Ebenen bis zum Knotenbudget. */
export const FORCE_EXPAND_DEPTH = 1_000_000;

export interface VizOptions {
  orientation: Orientation;
  expandDepth: number;
  levelGap: number;
  nodeGap: number;
  labelMode: LabelMode;
  includeNonHtml: boolean;
  includeNonIndexable: boolean;
  palette: Palette;
  colorMode: ColorMode;
  metric: string;
  colorMetric: string;
  sizeByMetric: boolean;
  minRadius: number;
  maxRadius: number;
  budget: number;
  r0: number;
  f: number;
  repulsion: number;
  springLength: number;
  seed: number;
  allowOverlap: boolean;
  /** Beschriftung ab Zoomstufe (Graph, Standard 1,5×). */
  labelZoom: number;
  lineWidth: number;
}

export const DEFAULT_OPTIONS: VizOptions = {
  orientation: TREE_LAYOUT_DEFAULTS.orientation,
  expandDepth: 3,
  levelGap: TREE_LAYOUT_DEFAULTS.levelGap,
  nodeGap: TREE_LAYOUT_DEFAULTS.nodeGap,
  labelMode: "path",
  includeNonHtml: false,
  includeNonIndexable: true,
  palette: "standard",
  colorMode: "indexability",
  metric: "crawl_depth",
  colorMetric: "crawl_depth",
  sizeByMetric: false,
  minRadius: SIZE_DEFAULTS.minRadius,
  maxRadius: SIZE_DEFAULTS.maxRadius,
  budget: DEFAULT_BUDGET,
  r0: DEPTH_DEFAULTS.r0,
  f: DEPTH_DEFAULTS.f,
  repulsion: FORCE_DEFAULTS.repulsion,
  springLength: FORCE_DEFAULTS.springLength,
  seed: FORCE_DEFAULTS.seed,
  allowOverlap: FORCE_DEFAULTS.allowOverlap,
  labelZoom: 1.5,
  lineWidth: 1,
};

/** Baum: Beschriftungen ab dieser Zoomstufe (bei kleinerer Darstellung wären sie unlesbar). */
export const TREE_LABEL_ZOOM = 0.6;

export function clampBudget(v: number): number {
  if (!Number.isFinite(v)) return DEFAULT_BUDGET;
  return Math.min(MAX_BUDGET, Math.max(1, Math.round(v)));
}

export function encodeOptions(kind: VizWindowKind, o: VizOptions): EncodeOptions {
  return {
    layout: layoutOf(kind) === "force" ? "force" : "tree",
    colorMode: o.colorMode,
    palette: o.palette,
    sizeByMetric: o.sizeByMetric,
    minRadius: o.minRadius,
    maxRadius: Math.max(o.minRadius, o.maxRadius),
    r0: o.r0,
    f: o.f,
  };
}

export function graphArgs(kind: VizWindowKind, snapshotId: string, root: number | null, o: VizOptions, exploreNode: number | null = null): VizGraphArgs {
  const force = layoutOf(kind) === "force";
  return {
    snapshotId,
    kind: coreKind(kind),
    root,
    budget: clampBudget(o.budget),
    metric: o.metric,
    includeNonHtml: o.includeNonHtml,
    includeNonIndexable: o.includeNonIndexable,
    expandDepth: force ? FORCE_EXPAND_DEPTH : Math.max(0, Math.round(o.expandDepth)),
    exploreNode,
    colorMetric: o.colorMode === "metric" ? o.colorMetric : null,
  };
}
