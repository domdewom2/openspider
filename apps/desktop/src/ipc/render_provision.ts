// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ChromiumStatus } from "./gen/ChromiumStatus";
import type { ChromiumStatusArgs } from "./gen/ChromiumStatusArgs";
import { client } from "./client";

export const render_provision = {
  /** `render_provision.status` */
  status: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("render_provision.status", args, signal),
  /** `render_provision.download` */
  download: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("render_provision.download", args, signal),
  /** `render_provision.cancel` */
  cancel: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("render_provision.cancel", args, signal),
  /** `render_provision.remove` */
  remove: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("render_provision.remove", args, signal),
} as const;
