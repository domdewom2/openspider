import { z } from "zod";
import type { RuleNode } from "../../ipc/gen/RuleNode";
import type { RuleOp } from "../../ipc/gen/RuleOp";
import type { SegmentDef } from "../../ipc/gen/SegmentDef";
import type { ValidationIssue } from "../../ipc/gen/ValidationIssue";
import { SEGMENT_PALETTE } from "../../ui/tokens/tokens";

export type Condition = { column: string | null; op: RuleOp; value: string | null };
/** Conditions inside a group are ANDed, the groups of a segment are ORed (CRAWL-134). */
export type RuleGroups = Condition[][];

export const OPS: readonly RuleOp[] = ["contains", "notContains", "equals", "notEquals", "startsWith", "endsWith", "regex", "notRegex", "greaterThan", "lessThan", "isEmpty", "isNotEmpty"];
export const NO_VALUE: readonly RuleOp[] = ["isEmpty", "isNotEmpty"];
export const NUMERIC: readonly RuleOp[] = ["greaterThan", "lessThan"];
/** Column that references the segment of a URL; used as “is in segment” (see report, interface question). */
export const SEGMENT_COLUMN = "Segment";

export const NAME_MAX = 64;

type Cond = Extract<RuleNode, { kind: "cond" }>;
const conditionsOf = (node: RuleNode): Condition[] =>
  node.kind === "cond" ? [{ column: node.column, op: node.op, value: node.value }] : node.children.flatMap(conditionsOf);

/** Reads any tree of the shared rule model as OR of AND groups. */
export function toGroups(node: RuleNode): RuleGroups {
  if (node.kind === "cond") return [conditionsOf(node)];
  if (node.op === "or") return node.children.map(conditionsOf);
  return [conditionsOf(node)];
}

export function fromGroups(groups: RuleGroups): RuleNode {
  const cond = (c: Condition): Cond => ({ kind: "cond", column: c.column, op: c.op, value: NO_VALUE.includes(c.op) ? null : c.value });
  return { kind: "group", op: "or", children: groups.map(g => ({ kind: "group", op: "and", children: g.map(cond) })) };
}

export const emptyCondition = (): Condition => ({ column: "Address", op: "contains", value: "" });

let counter = 0;
export function newId(existing: readonly SegmentDef[]): string {
  const ids = new Set(existing.map(s => s.id));
  let id: string;
  do { id = `seg-${Date.now().toString(36)}-${(counter++).toString(36)}`; } while (ids.has(id));
  return id;
}

/** First free palette colour (stored as the light-theme hex; the dark variant derives from the palette index). */
export function nextColor(existing: readonly SegmentDef[]): string {
  const used = new Set(existing.map(s => s.color.toLowerCase()));
  const free = SEGMENT_PALETTE.find(c => !used.has(c.light.toLowerCase()));
  return (free ?? SEGMENT_PALETTE[existing.length % SEGMENT_PALETTE.length]!).light;
}

export function newSegment(existing: readonly SegmentDef[], name = ""): SegmentDef {
  return { id: newId(existing), name, color: nextColor(existing), rules: fromGroups([[emptyCondition()]]), order: existing.length };
}

/** Moves a segment and renumbers `order`, which is the priority (first matching segment wins). */
export function move(list: readonly SegmentDef[], from: number, to: number): SegmentDef[] {
  if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return renumber(list);
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item!);
  return renumber(next);
}

export const renumber = (list: readonly SegmentDef[]): SegmentDef[] => list.map((s, order) => (s.order === order ? s : { ...s, order }));

export type FieldError = { key: string; args?: Record<string, string | number> };

/** Name rules (CRAWL-134): 1–64 characters, unique without regard to case. */
export function nameError(list: readonly SegmentDef[], index: number): FieldError | null {
  const name = list[index]?.name ?? "";
  const n = [...name.trim()].length;
  if (n < 1) return { key: "sgd-err-name-empty" };
  if ([...name].length > NAME_MAX) return { key: "sgd-err-name-long", args: { max: NAME_MAX } };
  const lower = name.trim().toLowerCase();
  if (list.some((s, i) => i !== index && s.name.trim().toLowerCase() === lower)) return { key: "sgd-err-name-duplicate" };
  return null;
}

/**
 * Local condition checks that need no core: a value is required, numeric operators need a number, and a reference to
 * another segment (column `Segment`) may only point to a segment higher up in the list.
 */
export function conditionError(list: readonly SegmentDef[], index: number, c: Condition): FieldError | null {
  const value = (c.value ?? "").trim();
  if (!NO_VALUE.includes(c.op) && value === "") return { key: "sgd-err-value-required" };
  if (NUMERIC.includes(c.op) && !/^-?\d+([.,]\d+)?$/.test(value)) return { key: "sgd-err-number" };
  if (c.column === SEGMENT_COLUMN && value !== "") {
    const at = list.findIndex(s => s.name.trim().toLowerCase() === value.toLowerCase());
    if (at < 0) return { key: "sgd-err-segment-unknown", args: { name: value } };
    if (at >= index) return { key: "sgd-err-segment-below", args: { name: value } };
  }
  return null;
}

/** Position of a core validation issue; tolerant towards both path spellings (`groups[g].conditions[c]` and `rules…children[g].children[c]`). */
export type IssueTarget = { kind: "condition"; group: number; condition: number } | { kind: "name" } | { kind: "color" } | { kind: "segment" };
export function issueTarget(path: string): IssueTarget {
  const m = /(?:groups|children)\[(\d+)\]\.(?:conditions|children)\[(\d+)\]/.exec(path);
  if (m) return { kind: "condition", group: Number(m[1]), condition: Number(m[2]) };
  if (/\bname\b/.test(path)) return { kind: "name" };
  if (/\bcolor\b/.test(path)) return { kind: "color" };
  return { kind: "segment" };
}

export const isBlocking = (i: ValidationIssue): boolean => !i.warning;

/** `true` while a segment has any local error or a blocking core issue; “OK” stays disabled then (UI-068). */
export function hasBlockingError(list: readonly SegmentDef[], index: number, issues: readonly ValidationIssue[] | null): boolean {
  const seg = list[index];
  if (!seg) return false;
  if (nameError(list, index)) return true;
  const conditions = toGroups(seg.rules).flat();
  if (conditions.length === 0 || conditions.some(c => conditionError(list, index, c))) return true;
  return !!issues?.some(isBlocking);
}

/** Segments with a condition on analysis or API columns are flagged “pending” until the data exists (CRAWL-134). */
export function pendingColumns(columns: readonly { key: string; needs?: { code: string } | null }[]): Set<string> {
  return new Set(columns.filter(c => c.needs && (c.needs.code === "NEEDS_CRAWL_ANALYSIS" || c.needs.code === "API_NOT_CONNECTED")).map(c => c.key));
}

// JSON exchange: the exported file is the list itself, so export → import is the identity (UI-068).
const ruleSchema: z.ZodType<RuleNode> = z.lazy(() => z.union([
  z.object({ kind: z.literal("group"), op: z.enum(["and", "or"]), children: z.array(ruleSchema) }),
  z.object({ kind: z.literal("cond"), column: z.string().nullable(), op: z.enum(OPS as [RuleOp, ...RuleOp[]]), value: z.string().nullable() }),
]));
const segmentSchema = z.object({ id: z.string().min(1), name: z.string(), color: z.string().regex(/^#[0-9a-fA-F]{6}$/), rules: ruleSchema, order: z.number().int() });

export function exportJson(list: readonly SegmentDef[]): string {
  return JSON.stringify(renumber(list), null, 2);
}

export type ImportResult = { ok: true; segments: SegmentDef[] } | { ok: false; error: string };
export function importJson(text: string): ImportResult {
  let raw: unknown;
  try { raw = JSON.parse(text); } catch (e) { return { ok: false, error: e instanceof Error ? e.message : "JSON" }; }
  const parsed = z.array(segmentSchema).safeParse(raw);
  if (!parsed.success) return { ok: false, error: parsed.error.issues[0]?.path.join(".") || "format" };
  const sorted = [...parsed.data].sort((a, b) => a.order - b.order);
  return { ok: true, segments: renumber(sorted as SegmentDef[]) };
}

/** Stable signature of everything the core evaluates; changes trigger validation and the preview counters. */
export const signature = (s: SegmentDef): string => JSON.stringify([s.id, s.name, s.color, s.rules, s.order]);
