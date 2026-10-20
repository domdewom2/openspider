// Regel-Tester (CRAWL-111 Include/Exclude, CRAWL-107 Umschreibung): rechnet ausschließlich über den Kern
// (`config.testRules`, `crawl.validateStartUrl`), also mit exakt derselben Funktion wie die Engine.
import { config } from "../../../ipc/config";
import { crawl } from "../../../ipc/crawl";
import { toCmdError } from "../../../ipc/errors";
import type { CmdError } from "../../../ipc/gen/CmdError";
import type { RuleDef } from "../../../ipc/gen/RuleDef";
import type { RuleTrace } from "../../../ipc/gen/RuleTrace";

export type ListName = "include" | "exclude";

export interface LineRef {
  list: ListName;
  /** 1-basiert wie in der Eingabe (leere Zeilen zählen mit). */
  line: number;
}

export type Verdict = "included" | "excluded" | "notIncluded";

export interface IncludeExcludeResult {
  /** Eingabe nach der Kodierung der Engine (`https://site.test/a b` → `…/a%20b`). */
  encoded: string;
  verdict: Verdict;
  /** Erste passende Zeile der entscheidenden Liste (Include bei „Eingeschlossen“, Exclude bei „Ausgeschlossen“). */
  match: LineRef | null;
  /** Zeilen, die nur als Teilstück passen würden (bei Vollübereinstimmung): Hinweis „`.*` ergänzen?“. */
  partialOnly: LineRef[];
  /** Zeilen mit Syntaxfehler; sie werden übersprungen, die übrigen Zeilen zählen trotzdem. */
  invalid: LineRef[];
}

export interface TestError {
  error: CmdError;
}

function toRules(include: readonly string[], exclude: readonly string[], blank: ReadonlySet<string>): RuleDef[] {
  return [
    ...include.map((pattern, i) => ({ pattern: blank.has(`include:${i}`) ? "" : pattern, include: true })),
    ...exclude.map((pattern, i) => ({ pattern: blank.has(`exclude:${i}`) ? "" : pattern, include: false })),
  ];
}

function refOf(index: number, includeCount: number): LineRef {
  return index < includeCount ? { list: "include", line: index + 1 } : { list: "exclude", line: index - includeCount + 1 };
}

/** Findet Zeilen mit Syntaxfehler, indem jede Zeile allein getestet wird (nur im Fehlerfall von `config.testRules`). */
async function findInvalid(include: readonly string[], exclude: readonly string[], input: string, partial: boolean): Promise<Set<string>> {
  const bad = new Set<string>();
  const probe = async (list: ListName, i: number, pattern: string) => {
    if (pattern.trim() === "") return;
    try {
      await config.testRules({ kind: "includeExclude", rules: [{ pattern, include: list === "include" }], input, regexPartialMatch: partial });
    } catch {
      bad.add(`${list}:${i}`);
    }
  };
  await Promise.all([...include.map((p, i) => probe("include", i, p)), ...exclude.map((p, i) => probe("exclude", i, p))]);
  return bad;
}

/**
 * Testet eine URL gegen die Include-/Exclude-Zeilen. Die Eingabe wird wie in der Engine kodiert, bevor abgeglichen wird;
 * Zeilen mit Syntaxfehler werden markiert und übersprungen.
 */
export async function testIncludeExclude(args: {
  include: readonly string[];
  exclude: readonly string[];
  partial: boolean;
  input: string;
}): Promise<IncludeExcludeResult | TestError> {
  let encoded: string;
  try {
    encoded = (await crawl.validateStartUrl({ input: args.input })).normalized;
  } catch (e) {
    return { error: toCmdError(e) };
  }
  let blank = new Set<string>();
  const run = async (partial: boolean): Promise<RuleTrace> => {
    try {
      return await config.testRules({ kind: "includeExclude", rules: toRules(args.include, args.exclude, blank), input: encoded, regexPartialMatch: partial });
    } catch (e) {
      if (blank.size > 0 || toCmdError(e).code !== "InvalidInput") throw e;
      blank = await findInvalid(args.include, args.exclude, encoded, partial);
      if (blank.size === 0) throw e;
      return config.testRules({ kind: "includeExclude", rules: toRules(args.include, args.exclude, blank), input: encoded, regexPartialMatch: partial });
    }
  };
  try {
    const full = await run(args.partial);
    const includeCount = args.include.length;
    const matched = full.steps.filter((s) => s.matched).map((s) => s.index);
    const firstInclude = matched.find((i) => i < includeCount);
    const firstExclude = matched.find((i) => i >= includeCount);
    let verdict: Verdict;
    let match: LineRef | null = null;
    if (full.included === true) {
      verdict = "included";
      match = firstInclude === undefined ? null : refOf(firstInclude, includeCount);
    } else if (firstExclude !== undefined) {
      verdict = "excluded";
      match = refOf(firstExclude, includeCount);
    } else {
      verdict = "notIncluded";
    }
    let partialOnly: LineRef[] = [];
    if (!args.partial) {
      const partialTrace = await run(true);
      const fullMatched = new Set(matched);
      partialOnly = partialTrace.steps.filter((s) => s.matched && !fullMatched.has(s.index)).map((s) => refOf(s.index, includeCount));
    }
    const invalid = [...blank].map((k) => {
      const [list, i] = k.split(":") as [ListName, string];
      return { list, line: Number(i) + 1 };
    });
    return { encoded, verdict, match, partialOnly, invalid };
  } catch (e) {
    return { error: toCmdError(e) };
  }
}

export interface RewriteStage {
  /** `parameters` = Ergebnis nach „Parameter entfernen“, `regex` = eine Ersetzungszeile. */
  kind: "parameters" | "regex";
  /** 1-basiert, nur bei `regex`. */
  line?: number;
  before: string;
  after: string;
  matched: boolean;
  error?: string | null;
}

export interface RewriteResult {
  output: string;
  stages: RewriteStage[];
}

/** Testet die URL-Umschreibung: Parameter entfernen, danach die Regex-Ersetzungen in Reihenfolge; Zwischenergebnisse je Schritt. */
export async function testRewrite(args: {
  removeParameters: readonly string[];
  removeAllParameters: boolean;
  rules: readonly { pattern: string; replacement: string }[];
  input: string;
}): Promise<RewriteResult | TestError> {
  try {
    const params = args.removeParameters.map((p) => p.trim()).filter((p) => p !== "");
    const base = { kind: "rewrite" as const, input: args.input, removeParameters: params, removeAllParameters: args.removeAllParameters };
    const afterParams = await config.testRules({ ...base, rules: [] });
    const active = args.rules.map((r, i) => ({ r, line: i + 1 })).filter((x) => x.r.pattern !== "");
    const trace = await config.testRules({ ...base, rules: active.map((x) => ({ pattern: x.r.pattern, replacement: x.r.replacement })) });
    const stages: RewriteStage[] = [
      { kind: "parameters", before: args.input, after: afterParams.output, matched: afterParams.output !== args.input },
      ...trace.steps.map((s) => ({ kind: "regex" as const, line: active[s.index]?.line ?? s.index + 1, before: s.before, after: s.after, matched: s.matched, error: s.error ?? null })),
    ];
    return { output: trace.output, stages };
  } catch (e) {
    return { error: toCmdError(e) };
  }
}

export const isTestError = (r: unknown): r is TestError => typeof r === "object" && r !== null && "error" in r;
