import { create } from "zustand";
import { catalog } from "../ipc/catalog";
import { config } from "../ipc/config";
import { uistate } from "../ipc/uistate";
import { defaultColumns, type ColumnState } from "../grid/DataGrid";
import type { ColumnMeta } from "../ipc/gen/ColumnMeta";
import { object, saveTableState } from "./tabs";

export function normalizeColumns(meta: ColumnMeta[], saved: ColumnState[] = [], profile?: string[]): ColumnState[] {
  const defaults = defaultColumns(meta);
  const known = new Set(defaults.map(c => c.key));
  const sameVisibleOrder = !profile || profile.join("\0") === exportColumns(saved).join("\0");
  const order = sameVisibleOrder && saved.length ? saved.map(c => c.key) : profile ?? [];
  const keys = [...new Set(["Row", "Address", ...order, ...defaults.map(c => c.key)])].filter(k => known.has(k));
  return keys.map(key => {
    const base = defaults.find(c => c.key === key)!;
    const previous = saved.find(c => c.key === key);
    return { ...base, ...(previous ?? {}), key,
      width: previous && Number.isFinite(previous.width) ? Math.max(40, Math.min(2000, previous.width)) : base.width,
      visible: key === "Row" || key === "Address" || (profile ? profile.includes(key) : previous?.visible ?? base.visible),
      pinned: key === "Row" || (previous?.pinned ?? base.pinned) };
  });
}
export function exportColumns(columns: ColumnState[]): string[] {
  return columns.filter(c => c.visible && c.key !== "Row").map(c => c.key);
}
let profileQueue: Promise<unknown> = Promise.resolve();
export function persistColumns(tab: string, columns: ColumnState[]): Promise<void> {
  const job = profileQueue.catch(() => undefined).then(async () => {
    const dto = await config.get();
    const root = object(dto.config); const ui = object(root.ui);
    await config.set({ ...dto, config: { ...root, ui: { ...ui, columns: { ...object(ui.columns), [tab]: exportColumns(columns) } } } });
    const saved = await uistate.load();
    await saveTableState({ tableColumns: { ...object(object(saved.state).tableColumns), [tab]: columns.map(c => ({ ...c })) } });
  });
  profileQueue = job;
  return job;
}
interface ColumnsState {
  metadata: Record<string, ColumnMeta[]>; byTab: Record<string, ColumnState[]>;
  load(tab: string): Promise<void>; change(tab: string, columns: ColumnState[]): Promise<void>; reset(tab: string): Promise<void>;
}
export const useColumns = create<ColumnsState>((set, get) => ({
  metadata: {}, byTab: {},
  async load(tab) {
    const [meta, dto, saved] = await Promise.all([catalog.columns({ tab }), config.get(), uistate.load()]);
    const profileValue = object(object(object(dto.config).ui).columns)[tab];
    const profile = Array.isArray(profileValue) ? profileValue.filter((v): v is string => typeof v === "string") : undefined;
    const raw = object(object(saved.state).tableColumns)[tab];
    const previous = Array.isArray(raw) ? raw.filter(v => typeof object(v).key === "string") as unknown as ColumnState[] : [];
    set(s => ({ metadata: { ...s.metadata, [tab]: meta }, byTab: { ...s.byTab, [tab]: normalizeColumns(meta, previous, profile) } }));
  },
  async change(tab, columns) {
    const normalized = normalizeColumns(get().metadata[tab] ?? [], columns);
    set(s => ({ byTab: { ...s.byTab, [tab]: normalized } }));
    await persistColumns(tab, normalized);
  },
  async reset(tab) { await get().change(tab, defaultColumns(get().metadata[tab] ?? [])); },
}));
