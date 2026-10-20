// Deterministic contract fixture for U20 (fx-ui-basic URLs, 60+ Internal columns, segments „Blog“/„Shop“).
// The core is simulated: `search.parse` answers from a fixed table and the views apply the received rule tree with a
// minimal stand-in evaluation, so UI tests run without K02/K14. The real grammar is checked against the bridge (e2e).
import { MockCore } from "../../../ipc/transport/mock";
import { IpcError } from "../../../ipc/errors";
import type { ColumnMeta } from "../../../ipc/gen/ColumnMeta";
import type { CrawlConfigDto } from "../../../ipc/gen/CrawlConfigDto";
import type { LocateArgs } from "../../../ipc/gen/LocateArgs";
import type { RowsArgs } from "../../../ipc/gen/RowsArgs";
import type { RuleNode } from "../../../ipc/gen/RuleNode";
import type { RuleOp } from "../../../ipc/gen/RuleOp";
import type { SegmentDef } from "../../../ipc/gen/SegmentDef";
import type { SegmentFilter } from "../../../ipc/gen/SegmentFilter";
import type { TabInfo } from "../../../ipc/gen/TabInfo";
import type { ViewOpenArgs } from "../../../ipc/gen/ViewOpenArgs";
import type { ViewSpec } from "../../../ipc/gen/ViewSpec";

const TEXT: RuleOp[] = ["contains", "notContains", "equals", "notEquals", "startsWith", "endsWith", "regex", "notRegex", "isEmpty", "isNotEmpty"];
const NUMBER: RuleOp[] = ["equals", "notEquals", "greaterThan", "lessThan", "isEmpty", "isNotEmpty"];
const column = (key: string, kind: ColumnMeta["kind"], defaultVisible = true, extra: Partial<ColumnMeta> = {}): ColumnMeta => ({ key, kind, defaultVisible,
  defaultWidth: 140, align: kind.kind === "int" ? "right" : "left", sortable: key !== "Row", searchable: key === "Row" ? [] : kind.kind === "int" ? NUMBER : TEXT, extension: false, ...extra });
const base = [column("Row", { kind: "int" }), column("Address", { kind: "url" }), column("Status Code", { kind: "int" }), column("Word Count", { kind: "int" }),
  column("Title 1", { kind: "text" }), column("Segment", { kind: "text" })];
export const internalColumns: ColumnMeta[] = [...base,
  ...Array.from({ length: 58 }, (_, i) => column(`Metric ${String(i + 1).padStart(2, "0")}`, { kind: "int" }, i < 4)),
  column("Hash", { kind: "hash" }, false, { needs: { code: "FEATURE_DISABLED", arg: "crawl.storeHash" } })];
export const externalColumns: ColumnMeta[] = [base[0], base[1], base[2], column("Content Type", { kind: "text" })];
const filters = (count: number) => [{ id: "All", labelTemplate: "All", params: {}, count, state: { kind: "ready" as const } }];
export const advancedTabs: TabInfo[] = ["Internal", "External", "Images", "AMP", "Page Titles"].map((id, i) => ({ id, defaultOrder: i, filters: filters(5), detailCapable: true }));
export const segmentDefs: SegmentDef[] = [
  { id: "seg-blog", name: "Blog", color: "#1E88E5", order: 0, rules: { kind: "cond", column: "Address", op: "contains", value: "/blog/" } },
  { id: "seg-shop", name: "Shop", color: "#FDD835", order: 1, rules: { kind: "cond", column: "Address", op: "contains", value: "/shop/" } },
];
const host = "https://fx-ui-basic.invalid";
export const fixtureRows = [
  { id: 1, Address: `${host}/Blog/Post-1`, "Status Code": 200, "Word Count": 40, "Title 1": "Post 1", Segment: null },
  { id: 2, Address: `${host}/blog/post-2`, "Status Code": 200, "Word Count": 600, "Title 1": "Post 2", Segment: "Blog" },
  { id: 3, Address: `${host}/news/blog-news`, "Status Code": 404, "Word Count": 300, "Title 1": "News", Segment: null },
  { id: 4, Address: `${host}/shop/shop/schuhe`, "Status Code": 200, "Word Count": 120, "Title 1": "Schuhe", Segment: "Shop" },
  { id: 5, Address: `${host}/shop/schuhe/shop`, "Status Code": 200, "Word Count": 10, "Title 1": "Shop", Segment: "Shop" },
] as Record<string, unknown>[];

/** Canned answers of the simulated `search.parse` (the real grammar lives in the core). */
export const PARSED: Record<string, RuleNode> = {
  "\"Word Count\" < 50 OR \"Status Code\" = 404": { kind: "group", op: "or", children: [
    { kind: "cond", column: "Word Count", op: "lessThan", value: "50" }, { kind: "cond", column: "Status Code", op: "equals", value: "404" }] },
  "Address ~ \"/blog/\" AND \"Word Count\" < 200": { kind: "group", op: "and", children: [
    { kind: "cond", column: "Address", op: "contains", value: "/blog/" }, { kind: "cond", column: "Word Count", op: "lessThan", value: "200" }] },
  "Address REGEX \"/([^/]+)/\\\\1/\"": { kind: "cond", column: "Address", op: "regex", value: "/([^/]+)/\\1/" },
  "Address ~ \"blog\"": { kind: "cond", column: "Address", op: "contains", value: "blog" },
};
const syntaxError = (position: number) => new IpcError({ code: "InvalidInput", messageKey: "err-search-syntax", args: { position, reason: "expected-value" }, field: "text", retryable: false });

function matches(node: RuleNode, row: Record<string, unknown>, sensitive: boolean, visible: string[]): boolean {
  if (node.kind === "group") return node.op === "and" ? node.children.every(c => matches(c, row, sensitive, visible)) : node.children.some(c => matches(c, row, sensitive, visible));
  if (node.column === null) return visible.some(key => matches({ ...node, column: key }, row, sensitive, visible));
  const raw = row[node.column]; const value = node.value ?? "";
  const text = raw === null || raw === undefined ? "" : String(raw);
  const [a, b] = sensitive ? [text, value] : [text.toLowerCase(), value.toLowerCase()];
  switch (node.op) {
    case "contains": return a.includes(b);
    case "notContains": return !a.includes(b);
    case "equals": return typeof raw === "number" ? raw === Number(value) : a === b;
    case "notEquals": return typeof raw === "number" ? raw !== Number(value) : a !== b;
    case "startsWith": return a.startsWith(b);
    case "endsWith": return a.endsWith(b);
    case "regex": return new RegExp(value, sensitive ? "" : "i").test(text);
    case "notRegex": return !new RegExp(value, sensitive ? "" : "i").test(text);
    case "greaterThan": return typeof raw === "number" && raw > Number(value);
    case "lessThan": return typeof raw === "number" && raw < Number(value);
    case "isEmpty": return text === "";
    case "isNotEmpty": return text !== "";
  }
}

export function advancedFixture() {
  const core = new MockCore();
  const views = new Map<string, ViewSpec>();
  let config: CrawlConfigDto = { schemaVersion: 2, generator: "fixture", config: { ui: { columns: {} } } };
  let segments = structuredClone(segmentDefs);
  let nextView = 0; const version = 1;
  const columnsOf = (tab: string) => tab === "Internal" ? internalColumns : externalColumns;
  const rows = (spec?: ViewSpec) => fixtureRows.filter(row => {
    if (spec?.kind !== "tab") return true;
    if (spec.segment?.kind === "segment" && row.Segment !== spec.segment.name) return false;
    if (spec.segment?.kind === "none" && row.Segment !== null) return false;
    if (spec.search?.kind === "simple") return String(row.Address).toLowerCase().includes(spec.search.text.toLowerCase());
    if (spec.search?.kind === "rules") return matches(spec.search.tree, row, spec.search.caseSensitive, ["Address", "Title 1"]);
    return true;
  });
  core.handlers.set("catalog.tabs", () => advancedTabs);
  core.handlers.set("catalog.columns", args => columnsOf((args as { tab: string }).tab));
  core.handlers.set("catalog.filterCounts", args => {
    const segment = (args as { segment?: SegmentFilter | null } | undefined)?.segment ?? null;
    const count = rows({ kind: "tab", tab: "Internal", filter: "All", sort: [], segment }).length;
    return { version, filters: advancedTabs.map(tab => ({ tab: tab.id, filter: { ...tab.filters[0], count } })) };
  });
  core.handlers.set("config.get", () => structuredClone(config));
  core.handlers.set("config.set", args => { config = structuredClone(args as CrawlConfigDto); return { invalidatedSteps: [], deferred: [] }; });
  core.handlers.set("search.parse", args => {
    const text = (args as { text: string }).text.trim();
    const tree = PARSED[text];
    if (!tree) throw syntaxError(Math.max(0, text.length - 1));
    return structuredClone(tree);
  });
  core.handlers.set("search.validateRegex", args => {
    try { new RegExp((args as { pattern: string }).pattern); return null; }
    catch { throw new IpcError({ code: "InvalidInput", messageKey: "err-search-regex", args: {}, field: "pattern", detail: "unclosed group", retryable: false }); }
  });
  core.handlers.set("segment.list", () => structuredClone(segments));
  core.handlers.set("view.open", args => {
    const { spec } = args as ViewOpenArgs; const viewId = `u20-${++nextView}`; views.set(viewId, spec);
    if (spec.kind === "tab" && spec.search?.kind === "rules" && JSON.stringify(spec.search.tree).includes("(a+)+$"))
      throw new IpcError({ code: "Timeout", messageKey: "err-search-timeout", args: {}, retryable: false });
    return { viewId, version, total: rows(spec).length, state: "ready", columns: columnsOf(spec.kind === "tab" ? spec.tab : "Internal") };
  });
  core.handlers.set("view.close", () => undefined);
  core.handlers.set("view.rows", args => {
    const a = args as RowsArgs; const page = rows(views.get(a.viewId)).slice(a.offset, a.offset + a.limit);
    return { viewId: a.viewId, version: a.version, offset: a.offset, columns: a.columns, rowKeys: page.map(r => r.id as number),
      cells: page.map(r => a.columns.map(c => c === "Row" ? r.id : r[c] ?? null)) };
  });
  core.handlers.set("view.locate", args => { const a = args as LocateArgs; const i = rows(views.get(a.viewId)).findIndex(r => r.id === a.urlId); return i < 0 ? null : i; });
  core.handlers.set("viz.open", () => ({ windowLabel: "viz-1" }));
  return { core, views, config: () => config, setSegments(next: SegmentDef[]) { segments = next; }, specs: () => [...views.values()] };
}
