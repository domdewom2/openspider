import { view } from "../ipc/view";
import { isSilentError } from "../ipc/errors";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import type { RowWindow } from "../ipc/gen/RowWindow";

export const LOADING = Symbol("loading");
export const BLOCK_SIZE = 200;
export const MAX_BLOCKS = 100;
export type CellValue = RowWindow["cells"][number][number];
export type ViewApi = Pick<typeof view, "rows" | "locate" | "copy" | "close">;
type Block = { keys: number[]; cells: CellValue[][] };

/** Canvas reads synchronously; only bounded row windows live in memory. */
export class ViewDataSource {
  readonly viewId: string;
  readonly columns: ViewHandle["columns"];
  version: number;
  total: number;
  private readonly blocks = new Map<number, Block>();
  private readonly pending = new Map<number, Promise<void>>();
  private controller = new AbortController();
  private generation = 0;
  private disposed = false;
  private listeners = new Set<() => void>();
  private first = 0;
  private last = 0;
  private focus: { key: number; row: number } | undefined;
  private tail = false;
  private sorted = false;
  onScrollTo?: (row: number) => void;
  onError?: (error: unknown) => void;

  constructor(handle: ViewHandle, readonly api: ViewApi = view) {
    this.viewId = handle.viewId;
    this.version = handle.version;
    this.total = handle.total ?? 0;
    this.columns = handle.columns;
  }
  get cachedRows(): number { return [...this.blocks.values()].reduce((n, b) => n + b.keys.length, 0); }
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
  private emit() { for (const listener of this.listeners) listener(); }
  private block(row: number): Block | undefined {
    const index = Math.floor(row / BLOCK_SIZE);
    const block = this.blocks.get(index);
    if (block) { this.blocks.delete(index); this.blocks.set(index, block); }
    return block;
  }
  getCell(col: number, row: number): CellValue | typeof LOADING {
    if (row < 0 || row >= this.total || col < 0 || col >= this.columns.length) return null;
    const block = this.block(row);
    return block ? block.cells[row % BLOCK_SIZE]?.[col] ?? null : LOADING;
  }
  rowKey(row: number): number | undefined { return this.block(row)?.keys[row % BLOCK_SIZE]; }
  setFocus(key: number, row: number): void { this.focus = { key, row }; }
  setSorted(sorted: boolean): void { this.sorted = sorted; if (sorted) this.tail = false; }
  /** Visible range is inclusive. Two screenfuls of prefetch on either side. */
  async ensureRange(first: number, last: number): Promise<void> {
    if (this.disposed || !this.total) return;
    this.first = Math.max(0, Math.min(Math.floor(first), this.total - 1));
    this.last = Math.max(this.first, Math.min(Math.floor(last), this.total - 1));
    this.tail = !this.sorted && this.last === this.total - 1;
    const visibleFirst = this.first;
    const visibleLast = this.last;
    const generation = this.generation;
    const height = Math.min(BLOCK_SIZE * 10, visibleLast - visibleFirst + 1);
    // Visible blocks finish before speculative requests; a jump never queues behind the buffer.
    await this.loadRange(visibleFirst, Math.min(visibleLast, visibleFirst + BLOCK_SIZE * MAX_BLOCKS - 1));
    if (generation !== this.generation || this.disposed || this.first !== visibleFirst || this.last !== visibleLast) return;
    await Promise.all([
      this.loadRange(Math.max(0, visibleFirst - height * 2), visibleFirst - 1),
      this.loadRange(visibleLast + 1, Math.min(this.total - 1, visibleLast + height * 2)),
    ]);
  }
  async ensureRow(row: number): Promise<void> {
    if (!this.disposed && row >= 0 && row < this.total) await this.load(Math.floor(row / BLOCK_SIZE));
  }
  private async loadRange(first: number, last: number): Promise<void> {
    const jobs: Promise<void>[] = [];
    for (let b = Math.floor(first / BLOCK_SIZE); b <= Math.floor(last / BLOCK_SIZE) && first <= last; b++) jobs.push(this.load(b));
    await Promise.all(jobs);
  }
  private load(index: number): Promise<void> {
    if (this.blocks.has(index)) return Promise.resolve();
    const existing = this.pending.get(index);
    if (existing) return existing;
    const generation = this.generation;
    const offset = index * BLOCK_SIZE;
    const columns = this.columns.map(c => c.key);
    const job = this.api.rows({ viewId: this.viewId, version: this.version, offset, limit: BLOCK_SIZE, columns }, this.controller.signal)
      .then(result => {
        if (generation !== this.generation || this.disposed) return;
        if (result.version !== this.version || result.viewId !== this.viewId) return;
        if (result.offset !== offset || result.columns.join("\0") !== columns.join("\0") || result.rowKeys.length !== result.cells.length || result.cells.length > BLOCK_SIZE) {
          throw new Error("Invalid row window");
        }
        this.blocks.set(index, { keys: result.rowKeys, cells: result.cells });
        while (this.blocks.size > MAX_BLOCKS) this.blocks.delete(this.blocks.keys().next().value!);
        this.emit();
      })
      .catch(error => { if (generation === this.generation && !this.disposed && !isSilentError(error)) this.onError?.(error); })
      .finally(() => { if (generation === this.generation) this.pending.delete(index); });
    this.pending.set(index, job);
    return job;
  }
  async locate(key: number): Promise<number | null> {
    const generation = this.generation;
    const row = await this.api.locate({ viewId: this.viewId, urlId: key }, this.controller.signal);
    return generation === this.generation && !this.disposed ? row : null;
  }
  async onViewChanged(version: number, total: number): Promise<void> {
    if (version <= this.version || this.disposed) return;
    const anchor = this.focus ?? { key: this.rowKey(this.first), row: this.first };
    const relative = anchor.row - this.first;
    const height = this.last - this.first;
    const tail = this.tail;
    this.controller.abort();
    this.controller = new AbortController();
    const generation = ++this.generation;
    this.version = version;
    this.total = total;
    this.blocks.clear(); this.pending.clear();
    this.emit();
    let first = Math.min(this.first, Math.max(0, total - 1));
    if (tail) first = Math.max(0, total - height - 1);
    else if (anchor.key !== undefined) {
      try {
        const row = await this.locate(anchor.key);
        if (row !== null) { first = Math.max(0, row - relative); if (this.focus) this.focus = { key: anchor.key, row }; }
      } catch (error) { if (!isSilentError(error)) this.onError?.(error); }
    }
    if (generation !== this.generation || this.disposed) return;
    this.onScrollTo?.(first);
    await this.ensureRange(first, Math.min(total - 1, first + height));
  }
  async keys(first: number, last: number): Promise<number[]> {
    const generation = this.generation;
    const keys: number[] = [];
    // Selection is explicit URL ids; read bounded windows, never retain the entire range as cells.
    for (let row = Math.max(0, first); row <= Math.min(last, this.total - 1); row += BLOCK_SIZE) {
      const result = await this.api.rows({ viewId: this.viewId, version: this.version, offset: row, limit: Math.min(BLOCK_SIZE, last - row + 1), columns: [] }, this.controller.signal);
      if (generation !== this.generation || result.version !== this.version) return [];
      keys.push(...result.rowKeys);
    }
    return keys;
  }
  async dispose(): Promise<void> {
    if (this.disposed) return;
    this.disposed = true; this.generation++; this.controller.abort();
    this.blocks.clear(); this.pending.clear(); this.listeners.clear();
    await this.api.close({ viewId: this.viewId });
  }
}
