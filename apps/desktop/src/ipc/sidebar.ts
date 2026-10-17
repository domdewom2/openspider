// Generiert von scripts/gen-ipc.mjs aus src/ipc/commands.json – nicht von Hand ändern (npm run gen:ipc).
import type { ApiRow } from "./gen/ApiRow";
import type { IssueExplainArgs } from "./gen/IssueExplainArgs";
import type { IssueRow } from "./gen/IssueRow";
import type { IssueText } from "./gen/IssueText";
import type { IssuesArgs } from "./gen/IssuesArgs";
import type { OverviewDto } from "./gen/OverviewDto";
import type { ResponseTimes } from "./gen/ResponseTimes";
import type { ResponseTimesArgs } from "./gen/ResponseTimesArgs";
import type { ScopeArgs } from "./gen/ScopeArgs";
import type { SegmentRow } from "./gen/SegmentRow";
import type { SegmentsViewArgs } from "./gen/SegmentsViewArgs";
import type { SiteStructure } from "./gen/SiteStructure";
import type { SiteStructureArgs } from "./gen/SiteStructureArgs";
import { client } from "./client";

export const sidebar = {
  /** `sidebar.overview` */
  overview: (args: ScopeArgs, signal?: AbortSignal) => client.invoke<OverviewDto>("sidebar.overview", args, signal),
  /** `sidebar.issues` */
  issues: (args: IssuesArgs, signal?: AbortSignal) => client.invoke<IssueRow[]>("sidebar.issues", args, signal),
  /** `sidebar.issueExplain` */
  issueExplain: (args: IssueExplainArgs, signal?: AbortSignal) => client.invoke<IssueText>("sidebar.issueExplain", args, signal),
  /** `sidebar.siteStructure` */
  siteStructure: (args: SiteStructureArgs, signal?: AbortSignal) => client.invoke<SiteStructure>("sidebar.siteStructure", args, signal),
  /** `sidebar.responseTimes` */
  responseTimes: (args: ResponseTimesArgs, signal?: AbortSignal) => client.invoke<ResponseTimes>("sidebar.responseTimes", args, signal),
  /** `sidebar.segments` */
  segments: (args: SegmentsViewArgs, signal?: AbortSignal) => client.invoke<SegmentRow[]>("sidebar.segments", args, signal),
  /** `sidebar.apiStatus` */
  apiStatus: (signal?: AbortSignal) => client.invoke<ApiRow[]>("sidebar.apiStatus", undefined, signal),
} as const;
