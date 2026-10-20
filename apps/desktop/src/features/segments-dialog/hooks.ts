import { useEffect, useMemo, useRef, useState } from "react";
import { segment as segmentApi } from "../../ipc/segment";
import { op } from "../../ipc/op";
import type { SegmentDef } from "../../ipc/gen/SegmentDef";
import type { SegmentPreview } from "../../ipc/gen/SegmentPreview";
import type { ValidationIssue } from "../../ipc/gen/ValidationIssue";
import { useSession } from "../../state/session";
import { signature } from "./model";

export const VALIDATE_DELAY_MS = 250;
export const PREVIEW_DELAY_MS = 400;

/**
 * Asks the core to validate every segment whose definition changed (regex with position, references, names). The
 * result belongs to one definition signature; until it arrives the segment counts as “pending”.
 */
export function useSegmentValidation(segments: readonly SegmentDef[], enabled: boolean) {
  const done = useRef(new Map<string, { sig: string; issues: ValidationIssue[] }>());
  const [, bump] = useState(0);
  const [error, setError] = useState<unknown>();
  useEffect(() => {
    if (!enabled) return;
    const todo = segments.filter(s => done.current.get(s.id)?.sig !== signature(s));
    if (todo.length === 0) return;
    let live = true;
    const timer = setTimeout(async () => {
      for (const s of todo) {
        const sig = signature(s);
        try {
          const issues = await segmentApi.validate({ segment: s });
          if (!live) return;
          done.current.set(s.id, { sig, issues });
        } catch (e) {
          if (live) setError(e);
          return;
        }
      }
      if (live) { setError(undefined); bump(v => v + 1); }
    }, VALIDATE_DELAY_MS);
    return () => { live = false; clearTimeout(timer); };
  }, [segments, enabled]);
  const issuesOf = (s: SegmentDef): ValidationIssue[] | null => {
    const hit = done.current.get(s.id);
    return hit && hit.sig === signature(s) ? hit.issues : null;
  };
  return { issuesOf, pending: segments.some(s => issuesOf(s) === null), error };
}

/**
 * Counters “matches n URLs” per segment from the core (`segment.preview`, long running). Every change of the list or of
 * its order starts a new operation; the previous one is cancelled and its late result ignored.
 */
export function useSegmentPreview(segments: readonly SegmentDef[], enabled: boolean) {
  const [opId, setOpId] = useState<string | null>(null);
  const [error, setError] = useState<unknown>();
  const status = useSession(s => (opId ? s.ops[opId] : undefined));
  const last = useRef<string | null>(null);
  const key = useMemo(() => JSON.stringify(segments.map(signature)), [segments]);
  useEffect(() => {
    if (!enabled) return;
    let live = true;
    const timer = setTimeout(async () => {
      if (last.current) void op.cancel({ opId: last.current }).catch(() => undefined);
      try {
        const handle = await segmentApi.preview({ segments: [...segments] });
        if (!live) { void op.cancel({ opId: handle.opId }).catch(() => undefined); return; }
        last.current = handle.opId;
        setOpId(handle.opId);
        setError(undefined);
      } catch (e) { if (live) setError(e); }
    }, PREVIEW_DELAY_MS);
    return () => { live = false; clearTimeout(timer); };
    // `segments` is covered by `key`; the effect must not run on identity changes alone.
  }, [key, enabled]);
  useEffect(() => () => { if (last.current) void op.cancel({ opId: last.current }).catch(() => undefined); }, []);
  const result = status?.state === "done" ? (status.result as SegmentPreview | null | undefined) : undefined;
  const counts = new Map<string, number>((result?.counts ?? []).map(c => [c.id, c.urls]));
  return { counts, unassigned: result?.unassigned ?? null, busy: !!opId && status?.state === "running", error: status?.state === "failed" ? status.error : error };
}
