import { create } from "zustand";
import { SelectionModel } from "../../state/selection";
/** Read-only integration point for detail panes and sidebars; the upper table publishes URL ids. */
export const useTableSelection = create<{ selection: SelectionModel; viewId?: string; version?: number }>(() => ({ selection: SelectionModel.empty() }));
