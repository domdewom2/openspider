import { z } from "zod";
import { toCmdError } from "../ipc/errors";
import { uistate } from "../ipc/uistate";
import type { JsonValue } from "../ipc/gen/serde_json/JsonValue";

const schema = z.object({ version: z.number().int().positive(), revision: z.number().int().nonnegative().nullish(), state: z.record(z.string(), z.json()) });
type State = Record<string, JsonValue>;
type Update = (state: State) => State;
let queue: Promise<void> = Promise.resolve();
let pending: { update: Update; resolve: () => void; reject: (error: unknown) => void }[] = [];
let timer: ReturnType<typeof setTimeout> | undefined;

/** Shared, validated read/modify/write boundary. Callers must calculate nested patches inside the update. */
export function saveUiState(update: Update): Promise<void> {
  const result = new Promise<void>((resolve, reject) => { pending.push({ update, resolve, reject }); });
  clearTimeout(timer);
  timer = setTimeout(() => { void flushUiState(); }, 1000);
  return result;
}

/** Flush pending changes, for shutdown and deterministic integration tests. */
export function flushUiState(): Promise<void> {
  clearTimeout(timer); timer = undefined;
  const batch = pending; pending = [];
  if (!batch.length) return queue;
  const job = queue.catch(() => undefined).then(async () => {
    for (let attempt = 0; ; attempt++) {
      const dto = schema.parse(await uistate.load());
      const state = batch.reduce((state, item) => item.update(state), dto.state as State);
      try {
        await uistate.save(schema.parse({ ...dto, state }));
        return;
      } catch (error) {
        if (toCmdError(error).code !== "Stale" || attempt >= 2) throw error;
      }
    }
  });
  queue = job.catch(() => undefined);
  void job.then(() => batch.forEach(item => item.resolve()), error => batch.forEach(item => item.reject(error)));
  return queue;
}
