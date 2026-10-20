import { create } from "zustand";
import { catalog } from "../ipc/catalog";
import { config } from "../ipc/config";
import { uistate } from "../ipc/uistate";
import { defaultColumns, type ColumnState } from "../grid/DataGrid";
import type { ColumnMeta } from "../ipc/gen/ColumnMeta";
import { object, useTabs } from "./tabs";
import { saveUiState } from "./uistate";
import type { JsonValue } from "../ipc/gen/serde_json/JsonValue";

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
/** Named column template (UI-009): ordered visible column keys; survives „reset all tables“ (UI-021). */
export interface ColumnTemplate { name: string; columns: string[] }
/** Apply a template to a tab's columns: unknown keys are ignored and reported as `missing`. */
export function applyTemplateColumns(current: ColumnState[], keys: string[]): { columns: ColumnState[]; missing: string[] } {
  const known = new Set(current.map(c => c.key));
  const present = keys.filter(k => known.has(k) && k !== "Row" && k !== "Address");
  const order = [...new Set(["Row", "Address", ...present, ...current.map(c => c.key)])].filter(k => known.has(k));
  const columns = order.map(key => { const c = current.find(d => d.key === key)!; return { ...c, visible: key === "Row" || key === "Address" || present.includes(key) }; });
  return { columns, missing: keys.filter(k => !known.has(k)) };
}
function templates(value: JsonValue | undefined): ColumnTemplate[] {
  return Array.isArray(value) ? value.map(object).filter(t => typeof t.name === "string" && Array.isArray(t.columns))
    .map(t => ({ name: t.name as string, columns: (t.columns as JsonValue[]).filter((c): c is string => typeof c === "string") })) : [];
}
let profileQueue: Promise<unknown> = Promise.resolve();
export function persistColumns(tab: string, columns: ColumnState[]): Promise<void> {
  const job = profileQueue.catch(() => undefined).then(async () => {
    const dto = await config.get();
    const root = object(dto.config); const ui = object(root.ui);
    await config.set({ ...dto, config: { ...root, ui: { ...ui, columns: { ...object(ui.columns), [tab]: exportColumns(columns) } } } });
    await saveUiState(state => ({ ...state, tableColumns: { ...object(state.tableColumns), [tab]: columns.map(c => ({ key: c.key, width: c.width, visible: c.visible, pinned: c.pinned, ...(c.sort ? { sort: c.sort } : {}) })) } }));
  });
  profileQueue = job;
  return job;
}
interface ColumnsState {
  metadata: Record<string, ColumnMeta[]>; byTab: Record<string, ColumnState[]>;
  load(tab: string): Promise<void>; change(tab: string, columns: ColumnState[]): Promise<void>; reset(tab: string): Promise<void>;
  templates: ColumnTemplate[]; loadTemplates(): Promise<void>;
  saveTemplate(name: string, columns: string[]): Promise<void>; renameTemplate(from: string, to: string): Promise<boolean>;
  deleteTemplate(name: string): Promise<void>;
  /** Apply a named template to any top tab; returns the template columns that tab does not have. */
  applyTemplate(tab: string, name: string): Promise<{ missing: string[]; saved: Promise<void> }>;
  /** UI-021: every table back to the shipped layout (visibility, order, width, pinning, sort); templates stay. */
  resetAll(): Promise<void>;
}
function persistTemplates(list: ColumnTemplate[]): Promise<void> {
  return saveUiState(state => ({ ...state, columnTemplates: list.map(t => ({ name: t.name, columns: t.columns })) }));
}
export const useColumns = create<ColumnsState>((set, get) => ({
  metadata: {}, byTab: {}, templates: [],
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
  async loadTemplates() { set({ templates: templates(object((await uistate.load()).state).columnTemplates) }); },
  async saveTemplate(name, columns) {
    const list = [...get().templates.filter(t => t.name !== name), { name, columns: columns.filter(c => c !== "Row") }];
    set({ templates: list }); await persistTemplates(list);
  },
  async renameTemplate(from, to) {
    if (!to.trim() || get().templates.some(t => t.name === to)) return false;
    const list = get().templates.map(t => t.name === from ? { ...t, name: to } : t);
    set({ templates: list }); await persistTemplates(list); return true;
  },
  async deleteTemplate(name) { const list = get().templates.filter(t => t.name !== name); set({ templates: list }); await persistTemplates(list); },
  async applyTemplate(tab, name) {
    const template = get().templates.find(t => t.name === name);
    if (!template) return { missing: [], saved: Promise.resolve() };
    if (!get().byTab[tab]) await get().load(tab);
    const { columns, missing } = applyTemplateColumns(get().byTab[tab] ?? [], template.columns);
    return { missing, saved: get().change(tab, columns) };
  },
  async resetAll() {
    set(s => ({ byTab: Object.fromEntries(Object.entries(s.metadata).map(([tab, meta]) => [tab, normalizeColumns(meta)])) }));
    useTabs.setState(s => ({ queries: Object.fromEntries(Object.entries(s.queries).map(([tab, q]) => [tab, { ...q, sort: [] }])) }));
    const job = profileQueue.catch(() => undefined).then(async () => {
      const dto = await config.get();
      const root = object(dto.config); const ui = object(root.ui);
      await config.set({ ...dto, config: { ...root, ui: { ...ui, columns: {} } } });
      await saveUiState(state => { const { tableColumns: _drop, ...rest } = state; return rest; });
    });
    profileQueue = job;
    await job;
  },
}));
