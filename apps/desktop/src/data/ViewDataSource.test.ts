import { describe, expect, it, vi } from "vitest";
import { LOADING, ViewDataSource, type ViewApi } from "./ViewDataSource";
import type { ViewHandle } from "../ipc/gen/ViewHandle";
import type { RowWindow } from "../ipc/gen/RowWindow";
import fixture from "../grid/fixtures/fx-ui-basic.json";
function setup() {
  const api: ViewApi = {
    rows: vi.fn(async (a: Parameters<ViewApi["rows"]>[0]) => ({ ...a, rowKeys: Array.from({ length: Math.min(a.limit, fixture.total - a.offset) }, (_, i) => a.offset + i + 1),
      cells: Array.from({ length: Math.min(a.limit, fixture.total - a.offset) }, (_, i) => a.columns.map((_, col) => col === 0 ? a.offset + i + 1 : fixture.row[col])) })),
    locate: vi.fn(async a => a.urlId - 1), copy: vi.fn(), close: vi.fn(),
  };
  return { source: new ViewDataSource(fixture as ViewHandle, api), api };
}
describe("gen-ui-large", () => {
  // spec: UI-140
  it("deduplicates aligned 200-row blocks and fetches two screens of buffer", async () => {
    const { source, api } = setup();
    expect(source.getCell(0, 199)).toBe(LOADING);
    await Promise.all([source.ensureRange(190, 210), source.ensureRange(190, 210)]);
    expect(vi.mocked(api.rows).mock.calls.map(([a]) => [a.offset, a.limit])).toEqual([[0, 200], [200, 200]]);
    expect(source.getCell(0, 199)).toBe(200);
    await source.ensureRange(1000, 1099);
    expect(vi.mocked(api.rows).mock.calls.map(([a]) => a.offset)).toEqual([0, 200, 1000, 800, 1200]);
  });
  // spec: UI-140
  it("bounds cache at 20,000 rows and preserves recently read blocks", async () => {
    const { source } = setup();
    await source.ensureRange(0, 0);
    for (let i = 1; i <= 110; i++) { source.getCell(0, 0); await source.ensureRange(i * 600, i * 600); }
    expect(source.cachedRows).toBeLessThanOrEqual(20000);
    expect(source.getCell(0, 0)).toBe(1);
    expect(source.getCell(0, 600)).toBe(LOADING);
    await source.ensureRange(4_999_999, 4_999_999);
    expect(source.getCell(0, 4_999_999)).toBe(5_000_000);
  });
  // spec: UI-140
  it("discards late responses from an older version even if cancellation is ignored", async () => {
    const { source, api } = setup();
    let resolve!: (r: RowWindow) => void;
    vi.mocked(api.rows).mockImplementationOnce(() => new Promise(r => { resolve = r; }));
    const old = source.ensureRange(0, 0);
    await source.onViewChanged(2, fixture.total);
    resolve({ viewId: fixture.viewId, version: 1, offset: 0, columns: fixture.columns.map(c => c.key), rowKeys: [999], cells: [[999]] });
    await old;
    expect(source.rowKey(0)).toBe(1);
    expect(source.version).toBe(2);
  });
  // spec: UI-140
  it("relocates the focus anchor at its screen offset and follows tail only until manual upward scroll", async () => {
    const { source, api } = setup();
    const scroll = vi.fn(); source.onScrollTo = scroll;
    await source.ensureRange(500, 519); source.setFocus(511, 510);
    vi.mocked(api.locate).mockResolvedValue(610);
    await source.onViewChanged(2, fixture.total);
    expect(scroll).toHaveBeenLastCalledWith(600);
    await source.ensureRange(fixture.total - 20, fixture.total - 1);
    await source.onViewChanged(3, fixture.total + 10);
    expect(scroll).toHaveBeenLastCalledWith(fixture.total - 10);
    await source.ensureRange(100, 119);
    await source.onViewChanged(4, fixture.total + 20);
    expect(scroll).not.toHaveBeenLastCalledWith(fixture.total);
  });
  // spec: UI-140
  it("does not tail in sorted views, ignores older ticks, and releases all cached state on close", async () => {
    const { source, api } = setup(); const scroll = vi.fn(); source.onScrollTo = scroll;
    source.setSorted(true); await source.ensureRange(fixture.total - 20, fixture.total - 1);
    await source.onViewChanged(2, fixture.total + 10);
    expect(scroll).toHaveBeenLastCalledWith(fixture.total - 20);
    await source.onViewChanged(1, 0); expect(source.total).toBe(fixture.total + 10);
    await source.dispose(); await source.dispose();
    expect(source.cachedRows).toBe(0); expect(api.close).toHaveBeenCalledTimes(1);
  });
  // spec: UI-142
  it("keeps the previous rows on screen until the new version is complete and then swaps rows, total and scroll together (A01-07)", async () => {
    const { source, api } = setup();
    const scroll = vi.fn(); source.onScrollTo = scroll;
    await source.ensureRange(0, 19);
    expect(source.getCell(0, 5)).toBe(6);
    let release!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const original = vi.mocked(api.rows).getMockImplementation()!;
    vi.mocked(api.rows).mockImplementationOnce(async a => { await gate; return original(a); });
    const change = source.onViewChanged(2, fixture.total + 3);
    await Promise.resolve();
    // While the new window is on its way nothing is cleared: no placeholders, old version and total stay in place.
    expect(source.getCell(0, 5)).toBe(6);
    expect(source.version).toBe(1); expect(source.total).toBe(fixture.total);
    expect(scroll).not.toHaveBeenCalled();
    release(); await change;
    expect(source.version).toBe(2); expect(source.total).toBe(fixture.total + 3);
    expect(source.getCell(0, 5)).toBe(6);
    expect(scroll).toHaveBeenCalledTimes(1);
  });
  // spec: UI-142
  it("coalesces announcements that arrive during a swap and ends on the latest version (A01-20)", async () => {
    const { source, api } = setup();
    await source.ensureRange(0, 19);
    let release!: () => void;
    const gate = new Promise<void>(resolve => { release = resolve; });
    const original = vi.mocked(api.rows).getMockImplementation()!;
    vi.mocked(api.rows).mockImplementationOnce(async a => { await gate; return original(a); });
    const first = source.onViewChanged(2, fixture.total + 1);
    const second = source.onViewChanged(3, fixture.total + 2);
    const third = source.onViewChanged(4, fixture.total + 3);
    release(); await Promise.all([first, second, third]);
    expect(source.version).toBe(4); expect(source.total).toBe(fixture.total + 3);
    expect(source.getCell(0, 5)).toBe(6);
  });
});
