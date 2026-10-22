import { importJson } from "../../../segments-dialog/model";
import type { RuleNode } from "../../../../ipc/gen/RuleNode";
import type { RuleOp } from "../../../../ipc/gen/RuleOp";
import type { SegmentDef } from "../../../../ipc/gen/SegmentDef";
export interface Condition { field: string; op: string; value: string }
export interface Segment { name: string; color: string; groups: { conditions: Condition[] }[] }
export const OPS = ["contains", "not_contains", "equals", "not_equals", "starts_with", "ends_with", "regex_matches", "regex_not_matches", "greater_than", "less_than", "greater_or_equal", "less_or_equal", "is_empty", "is_not_empty", "has_issue", "has_no_issue", "in_segment", "in_no_segment"] as const;
export const emptyCondition = (): Condition => ({ field: "Address", op: "contains", value: "" });
const opMap: Record<string, RuleOp> = { contains: "contains", not_contains: "notContains", equals: "equals", not_equals: "notEquals", starts_with: "startsWith", ends_with: "endsWith", regex_matches: "regex", regex_not_matches: "notRegex", greater_than: "greaterThan", less_than: "lessThan", greater_or_equal: "greaterThan", less_or_equal: "lessThan", is_empty: "isEmpty", is_not_empty: "isNotEmpty" };
function rule(c: Condition): RuleNode {
  const column = c.op.startsWith("has_") ? "Issues" : c.op.startsWith("in_") ? "Segment" : c.field;
  const op = c.op === "in_no_segment" ? "isEmpty" : c.op === "in_segment" || c.op === "has_issue" ? "equals" : c.op === "has_no_issue" ? "notEquals" : opMap[c.op]!;
  const cond: RuleNode = { kind: "cond", column, op, value: c.value };
  return c.op === "greater_or_equal" || c.op === "less_or_equal" ? { kind: "group", op: "or", children: [cond, { kind: "cond", column, op: "equals", value: c.value }] } : cond;
}
/** Match the core's SegmentConfig -> SegmentDef adapter, preserving AND/OR and inclusive numeric comparisons. */
export function previewDefs(list: Segment[]): SegmentDef[] {
  return list.map((s, order) => ({ id: `draft-${order}`, name: s.name, color: s.color, order, rules: { kind: "group", op: "or", children: s.groups.map(g => ({ kind: "group", op: "and", children: g.conditions.map(rule) })) } }));
}
function conditionOf(node: Extract<RuleNode, { kind: "cond" }>): Condition {
  const field = node.column ?? "Address";
  const reverse: Record<RuleOp, string> = { contains: "contains", notContains: "not_contains", equals: "equals", notEquals: "not_equals", startsWith: "starts_with", endsWith: "ends_with", regex: "regex_matches", notRegex: "regex_not_matches", greaterThan: "greater_than", lessThan: "less_than", isEmpty: "is_empty", isNotEmpty: "is_not_empty" };
  const op = field === "Segment" && node.op === "equals" ? "in_segment" : field === "Segment" && node.op === "isEmpty" ? "in_no_segment" : field === "Issues" && node.op === "equals" ? "has_issue" : field === "Issues" && node.op === "notEquals" ? "has_no_issue" : reverse[node.op];
  return { field, op, value: node.value ?? "" };
}
function groupsOf(node: RuleNode): Condition[][] {
  if (node.kind === "cond") return [[conditionOf(node)]];
  if (node.op === "or") {
    const groups = node.children.flatMap(groupsOf);
    if (groups.length > 4096) throw new Error("too many groups");
    return groups;
  }
  let groups: Condition[][] = [[]];
  for (const child of node.children) {
    const next = groupsOf(child);
    if (groups.length * next.length > 4096) throw new Error("too many groups");
    groups = groups.flatMap(a => next.map(b => [...a, ...b]));
  }
  return groups;
}
/** Accept the shared segment-dialog exchange format and native profile definitions. */
export function importSegments(text: string): Segment[] {
  const shared = importJson(text);
  if (shared.ok) return shared.segments.map(s => ({ name: s.name, color: s.color, groups: groupsOf(s.rules).map(conditions => ({ conditions })) }));
  const value: unknown = JSON.parse(text);
  if (!Array.isArray(value) || !value.every(s => typeof s === "object" && s !== null && typeof s.name === "string" && typeof s.color === "string" && Array.isArray(s.groups) && s.groups.every((g: { conditions?: unknown }) => Array.isArray(g?.conditions) && g.conditions.every((c: Condition) => c && typeof c.field === "string" && typeof c.value === "string" && OPS.includes(c.op as typeof OPS[number]))))) throw new Error("format");
  return value as Segment[];
}
