import { useEffect, useRef, useState } from "react";
import { useT } from "../../../../i18n";
import { op } from "../../../../ipc/op";
import { crawls } from "../../../../ipc/crawls";
import { toCmdError } from "../../../../ipc/errors";
import { useSession } from "../../../../state/session";

interface MoveBatch {
  destination: string;
  batches: string[][];
  index: number;
  opId: string | null;
  starting: boolean;
}

/** The crawl command accepts at most 1,000 IDs. Continue only after each OpHandle completes. */
export function useRelocation() {
  const t = useT();
  const [batch, setBatch] = useState<MoveBatch | null>(null);
  const stop = useRef(false);
  const [cancelled, setCancelled] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const status = useSession((s) => batch?.opId ? s.ops[batch.opId] : undefined);
  async function begin(ids: string[], destination: string) {
    setError(null); stop.current = false; setCancelled(false);
    if (!ids.length) return;
    const batches: string[][] = [];
    for (let i = 0; i < ids.length; i += 1000) batches.push(ids.slice(i, i + 1000));
    const initial = { destination, batches, index: 0, opId: null, starting: true };
    setBatch(initial);
    try {
      const handle = await crawls.move({ ids: batches[0]!, destinationDir: destination });
      setBatch({ ...initial, opId: handle.opId, starting: false });
      if (stop.current) await op.cancel({ opId: handle.opId });
    } catch (e) { setBatch(null); throw e; }
  }
  useEffect(() => {
    if (stop.current || !batch || batch.starting || status?.state !== "done" || batch.index + 1 >= batch.batches.length) return;
    const next = { ...batch, index: batch.index + 1, starting: true };
    setBatch(next);
    let active = true;
    void crawls.move({ ids: next.batches[next.index]!, destinationDir: next.destination }).then(async (handle) => {
      if (active) setBatch({ ...next, opId: handle.opId, starting: false });
      if (stop.current) await op.cancel({ opId: handle.opId });
    }).catch((e: unknown) => {
      if (active) { setError(toCmdError(e).detail ?? t("u14-save-error")); setBatch(null); }
    });
    // Changing sections leaves the operation in the core; no queued work starts after unmount.
    return () => { active = false; };
  }, [batch?.opId, status?.state]);
  async function cancel() {
    stop.current = true; setCancelled(true);
    if (batch?.opId && !batch.starting && status?.state !== "done") await op.cancel({ opId: batch.opId });
  }
  const busy = !!batch && (batch.starting || !status || status.state === "running" || (!cancelled && status.state === "done" && batch.index + 1 < batch.batches.length));
  return { begin, cancel, status: cancelled && status?.state === "done" ? { ...status, state: "cancelled" as const } : status, opId: batch?.opId ?? null, busy, error };
}
