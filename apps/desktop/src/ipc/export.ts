// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ExportBulkArgs } from "./gen/ExportBulkArgs";
import type { ExportCatalog } from "./gen/ExportCatalog";
import type { ExportDetailArgs } from "./gen/ExportDetailArgs";
import type { ExportPanelArgs } from "./gen/ExportPanelArgs";
import type { ExportSitemapArgs } from "./gen/ExportSitemapArgs";
import type { ExportViewArgs } from "./gen/ExportViewArgs";
import type { OpHandle } from "./gen/OpHandle";
import type { SuggestNameArgs } from "./gen/SuggestNameArgs";
import { client } from "./client";

export const exportApi = {
  /** `export.catalog` */
  catalog: (signal?: AbortSignal) => client.invoke<ExportCatalog>("export.catalog", undefined, signal),
  /** `export.suggestName` */
  suggestName: (args: SuggestNameArgs, signal?: AbortSignal) => client.invoke<string>("export.suggestName", args, signal),
  /** `export.view` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  view: (args: ExportViewArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.view", args, signal),
  /** `export.detail` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  detail: (args: ExportDetailArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.detail", args, signal),
  /** `export.bulk` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  bulk: (args: ExportBulkArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.bulk", args, signal),
  /** `export.report` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  report: (args: ExportBulkArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.report", args, signal),
  /** `export.sitemap` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  sitemap: (args: ExportSitemapArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.sitemap", args, signal),
  /** `export.issues` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  issues: (args: ExportPanelArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.issues", args, signal),
  /** `export.sidebar` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  sidebar: (args: ExportPanelArgs, signal?: AbortSignal) => client.invoke<OpHandle>("export.sidebar", args, signal),
} as const;
