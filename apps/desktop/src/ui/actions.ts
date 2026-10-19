// Aktionsregister: Kürzel, Menüs und Schaltflächen lösen dieselben Aktions-IDs aus (UI-113).
import { useEffect, useRef } from "react";
import type { ActionId } from "./shortcuts";

export interface ActionContext {
  /** Auslöser, z. B. `shortcut` oder `menu`. */
  source: "shortcut" | "menu" | "ui";
  event?: Event;
}

/** Rückgabe `false` = nicht behandelt (der Dispatcher unterdrückt dann die Standardaktion nicht). */
export type ActionHandler = (ctx: ActionContext) => boolean | void | Promise<boolean | void>;

const handlers = new Map<ActionId, ActionHandler[]>();
const actionGuards = new Set<(id: ActionId, ctx: ActionContext) => boolean>();

/** Optional window guards; without a registration action dispatch is unchanged. */
export function registerActionGuard(guard: (id: ActionId, ctx: ActionContext) => boolean): () => void {
  actionGuards.add(guard);
  return () => { actionGuards.delete(guard); };
}

const registryListeners = new Set<() => void>();

/** Menüaktivierung folgt auch verspätet registrierten Feature-Controllern. */
export function onActionRegistryChanged(listener: () => void): () => void {
  registryListeners.add(listener);
  return () => registryListeners.delete(listener);
}

export function registerAction(id: ActionId, handler: ActionHandler): () => void {
  const list = handlers.get(id) ?? [];
  list.push(handler);
  handlers.set(id, list);
  for (const listener of registryListeners) listener();
  return () => {
    const cur = handlers.get(id);
    if (!cur) return;
    const i = cur.indexOf(handler);
    if (i >= 0) cur.splice(i, 1);
    for (const listener of registryListeners) listener();
  };
}

/** Meldet die Aktion zusätzlich als DOM-Ereignis `os:action` (für E2E: „erwartetes Aktionsereignis“). */
function announce(id: ActionId, ctx: ActionContext): void {
  if (typeof document !== "undefined") document.dispatchEvent(new CustomEvent("os:action", { detail: { id, source: ctx.source } }));
}

/** Führt die Aktion aus; der zuletzt registrierte Handler hat Vorrang. Liefert `true`, wenn jemand sie behandelt hat. */
export function runAction(id: ActionId, ctx: ActionContext = { source: "ui" }): boolean {
  if ([...actionGuards].some(guard => !guard(id, ctx))) return false;
  announce(id, ctx);
  const list = handlers.get(id) ?? [];
  for (let i = list.length - 1; i >= 0; i--) {
    const h = list[i] as ActionHandler;
    const r = h(ctx);
    if (r !== false) return true;
  }
  return false;
}

export function hasAction(id: ActionId): boolean {
  return (handlers.get(id)?.length ?? 0) > 0;
}

export function useAction(id: ActionId, handler: ActionHandler): void {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(() => registerAction(id, (c) => ref.current(c)), [id]);
}
