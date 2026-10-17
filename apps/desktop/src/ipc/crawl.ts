// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { AddUrlsArgs } from "./gen/AddUrlsArgs";
import type { CrawlPathDto } from "./gen/CrawlPathDto";
import type { CrawlRef } from "./gen/CrawlRef";
import type { CrawlSummaryDto } from "./gen/CrawlSummaryDto";
import type { ListPreview } from "./gen/ListPreview";
import type { ListSource } from "./gen/ListSource";
import type { NormalizedUrl } from "./gen/NormalizedUrl";
import type { OpHandle } from "./gen/OpHandle";
import type { OpenCrawlArgs } from "./gen/OpenCrawlArgs";
import type { SaveAsArgs } from "./gen/SaveAsArgs";
import type { SelectionArgs } from "./gen/SelectionArgs";
import type { StartRequest } from "./gen/StartRequest";
import type { UrlIdArgs } from "./gen/UrlIdArgs";
import type { ValidateStartUrlArgs } from "./gen/ValidateStartUrlArgs";
import { client } from "./client";

export const crawl = {
  /** `crawl.validateStartUrl` */
  validateStartUrl: (args: ValidateStartUrlArgs, signal?: AbortSignal) => client.invoke<NormalizedUrl>("crawl.validateStartUrl", args, signal),
  /** `crawl.parseList` */
  parseList: (args: ListSource, signal?: AbortSignal) => client.invoke<ListPreview>("crawl.parseList", args, signal),
  /** `crawl.start` */
  start: (args: StartRequest, signal?: AbortSignal) => client.invoke<CrawlRef>("crawl.start", args, signal),
  /** `crawl.pause` */
  pause: (signal?: AbortSignal) => client.invoke<void>("crawl.pause", undefined, signal),
  /** `crawl.resume` */
  resume: (signal?: AbortSignal) => client.invoke<void>("crawl.resume", undefined, signal),
  /** `crawl.stop` */
  stop: (signal?: AbortSignal) => client.invoke<void>("crawl.stop", undefined, signal),
  /** `crawl.clear` */
  clear: (signal?: AbortSignal) => client.invoke<void>("crawl.clear", undefined, signal),
  /** `crawl.open` */
  open: (args: OpenCrawlArgs, signal?: AbortSignal) => client.invoke<CrawlSummaryDto>("crawl.open", args, signal),
  /** `crawl.saveAs` – lang laufend: liefert sofort `OpHandle`, Ergebnis (FileResult) über `Tick.ops[].result`. */
  saveAs: (args: SaveAsArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawl.saveAs", args, signal),
  /** `crawl.recrawl` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  recrawl: (args: SelectionArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawl.recrawl", args, signal),
  /** `crawl.remove` – lang laufend: liefert sofort `OpHandle`, Ergebnis (CountResult) über `Tick.ops[].result`. */
  remove: (args: SelectionArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawl.remove", args, signal),
  /** `crawl.addUrls` – lang laufend: liefert sofort `OpHandle`, Ergebnis (AddUrlsResult) über `Tick.ops[].result`. */
  addUrls: (args: AddUrlsArgs, signal?: AbortSignal) => client.invoke<OpHandle>("crawl.addUrls", args, signal),
  /** `crawl.path` */
  path: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<CrawlPathDto[]>("crawl.path", args, signal),
} as const;
