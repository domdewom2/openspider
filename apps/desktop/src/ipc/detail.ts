// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { A11yNodeArgs } from "./gen/A11yNodeArgs";
import type { A11yNodeDetail } from "./gen/A11yNodeDetail";
import type { DuplicatesArgs } from "./gen/DuplicatesArgs";
import type { DuplicatesResult } from "./gen/DuplicatesResult";
import type { HeadersDto } from "./gen/HeadersDto";
import type { ImagePreview } from "./gen/ImagePreview";
import type { LighthouseArgs } from "./gen/LighthouseArgs";
import type { LighthouseDto } from "./gen/LighthouseDto";
import type { NameValue } from "./gen/NameValue";
import type { OpHandle } from "./gen/OpHandle";
import type { ScreenshotInfo } from "./gen/ScreenshotInfo";
import type { SdTree } from "./gen/SdTree";
import type { SerpArgs } from "./gen/SerpArgs";
import type { SerpData } from "./gen/SerpData";
import type { SerpEditArgs } from "./gen/SerpEditArgs";
import type { SerpMeasure } from "./gen/SerpMeasure";
import type { SerpMeasureArgs } from "./gen/SerpMeasureArgs";
import type { SourceDiffArgs } from "./gen/SourceDiffArgs";
import type { SourceDoc } from "./gen/SourceDoc";
import type { SourceLinesArgs } from "./gen/SourceLinesArgs";
import type { SourceOpenArgs } from "./gen/SourceOpenArgs";
import type { SpellingContext } from "./gen/SpellingContext";
import type { SpellingContextArgs } from "./gen/SpellingContextArgs";
import type { UrlIdArgs } from "./gen/UrlIdArgs";
import { client } from "./client";

export const detail = {
  /** `detail.urlDetails` */
  urlDetails: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<NameValue[]>("detail.urlDetails", args, signal),
  /** `detail.headers` */
  headers: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<HeadersDto>("detail.headers", args, signal),
  /** `detail.serp` */
  serp: (args: SerpArgs, signal?: AbortSignal) => client.invoke<SerpData>("detail.serp", args, signal),
  /** `detail.serpMeasure` */
  serpMeasure: (args: SerpMeasureArgs, signal?: AbortSignal) => client.invoke<SerpMeasure>("detail.serpMeasure", args, signal),
  /** `detail.serpSaveEdit` */
  serpSaveEdit: (args: SerpEditArgs, signal?: AbortSignal) => client.invoke<void>("detail.serpSaveEdit", args, signal),
  /** `detail.serpReset` */
  serpReset: (args: SerpEditArgs, signal?: AbortSignal) => client.invoke<void>("detail.serpReset", args, signal),
  /** `detail.sourceOpen` */
  sourceOpen: (args: SourceOpenArgs, signal?: AbortSignal) => client.invoke<SourceDoc>("detail.sourceOpen", args, signal),
  /** `detail.sourceLines` */
  sourceLines: (args: SourceLinesArgs, signal?: AbortSignal) => client.invoke<string[]>("detail.sourceLines", args, signal),
  /** `detail.sourceDiff` – lang laufend: liefert sofort `OpHandle`, Ergebnis (DiffHunks) über `Tick.ops[].result`. */
  sourceDiff: (args: SourceDiffArgs, signal?: AbortSignal) => client.invoke<OpHandle>("detail.sourceDiff", args, signal),
  /** `detail.screenshot` */
  screenshot: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<ScreenshotInfo>("detail.screenshot", args, signal),
  /** `detail.imagePreview` */
  imagePreview: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<ImagePreview>("detail.imagePreview", args, signal),
  /** `detail.structuredData` */
  structuredData: (args: UrlIdArgs, signal?: AbortSignal) => client.invoke<SdTree>("detail.structuredData", args, signal),
  /** `detail.duplicates` */
  duplicates: (args: DuplicatesArgs, signal?: AbortSignal) => client.invoke<DuplicatesResult>("detail.duplicates", args, signal),
  /** `detail.lighthouse` */
  lighthouse: (args: LighthouseArgs, signal?: AbortSignal) => client.invoke<LighthouseDto>("detail.lighthouse", args, signal),
  /** `detail.spellingContext` */
  spellingContext: (args: SpellingContextArgs, signal?: AbortSignal) => client.invoke<SpellingContext>("detail.spellingContext", args, signal),
  /** `detail.a11yNode` */
  a11yNode: (args: A11yNodeArgs, signal?: AbortSignal) => client.invoke<A11yNodeDetail>("detail.a11yNode", args, signal),
} as const;
