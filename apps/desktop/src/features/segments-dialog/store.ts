// Window-wide state of the segment dialog: other features (segments tab, configuration dialog, table toolbar) open it
// through `openSegmentsDialog()` or the action `segments.configure`.
import { create } from "zustand";

export const useSegmentsDialog = create<{ open: boolean }>(() => ({ open: false }));

export function openSegmentsDialog(): void {
  useSegmentsDialog.setState({ open: true });
}

export function closeSegmentsDialog(): void {
  useSegmentsDialog.setState({ open: false });
}
