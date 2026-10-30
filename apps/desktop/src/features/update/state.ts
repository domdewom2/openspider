import { create } from "zustand";
import { update } from "../../ipc/update";
import type { UpdateState } from "../../ipc/gen/UpdateState";
import type { AvailableUpdate } from "../../ipc/gen/AvailableUpdate";
import type { InstallWhen } from "../../ipc/gen/InstallWhen";
export const useUpdate = create<{ state: UpdateState; release: AvailableUpdate | null; open: boolean; queued: InstallWhen | null }>(() => ({ state: "idle", release: null, open: false, queued: null }));
export function accept(state: UpdateState) {
  useUpdate.setState({ state, ...(typeof state === "object" && "available" in state ? { release: state.available } : {}) });
}
export async function checkUpdates() {
  useUpdate.setState({ open: true, queued: null });
  accept("checking");
  try { accept(await update.check()); } catch { accept({ failed: "check" }); }
}
export async function downloadUpdate() {
  try { accept(await update.download()); } catch { accept({ failed: "download" }); }
}
export async function installUpdate(when: InstallWhen) {
  try {
    const state = await update.install({ when });
    accept(state);
    if (typeof state === "object" && "ready" in state) useUpdate.setState({ queued: when });
  } catch { accept({ failed: "install" }); }
}
