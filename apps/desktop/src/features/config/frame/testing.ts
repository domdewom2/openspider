// Testkern für vitest und die Vertragsseite von E2E-A (nicht im Produktbündel): ein Mock der `config.*`-/`profile.*`-Commands,
// der die Werkseinstellungen aus dem echten JSON-Schema des Kerns ableitet. Die Auswertung der Regeln ersetzt nur die Engine –
// Verhalten gegen den echten Kern prüft `e2e/config/bridge.spec.ts`.
import type { MockCore } from "../../../ipc/transport/mock";
import { IpcError, localError } from "../../../ipc/errors";
import type { ConfigProfileInfo } from "../../../ipc/gen/ConfigProfileInfo";
import type { CrawlConfigDto } from "../../../ipc/gen/CrawlConfigDto";
import type { DiffRow } from "../../../ipc/gen/DiffRow";
import type { RuleDef } from "../../../ipc/gen/RuleDef";
import type { RuleTrace } from "../../../ipc/gen/RuleTrace";
import type { TestRulesArgs } from "../../../ipc/gen/TestRulesArgs";
import type { ValidationIssue } from "../../../ipc/gen/ValidationIssue";
import schemaRaw from "../../../../../../docs/spec/schemas/osconfig.schema.json?raw";
import { changedPaths, getPath, isLiveField, STRUCTURAL_FIELDS, setPath, type ConfigJson, type RunMode } from "./model";
import { defaultsFromSchema, fieldMeta, type ConfigSchema } from "./schema";

export const SCHEMA = JSON.parse(schemaRaw) as ConfigSchema;
export const defaultConfig = (): ConfigJson => structuredClone(defaultsFromSchema(SCHEMA));

export interface FakeConfigCore {
  config: ConfigJson;
  setCalls: CrawlConfigDto[];
  profiles: Map<string, ConfigJson>;
  defaultProfile: string | null;
  /** Wie `check_state` in `os-app`: bestimmt, welche Änderungen `config.set` ablehnt. */
  mode: RunMode;
  /** Beim nächsten `config.set` zurückzugebender Fehler (z. B. Chromium). */
  failNextSet: { code: "InvalidInput" | "Engine" | "Io"; messageKey: string; field?: string; detail?: string } | null;
  pauseCalls: number;
}

const err = (code: Parameters<typeof localError>[0], key: string, field?: string, detail?: string) =>
  new IpcError({ ...localError(code, key), field: field ?? null, detail: detail ?? null });

function leafPaths(v: unknown, prefix = "", out: string[] = []): string[] {
  if (typeof v === "object" && v !== null && !Array.isArray(v)) for (const [k, x] of Object.entries(v)) leafPaths(x, prefix ? `${prefix}.${k}` : k, out);
  else if (prefix) out.push(prefix);
  return out;
}

function validate(cfg: ConfigJson): ValidationIssue[] {
  const issues: ValidationIssue[] = [];
  const add = (path: string, reason: string) => issues.push({ path, messageKey: "err-config-validation", args: { code: "InvalidValue", reason }, line: null, column: null, warning: false });
  for (const path of leafPaths(cfg)) {
    const meta = fieldMeta(SCHEMA, path);
    const v = getPath(cfg, path);
    if (!meta || typeof v !== "number") continue;
    if (meta.minimum !== undefined && v < meta.minimum) add(path, `muss mindestens ${meta.minimum} sein (ist ${v})`);
    if (meta.maximum !== undefined && v > meta.maximum) add(path, `darf höchstens ${meta.maximum} sein (ist ${v})`);
  }
  const regex = (path: string, list: unknown) => {
    (Array.isArray(list) ? list : []).forEach((p, i) => {
      try {
        new RegExp(String(p));
      } catch {
        add(`${path}[${i}]`, "ungültiger regulärer Ausdruck");
      }
    });
  };
  regex("scope.include", getPath(cfg, "scope.include"));
  regex("scope.exclude", getPath(cfg, "scope.exclude"));
  const rr = getPath(cfg, "rewrite.regex_replace");
  (Array.isArray(rr) ? rr : []).forEach((r, i) => {
    try {
      new RegExp(String((r as { pattern: string }).pattern));
    } catch {
      add(`rewrite.regex_replace[${i}]`, "ungültiger regulärer Ausdruck");
    }
  });
  return issues;
}

function matches(pattern: string, url: string, partial: boolean): boolean {
  return new RegExp(partial ? `(?:${pattern})` : `^(?:${pattern})$`).test(url);
}

function testRules(a: TestRulesArgs): RuleTrace {
  if (a.kind === "includeExclude") {
    const partial = a.regexPartialMatch ?? false;
    const live = a.rules.filter((r: RuleDef) => r.pattern.trim() !== "");
    for (const r of live) {
      try {
        new RegExp(r.pattern);
      } catch {
        throw err("InvalidInput", "err-rule-pattern", "rules");
      }
    }
    const steps = a.rules.map((r, index) => ({ index, matched: r.pattern.trim() !== "" && matches(r.pattern, a.input, partial), before: a.input, after: a.input, error: null }));
    const excluded = steps.some((s, i) => s.matched && a.rules[i]?.include !== true);
    const hasInclude = a.rules.some((r) => r.include === true && r.pattern.trim() !== "");
    const included = steps.some((s, i) => s.matched && a.rules[i]?.include === true);
    return { output: a.input, included: !excluded && (!hasInclude || included), steps };
  }
  const url = new URL(a.input);
  if (a.removeAllParameters) url.search = "";
  for (const p of a.removeParameters ?? []) url.searchParams.delete(p);
  let output = url.toString();
  const steps = a.rules.map((r, index) => {
    const before = output;
    try {
      output = output.replace(new RegExp(r.pattern), r.replacement ?? "");
      return { index, matched: before !== output, before, after: output, error: null };
    } catch {
      return { index, matched: false, before, after: before, error: "invalid regular expression" };
    }
  });
  return { output, included: null, steps };
}

/** Installiert die Handler im Mock-Kern; gibt den veränderbaren Zustand zurück. */
export function installConfigCore(core: MockCore, initial: ConfigJson = defaultConfig()): FakeConfigCore {
  const st: FakeConfigCore = { config: structuredClone(initial), setCalls: [], profiles: new Map(), defaultProfile: null, mode: "free", failNextSet: null, pauseCalls: 0 };
  const dto = (config: ConfigJson): CrawlConfigDto => ({ schemaVersion: 2, generator: "openspider test", config: structuredClone(config) as CrawlConfigDto["config"] });
  const info = (name: string): ConfigProfileInfo => ({ name, isDefault: st.defaultProfile === name, modifiedAt: "2026-10-20T10:00:00.000Z", path: `/profiles/${name}.osconfig` });
  const h = core.handlers;
  h.set("config.get", () => dto(st.config));
  h.set("config.schema", () => structuredClone(SCHEMA));
  h.set("config.validate", (args) => validate((args as CrawlConfigDto).config as ConfigJson));
  h.set("config.diff", () => {
    const base = defaultConfig();
    return changedPaths(st.config, base)
      .filter((p) => p !== "generator" && p !== "schema_version")
      .map((path): DiffRow => ({ path, labelKey: `config-${path}`, current: (getPath(st.config, path) ?? null) as DiffRow["current"], base: (getPath(base, path) ?? null) as DiffRow["base"] }));
  });
  h.set("config.set", (args) => {
    const next = (args as CrawlConfigDto).config as ConfigJson;
    st.setCalls.push(structuredClone(args as CrawlConfigDto));
    if (st.failNextSet) {
      const f = st.failNextSet;
      st.failNextSet = null;
      throw err(f.code, f.messageKey, f.field, f.detail);
    }
    const found = validate(next);
    if (found[0]) throw err("InvalidInput", "err-config-validation", found[0].path);
    const paths = changedPaths(st.config, next).filter((p) => p !== "generator" && p !== "schema_version");
    if (st.mode === "running") {
      const bad = paths.find((p) => !isLiveField(p));
      if (bad) throw err("CrawlRunning", "err-config-pause-required", bad);
    } else if (st.mode === "paused") {
      const bad = paths.find((p) => STRUCTURAL_FIELDS.includes(p));
      if (bad) throw err("InvalidInput", "err-config-new-crawl-required", bad);
    }
    st.config = structuredClone(next);
    return { invalidatedSteps: [], deferred: st.mode === "running" ? paths.filter((p) => p !== "speed.threads") : [] };
  });
  h.set("config.testRules", (args) => testRules(args as TestRulesArgs));
  h.set("crawl.validateStartUrl", (args) => {
    try {
      return { normalized: new URL((args as { input: string }).input).href };
    } catch {
      throw err("InvalidInput", "err-url", "input");
    }
  });
  h.set("config.testRobots", (args) => {
    const a = args as { url: string; content?: string | null };
    const content = a.content ?? "User-agent: *\nDisallow: /live-blocked/\n";
    const path = new URL(a.url).pathname;
    let line: number | null = null;
    content.split(/\r?\n/).forEach((l, i) => {
      const m = /^\s*Disallow:\s*(\S+)/i.exec(l);
      if (m && path.startsWith(m[1] as string) && line === null) line = i + 1;
    });
    return { allowed: line === null, matchedLine: line, userAgent: "OpenSpider" };
  });
  h.set("config.loadRobots", () => "User-agent: *\nDisallow: /live-blocked/\n");
  h.set("profile.list", () => [...st.profiles.keys()].sort().map(info));
  h.set("profile.load", (args) => {
    const name = (args as { name: string }).name;
    const p = st.profiles.get(name);
    if (!p) throw err("NotFound", "err-profile-not-found");
    if (st.mode === "running") throw err("CrawlRunning", "err-config-pause-required", "profile");
    return dto(p);
  });
  h.set("profile.save", (args) => {
    const a = args as { name: string; config: CrawlConfigDto; overwrite: boolean };
    if (st.profiles.has(a.name) && !a.overwrite) throw err("InvalidInput", "err-file-exists", "name");
    st.profiles.set(a.name, structuredClone(a.config.config as ConfigJson));
    return info(a.name);
  });
  h.set("profile.delete", (args) => {
    st.profiles.delete((args as { name: string }).name);
    if (st.defaultProfile === (args as { name: string }).name) st.defaultProfile = null;
    return null;
  });
  h.set("profile.setDefault", (args) => {
    st.defaultProfile = (args as { name: string | null }).name ?? null;
    return null;
  });
  h.set("crawl.pause", () => {
    st.pauseCalls += 1;
    return null;
  });
  return st;
}

/** Hilfsfunktion für Tests: Konfiguration mit gesetzten Pfaden. */
export function withConfig(paths: Record<string, unknown>): ConfigJson {
  let cfg = defaultConfig();
  for (const [p, v] of Object.entries(paths)) cfg = setPath(cfg, p, v);
  return cfg;
}
