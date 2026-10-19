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
import { fxUiBasic, fxUiDirs, fxUiWords, selectGraph, type SynthTree } from "./synthetic";

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
  readonly snapshots = new Map<string, Snap>();
  private readonly subs = new Map<string, Set<(e: UiEvent) => void>>();
  private next = 0;
  private op = 0;
  /** Verzögerung je `viz.graph`-Antwort (ms), z. B. für Ladeanzeigen. */
  delayMs = 0;

  constructor(
    public crawl: () => SynthTree = fxUiBasic,
    public directories: () => SynthTree = fxUiDirs,
    public words: () => WordFreq[] = fxUiWords,
  ) {}

  private trees = new Map<string, SynthTree>();
  private tree(kind: string): SynthTree {
    const key = kind === "directoryTree" ? "dir" : "crawl";
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
    if (a.kind !== "crawlTree" && a.kind !== "crawlGraph" && a.kind !== "directoryTree") throw err("Unsupported", "err-viz-kind");
    if (!(a.budget >= 1 && a.budget <= 50_000)) throw err("InvalidInput", "err-invalid-input", "budget");
    const base = JSON.stringify({ ...a, exploreNode: null });
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
    return this.words().slice(0, a.maxTerms ?? 100);
  }

  private exportCsv(a: VizExportArgs): { opId: string; labelKey: string; cancellable: boolean } {
    const s = this.snapshots.get(a.snapshotId);
    if (!s?.last || a.nodes.length !== s.last.nodes.length || a.edges.length !== s.last.edges.length) throw err("InvalidInput", "err-invalid-input", "nodes");
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
      invoke: <T,>(cmd: string, args: unknown) => this.invoke(cmd, args, window) as Promise<T>,
      subscribe: (w, cb) => {
        let set = this.subs.get(w);
        if (!set) this.subs.set(w, (set = new Set()));
        set.add(cb);
        return () => set?.delete(cb);
      },
    };
  }
}
