// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AppInfo } from "./gen/AppInfo";
import { client } from "./client";

export const app = {
  /** `app.info` */
  info: (signal?: AbortSignal) => client.invoke<AppInfo>("app.info", undefined, signal),
} as const;
