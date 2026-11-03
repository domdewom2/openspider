// Test-Kern für die `viz.*`-Commands (vitest und E2E-Bridge in e2e/viz): Schnappschüsse aus synthetischen Bäumen,
// Antworten mit der Semantik des Kerns (Budget, Teilbaum öffnen, Fehler), Ereignisse an Fenster.
import type { CmdError } from "../../../ipc/gen/CmdError";
import type { GraphDto } from "../../../ipc/gen/GraphDto";
import type { UiEvent } from "../../../ipc/gen/UiEvent";
import type { VizExportArgs } from "../../../ipc/gen/VizExportArgs";
import type { VizGraphArgs } from "../../../ipc/gen/VizGraphArgs";
import type { VizOpenArgs } from "../../../ipc/gen/VizOpenArgs";
import type { VizWordsArgs } from "../../../ipc/gen/VizWordsArgs";
import type { WordFreq } from "../../../ipc/gen/WordFreq";
import type { Transport } from "../../../ipc/transport/types";
import { windowLabelFor } from "../labels";
import { fxUiBasic, fxUiBodyWords, fxUiDirs, fxUiWords, selectGraph, selectLinks, type SynthTree } from "./synthetic";

export interface FakeCall {
  cmd: string;
  args: unknown;
  window: string;
}

const err = (code: CmdError["code"], messageKey: string, field?: string): { cmdError: CmdError } => ({ cmdError: { code, messageKey, args: {}, field: field ?? null, retryable: false } });

interface Snap {
  kind: string;
  root: number | null;
  last: GraphDto | null;
  lastKey: string | null;
  explored: number[];
}

export class FakeVizCore {
  readonly calls: FakeCall[] = [];
  /** Letzter angenommener CSV-Export (Kanten mit `edgeType`, UI-090/097). */
  lastExport: VizExportArgs | null = null;
  readonly snapshots = new Map<string, Snap>();
  private readonly subs = new Map<string, Set<(e: UiEvent) => void>>();
  private next = 0;
  private op = 0;
  /** Verzögerung je `viz.graph`-Antwort (ms), z. B. für Ladeanzeigen. */
  delayMs = 0;
  /** Body-Wortwolke (UI-100): Begriffe der Zielseite; `false` = kein HTML gespeichert. */
  bodyWords: (() => WordFreq[]) | false = fxUiBodyWords;

  constructor(
    public crawl: () => SynthTree = fxUiBasic,
    public directories: () => SynthTree = fxUiDirs,
    public words: () => WordFreq[] = fxUiWords,
  ) {}

  private trees = new Map<string, SynthTree>();
  private tree(kind: string): SynthTree {
    const key = kind.startsWith("directory") ? "dir" : "crawl";
    let t = this.trees.get(key);
    if (!t) this.trees.set(key, (t = key === "dir" ? this.directories() : this.crawl()));
    return t;
  }

  /** Neue Daten (z. B. nach erneutem Crawlen) – Schnappschüsse sehen sie erst nach `viz.open` mit `refreshSnapshotId`. */
  replaceCrawl(next: () => SynthTree): void {
    this.crawl = next;
  }

  emit(window: string, e: UiEvent): void {
    for (const cb of this.subs.get(window) ?? []) cb(e);
  }

  /** Meldung „Daten geändert“ wie der Kern (`viz-data-changed` mit `snapshotId`). */
  notifyChanged(snapshotId: string): void {
    this.emit(windowLabelFor(snapshotId), { kind: "notice", notice: { level: "info", code: "viz-data-changed", args: { snapshotId } } });
  }

  async invoke(cmd: string, args: unknown, window = "main"): Promise<unknown> {
    this.calls.push({ cmd, args, window });
    switch (cmd) {
      case "viz.open":
        return this.open(args as VizOpenArgs);
      case "viz.graph":
        if (this.delayMs) await new Promise((r) => setTimeout(r, this.delayMs));
        return this.graph(args as VizGraphArgs);
      case "viz.words":
        return this.wordList(args as VizWordsArgs);
      case "viz.exportCsv":
        return this.exportCsv(args as VizExportArgs);
      case "viz.focusInMain":
        return null;
      case "settings.get":
      case "uistate.load":
        throw err("Unsupported", "err-unsupported");
      default:
        return null;
    }
  }

  private open(a: VizOpenArgs): { snapshotId: string; window: string } {
    if (a.refreshSnapshotId) {
      const s = this.snapshots.get(a.refreshSnapshotId);
      if (!s) throw err("Stale", "err-viz-snapshot-closed");
      this.trees.clear();
      s.last = null;
      return { snapshotId: a.refreshSnapshotId, window: windowLabelFor(a.refreshSnapshotId) };
    }
    const id = `snap-${++this.next}`;
    this.snapshots.set(id, { kind: a.kind, root: a.rootUrlId ?? null, last: null, lastKey: null, explored: [] });
    return { snapshotId: id, window: windowLabelFor(id) };
  }

  private graph(a: VizGraphArgs): GraphDto {
    const s = this.snapshots.get(a.snapshotId);
    if (!s) throw err("Stale", "err-viz-snapshot-closed");
    if (a.kind === "anchorWords" || a.kind === "bodyWords") throw err("Unsupported", "err-viz-kind");
    const max = a.kind.endsWith("3d") ? 100_000 : 50_000;
    if (!(a.budget >= 1 && a.budget <= max)) throw err("InvalidInput", "err-invalid-input", "budget");
    const base = JSON.stringify({ ...a, exploreNode: null, linksNode: null, linksMode: null, linksIncludeNofollow: null, linksExcludeNavFooter: null });
    const explored = a.exploreNode != null && s.lastKey === base ? [...s.explored, a.exploreNode] : a.exploreNode != null ? [a.exploreNode] : [];
    try {
      const dto = selectGraph(this.tree(a.kind), {
        root: a.root ?? s.root,
        budget: a.budget,
        expandDepth: a.expandDepth ?? 3,
        explore: explored,
        metric: a.metric,
        colorMetric: a.colorMetric ?? null,
      });
      if (a.linksNode != null) {
        // Links eines Knotens (UI-097) wie im Kern: eindeutige Paare gemäß Optionen, Kanten nur zwischen gezeigten Knoten.
        const mode = a.linksMode ?? "inlinks";
        if (!["inlinks", "childInlinks", "outlinks", "childOutlinks"].includes(mode)) throw new Error("linksMode");
        const pairs = selectLinks(this.tree(a.kind), a.linksNode, mode, a.linksIncludeNofollow ?? true, a.linksExcludeNavFooter ?? false);
        const shown = new Set(dto.nodes.map((n) => n.id));
        const type = mode === "inlinks" || mode === "childInlinks" ? "inlink" : "outlink";
        dto.edges.push(...pairs.filter(([x, y]) => shown.has(x) && shown.has(y)).map(([source, target]) => ({ source, target, edgeType: type })));
        dto.linkTotal = pairs.length;
      }
      s.last = dto;
      s.lastKey = base;
      s.explored = explored;
      return dto;
    } catch (e) {
      throw err("InvalidInput", "err-invalid-input", (e as Error).message);
    }
  }

  private wordList(a: VizWordsArgs): WordFreq[] {
    if (!this.snapshots.has(a.snapshotId)) throw err("Stale", "err-viz-snapshot-closed");
    if (a.kind === "body") {
      if (!this.bodyWords) throw { cmdError: { code: "Unsupported", messageKey: "err-viz-html-not-stored", args: { state: { code: "HTML_NOT_STORED" } }, field: null, retryable: false } satisfies CmdError };
      return this.bodyWords()
        .filter((w) => [...w.word].length >= (a.minLength ?? 3))
        .slice(0, a.maxTerms ?? 100);
    }
    return this.words().slice(0, a.maxTerms ?? 100);
  }

  private exportCsv(a: VizExportArgs): { opId: string; labelKey: string; cancellable: boolean } {
    const s = this.snapshots.get(a.snapshotId);
    if (!s?.last || a.nodes.length !== s.last.nodes.length || a.edges.length !== s.last.edges.length) throw err("InvalidInput", "err-invalid-input", "nodes");
    this.lastExport = a;
    const opId = `op-${++this.op}`;
    // Operationen stehen wie im Kern im Tick jedes Fensters.
    setTimeout(() => {
      for (const window of this.subs.keys())
        this.emit(window, {
          kind: "tick",
          tick: { seq: this.op, at: "2026-10-19T00:00:00Z", views: [], countsVersion: 0, issuesVersion: 0, sidebarVersion: 0, notices: [], ops: [{ opId, labelKey: "op-viz-export", done: 1, total: 1, state: "done", result: { path: a.dir, bytes: 1 } }] },
        });
    }, 5);
    return { opId, labelKey: "op-viz-export", cancellable: true };
  }

  transport(window = "main"): Transport {
    return {
      name: "mock",
      // Fehler wie über die Bridge: `{ cmdError }` wird zum CmdError (sonst sähe die Oberfläche nur „unerwartet“).
      invoke: <T,>(cmd: string, args: unknown) =>
        (this.invoke(cmd, args, window) as Promise<T>).catch((e: unknown) => {
          throw (e as { cmdError?: unknown } | null)?.cmdError ?? e;
        }),
      subscribe: (w, cb) => {
        let set = this.subs.get(w);
        if (!set) this.subs.set(w, (set = new Set()));
        set.add(cb);
        return () => set?.delete(cb);
      },
    };
  }
}
