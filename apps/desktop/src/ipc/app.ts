// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AppInfo } from "./gen/AppInfo";
import type { BoolArgs } from "./gen/BoolArgs";
import type { QuitDecision } from "./gen/QuitDecision";
import { client } from "./client";

export const app = {
  /** `app.info` */
  info: (signal?: AbortSignal) => client.invoke<AppInfo>("app.info", undefined, signal),
  /** `app.requestQuit` */
  requestQuit: (signal?: AbortSignal) => client.invoke<QuitDecision>("app.requestQuit", undefined, signal),
  /** `app.confirmQuit` */
  confirmQuit: (args: BoolArgs, signal?: AbortSignal) => client.invoke<void>("app.confirmQuit", args, signal),
} as const;
