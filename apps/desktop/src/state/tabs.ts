import { saveUiState } from "./uistate";
import { create } from "zustand";
import { catalog } from "../ipc/catalog";
import { uistate } from "../ipc/uistate";
import type { TabInfo } from "../ipc/gen/TabInfo";
import type { SortKey } from "../ipc/gen/SortKey";
import type { JsonValue } from "../ipc/gen/serde_json/JsonValue";

export interface TabQuery { filter: string; search: string; sort: SortKey[] }
export const defaultQuery = (): TabQuery => ({ filter: "All", search: "", sort: [] });
export function cycleSort(sort: SortKey[], column: string, extend = false): SortKey[] {
  const previous = sort.find(s => s.column === column);
  const rest = extend ? sort.filter(s => s.column !== column) : [];
  const next = previous?.dir === "desc" ? null : { column, dir: previous?.dir === "asc" ? "desc" as const : "asc" as const };
  if (!extend) return next ? [next] : [];
  if (!previous) return next ? [...rest, next] : rest;
  return sort.flatMap(s => s.column === column ? next ? [next] : [] : [s]);
}
export function object(value: JsonValue | undefined): Record<string, JsonValue> {
  return value !== null && typeof value === "object" && !Array.isArray(value) ? value : {};
}
/** Merge only this feature's keys through the shared persistence queue. */
export function saveTableState(patch: Record<string, JsonValue>): Promise<void> {
  return saveUiState(state => ({ ...state, ...patch }));
}
interface TabsState {
  catalog: TabInfo[]; order: string[]; hidden: string[]; active: string; queries: Record<string, TabQuery>;
  loaded: boolean; error: unknown; focusId?: number; focusRequest: number;
  load(): Promise<void>; select(id: string, filter?: string): void;
  query(patch: Partial<TabQuery>): void; reorder(ids: string[]): Promise<void>;
  show(id: string, visible: boolean): Promise<void>; reset(): Promise<void>;
  focus(id: string, urlId: number): void;
}
export const useTabs = create<TabsState>((set, get) => ({
  catalog: [], order: [], hidden: [], active: "Internal", queries: {}, loaded: false, error: null, focusRequest: 0,
  async load() {
    try {
      const [tabs, dto] = await Promise.all([catalog.tabs(), uistate.load()]);
      const defaults = [...tabs].sort((a, b) => a.defaultOrder - b.defaultOrder).map(t => t.id);
      const saved = object(object(dto.state).topTabs);
      const order = Array.isArray(saved.order) ? saved.order.filter((v): v is string => typeof v === "string" && defaults.includes(v)) : [];
      const hidden = Array.isArray(saved.hidden) ? saved.hidden.filter((v): v is string => typeof v === "string" && v !== "Internal" && defaults.includes(v)) : [];
      set({ catalog: tabs, order: [...new Set([...order, ...defaults])], hidden, loaded: true, error: null });
    } catch (error) { set({ error }); }
  },
  select(id, filter) {
    if (!get().catalog.some(t => t.id === id)) return;
    const query = get().queries[id] ?? defaultQuery();
    set({ active: id, queries: { ...get().queries, [id]: { ...query, ...(filter ? { filter } : {}) } } });
  },
  query(patch) { const s = get(); set({ queries: { ...s.queries, [s.active]: { ...(s.queries[s.active] ?? defaultQuery()), ...patch } } }); },
  async reorder(ids) {
    const order = [...new Set([...ids, ...get().order])].filter(id => get().catalog.some(t => t.id === id));
    set({ order }); await saveTableState({ topTabs: { order, hidden: get().hidden } });
  },
  async show(id, visible) {
    if (id === "Internal" || !get().catalog.some(t => t.id === id)) return;
    const hidden = visible ? get().hidden.filter(v => v !== id) : [...new Set([...get().hidden, id])];
    set({ hidden, ...(id === get().active && !visible ? { active: "Internal" } : {}) });
    await saveTableState({ topTabs: { order: get().order, hidden } });
  },
  async reset() {
    const order = [...get().catalog].sort((a, b) => a.defaultOrder - b.defaultOrder).map(t => t.id);
    set({ order, hidden: [] }); await saveTableState({ topTabs: { order, hidden: [] } });
  },
  focus(id, urlId) {
    get().select(id, "All"); get().query({ search: "" });
    set({ hidden: get().hidden.filter(t => t !== id), focusId: urlId, focusRequest: get().focusRequest + 1 });
  },
}));
