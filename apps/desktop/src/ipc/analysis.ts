// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AnalysisStartArgs } from "./gen/AnalysisStartArgs";
import type { AnalysisStatus } from "./gen/AnalysisStatus";
import type { OpHandle } from "./gen/OpHandle";
import { client } from "./client";

export const analysis = {
  /** `analysis.start` – lang laufend: liefert sofort `OpHandle`, Ergebnis (AnalysisStatus) über `Tick.ops[].result`. */
  start: (args: AnalysisStartArgs, signal?: AbortSignal) => client.invoke<OpHandle>("analysis.start", args, signal),
  /** `analysis.status` */
  status: (signal?: AbortSignal) => client.invoke<AnalysisStatus>("analysis.status", undefined, signal),
} as const;
