import { useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { uistate } from "../../../ipc/uistate";
import { saveUiState, updateUiState } from "../../../state/uistate";
import { z } from "zod";

const schema = z.object({ selected: z.string().catch("overview"), order: z.array(z.string()).catch([]), hidden: z.array(z.string()).catch([]), collapsed: z.array(z.string()).catch([]) });
export type SidebarLayout = z.infer<typeof schema>;
export const DEFAULT_LAYOUT: SidebarLayout = { selected: "overview", order: [], hidden: [], collapsed: [] };
const object = (v: unknown): Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v) ? v as Record<string, unknown> : {};

/** Store only our namespace, keeping all other packages' UI preferences intact. */
export function useSidebarLayout() {
  const [layout, setLayout] = useState(DEFAULT_LAYOUT);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<unknown>();
  const initial = useRef<SidebarLayout | null>(null);
  useEffect(() => {
    let live = true;
    void uistate.load().then(dto => {
      if (!live) return;
      const parsed = schema.parse(object(dto.state).sidebar ?? {});
      initial.current = parsed;
      setLayout(parsed);
      setLoaded(true);
    }).catch(e => { if (live) { setError(e); setLoaded(true); } });
    return () => { live = false; };
  }, []);
  useEffect(() => {
    // Unchanged layouts are not written: an extra save at start-up would overwrite the .bak of a corrupt ui-state.json (UI-125).
    if (!loaded || layout === initial.current) return;
    let live = true;
    const timer = setTimeout(() => {
      void updateUiState(state => ({ ...state, sidebar: layout })).catch(e => { if (live) setError(e); });
    }, 1000);
    return () => { live = false; clearTimeout(timer); };
  }, [layout, loaded]);
  return { layout, setLayout, error, loaded };
}

// --- UI-102: state of the chart area below the Overview tree (height share, collapsed, view) ------------------------

const chartSchema = z.object({
  /** Height as a share of the Overview tab's content height; `null` = default (max(180 px, 40 %)). */
  share: z.number().min(0).max(1).nullable().catch(null),
  collapsed: z.boolean().catch(false),
  view: z.enum(["chart", "table"]).catch("chart"),
});
export type ChartLayout = z.infer<typeof chartSchema>;
export const DEFAULT_CHART_LAYOUT: ChartLayout = { share: null, collapsed: false, view: "chart" };
/** Own ui-state namespace: the sidebar layout above writes its namespace as a whole. */
export const CHART_LAYOUT_KEY = "overviewChart";

/** Loaded once per window; changes are written through the shared, batched ui-state boundary (UI-125). */
export const useChartLayout = create<{ layout: ChartLayout; loaded: boolean; error: unknown; load(): Promise<void>; update(patch: Partial<ChartLayout>): void; reset(): void }>((set, get) => {
  let loading: Promise<void> | null = null;
  return {
    layout: DEFAULT_CHART_LAYOUT,
    loaded: false,
    error: undefined,
    load() {
      loading ??= uistate.load().then(dto => {
        const parsed = chartSchema.parse(object(object(dto.state)[CHART_LAYOUT_KEY]));
        // A change before loading finished wins over the stored value.
        if (!get().loaded) set({ layout: { ...parsed, ...pendingPatch(get().layout) }, loaded: true });
      }).catch(error => { set({ loaded: true, error }); });
      return loading;
    },
    update(patch) {
      const layout = { ...get().layout, ...patch };
      if (layout.share === get().layout.share && layout.collapsed === get().layout.collapsed && layout.view === get().layout.view) return;
      set({ layout });
      void saveUiState(state => ({ ...state, [CHART_LAYOUT_KEY]: { ...layout } })).catch(error => set({ error }));
    },
    /** Tests and a new window start from the stored state again. */
    reset() {
      loading = null;
      set({ layout: DEFAULT_CHART_LAYOUT, loaded: false, error: undefined });
    },
  };
});
function pendingPatch(layout: ChartLayout): Partial<ChartLayout> {
  return layout === DEFAULT_CHART_LAYOUT ? {} : layout;
}
