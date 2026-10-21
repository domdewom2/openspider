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

/** Counters belong to the entire ordered draft and its crawl data snapshot. */
export function useDraftPreview(segments: SegmentDef[], enabled: boolean) {
  const revision = useSession(previewRevision);
  const key = JSON.stringify([segments, revision]);
  const [request, setRequest] = useState<{ key: string; opId?: string; error?: unknown } | null>(null);
  const status = useSession(s => request?.opId ? s.ops[request.opId] : undefined);
  useEffect(() => {
    setRequest(null);
    if (!enabled) return;
    let live = true;
    let opId: string | undefined;
    const timer = setTimeout(() => {
      void segment.preview({ segments }).then(handle => {
        opId = handle.opId;
        if (live) setRequest({ key, opId });
        else void op.cancel({ opId }).catch(() => undefined);
      }).catch(error => { if (live) setRequest({ key, error }); });
    }, 400);
    return () => { live = false; clearTimeout(timer); if (opId) void op.cancel({ opId }).catch(() => undefined); };
    // The serialized key covers the complete ordered list and data revision.
  }, [key, enabled]);
  const current = enabled && request?.key === key;
  const result = current && status?.state === "done" ? status.result as SegmentPreview | null : null;
  return {
    counts: new Map((result?.counts ?? []).map(c => [c.id, c.urls])),
    busy: enabled && (!current || (!request.error && (!status || status.state === "running"))),
    error: current ? (request.error ?? (status?.state === "failed" ? status.error : undefined)) : undefined,
  };
}
