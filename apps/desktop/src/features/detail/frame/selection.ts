import { create } from "zustand";
import type { DetailSelection, NavigationRequest } from "./types";

/** Shared handoff for the top-table owner: publish URL ids and the focus on every selection change. */
export const useDetailSelection = create<{ selection: DetailSelection; setSelection: (selection: DetailSelection) => void }>((set) => ({
  selection: { urlIds: [] },
  setSelection: (selection) => set({ selection }),
}));
export const setDetailSelection = (selection: DetailSelection): void => useDetailSelection.getState().setSelection(selection);
export const DETAIL_NAVIGATE_EVENT = "os:detail-navigate";
export const DETAIL_BACK_EVENT = "os:detail-back";
/** The top table resolves the address to a URL id and chooses Internal/External from the crawl metadata. */
export function requestNavigation(request: NavigationRequest): void {
  document.dispatchEvent(new CustomEvent(DETAIL_NAVIGATE_EVENT, { detail: request }));
}
