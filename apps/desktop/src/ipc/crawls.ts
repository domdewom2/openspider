// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { CrawlListQuery } from "./gen/CrawlListQuery";
import type { CrawlListing } from "./gen/CrawlListing";
import type { CrawlMoveArgs } from "./gen/CrawlMoveArgs";
import type { CrawlRenameArgs } from "./gen/CrawlRenameArgs";
import type { CrawlsExportArgs } from "./gen/CrawlsExportArgs";
import type { CrawlsImportArgs } from "./gen/CrawlsImportArgs";
import type { IdsArgs } from "./gen/IdsArgs";
import type { OpHandle } from "./gen/OpHandle";
import type { PathArgs } from "./gen/PathArgs";
import { client } from "./client";

export const crawls = {
  /** `crawls.checkLocation` */
  checkLocation: (args: PathArgs, signal?: AbortSignal) => client.invoke<string[]>("crawls.checkLocation", args, signal),
  /** `crawls.list` */
  list: (args: CrawlListQuery, signal?: AbortSignal) => client.invoke<CrawlListing[]>("crawls.list", args, signal),
  /** `crawls.rename` */
  rename: (args: CrawlRenameArgs, signal?: AbortSignal) => client.invoke<void>("crawls.rename", args, signal),
  /** `crawls.move` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  move: (args: CrawlMoveArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawls.move", args, signal),
  /** `crawls.lock` */
  lock: (args: IdsArgs, signal?: AbortSignal) => client.invoke<void>("crawls.lock", args, signal),
  /** `crawls.unlock` */
  unlock: (args: IdsArgs, signal?: AbortSignal) => client.invoke<void>("crawls.unlock", args, signal),
  /** `crawls.duplicate` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  duplicate: (args: IdsArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawls.duplicate", args, signal),
  /** `crawls.delete` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  delete: (args: IdsArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawls.delete", args, signal),
  /** `crawls.import` – lang laufend: liefert sofort `OpHandle`, Ergebnis (ImportResult) über `Tick.ops[].result`. */
  import: (args: CrawlsImportArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawls.import", args, signal),
  /** `crawls.export` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  export: (args: CrawlsExportArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawls.export", args, signal),
  /** `crawls.reveal` */
  reveal: (args: IdsArgs, signal?: AbortSignal) => client.invoke<void>("crawls.reveal", args, signal),
  /** `crawls.copyIds` */
  copyIds: (args: IdsArgs, signal?: AbortSignal) => client.invoke<void>("crawls.copyIds", args, signal),
} as const;
