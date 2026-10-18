import { useEffect, useState } from "react";
import { uistate } from "../../../ipc/uistate";
import type { UiStateJson } from "../../../ipc/gen/UiStateJson";
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
  useEffect(() => {
    let live = true;
    void uistate.load().then(dto => {
      if (!live) return;
      setLayout(schema.parse(object(dto.state).sidebar ?? {}));
      setLoaded(true);
    }).catch(e => { if (live) { setError(e); setLoaded(true); } });
    return () => { live = false; };
  }, []);
  useEffect(() => {
    if (!loaded) return;
    let live = true;
    const timer = setTimeout(() => {
      void uistate.load().then(dto => {
        if (!live) return;
        const next = { ...dto, state: { ...object(dto.state), sidebar: layout } } as UiStateJson;
        return uistate.save(next);
      }).catch(e => { if (live) setError(e); });
    }, 1000);
    return () => { live = false; clearTimeout(timer); };
  }, [layout, loaded]);
  return { layout, setLayout, error, loaded };
}
