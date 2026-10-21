// Mock-Kern der Fach-Detailtabs für vitest und E2E-A (die Kern-Commands von M2-K09/D20/D21 sind noch nicht gemergt).
import { MockCore } from "../../../../ipc/transport/mock";
import type { ViewOpenArgs } from "../../../../ipc/gen/ViewOpenArgs";
import type { RowsArgs } from "../../../../ipc/gen/RowsArgs";
import type { LocateArgs } from "../../../../ipc/gen/LocateArgs";
import type { ViewCloseArgs } from "../../../../ipc/gen/ViewCloseArgs";
import type { ViewSpec } from "../../../../ipc/gen/ViewSpec";
import type { DiffHunk } from "../../../../ipc/gen/DiffHunk";
import type { DiffHunks } from "../../../../ipc/gen/DiffHunks";
import type { DiffLine } from "../../../../ipc/gen/DiffLine";
import type { SerpMeasureArgs } from "../../../../ipc/gen/SerpMeasureArgs";
import type { SerpMeasureRow } from "../../../../ipc/gen/SerpMeasureRow";
import type { SourceOpenArgs } from "../../../../ipc/gen/SourceOpenArgs";
import type { SourceLinesArgs } from "../../../../ipc/gen/SourceLinesArgs";
import type { SourceDiffArgs } from "../../../../ipc/gen/SourceDiffArgs";
import type { DuplicatesArgs } from "../../../../ipc/gen/DuplicatesArgs";
import type { SpellingContextArgs } from "../../../../ipc/gen/SpellingContextArgs";
import type { A11yNodeArgs } from "../../../../ipc/gen/A11yNodeArgs";
import type { SerpEditArgs } from "../../../../ipc/gen/SerpEditArgs";
import type { SerpEdit } from "../../../../ipc/gen/SerpEdit";
import type { ExportDetailArgs } from "../../../../ipc/gen/ExportDetailArgs";
import type { ColumnMeta } from "../../../../ipc/gen/ColumnMeta";
import type { EmptyState } from "../../../../ipc/gen/EmptyState";
import { useSession } from "../../../../state/session";
import { diffWords } from "../../duplicates/diff";
import * as data from "./data";

type Row = Record<string, string | number | boolean | null>;
export interface FachOptions {
  /** Überschreibt Abschnitte der Crawl-Konfiguration (flache Zusammenführung je Abschnitt). */
  config: Record<string, Record<string, unknown>>;
  /** Anzahl Zeilen der Quelltext-Dokumente (für die Größenprüfung); 0 = Fixture. */
  largeLines: number;
  /** Verzögerung der Diff-Operation in ms. */
  diffDelay: number;
}
export const DEFAULT_FACH_CONFIG = {
  rendering: { mode: "javascript", js_error_reporting: true, screenshots: { enabled: true } },
  extraction: { store_html: true, store_rendered_html: true, accessibility: true, structured_data: { json_ld: true, microdata: true, rdfa: true } },
  duplicates: { exact_enabled: true, near_enabled: true }, spelling: { enabled: true }, grammar: { enabled: false },
};
const LIMITS = { desktop: [561, 985], mobile: [561, 680], tablet: [561, 680] } as const;
const IMPACT = ["Critical", "Serious", "Moderate", "Minor", ""];

/** Deterministisches Breitenmodell des Mock-Kerns (das echte Messen gehört dem Kern, UI-038): W = 18,886 px, a = 7,8 px bei 14 px. */
export function mockWidth(text: string, fontPx: number, bold = false): number {
  let sum = 0;
  for (const ch of text) sum += ch === "W" ? 18.886 : ch === "a" ? 11.143 : ch === " " ? 5.5 : /[il.,'|!]/.test(ch) ? 5 : 10.5;
  return sum * (fontPx / 20) * (bold ? 1.07 : 1);
}
function measureField(text: string, fontPx: number, max: number, terms: string[]): SerpMeasureRow {
  const width = (value: string) => {
    if (terms.length === 0) return mockWidth(value, fontPx);
    const pattern = new RegExp(`(${terms.map(t => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|")})`, "giu");
    return value.split(pattern).reduce((sum, part, i) => sum + mockWidth(part, fontPx, i % 2 === 1), 0);
  };
  const chars = Array.from(text); const px = width(text);
  if (px <= max) return { chars: chars.length, px, maxPx: max, remaining: max - px, truncated: false, cutAt: null };
  let cut = 0; let at = 0;
  for (const word of text.split(" ")) { at += (at ? 1 : 0) + Array.from(word).length; if (width(chars.slice(0, at).join("") + " …") <= max) cut = at; else break; }
  if (cut === 0) while (cut < chars.length && width(chars.slice(0, cut + 1).join("") + " …") <= max) cut++;
  return { chars: chars.length, px, maxPx: max, remaining: max - px, truncated: true, cutAt: cut };
}
type Op = { kind: "context" | "added" | "removed"; text: string };
/** Hunks (Kontext 1) aus dem Myers-Ergebnis; Anfänge 1-basiert. */
export function mockHunks(a: string[], b: string[]): DiffHunks {
  const diff = diffWords(a, b); const ops: Op[] = []; let i = 0; let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && diff.removed[i]) ops.push({ kind: "removed", text: a[i++]! });
    else if (j < b.length && diff.added[j]) ops.push({ kind: "added", text: b[j++]! });
    else { ops.push({ kind: "context", text: a[i]! }); i++; j++; }
  }
  const changed = ops.map((o, index) => o.kind !== "context" ? index : -1).filter(index => index >= 0);
  const hunks: DiffHunk[] = []; let k = 0;
  while (k < changed.length) {
    let end = k; while (end + 1 < changed.length && changed[end + 1]! - changed[end]! <= 3) end++;
    const from = Math.max(0, changed[k]! - 1); const to = Math.min(ops.length - 1, changed[end]! + 1); const lines: DiffLine[] = ops.slice(from, to + 1).map(o => ({ kind: o.kind, text: o.text }));
    const before = ops.slice(0, from); const oldStart = before.filter(o => o.kind !== "added").length + 1; const newStart = before.filter(o => o.kind !== "removed").length + 1;
    hunks.push({ oldStart, oldLines: lines.filter(l => l.kind !== "added").length, newStart, newLines: lines.filter(l => l.kind !== "removed").length, lines });
    k = end + 1;
  }
  return { added: diff.addedCount, removed: diff.removedCount, hunks };
}

export function fachFixtureCore(initial: Partial<FachOptions> = {}) {
  const core = new MockCore(); const options: FachOptions = { config: {}, largeLines: 0, diffDelay: 0, ...initial };
  const edits = new Map<number, SerpEdit>(); const views = new Map<string, { spec: ViewSpec; rows: Row[]; keys: number[] }>();
  let next = 0; let seq = 100; let op = 0;
  const effective = () => { const out: Record<string, unknown> = structuredClone(DEFAULT_FACH_CONFIG); for (const [k, v] of Object.entries(options.config)) out[k] = { ...(out[k] as object), ...v }; return out; };
  const docs = new Map<string, string[]>();
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: effective() }));
  core.handlers.set("detail.urlDetails", raw => { const id = (raw as { urlId: number }).urlId; return [{ name: "Address", value: data.SERP[id]?.address ?? "https://site.test/" }, { name: "Crawl Timestamp", value: "2026-10-13T08:00:00Z" }]; });
  core.handlers.set("detail.serp", raw => { const { urlId } = raw as { urlId: number }; const base = data.SERP[urlId] ?? data.SERP[1]!; return { title: base.title, description: base.description, host: base.host, breadcrumb: base.breadcrumb, favicon: base.favicon, edited: edits.get(urlId) ?? null }; });
  core.handlers.set("detail.serpMeasure", raw => {
    const a = raw as SerpMeasureArgs; const [titleMax, descriptionMax] = LIMITS[a.device];
    return { rows: [measureField(a.title, 20, titleMax, a.boldTerms), measureField(a.prefix + a.description, 14, descriptionMax, a.boldTerms)] };
  });
  core.handlers.set("detail.serpSaveEdit", raw => { const a = raw as SerpEditArgs; edits.set(a.urlId, { title: a.title ?? null, description: a.description ?? null, editedAt: "2026-10-21T10:00:00Z" }); });
  core.handlers.set("detail.serpReset", raw => { edits.delete((raw as SerpEditArgs).urlId); });
  const document = (which: string, mode: string): string[] => {
    if (options.largeLines > 0 && mode === "html") return Array.from({ length: options.largeLines }, (_, i) => `<div class="row-${which}">line ${i + 1}</div>`);
    return mode === "text" ? (which === "original" ? data.ORIGINAL_TEXT : data.RENDERED_TEXT) : which === "original" ? data.ORIGINAL_HTML : data.RENDERED_HTML;
  };
  core.handlers.set("detail.sourceOpen", raw => {
    const a = raw as SourceOpenArgs; const lines = document(a.which, a.mode); const docId = `${a.which}:${a.mode}:${a.pretty}`; docs.set(docId, lines);
    return { docId, lines: lines.length, bytes: lines.join("\n").length };
  });
  core.handlers.set("detail.sourceLines", raw => { const a = raw as SourceLinesArgs; return (docs.get(a.docId) ?? []).slice(a.from, a.from + a.count); });
  core.handlers.set("detail.sourceDiff", raw => {
    const a = raw as SourceDiffArgs; const opId = `diff-${++op}`;
    const html = a.mode === "lines";
    const left = html ? document("original", "html") : data.ORIGINAL_TEXT.join(" ").split(/\s+/); const right = html ? document("rendered", "html") : data.RENDERED_TEXT.join(" ").split(/\s+/);
    const result = mockHunks(left, right);
    setTimeout(() => useSession.getState().applyTick({ seq: ++seq, at: "2026-10-21T10:00:00Z", views: [], countsVersion: 0, issuesVersion: 0, sidebarVersion: 0, notices: [],
      ops: [{ opId, labelKey: "detail-diff", done: 1, total: 1, state: "done", result: result as never }] }), options.diffDelay);
    return { opId, labelKey: "detail-diff", cancellable: true };
  });
  core.handlers.set("detail.screenshot", () => ({ url: "ospider://shot/fixture/1", width: 1024, height: 1536, tiles: 3 }));
  core.handlers.set("detail.structuredData", () => ({ nodes: structuredClone(data.SD_NODES) }));
  core.handlers.set("detail.duplicates", raw => {
    const a = raw as DuplicatesArgs; const entry = (a.kind === "exact" ? data.DUPLICATES.exact : data.DUPLICATES.near)[a.urlId];
    return entry ? { partners: structuredClone(entry.partners), text: entry.text } : { partners: [], text: "" };
  });
  core.handlers.set("detail.spellingContext", raw => { const a = raw as SpellingContextArgs; return structuredClone(data.SPELLING[a.findingId - 1]!.context); });
  core.handlers.set("detail.a11yNode", raw => { const a = raw as A11yNodeArgs; for (const [rule, nodes] of Object.entries(data.A11Y_NODES)) { const node = nodes.find(n => n.id === a.nodeId); if (node) return { ruleId: rule, impact: "Critical", target: node["Location on Page"], snippet: node.HTML, summaryKey: "a11y-summary-fixture" }; } throw new Error("unknown node"); });
  core.handlers.set("analysis.start", () => ({ opId: "analysis-1", labelKey: "analysis", cancellable: true }));
  core.handlers.set("export.catalog", () => ({ bulk: [{ key: "JavaScript", labelKey: "export-group-javascript", items: [{ name: "JavaScript:Pages With JavaScript Issues", labelKey: "x", enabled: true }] }],
    reports: [{ key: "JavaScript", labelKey: "export-group-javascript", items: [{ name: "JavaScript:JavaScript Console Log Summary", labelKey: "y", enabled: true }] }], sitemaps: [] }));
  core.handlers.set("export.detail", raw => ({ opId: `export-${(raw as ExportDetailArgs).format}`, labelKey: "detail-export", cancellable: false }));

  const tableRows = (spec: ViewSpec & { kind: "detail" }): { rows: Row[]; keys: number[]; empty?: EmptyState } => {
    const params = (spec.params ?? {}) as Record<string, string>; const cfg = effective() as typeof DEFAULT_FACH_CONFIG;
    switch (spec.detail) {
      case "consoleLog": { const rows = data.CONSOLE.filter(r => !params.level || r.Level === params.level); return { rows, keys: rows.map((_, i) => i + 1) }; }
      case "blockedResources": return { rows: data.BLOCKED, keys: data.BLOCKED.map((_, i) => i + 1) };
      case "network": return { rows: data.NETWORK, keys: data.NETWORK.map((_, i) => i + 1) };
      case "spelling": {
        const all = data.SPELLING.map((r, i) => ({ row: r as unknown as Row, key: i + 1 })).filter(x => (!params.section || x.row["Page Section"] === params.section) && (!params.type || x.row.Type === params.type));
        const disabled = !cfg.spelling.enabled && !(cfg.grammar as { enabled: boolean }).enabled;
        return disabled ? { rows: [], keys: [], empty: { code: "FEATURE_DISABLED", arg: "spelling.enabled", action: { kind: "openSetting", anchor: "spelling.enabled" } } }
          : { rows: all.map(x => x.row), keys: all.map(x => x.key) };
      }
      case "a11yRules": {
        let rows = data.A11Y_RULES.filter(r => !params.result || r.Result === params.result) as Row[];
        const sort = spec.sort; if (sort[0]?.column === "Impact") rows = [...rows].sort((x, y) => IMPACT.indexOf(String(x.Impact)) - IMPACT.indexOf(String(y.Impact)) || Number(y.Nodes || 0) - Number(x.Nodes || 0) || String(x.Issue).localeCompare(String(y.Issue)));
        return { rows, keys: rows.map((_, i) => i + 1) };
      }
      case "a11yNodes": { const nodes = data.A11Y_NODES[params.rule ?? ""] ?? []; return { rows: nodes as unknown as Row[], keys: nodes.map(n => n.id) }; }
      default: return { rows: [], keys: [] };
    }
  };
  core.handlers.set("view.open", raw => {
    const { spec, columns } = raw as ViewOpenArgs; if (spec.kind !== "detail") throw new Error(`unsupported view ${spec.kind}`);
    const { rows, keys, empty } = tableRows(spec); const viewId = `fach-${++next}`; views.set(viewId, { spec, rows, keys });
    return { viewId, version: 1, total: rows.length, columns: columns.map(key => ({ key, kind: { kind: "text" }, defaultVisible: true, defaultWidth: 160, align: "left", sortable: true, searchable: ["contains"], extension: false } satisfies ColumnMeta)), state: "ready", emptyState: empty ?? null };
  });
  core.handlers.set("view.rows", raw => {
    const a = raw as RowsArgs; const view = views.get(a.viewId)!; const rows = view.rows.slice(a.offset, a.offset + a.limit);
    return { viewId: a.viewId, version: a.version, offset: a.offset, columns: a.columns, rowKeys: view.keys.slice(a.offset, a.offset + a.limit), cells: rows.map(row => a.columns.map(c => row[c] ?? null)) };
  });
  core.handlers.set("view.locate", raw => { const a = raw as LocateArgs; const i = views.get(a.viewId)?.keys.indexOf(a.urlId) ?? -1; return i >= 0 ? i : null; });
  core.handlers.set("view.close", raw => { views.delete((raw as ViewCloseArgs).viewId); });
  core.handlers.set("view.copy", () => ({ rows: 1, cells: 1 }));
  return { core, options, views, edits };
}
