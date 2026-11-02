import { createContext, useContext, useLayoutEffect, useRef, useState } from "react";
import { isSilentError } from "../../../ipc/errors";

export const DetailVisible = createContext(true);
/** An explicit tab click uses the existing selection without another selection debounce. */
export const DetailRequestUrgent = createContext(false);

export const SELECTION_DEBOUNCE_MS = 100;
/** A new selection invalidates immediately. A revision revalidates plain data while keeping its last answer. */
export function useDetailRequest<T>(key: string, load: (signal: AbortSignal) => Promise<T>, dispose?: (value: T) => void, revision?: string | number): { value?: T; error?: unknown; loading: boolean } {
  const visible = useContext(DetailVisible);
  const urgent = useContext(DetailRequestUrgent);
  const urgentRef = useRef(urgent); urgentRef.current = urgent;
  const shown = useRef(visible); shown.current = visible;
  const resume = useRef<(() => void) | undefined>(undefined);
  const [state, setState] = useState<{ key: string; value?: T; error?: unknown; loading: boolean }>({ key, loading: true });
  useLayoutEffect(() => {
    const controller = new AbortController();
    let value: T | undefined;
    let waiting = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const run = () => {
      if (!shown.current) { waiting = true; return; }
      setState(previous => {
        const next = { key, loading: true, ...(revision !== undefined && previous.key === key ? { value: previous.value } : {}) };
        // Keep the same state object when nothing changes: the first request of a freshly mounted panel must not cost another render.
        return previous.key === key && previous.loading && previous.error === undefined && previous.value === next.value ? previous : next;
      });
      void load(controller.signal).then(result => {
        if (controller.signal.aborted) { dispose?.(result); return; }
        value = result;
        setState({ key, value: result, loading: false });
      }).catch(error => {
        if (!controller.signal.aborted && !isSilentError(error)) setState({ key, error, loading: false });
      });
    };
    // UI-145: an explicit tab click sends its request within the click task (microtask after the commit). A timer
    // task would queue behind painting and live updates (measured up to 275 ms during a crawl); selection changes
    // keep the debounce. An effect cleaned up in the same commit (remount) sends nothing.
    const schedule = () => {
      if (urgentRef.current) queueMicrotask(() => { if (!controller.signal.aborted) run(); });
      else timer = setTimeout(run, SELECTION_DEBOUNCE_MS);
    };
    const wake = () => { if (waiting) { waiting = false; schedule(); } };
    resume.current = wake;
    schedule();
    return () => { clearTimeout(timer); controller.abort(); if (resume.current === wake) resume.current = undefined; if (value !== undefined) dispose?.(value); };
    // Key and revision describe the request; callbacks come from its owning render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, revision]);
  useLayoutEffect(() => { if (visible) resume.current?.(); }, [visible]);
  return state.key === key ? state : { loading: true };
}
