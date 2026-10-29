import { createContext, useContext, useEffect, useRef, useState } from "react";
import { isSilentError } from "../../../ipc/errors";

export const DetailVisible = createContext(true);

export const SELECTION_DEBOUNCE_MS = 100;
/** A new selection invalidates immediately. A revision revalidates plain data while keeping its last answer. */
export function useDetailRequest<T>(key: string, load: (signal: AbortSignal) => Promise<T>, dispose?: (value: T) => void, revision?: string | number): { value?: T; error?: unknown; loading: boolean } {
  const visible = useContext(DetailVisible);
  const shown = useRef(visible); shown.current = visible;
  const resume = useRef<(() => void) | undefined>(undefined);
  const [state, setState] = useState<{ key: string; value?: T; error?: unknown; loading: boolean }>({ key, loading: true });
  useEffect(() => {
    const controller = new AbortController();
    let value: T | undefined;
    let waiting = false;
    let timer: ReturnType<typeof setTimeout>;
    const schedule = () => { timer = setTimeout(() => {
      if (!shown.current) { waiting = true; return; }
      setState(previous => ({ key, loading: true, ...(revision !== undefined && previous.key === key ? { value: previous.value } : {}) }));
      void load(controller.signal).then(result => {
        if (controller.signal.aborted) { dispose?.(result); return; }
        value = result;
        setState({ key, value: result, loading: false });
      }).catch(error => {
        if (!controller.signal.aborted && !isSilentError(error)) setState({ key, error, loading: false });
      });
    }, SELECTION_DEBOUNCE_MS); };
    const wake = () => { if (waiting) { waiting = false; schedule(); } };
    resume.current = wake;
    schedule();
    return () => { clearTimeout(timer); controller.abort(); if (resume.current === wake) resume.current = undefined; if (value !== undefined) dispose?.(value); };
    // Key and revision describe the request; callbacks come from its owning render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, revision]);
  useEffect(() => { if (visible) resume.current?.(); }, [visible]);
  return state.key === key ? state : { loading: true };
}
