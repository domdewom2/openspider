import { client } from "../../src/ipc/client";
import type { ViewHandle } from "../../src/ipc/gen/ViewHandle";
import type { OpStatus } from "../../src/ipc/gen/OpStatus";
// Subscribe before starting builds, including completions preceding the building reply.
const operations = new Map<string, OpStatus>();
client.subscribe("main", (event) => {
  if (event.kind === "tick")
    for (const status of event.tick.ops) operations.set(status.opId, status);
});
export async function settled(handle: ViewHandle): Promise<ViewHandle> {
  if (handle.state !== "building" || !handle.op) return handle;
  const deadline = performance.now() + 120_000;
  while (performance.now() < deadline) {
    const status = operations.get(handle.op.opId);
    if (status?.state === "done") {
      operations.delete(handle.op.opId);
      return status.result as unknown as ViewHandle;
    }
    if (status && ["failed", "cancelled"].includes(status.state))
      throw new Error(JSON.stringify(status));
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error("View build timeout");
}
