import { isTauri } from "../../ipc/transport/tauri";
import { useLayout, windowSchema, type WindowBounds } from "../../state/layout";

export interface ScreenArea { x: number; y: number; width: number; height: number; scaleFactor: number }
/** Persist position in desktop physical coordinates, size in logical pixels (also with mixed-DPI monitors). */
export function restoreBounds(saved: WindowBounds, screens: ScreenArea[], primary: ScreenArea): WindowBounds {
  const screen = screens.find((s) => saved.x + saved.width * saved.scaleFactor > s.x + 32
    && saved.y + saved.height * saved.scaleFactor > s.y + 32
    && saved.x < s.x + s.width - 32 && saved.y < s.y + s.height - 32) ?? primary;
  const width = Math.max(1024, Math.min(saved.width, screen.width / screen.scaleFactor));
  const height = Math.max(640, Math.min(saved.height, screen.height / screen.scaleFactor));
  return { x: Math.max(screen.x, Math.min(saved.x, screen.x + Math.max(0, screen.width - width * screen.scaleFactor))),
    y: Math.max(screen.y, Math.min(saved.y, screen.y + Math.max(0, screen.height - height * screen.scaleFactor))),
    width, height, scaleFactor: screen.scaleFactor };
}

/** Installs only on the main window, after uistate.load; returns a StrictMode-safe disposer. */
export async function bindWindowGeometry(): Promise<() => void> {
  if (!isTauri()) return () => undefined;
  const { getCurrentWindow, availableMonitors, primaryMonitor, PhysicalPosition, LogicalSize } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  const saved = windowSchema.safeParse(useLayout.getState().sections.window);
  if (saved.success) {
    const monitors = await availableMonitors();
    const primary = await primaryMonitor();
    const area = (m: typeof monitors[number]): ScreenArea => ({ ...m.workArea.position, ...m.workArea.size, scaleFactor: m.scaleFactor });
    const main = primary ?? monitors[0];
    if (main) {
      const bounds = restoreBounds(saved.data, monitors.map(area), area(main));
      await win.setSize(new LogicalSize(bounds.width, bounds.height));
      await win.setPosition(new PhysicalPosition(bounds.x, bounds.y));
    }
  }
  let disposed = false;
  let chain = Promise.resolve();
  const capture = () => {
    chain = chain.then(async () => {
      if (disposed || await win.isMinimized() || await win.isMaximized()) return;
      const [position, size, scaleFactor] = await Promise.all([win.outerPosition(), win.innerSize(), win.scaleFactor()]);
      if (disposed) return;
      const logical = size.toLogical(scaleFactor);
      useLayout.getState().setSection("window", { x: position.x, y: position.y, width: logical.width, height: logical.height, scaleFactor });
    }).catch(() => undefined);
  };
  const offs: (() => void)[] = [];
  try {
    offs.push(await win.onMoved(capture));
    offs.push(await win.onResized(capture));
    capture();
  } catch (e) {
    offs.forEach((off) => off());
    throw e;
  }
  return () => { disposed = true; offs.forEach((off) => off()); };
}
