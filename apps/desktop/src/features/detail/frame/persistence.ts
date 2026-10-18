import { uistate } from "../../../ipc/uistate";
import type { UiStateJson } from "../../../ipc/gen/UiStateJson";
import type { JsonValue } from "../../../ipc/gen/serde_json/JsonValue";
type State = { [key: string]: JsonValue };
let tail: Promise<unknown> = Promise.resolve();
const pending = new Map<string, ReturnType<typeof setTimeout>>();

/** Serial read/merge/write prevents this region's tab and column saves from overwriting one another. */
export function updateDetailState(update: (state: State) => State): Promise<UiStateJson> {
  const task = tail.catch(() => undefined).then(async () => {
    const dto = await uistate.load();
    const state = dto.state && typeof dto.state === "object" && !Array.isArray(dto.state) ? dto.state : {};
    return uistate.save({ ...dto, state: update(state) });
  });
  tail = task.catch(() => undefined);
  return task;
}
/** Timers survive a panel unmount, so switching tabs does not discard a just-edited layout. */
export function scheduleDetailState(key: string, update: (state: State) => State, onError: (error: unknown) => void): void {
  clearTimeout(pending.get(key));
  pending.set(key, setTimeout(() => {
    pending.delete(key);
    void updateDetailState(update).catch(onError);
  }, 1000));
}
