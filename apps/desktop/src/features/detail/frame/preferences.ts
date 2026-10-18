import { useEffect, useState } from "react";
import { updateDetailState } from "./persistence";
import { z } from "zod";
import { uistate } from "../../../ipc/uistate";
import { DETAIL_TABS, type DetailTabId } from "./types";
const schema = z.object({ order: z.array(z.string()).default([]), hidden: z.array(z.string()).default([]) });
export interface DetailPreferences { order: DetailTabId[]; hidden: DetailTabId[] }
export function normalizeDetailPreferences(value: unknown): DetailPreferences {
  const parsed = schema.safeParse(value); const preferences = parsed.success ? parsed.data : { order: [], hidden: [] };
  const known = [...DETAIL_TABS, "crawlPath"] as DetailTabId[];
  return { order: [...new Set([...preferences.order.filter(id => known.includes(id as DetailTabId)), ...known])] as DetailTabId[],
    hidden: preferences.hidden.filter(id => id !== "urlDetails" && known.includes(id as DetailTabId)) as DetailTabId[] };
}
export function useDetailPreferences(onError: (error: unknown) => void) {
  const [preferences, setPreferences] = useState(() => normalizeDetailPreferences(null));
  const [loaded, setLoaded] = useState(false); const [dirty, setDirty] = useState(false);
  useEffect(() => {
    let active = true;
    void uistate.load().then(dto => {
      if (active) { const state = dto.state && typeof dto.state === "object" && !Array.isArray(dto.state) ? dto.state : {}; setPreferences(normalizeDetailPreferences(state.detail)); setLoaded(true); }
    }).catch(error => { if (active) { setLoaded(true); onError(error); } });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (!loaded || !dirty) return;
    const timer = setTimeout(() => {
      // Read the latest document before saving, preserving unrelated regions of the UI state.
      void updateDetailState(state => ({ ...state, detail: { order: preferences.order, hidden: preferences.hidden } })).catch(onError);
    }, 1000);
    return () => clearTimeout(timer);
  }, [preferences, loaded, dirty]);
  return [preferences, (next: DetailPreferences) => { setPreferences(normalizeDetailPreferences(next)); setDirty(true); }, loaded] as const;
}
