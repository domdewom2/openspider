import { useEffect, useState } from "react";
import { segment } from "../../../../ipc/segment";
import { op } from "../../../../ipc/op";
import type { SegmentDef } from "../../../../ipc/gen/SegmentDef";
import type { SegmentPreview } from "../../../../ipc/gen/SegmentPreview";
import { useSession, type SessionState } from "../../../../state/session";

/** Data changes, including API completion before the next counts tick, invalidate both counts and columns. */
export const previewRevision = (s: SessionState) => JSON.stringify([
  s.crawl?.crawlId, s.countsVersion, s.analysis?.running, s.analysis?.steps,
  Object.values(s.ops).filter(o => o.labelKey.startsWith("op-api-") && o.state !== "running")
    .map(o => [o.opId, o.state]).sort(),
]);

type Snapshot = {
  key: string;
  revision?: string;
  countsVersion?: number;
  result?: SegmentPreview | null;
  busy: boolean;
  error?: unknown;
};

/** Debounce edits, but coalesce live data without postponing or aborting ongoing work. */
export function useDraftPreview(segments: SegmentDef[], enabled: boolean) {
  const revision = useSession(previewRevision);
  const crawlId = useSession(s => s.crawl?.crawlId);
  const key = JSON.stringify([segments, crawlId]);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  useEffect(() => {
    setSnapshot(null);
    if (!enabled) return;
    let live = true;
    let latestRevision = previewRevision(useSession.getState());
    let requestedRevision: string | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let active: { revision: string; countsVersion: number; opId?: string } | undefined;
    const cancel = (opId: string) => { void op.cancel({ opId }).catch(() => undefined); };
    const start = () => {
      timer = undefined;
      const request = { revision: latestRevision, countsVersion: useSession.getState().countsVersion, opId: undefined as string | undefined };
      requestedRevision = request.revision;
      active = request;
      setSnapshot(s => ({ ...s, key, busy: true, error: undefined }));
      void segment.preview({ segments }).then(handle => {
        if (!live) { cancel(handle.opId); return; }
        request.opId = handle.opId;
        update(); // The completion tick may have arrived before the handle.
      }).catch(error => {
        if (!live) return;
        active = undefined;
        setSnapshot(s => ({ ...s, key, busy: false, error }));
        if (latestRevision !== requestedRevision) start();
      });
    };
    const update = () => {
      latestRevision = previewRevision(useSession.getState());
      const status = active?.opId ? useSession.getState().ops[active.opId] : undefined;
      if (active && status && status.state !== "running") {
        const completed = active;
        active = undefined;
        if (status.state === "done") {
          setSnapshot({ key, revision: completed.revision, countsVersion: completed.countsVersion, result: status.result as SegmentPreview | null, busy: false });
        } else {
          setSnapshot(s => ({ ...s, key, busy: false, error: status.error ?? new Error("Preview cancelled") }));
        }
        // Finish one snapshot, then request only the newest revision, never intermediate ticks.
        if (latestRevision !== requestedRevision) start();
      } else if (!active && timer === undefined && latestRevision !== requestedRevision) {
        setSnapshot(s => ({ ...s, key, busy: true }));
        // A fixed window from the first change bounds the wait even with 250-ms ticks.
        timer = setTimeout(start, 400);
      }
    };
    const unsubscribe = useSession.subscribe(update);
    update();
    return () => {
      live = false;
      unsubscribe();
      clearTimeout(timer);
      if (active?.opId) cancel(active.opId);
    };
    // The serialized key covers the ordered rules and crawl. Data revisions use the bounded scheduler above.
  }, [key, enabled]);
  const current = enabled && snapshot?.key === key ? snapshot : null;
  return {
    counts: new Map((current?.result?.counts ?? []).map(c => [c.id, c.urls])),
    resultRevision: current?.revision,
    countsVersion: current?.countsVersion,
    stale: current?.revision !== undefined && current.revision !== revision,
    busy: enabled && (!current || current.busy),
    error: current?.error,
  };
}
