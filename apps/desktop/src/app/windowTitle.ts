// Fenstertitel in der Oberflächensprache (UI-121, B3-UI): Tauri übernimmt `document.title` nicht in den nativen Titel
// des Fensters. Beide werden hier gesetzt, bei jedem Sprachwechsel erneut (der Titel hängt an `t`).
import { useEffect } from "react";
import { isTauri } from "../ipc/transport/tauri";

export async function applyWindowTitle(title: string): Promise<void> {
  if (typeof document !== "undefined") document.title = title;
  if (!isTauri()) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setTitle(title);
  } catch {
    // A closing window cannot be renamed; the next title of an open window still applies.
  }
}

export function useWindowTitle(title: string): void {
  useEffect(() => { void applyWindowTitle(title); }, [title]);
}
