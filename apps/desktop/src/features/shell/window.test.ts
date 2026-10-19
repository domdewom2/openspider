import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useLayout } from "../../state/layout";
import { bindWindowGeometry } from "./window";

const native = vi.hoisted(() => {
  const moveOff = vi.fn(); const resizeOff = vi.fn();
  return {
    move: undefined as (() => void) | undefined, resize: undefined as (() => void) | undefined,
    moveOff, resizeOff,
    win: {
      setSize: vi.fn(), setPosition: vi.fn(),
      isMinimized: vi.fn(async () => false), isMaximized: vi.fn(async () => false),
      outerPosition: vi.fn(async () => ({ x: 40, y: 60 })),
      innerSize: vi.fn(async () => ({ toLogical: (_scale: number) => ({ width: 1200, height: 800 }) })),
      scaleFactor: vi.fn(async () => 2),
      onMoved: vi.fn(async (cb: () => void) => { native.move = cb; return moveOff; }),
      onResized: vi.fn(async (cb: () => void) => { native.resize = cb; return resizeOff; }),
    },
  };
});
vi.mock("../../ipc/transport/tauri", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => {
  const monitor = { workArea: { position: { x: 0, y: 40 }, size: { width: 2880, height: 1760 } }, scaleFactor: 2 };
  return { getCurrentWindow: () => native.win, availableMonitors: async () => [monitor], primaryMonitor: async () => monitor,
    LogicalSize: class { constructor(public width: number, public height: number) {} },
    PhysicalPosition: class { constructor(public x: number, public y: number) {} } };
});
beforeEach(() => { vi.clearAllMocks(); native.win.isMaximized.mockResolvedValue(false); native.win.isMinimized.mockResolvedValue(false); });
afterEach(() => useLayout.setState({ sections: {} }));

// spec: UI-125 [P1]
it("restores native geometry, captures only normal window bounds and removes both subscriptions", async () => {
  useLayout.setState({ sections: { window: { x: 9000, y: 500, width: 1400, height: 900, scaleFactor: 2 } } });
  const off = await bindWindowGeometry();
  expect(native.win.setPosition).toHaveBeenCalledWith({ x: 80, y: 40 });
  expect(native.win.setSize).toHaveBeenCalledWith({ width: 1400, height: 880 });
  await vi.waitFor(() => expect(useLayout.getState().sections.window).toEqual({ x: 40, y: 60, width: 1200, height: 800, scaleFactor: 2 }));
  native.win.isMaximized.mockResolvedValue(true);
  const before = native.win.outerPosition.mock.calls.length;
  native.move?.(); native.resize?.();
  await new Promise<void>((resolve) => queueMicrotask(resolve));
  expect(native.win.outerPosition).toHaveBeenCalledTimes(before);
  off();
  expect(native.moveOff).toHaveBeenCalledTimes(1); expect(native.resizeOff).toHaveBeenCalledTimes(1);
});

// spec: UI-125 [P1]
it("releases the first subscription if installing the second one fails", async () => {
  native.win.onResized.mockRejectedValueOnce(new Error("window gone"));
  await expect(bindWindowGeometry()).rejects.toThrow("window gone");
  expect(native.moveOff).toHaveBeenCalledTimes(1);
});
