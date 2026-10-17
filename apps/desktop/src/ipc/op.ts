// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { OpCancelArgs } from "./gen/OpCancelArgs";
import { client } from "./client";

export const op = {
  /** `op.cancel` */
  cancel: (args: OpCancelArgs, signal?: AbortSignal) => client.invoke<void>("op.cancel", args, signal),
} as const;
