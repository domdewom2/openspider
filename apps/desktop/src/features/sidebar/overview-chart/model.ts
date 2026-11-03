// UI-102: chart model of the selected Overview entry. Pure functions over the loaded `sidebar.overview` answer: no new
// count, no IPC. Which filters of a tab form a partition comes from the core (`OverviewTab.partition`).
import { filterLabel, tabLabel } from "../../../i18n/canon";
import type { TFn } from "../../../i18n";
import type { FilterInfo } from "../../../ipc/gen/FilterInfo";
import type { FilterStateDto } from "../../../ipc/gen/FilterStateDto";
import type { IssueBadge } from "../../../ipc/gen/IssueBadge";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import type { OverviewTab } from "../../../ipc/gen/OverviewTab";
import type { SearchSpec } from "../../../ipc/gen/SearchSpec";
import type { SummaryRow } from "../../../ipc/gen/SummaryRow";
import { THEME_TOKENS, type TokenName } from "../../../ui/tokens/tokens";
import { hasCount } from "../analysis/FilterStatus";
import { SUMMARY_KEYS, type EntryRef } from "../overview/selection";

/** Ring up to this many categories with a count > 0 (including residual categories). */
export const RING_MAX = 8;
/** Bars of a partition: at most this many, the last one bundling the rest as „Weitere“. */
export const BAR_MAX = 10;

export type CategoryRole = "filter" | "summary" | "unassigned" | "hidden" | "others";
export type ChartKind = "ring" | "bars" | "overlap";

/** Table target of a category (UI-060): tab + filter, optionally with a condition. */
export interface NavTarget { tab: string; filter: string; search?: SearchSpec }

export interface ChartCategory {
  key: string;
  role: CategoryRole;
  /** Display name in the UI language. */
  label: string;
  /** Canonical English name (CSV export). */
  canonical: string;
  /** Filter id for filter categories; summary categories name their table filter. */
  filter: string | null;
  /** `null` = no counter (prerequisite missing, analysis pending; UI-006). */
  count: number | null;
  /** Share of the base in percent, same rounding as `% of Total` of the Overview. */
  pct: number | null;
  state: FilterStateDto | null;
  issue: IssueBadge | null;
  color: TokenName;
  /** 0 = solid; 1, 2 … = hatch patterns for the 2nd, 3rd … category sharing a colour. */
  hatch: number;
  nav: NavTarget | null;
  /** „Weitere“: the bundled categories. */
  members?: ChartCategory[];
}

export interface ChartHeader {
  entry: EntryRef;
  /** Canonical entry name (`Summary` or tab id). */
  name: string;
  label: string;
  base: number;
}

export type ChartModel =
  | { kind: "missing" }
  | { kind: "zero"; header: ChartHeader }
  | { kind: "noCounts"; header: ChartHeader; categories: ChartCategory[] }
  | {
    kind: "chart";
    header: ChartHeader;
    chart: ChartKind;
    /** Drawn segments/bars in drawing order (descending; „Weitere“ last). */
    drawn: ChartCategory[];
    /** Legend after the drawn categories: count 0 (muted), then without counter („–“). Overlap: only without counter. */
    tail: ChartCategory[];
    /** Overlap only: filters with 0 matches, collapsed into „n Filter ohne Treffer“. */
    zero: ChartCategory[];
    /** Table and CSV: every category individually, in legend order without bundling. */
    table: ChartCategory[];
    /** Category sum larger than the base (error case): bars without „Weitere“, warning in the log. */
    overflow: boolean;
  };

/** Same integer rounding as the core (`os_analyze::views::counts::percent`): tenths, halves up. */
export function pctOf(count: number, base: number): number {
  return base > 0 ? Math.floor((count * 2000 + base) / (2 * base)) / 10 : 0;
}

const STATUS_CLASS = /\(([1-5])XX\)$/;

/**
 * Response Codes: semantic status colours (orchestrator decision 03.11.2026): 2XX a calm green (`diff.add`, not the
 * near-black text colour of `status.2xx`), 3XX amber, 4XX/5XX the status reds (same base colour, 5XX hatched as UI-102
 * requires), classes without status (blocked, no response, other) neutral grey.
 */
function statusColor(filter: string): TokenName {
  const cls = STATUS_CLASS.exec(filter)?.[1];
  if (cls === "2") return "diff.add";
  return cls === "3" || cls === "4" || cls === "5" ? `status.${cls}xx` as TokenName : "status.none";
}

const PALETTE: TokenName[] = ["chart.1", "chart.2", "chart.3", "chart.4", "chart.5", "chart.6", "chart.7", "chart.8"];
const PRIORITY: Record<IssueBadge["priority"], TokenName> = { high: "issue.prio.high", medium: "issue.prio.medium", low: "issue.prio.low" };

/** Each further category of the same colour (value in both themes, e.g. 4XX and 5XX) gets the next hatch pattern, in
 * catalog order so it stays stable during live updates. */
function assignHatches(categories: ChartCategory[]): void {
  const seen = new Map<string, number>();
  for (const c of categories) {
    const colour = `${THEME_TOKENS.light[c.color]}|${THEME_TOKENS.dark[c.color]}`.toLowerCase();
    const n = seen.get(colour) ?? 0;
    c.hatch = n;
    seen.set(colour, n + 1);
  }
}

const counted = (f: FilterInfo) => hasCount(f.state) && f.count != null;

function filterCategory(tab: OverviewTab, f: FilterInfo, t: TFn, base: number, color: TokenName): ChartCategory {
  const count = counted(f) ? f.count! : null;
  return {
    key: `f:${f.id}`, role: "filter", label: filterLabel(t, f), canonical: f.id, filter: f.id, count,
    pct: count == null ? null : f.pct ?? pctOf(count, base), state: f.state, issue: f.issue ?? null, color, hatch: 0,
    nav: { tab: tab.tab, filter: f.id },
  };
}

function residual(role: "unassigned" | "hidden", count: number, base: number, t: TFn, pct?: number): ChartCategory {
  return {
    key: role, role, label: t(`ovc-cat-${role}`), canonical: role === "unassigned" ? "Unassigned" : "Not Displayed", filter: null,
    count, pct: pct ?? pctOf(count, base), state: null, issue: null, color: "chart.rest", hatch: 0, nav: null,
  };
}

const and = (column: string, op: "equals" | "isEmpty", value: string | null): SearchSpec => ({
  kind: "rules", caseSensitive: false, tree: { kind: "group", op: "and", children: [{ kind: "cond", column, op, value }] },
});

/** UI-102 summary partition from the summary metrics; percentages exactly like the Overview's summary rows. */
function summaryCategories(summary: readonly SummaryRow[], t: TFn): { base: number; categories: ChartCategory[]; consistent: boolean } {
  const value = (i: number) => summary.find(r => r.key === SUMMARY_KEYS[i])?.count ?? 0;
  const [encountered, displayed, internal, external, indexable, nonIndexable] = [value(0), value(4), value(5), value(6), value(7), value(8)];
  const pct = (n: number) => encountered ? n / encountered * 100 : 0;
  const make = (key: string, canonical: string, count: number, color: TokenName, nav: NavTarget | null, role: CategoryRole = "summary"): ChartCategory => ({
    key, role, label: t(`ovc-cat-${key}`), canonical, filter: role === "summary" ? nav?.filter ?? null : null, count, pct: pct(count),
    state: null, issue: null, color, hatch: 0, nav,
  });
  const noStatus = internal - indexable - nonIndexable;
  const hidden = encountered - displayed;
  const categories = [
    make("indexable", "Internal Indexable", indexable, "diff.add", { tab: "Internal", filter: "HTML", search: and("Indexability", "equals", "Indexable") }),
    make("nonIndexable", "Internal Non-Indexable", nonIndexable, "status.4xx", { tab: "Internal", filter: "HTML", search: and("Indexability", "equals", "Non-Indexable") }),
    make("noStatus", "Internal without Indexability Status", Math.max(0, noStatus), "chart.1", { tab: "Internal", filter: "All", search: and("Indexability", "isEmpty", null) }),
    make("external", "External", external, "chart.2", { tab: "External", filter: "All" }),
    make("hidden", "Not Displayed", Math.max(0, hidden), "chart.rest", null, "hidden"),
  ];
  return { base: encountered, categories, consistent: noStatus >= 0 && hidden >= 0 };
}

/** Descending by count; ties keep the catalog order, residual categories last. */
function byCount(a: ChartCategory, b: ChartCategory, order: Map<string, number>): number {
  const rest = (c: ChartCategory) => c.role === "unassigned" || c.role === "hidden" ? 1 : 0;
  return (b.count ?? 0) - (a.count ?? 0) || rest(a) - rest(b) || (order.get(a.key) ?? 0) - (order.get(b.key) ?? 0);
}

function others(members: ChartCategory[], base: number, t: TFn, summary: boolean): ChartCategory {
  const count = members.reduce((s, c) => s + (c.count ?? 0), 0);
  return {
    key: "others", role: "others", label: t("ovc-cat-others"), canonical: "Remaining", filter: null, count,
    pct: summary ? (base ? count / base * 100 : 0) : pctOf(count, base), state: null, issue: null, color: "chart.other", hatch: 0, nav: null, members,
  };
}

function header(entry: EntryRef, name: string, label: string, base: number): ChartHeader {
  return { entry, name, label, base };
}

/** Logged once per entry and values (UI-102: category sum larger than the base). */
export type OverflowLog = (entry: string, sum: number, base: number) => void;

/**
 * The chart of an entry. A filter row shows the chart of its tab block (the caller passes the tab entry). Missing tab →
 * `missing` (the caller shows the empty state).
 */
export function buildChart(data: OverviewDto, entry: EntryRef, t: TFn, onOverflow?: OverflowLog): ChartModel {
  if (entry.kind === "summary") {
    const { base, categories, consistent } = summaryCategories(data.summary, t);
    const head = header(entry, "Summary", t("sidebar-summary"), base);
    if (base === 0) return { kind: "zero", header: head };
    assignHatches(categories);
    return arrange(head, categories, true, consistent, t, onOverflow, true);
  }
  const tab = data.tabs.find(x => x.tab === entry.tab);
  if (!tab) return { kind: "missing" };
  const all = tab.filters.find(f => f.id === (tab.partition?.base ?? "All"));
  const base = all && counted(all) ? all.count! : 0;
  const head = header(entry, tab.tab, tabLabel(t, tab.tab), base);
  const partition = tab.partition ? tab.partition.filters.map(id => tab.filters.find(f => f.id === id)).filter((f): f is FilterInfo => !!f) : null;
  const members = partition ?? tab.filters.filter(f => f !== all);
  const rc = tab.tab === "Response Codes";
  let paletteIndex = 0;
  const categories = members.map(f => {
    const color = rc ? statusColor(f.id) : !partition && f.issue ? PRIORITY[f.issue.priority] : PALETTE[paletteIndex++ % PALETTE.length]!;
    return filterCategory(tab, f, t, base, color);
  });
  if (!categories.some(c => c.count != null) || (all && !counted(all))) return { kind: "noCounts", header: head, categories };
  if (base === 0) return { kind: "zero", header: head };
  assignHatches(categories);
  return arrange(head, categories, !!partition, true, t, onOverflow, false);
}

function arrange(head: ChartHeader, categories: ChartCategory[], partition: boolean, consistent: boolean, t: TFn, onOverflow: OverflowLog | undefined, summary: boolean): ChartModel {
  const order = new Map(categories.map((c, i) => [c.key, i]));
  const withCount = categories.filter(c => c.count != null);
  const missing = categories.filter(c => c.count == null);
  const sum = withCount.reduce((s, c) => s + c.count!, 0);
  let all = categories;
  const overflow = partition && (!consistent || sum > head.base);
  if (overflow) onOverflow?.(head.name, sum, head.base);
  if (partition && !overflow && sum < head.base) {
    const rest = residual("unassigned", head.base - sum, head.base, t, summary ? (head.base - sum) / head.base * 100 : undefined);
    order.set(rest.key, categories.length);
    withCount.push(rest);
    all = [...categories, rest];
  }
  const positive = withCount.filter(c => c.count! > 0).sort((a, b) => byCount(a, b, order));
  const zero = withCount.filter(c => c.count === 0);
  const legendOrder = [...positive, ...zero, ...missing];
  const table = legendOrder.filter(c => all.includes(c));
  if (!partition || overflow) {
    return { kind: "chart", header: head, chart: "overlap", drawn: positive, tail: missing, zero, table, overflow };
  }
  if (positive.length <= RING_MAX) {
    return { kind: "chart", header: head, chart: "ring", drawn: positive, tail: [...zero, ...missing], zero: [], table, overflow };
  }
  const drawn = positive.length > BAR_MAX ? [...positive.slice(0, BAR_MAX - 1), others(positive.slice(BAR_MAX - 1), head.base, t, summary)] : positive;
  return { kind: "chart", header: head, chart: "bars", drawn, tail: [...zero, ...missing], zero: [], table, overflow };
}

/** The category a selected filter row highlights (none for `All` and filters outside the chart). */
export function highlightKey(model: ChartModel, entry: EntryRef | null): string | null {
  if (model.kind !== "chart" || entry?.kind !== "filter") return null;
  const key = `f:${entry.filter}`;
  if (model.drawn.some(c => c.key === key) || model.tail.some(c => c.key === key) || model.zero.some(c => c.key === key)) return key;
  return model.drawn.find(c => c.members?.some(m => m.key === key))?.key ?? null;
}

/** Legend order of the focus group: drawn categories (expanded „Weitere“ members after it), then the tail. */
export function legendOrder(model: ChartModel, expanded: boolean): ChartCategory[] {
  if (model.kind !== "chart") return [];
  const drawn = model.drawn.flatMap(c => c.members && expanded ? [c, ...c.members] : [c]);
  return [...drawn, ...model.tail];
}

/** Accessible description: the three largest categories (UI-102, UI-126). */
export function describeTop(model: ChartModel, format: (c: ChartCategory) => string): string {
  if (model.kind !== "chart") return "";
  return model.drawn.slice(0, 3).map(format).join("; ");
}
