import { expect, it, vi } from "vitest";
import { createEventBus } from "./events";
import { toUiEvent } from "./transport/bridge";

// Infrastruktur (M2-F05b); Fachprüfung M2-K16.
// spec: UI-140
it("verteilt focusUrl über die vorhandene Registrierung und meldet Hörer ab", () => {
  const fixture = { crawlId: "fixture", urlId: 42, isInternal: true };
  const event = toUiEvent("focusUrl", fixture);
  expect(event).toEqual({ kind: "focusUrl", ...fixture });
  const bus = createEventBus();
  const listener = vi.fn();
  const off = bus.on("focusUrl", listener);
  bus.emit(event!);
  expect(listener).toHaveBeenCalledExactlyOnceWith(event);
  off();
  bus.emit(event!);
  expect(listener).toHaveBeenCalledTimes(1);
  expect(bus.listenerCount()).toBe(0);
});
