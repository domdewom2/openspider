// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { CrashFrontendArgs } from "./gen/CrashFrontendArgs";
import type { CrashIdArgs } from "./gen/CrashIdArgs";
import type { CrashIdsArgs } from "./gen/CrashIdsArgs";
import type { CrashOverview } from "./gen/CrashOverview";
import type { CrashSendArgs } from "./gen/CrashSendArgs";
import type { CrashSendResult } from "./gen/CrashSendResult";
import { client } from "./client";

export const crash = {
  /** `crash.overview` */
  overview: (signal?: AbortSignal) => client.invoke<CrashOverview>("crash.overview", undefined, signal),
  /** `crash.read` */
  read: (args: CrashIdArgs, signal?: AbortSignal) => client.invoke<string>("crash.read", args, signal),
  /** `crash.send` */
  send: (args: CrashSendArgs, signal?: AbortSignal) => client.invoke<CrashSendResult>("crash.send", args, signal),
  /** `crash.dismiss` */
  dismiss: (args: CrashIdsArgs, signal?: AbortSignal) => client.invoke<void>("crash.dismiss", args, signal),
  /** `crash.delete` */
  delete: (args: CrashIdArgs, signal?: AbortSignal) => client.invoke<void>("crash.delete", args, signal),
  /** `crash.reportFrontend` */
  reportFrontend: (args: CrashFrontendArgs, signal?: AbortSignal) => client.invoke<boolean>("crash.reportFrontend", args, signal),
} as const;
