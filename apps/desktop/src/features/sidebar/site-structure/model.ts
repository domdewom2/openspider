import type { DepthRow } from "../../../ipc/gen/DepthRow";
import type { PathNode } from "../../../ipc/gen/PathNode";
import type { RuleNode } from "../../../ipc/gen/RuleNode";
import type { SearchSpec } from "../../../ipc/gen/SearchSpec";
import type { SiteStructureView } from "../../../ipc/gen/SiteStructureView";

/** Class keys as delivered by the core (`os_analyze::aggregates`), in display order per view. */
export const INDEXABILITY_CLASSES = ["Indexable", "Non-Indexable"] as const;
export const RESPONSE_CLASSES = ["2xx", "3xx", "4xx", "5xx", "Blocked by robots.txt", "No Response"] as const;

export function classesOf(view: SiteStructureView): readonly string[] {
  return view === "indexability" ? INDEXABILITY_CLASSES : view === "responseCodes" ? RESPONSE_CLASSES : [];
}

/** UI-007 status colours; blocked/no-response are both neutral grey, told apart by a hatch pattern in CSS. */
export const CLASS_COLOR: Record<string, string> = {
  Indexable: "var(--os-status-2xx)",
  "Non-Indexable": "var(--os-status-4xx)",
  "2xx": "var(--os-status-2xx)",
  "3xx": "var(--os-status-3xx)",
  "4xx": "var(--os-status-4xx)",
  "5xx": "var(--os-status-5xx)",
  "Blocked by robots.txt": "var(--os-status-none)",
  "No Response": "var(--os-status-none)",
};
export const DEFAULT_COLOR = "var(--os-accent)";

/** `% of Total` with one decimal; the shares of a table sum to 100 % ± 0.1 (UI-066). */
export function pct(count: number, total: number): number {
  return total > 0 ? Math.round((count / total) * 1000) / 10 : 0;
}

export const depthLabel = (depth: number): string => (depth >= 10 ? "10+" : String(depth));

export interface DepthViewRow {
  depth: number;
  label: string;
  count: number;
  pct: number;
  parts: { key: string; value: number }[];
}

/** Rows `1 … 9, 10+`; the start URL (depth 0) appears in no row (UI-066). */
export function depthRows(depths: readonly DepthRow[], view: SiteStructureView): DepthViewRow[] {
  const merged = new Map<number, DepthRow>();
  for (const d of depths) {
    if (d.depth < 1) continue;
    const key = Math.min(d.depth, 10);
    const cur = merged.get(key);
    if (!cur) merged.set(key, { ...d, depth: key, byClass: { ...d.byClass } });
    else {
      cur.count += d.count;
      for (const [k, v] of Object.entries(d.byClass)) cur.byClass[k] = (cur.byClass[k] ?? 0) + v;
    }
  }
  const total = [...merged.values()].reduce((s, d) => s + d.count, 0);
  return [...merged.values()].sort((a, b) => a.depth - b.depth).map(d => ({
    depth: d.depth,
    label: depthLabel(d.depth),
    count: d.count,
    pct: pct(d.count, total),
    parts: view === "crawlDepth" ? [{ key: "all", value: d.count }] : classesOf(view).map(key => ({ key, value: d.byClass[key] ?? 0 })),
  }));
}

export interface PathRow { node: PathNode; level: number; expanded: boolean; expandable: boolean; more: boolean }

/** Children fetched lazily per directory (`path` argument of `sidebar.siteStructure`). */
export interface LoadedChildren { children: Readonly<Record<string, readonly PathNode[]>>; complete: ReadonlySet<string> }
export const NOTHING_LOADED: LoadedChildren = { children: {}, complete: new Set() };

export function mergeChildren(node: PathNode, loaded: LoadedChildren): readonly PathNode[] {
  const seen = new Set(node.children.map(c => c.path));
  return [...node.children, ...(loaded.children[node.path] ?? []).filter(c => !seen.has(c.path))];
}

/** Flattens the tree; the first three levels are open unless the user toggled them (UI-066). */
export function flattenPaths(nodes: readonly PathNode[], toggled: ReadonlySet<string>, loaded: LoadedChildren, level = 1, out: PathRow[] = []): PathRow[] {
  for (const node of nodes) {
    const children = mergeChildren(node, loaded);
    const more = node.hasMoreChildren && !loaded.complete.has(node.path);
    const expandable = children.length > 0 || more;
    const expanded = expandable && (level <= 3) !== toggled.has(node.path);
    out.push({ node, level, expanded, expandable, more });
    if (expanded) flattenPaths(children, toggled, loaded, level + 1, out);
  }
  return out;
}

type Cond = Extract<RuleNode, { kind: "cond" }>;
const cond = (column: string, op: Cond["op"], value: string | null): RuleNode => ({ kind: "cond", column, op, value });
export const andRules = (...children: RuleNode[]): SearchSpec => ({ kind: "rules", tree: { kind: "group", op: "and", children }, caseSensitive: true });
export { cond };

/** Click on a path: `Address` begins with the directory URL (Tab Internal). */
export function pathSearch(path: string): SearchSpec { return andRules(cond("Address", "startsWith", path)); }

const STATUS_RANGE: Record<string, [number, number]> = { "2xx": [199, 300], "3xx": [299, 400], "4xx": [399, 500], "5xx": [499, 600] };

function classCondition(view: SiteStructureView, cls: string): RuleNode[] {
  if (view === "indexability") return [cond("Indexability", "equals", cls)];
  const range = STATUS_RANGE[cls];
  if (range) return [cond("Status Code", "greaterThan", String(range[0])), cond("Status Code", "lessThan", String(range[1]))];
  if (cls === "Blocked by robots.txt") return [cond("Indexability Status", "startsWith", "Blocked by Robots.txt")];
  return [cond("Status Code", "isEmpty", null)];
}

/** Click on a depth bar: `Crawl Depth = n` (`10+` → greater than 9) plus the clicked class. */
export function depthSearch(depth: number, view: SiteStructureView, cls?: string): SearchSpec {
  const children = [depth >= 10 ? cond("Crawl Depth", "greaterThan", "9") : cond("Crawl Depth", "equals", String(depth))];
  if (cls && view !== "crawlDepth") children.push(...classCondition(view, cls));
  return andRules(...children);
}

/** Tab Internal, filter All: the table the sidebar aggregates describe. */
export const TABLE_TARGET = { tab: "Internal", filter: "All" } as const;
