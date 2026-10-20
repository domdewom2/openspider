import { create } from "zustand";
import { useT } from "../../../../i18n";
import { op } from "../../../../ipc/op";
import { crawls } from "../../../../ipc/crawls";
import { toCmdError, type CmdError } from "../../../../ipc/errors";
import { useSession } from "../../../../state/session";

interface MoveBatch {
  destination: string;
  batches: string[][];
  index: number;
  opId: string | null;
  starting: boolean;
}

/** Window-lifetime queue: section unmounts must not interrupt a multi-batch relocation. */
export const useRelocationState = create<{
  batch: MoveBatch | null;
  cancelled: boolean;
  error: CmdError | null;
}>(() => ({ batch: null, cancelled: false, error: null }));

async function startBatch(batch: MoveBatch): Promise<void> {
  let current = batch;
  try {
    const handle = await crawls.move({ ids: batch.batches[batch.index]!, destinationDir: batch.destination });
    if (useRelocationState.getState().batch !== batch) return;
    current = { ...batch, opId: handle.opId, starting: false };
    useRelocationState.setState({ batch: current });
    if (useRelocationState.getState().cancelled) await op.cancel({ opId: handle.opId });
    else continueRelocation(); // The completion tick may have arrived before the command response.
  } catch (e) {
    if (useRelocationState.getState().batch === current) {
      useRelocationState.setState({ error: toCmdError(e), batch: null });
    }
    throw e;
  }
}

function continueRelocation() {
  const { batch, cancelled } = useRelocationState.getState();
  if (!batch || cancelled || batch.starting || !batch.opId) return;
  const status = useSession.getState().ops[batch.opId];
  if (status?.state !== "done" || batch.index + 1 >= batch.batches.length) return;
  const next = { ...batch, index: batch.index + 1, starting: true };
  // Mark the next batch before dispatching, so repeated ticks cannot start it twice.
  useRelocationState.setState({ batch: next });
  void startBatch(next).catch(() => undefined); // The controller keeps the error for the next mounted panel.
}
useSession.subscribe(continueRelocation);

async function begin(ids: string[], destination: string) {
  const batches: string[][] = [];
  for (let i = 0; i < ids.length; i += 1000) batches.push(ids.slice(i, i + 1000));
  const batch = batches.length ? { destination, batches, index: 0, opId: null, starting: true } : null;
  useRelocationState.setState({ batch, cancelled: false, error: null });
  if (batch) await startBatch(batch);
}

async function cancel() {
  const { batch } = useRelocationState.getState();
  useRelocationState.setState({ cancelled: true });
  if (batch?.opId && !batch.starting && useSession.getState().ops[batch.opId]?.state !== "done") {
    await op.cancel({ opId: batch.opId });
  }
}

/** The crawl command accepts at most 1,000 IDs; only a completed OpHandle starts the next batch. */
export function useRelocation() {
  const t = useT();
  const { batch, cancelled, error } = useRelocationState();
  const status = useSession((s) => batch?.opId ? s.ops[batch.opId] : undefined);
  const busy = !!batch && (batch.starting || !status || status.state === "running" || (!cancelled && status.state === "done" && batch.index + 1 < batch.batches.length));
  return { begin, cancel, status: cancelled && status?.state === "done" ? { ...status, state: "cancelled" as const } : status, opId: batch?.opId ?? null, busy, error: error ? error.detail ?? t("u14-save-error") : null };
}
