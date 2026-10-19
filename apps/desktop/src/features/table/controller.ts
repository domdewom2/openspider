import { useTableSelection } from "./selection";
import { ViewDataSource } from "../../data/ViewDataSource";
import { view } from "../../ipc/view";
import { catalog } from "../../ipc/catalog";
import { isSilentError } from "../../ipc/errors";
import type { FilterCounts } from "../../ipc/gen/FilterCounts";
import type { ViewSpec } from "../../ipc/gen/ViewSpec";
import type { Tick } from "../../ipc/gen/Tick";
import { SelectionModel } from "../../state/selection";
import type { TabQuery } from "../../state/tabs";

export function tabSpec(tab: string, query: TabQuery): ViewSpec {
  return { kind: "tab", tab, filter: query.filter, search: query.search ? { kind: "simple", text: query.search } : null, sort: query.sort };
}
/** Owns one active view. An open finishing after a tab switch is closed immediately. */
export class TableController {
  source?: ViewDataSource;
  selection = SelectionModel.empty();
  counts: FilterCounts = { version: -1, filters: [] };
  error: unknown = null;
  pending = false;
  private generation = 0;
  private countsRequest = 0;
  private listeners = new Set<() => void>();
  private revision = 0;
  private unsubscribe?: () => void;
  private disposed = false;
  private opening: Promise<void> = Promise.resolve();
  subscribe = (cb: () => void) => { this.listeners.add(cb); return () => { this.listeners.delete(cb); }; };
  snapshot = () => this.revision;
  private emit() { this.revision++; for (const cb of this.listeners) cb(); }
  fail = (error: unknown) => { if (!this.disposed && !isSilentError(error)) { this.error = error; this.emit(); } };
  setSelection = (selection: SelectionModel) => { this.selection = selection; useTableSelection.setState({ selection, viewId: this.source?.viewId, version: this.source?.version }); this.emit(); };
  activate() { this.disposed = false; }
  resetCrawl() { this.countsRequest++; this.counts = { version: -1, filters: [] }; this.setSelection(SelectionModel.empty()); }
  async open(tab: string, query: TabQuery): Promise<void> {
    const generation = ++this.generation;
    const previous = this.source;
    this.source = undefined; this.unsubscribe?.(); this.pending = true; this.error = null; this.emit();
    // Retain explicit URL ids across tabs; an all-selection belongs to its old snapshot.
    if (this.selection.value.kind === "all") this.setSelection(SelectionModel.empty());
    if (previous) void previous.dispose().catch(this.fail);
    const job = async () => {
      try {
        const columns = await catalog.columns({ tab });
        if (generation !== this.generation || this.disposed) return;
        const handle = await view.open({ spec: tabSpec(tab, query), columns: columns.map(c => c.key) });
        if (generation !== this.generation || this.disposed) { await view.close({ viewId: handle.viewId }); return; }
        const source = new ViewDataSource(handle);
        source.setSorted(query.sort.length > 0); source.onError = this.fail;
        this.source = source; this.unsubscribe = source.subscribe(() => this.emit());
        useTableSelection.setState({ selection: this.selection, viewId: source.viewId, version: source.version });
        this.pending = false; this.emit();
        await source.ensureRange(0, 30);
      } catch (error) { if (generation === this.generation) { this.pending = false; this.fail(error); } }
    };
    this.opening = job();
    await this.opening;
  }
  async refreshCounts(): Promise<void> {
    const request = ++this.countsRequest;
    try {
      const counts = await catalog.filterCounts({});
      if (!this.disposed && request === this.countsRequest && counts.version >= this.counts.version) { this.counts = counts; this.emit(); }
    } catch (error) { this.fail(error); }
  }
  async tick(tick: Tick): Promise<void> {
    const jobs: Promise<void>[] = [];
    if (tick.countsVersion > this.counts.version) jobs.push(this.refreshCounts());
    const source = this.source;
    const changed = tick.views.find(v => v.viewId === source?.viewId);
    if (source && changed) jobs.push(source.onViewChanged(changed.version, changed.total ?? source.total));
    await Promise.all(jobs);
    if (source && source === this.source) this.setSelection(this.selection.forVersion(source.viewId, source.version));
  }
  async focus(id: number): Promise<number | null> {
    await this.opening;
    const source = this.source;
    if (!source) return null;
    const row = await source.locate(id);
    if (source !== this.source) return null;
    if (row === null) { if (this.selection.value.kind === "ids" && this.selection.value.ids.length === 1) this.setSelection(SelectionModel.empty()); return null; }
    await source.ensureRow(row);
    if (source !== this.source) return null;
    this.setSelection(this.selection.select(id)); source.setFocus(id, row);
    return row;
  }
  dispose() {
    this.disposed = true; this.generation++; this.countsRequest++; this.unsubscribe?.(); this.listeners.clear();
    void this.source?.dispose().catch(() => undefined); this.source = undefined;
    useTableSelection.setState({ selection: SelectionModel.empty(), viewId: undefined, version: undefined });
  }
}
