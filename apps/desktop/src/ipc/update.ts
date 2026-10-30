// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { UpdateInstallArgs } from "./gen/UpdateInstallArgs";
import type { UpdateSetAutoArgs } from "./gen/UpdateSetAutoArgs";
import type { UpdateState } from "./gen/UpdateState";
import { client } from "./client";

export const update = {
  /** `update.check` */
  check: (signal?: AbortSignal) => client.invoke<UpdateState>("update.check", undefined, signal),
  /** `update.status` */
  status: (signal?: AbortSignal) => client.invoke<UpdateState>("update.status", undefined, signal),
  /** `update.download` */
  download: (signal?: AbortSignal) => client.invoke<UpdateState>("update.download", undefined, signal),
  /** `update.install` */
  install: (args: UpdateInstallArgs, signal?: AbortSignal) => client.invoke<UpdateState>("update.install", args, signal),
  /** `update.set_auto` */
  set_auto: (args: UpdateSetAutoArgs, signal?: AbortSignal) => client.invoke<void>("update.set_auto", args, signal),
} as const;
