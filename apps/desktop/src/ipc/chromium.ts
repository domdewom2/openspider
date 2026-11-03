// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ChromiumStatus } from "./gen/ChromiumStatus";
import type { ChromiumStatusArgs } from "./gen/ChromiumStatusArgs";
import { client } from "./client";

export const chromium = {
  /** `chromium.status` */
  status: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("chromium.status", args, signal),
  /** `chromium.download` */
  download: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("chromium.download", args, signal),
  /** `chromium.cancel` */
  cancel: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("chromium.cancel", args, signal),
  /** `chromium.remove` */
  remove: (args: ChromiumStatusArgs, signal?: AbortSignal) => client.invoke<ChromiumStatus>("chromium.remove", args, signal),
} as const;
