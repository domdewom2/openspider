import { useEffect, useState } from "react";
import { isSilentError } from "../../../ipc/errors";

export const SELECTION_DEBOUNCE_MS = 100;
/** A change invalidates the old result immediately, including during the debounce window. */
export function useDetailRequest<T>(key: string, load: (signal: AbortSignal) => Promise<T>, dispose?: (value: T) => void): { value?: T; error?: unknown; loading: boolean } {
  const [state, setState] = useState<{ key: string; value?: T; error?: unknown; loading: boolean }>({ key, loading: true });
  useEffect(() => {
    const controller = new AbortController();
    let value: T | undefined;
    const timer = setTimeout(() => {
      setState({ key, loading: true });
      void load(controller.signal).then(result => {
        if (controller.signal.aborted) { dispose?.(result); return; }
        value = result;
        setState({ key, value: result, loading: false });
      }).catch(error => {
        if (!controller.signal.aborted && !isSilentError(error)) setState({ key, error, loading: false });
      });
    }, SELECTION_DEBOUNCE_MS);
    return () => { clearTimeout(timer); controller.abort(); if (value !== undefined) dispose?.(value); };
    // The key describes the entire request; callbacks are supplied by its owning render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return state.key === key ? state : { loading: true };
}
