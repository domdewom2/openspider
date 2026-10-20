// Pure helpers of the advanced search (UI-011 [P1]). Parsing and evaluation stay in the core (`search.parse`, view specs);
// the UI only edits and prints the rule tree it received.
import type { ColumnKind } from "../../ipc/gen/ColumnKind";
import type { ColumnMeta } from "../../ipc/gen/ColumnMeta";
import type { RuleNode } from "../../ipc/gen/RuleNode";
import type { RuleOp } from "../../ipc/gen/RuleOp";
import type { RuleQuery } from "../../state/tabs";

export type Cond = Extract<RuleNode, { kind: "cond" }>;
export type Group = Extract<RuleNode, { kind: "group" }>;
/** Index path from the root group to a node. */
export type Path = number[];

export const TEXT_OPS: RuleOp[] = ["contains", "notContains", "equals", "notEquals", "startsWith", "endsWith", "regex", "notRegex", "isEmpty", "isNotEmpty"];
export const NUMERIC_OPS: RuleOp[] = ["equals", "notEquals", "greaterThan", "lessThan", "isEmpty", "isNotEmpty"];
const SYMBOL: Record<RuleOp, string> = { contains: "~", notContains: "!~", equals: "=", notEquals: "!=", startsWith: "^=", endsWith: "$=",
  regex: "REGEX", notRegex: "NOTREGEX", greaterThan: ">", lessThan: "<", isEmpty: "IS EMPTY", isNotEmpty: "IS NOT EMPTY" };
const KEYWORDS = new Set(["AND", "OR", "NOT", "IS", "EMPTY", "ISEMPTY", "ISNOTEMPTY", "CONTAINS", "NOTCONTAINS", "STARTSWITH", "ENDSWITH", "REGEX", "NOTREGEX"]);

export const isNumericKind = (kind: ColumnKind) => kind.kind === "int" || kind.kind === "float" || kind.kind === "seconds3";
export const isUnary = (op: RuleOp) => op === "isEmpty" || op === "isNotEmpty";
export const isRegex = (op: RuleOp) => op === "regex" || op === "notRegex";

/** Operators offered for a column; numeric comparisons never on text columns, nor on „all visible columns“ (`null`). */
export function operatorsFor(column: ColumnMeta | null): RuleOp[] {
  if (!column) return TEXT_OPS;
  const base = isNumericKind(column.kind) ? NUMERIC_OPS : TEXT_OPS;
  return column.searchable.length ? base.filter(op => column.searchable.includes(op)) : base;
}

/** Number entry in the UI language: its decimal separator and „.“ are accepted; the result uses „.“ for the core. */
export function normalizeNumber(value: string, locale: string): string | null {
  const decimal = new Intl.NumberFormat(locale).formatToParts(1.5).find(p => p.type === "decimal")?.value ?? ".";
  const text = value.trim();
  const pattern = decimal === "." ? /^[+-]?(\d+\.?\d*|\.\d+)$/ : new RegExp(`^[+-]?(\\d+([.${decimal}]\\d*)?|[.${decimal}]\\d+)$`);
  if (!pattern.test(text)) return null;
  const normalized = text.replace(decimal, ".");
  return Number.isFinite(Number(normalized)) ? normalized : null;
}

function quote(text: string): string {
  return `"${text.replaceAll("\\", "\\\\").replaceAll("\"", "\\\"")}"`;
}
function name(column: string | null): string {
  if (column === null) return "*";
  return /^[A-Za-z0-9_.-]+$/.test(column) && !KEYWORDS.has(column.toUpperCase()) ? column : quote(column);
}
/** Text syntax for display and history, e.g. `Address ~ "/blog/" AND "Word Count" < "200"`; `search.parse` reads it back. */
export function formatRule(node: RuleNode, nested = false): string {
  if (node.kind === "cond") return `${name(node.column)} ${SYMBOL[node.op]}${isUnary(node.op) ? "" : ` ${quote(node.value ?? "")}`}`;
  const text = node.children.map(child => formatRule(child, true)).join(node.op === "and" ? " AND " : " OR ");
  return nested && node.children.length > 1 ? `(${text})` : text;
}

/** The editor always works on a root group. */
export const asGroup = (node: RuleNode): Group => node.kind === "group" ? node : { kind: "group", op: "and", children: [node] };
export const emptyCond = (column: string | null = "Address"): Cond => ({ kind: "cond", column, op: "contains", value: "" });
export const emptyGroup = (): Group => ({ kind: "group", op: "and", children: [emptyCond()] });

export function nodeAt(root: RuleNode, path: Path): RuleNode | undefined {
  let node: RuleNode | undefined = root;
  for (const i of path) node = node?.kind === "group" ? node.children[i] : undefined;
  return node;
}
export function updateAt(root: RuleNode, path: Path, update: (node: RuleNode) => RuleNode): RuleNode {
  if (!path.length) return update(root);
  if (root.kind !== "group") return root;
  const [head, ...rest] = path;
  return { ...root, children: root.children.map((child, i) => i === head ? updateAt(child, rest, update) : child) };
}
export function removeAt(root: RuleNode, path: Path): RuleNode {
  if (!path.length) return root;
  const parent = path.slice(0, -1); const index = path[path.length - 1];
  return updateAt(root, parent, node => node.kind === "group" ? { ...node, children: node.children.filter((_, i) => i !== index) } : node);
}
export function appendAt(root: RuleNode, path: Path, child: RuleNode): RuleNode {
  return updateAt(root, path, node => node.kind === "group" ? { ...node, children: [...node.children, child] } : node);
}
export function depth(node: RuleNode): number {
  return node.kind === "group" ? 1 + Math.max(0, ...node.children.map(depth)) : 0;
}
export function conditions(node: RuleNode, path: Path = []): { cond: Cond; path: Path }[] {
  return node.kind === "cond" ? [{ cond: node, path }] : node.children.flatMap((child, i) => conditions(child, [...path, i]));
}
/** Drops empty groups so that the core never receives `children: []`. */
export function prune(node: RuleNode): RuleNode | null {
  if (node.kind === "cond") return node;
  const children = node.children.map(prune).filter((c): c is RuleNode => c !== null);
  return children.length ? { ...node, children } : null;
}

export const HISTORY_LIMIT = 20;
/** Most recent first, unique by text and case switch, at most 20 entries. */
export function pushHistory(history: RuleQuery[], entry: RuleQuery): RuleQuery[] {
  return [entry, ...history.filter(h => h.text !== entry.text || h.caseSensitive !== entry.caseSensitive)].slice(0, HISTORY_LIMIT);
}
