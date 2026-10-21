import { useEffect, useState } from "react";
import { segment } from "../../../../ipc/segment";
import { op } from "../../../../ipc/op";
import type { SegmentDef } from "../../../../ipc/gen/SegmentDef";
import type { SegmentPreview } from "../../../../ipc/gen/SegmentPreview";
import { useSession } from "../../../../state/session";

/** Counters belong to the entire ordered draft, never to an earlier rule or priority. */
export function useDraftPreview(segments: SegmentDef[], enabled: boolean) {
  const key = JSON.stringify(segments);
  const [request, setRequest] = useState<{ key: string; opId?: string; error?: unknown } | null>(null);
  const status = useSession(s => request?.opId ? s.ops[request.opId] : undefined);
  useEffect(() => {
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
    // The serialized key covers the complete ordered list.
  }, [key, enabled]);
  const current = enabled && request?.key === key;
  const result = current && status?.state === "done" ? status.result as SegmentPreview | null : null;
  return {
    counts: new Map((result?.counts ?? []).map(c => [c.id, c.urls])),
    busy: enabled && (!current || (!request.error && (!status || status.state === "running"))),
    error: current ? (request.error ?? (status?.state === "failed" ? status.error : undefined)) : undefined,
  };
}
