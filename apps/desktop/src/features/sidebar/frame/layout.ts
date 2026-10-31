import { useEffect, useRef, useState } from "react";
import { uistate } from "../../../ipc/uistate";
import { updateUiState } from "../../../state/uistate";
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
