// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { BundleArgs } from "./gen/BundleArgs";
import type { DebugInfo } from "./gen/DebugInfo";
import type { LogFrontendArgs } from "./gen/LogFrontendArgs";
import type { OpHandle } from "./gen/OpHandle";
import type { UsageStats } from "./gen/UsageStats";
import type { UsageStatsArgs } from "./gen/UsageStatsArgs";
import { client } from "./client";

export const diag = {
  /** `diag.debugInfo` */
  debugInfo: (signal?: AbortSignal) => client.invoke<DebugInfo>("diag.debugInfo", undefined, signal),
  /** `diag.bundle` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  bundle: (args: BundleArgs, signal?: AbortSignal) => client.invoke<OpHandle>("diag.bundle", args, signal),
  /** `diag.logFrontend` */
  logFrontend: (args: LogFrontendArgs, signal?: AbortSignal) => client.invoke<void>("diag.logFrontend", args, signal),
  /** `diag.openLogs` */
  openLogs: (signal?: AbortSignal) => client.invoke<void>("diag.openLogs", undefined, signal),
  /** `diag.usageStats` */
  usageStats: (args: UsageStatsArgs, signal?: AbortSignal) => client.invoke<UsageStats>("diag.usageStats", args, signal),
} as const;
