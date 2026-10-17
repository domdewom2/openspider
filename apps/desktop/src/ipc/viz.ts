// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { GraphDto } from "./gen/GraphDto";
import type { OpHandle } from "./gen/OpHandle";
import type { UrlIdArgs } from "./gen/UrlIdArgs";
import type { VizExportArgs } from "./gen/VizExportArgs";
import type { VizGraphArgs } from "./gen/VizGraphArgs";
import type { VizOpenArgs } from "./gen/VizOpenArgs";
import type { VizOpenResult } from "./gen/VizOpenResult";
import type { VizWordsArgs } from "./gen/VizWordsArgs";
import type { WordFreq } from "./gen/WordFreq";
import { client } from "./client";

export const viz = {
  /** `viz.open` */
  open: (args: VizOpenArgs, signal?: AbortSignal) => client.invoke<VizOpenResult>("viz.open", args, signal),
  /** `viz.graph` */
  graph: (args: VizGraphArgs, signal?: AbortSignal) => client.invoke<GraphDto>("viz.graph", args, signal),
  /** `viz.words` */
  words: (args: VizWordsArgs, signal?: AbortSignal) => client.invoke<WordFreq[]>("viz.words", args, signal),
  /** `viz.exportCsv` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  exportCsv: (args: VizExportArgs, signal?: AbortSignal) => client.invoke<OpHandle>("viz.exportCsv", args, signal),
  /** `viz.focusInMain` */
  focusInMain: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<void>("viz.focusInMain", args, signal),
} as const;
