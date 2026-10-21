// Lädt ein Quelltext-Dokument in Blöcken in eine Code-Ansicht (UI-041: bis 20 MB ohne Einfrieren, der Kern liefert Zeilenfenster).
import { useEffect, useState, type RefObject } from "react";
import { detail } from "../../../ipc/detail";
import { isSilentError } from "../../../ipc/errors";
import type { SourceMode } from "../../../ipc/gen/SourceMode";
import type { SourceWhich } from "../../../ipc/gen/SourceWhich";
import { SELECTION_DEBOUNCE_MS } from "../frame/request";
import type { CodePaneHandle } from "./CodePane";

export const CHUNK_LINES = 4000;
export interface DocState { status: "idle" | "loading" | "ready" | "error"; loaded: number; lines: number; bytes: number; error?: unknown }
const IDLE: DocState = { status: "idle", loaded: 0, lines: 0, bytes: 0 };
export function useDocLoader(handle: RefObject<CodePaneHandle | null>, o: { enabled: boolean; urlId: number; crawlId?: string; which: SourceWhich; mode: SourceMode; pretty: boolean }): DocState {
  const [state, setState] = useState<DocState>(IDLE);
  useEffect(() => {
    if (!o.enabled) { setState(IDLE); return; }
    const controller = new AbortController();
    const timer = setTimeout(() => {
      const pane = handle.current; if (!pane) return;
      pane.reset(); setState({ status: "loading", loaded: 0, lines: 0, bytes: 0 });
      void (async () => {
        try {
          const doc = await detail.sourceOpen({ urlId: o.urlId, which: o.which, mode: o.mode, pretty: o.pretty }, controller.signal);
          if (controller.signal.aborted) return;
          setState({ status: "loading", loaded: 0, lines: doc.lines, bytes: doc.bytes });
          let from = 0;
          while (from < doc.lines) {
            const chunk = await detail.sourceLines({ docId: doc.docId, from, count: Math.min(CHUNK_LINES, doc.lines - from) }, controller.signal);
            if (controller.signal.aborted) return;
            if (chunk.length === 0) break;
            handle.current?.append((from > 0 ? "\n" : "") + chunk.join("\n")); from += chunk.length;
            setState({ status: "loading", loaded: from, lines: doc.lines, bytes: doc.bytes });
          }
          setState({ status: "ready", loaded: from, lines: doc.lines, bytes: doc.bytes });
        } catch (error) {
          if (!controller.signal.aborted && !isSilentError(error)) setState({ ...IDLE, status: "error", error });
        }
      })();
    }, SELECTION_DEBOUNCE_MS);
    return () => { clearTimeout(timer); controller.abort(); };
    // `handle` ist ein stabiles Ref-Objekt; die übrigen Werte beschreiben das Dokument vollständig.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [o.enabled, o.urlId, o.crawlId, o.which, o.mode, o.pretty]);
  return state;
}
