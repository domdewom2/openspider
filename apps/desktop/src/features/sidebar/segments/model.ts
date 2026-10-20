import type { SegmentRow } from "../../../ipc/gen/SegmentRow";
import type { SiteStructureView } from "../../../ipc/gen/SiteStructureView";
import { pct } from "../site-structure/model";

/** Views of UI-067. `overview` and `issuePriority` read extra keys of the `indexability` aggregate. */
export type SegmentsView = "overview" | "crawlDepth" | "responseCodes" | "indexability" | "issuePriority";
export const SEGMENT_VIEWS: readonly SegmentsView[] = ["overview", "crawlDepth", "responseCodes", "indexability", "issuePriority"];

/** `sidebar.segments` takes the aggregate of `SiteStructureView`; the two table-only views reuse `indexability`. */
export function coreView(view: SegmentsView): SiteStructureView {
  return view === "crawlDepth" || view === "responseCodes" ? view : "indexability";
}

export interface SegmentColumn { key: string; header: string }

const col = (key: string, header = key): SegmentColumn => ({ key, header });
export const OVERVIEW_COLUMNS = [col("Indexable"), col("Non-Indexable"), col("Issues"), col("Warnings"), col("Opportunities"), col("High"), col("Medium"), col("Low")];
export const DEPTH_COLUMNS = [...Array.from({ length: 10 }, (_, i) => col(String(i))), col("10+")];
export const RESPONSE_COLUMNS = ["2xx", "3xx", "4xx", "5xx", "Blocked by robots.txt", "No Response"].map(k => col(k));

export function columnsOf(view: SegmentsView): SegmentColumn[] {
  switch (view) {
    case "overview": return OVERVIEW_COLUMNS;
    case "crawlDepth": return DEPTH_COLUMNS;
    case "responseCodes": return RESPONSE_COLUMNS;
    case "indexability": return OVERVIEW_COLUMNS.slice(0, 2);
    case "issuePriority": return OVERVIEW_COLUMNS.slice(5);
  }
}

/** Value of a class column; depths ≥ 10 collapse into `10+`; a missing key yields `null` (shown as “–”). */
export function classValue(row: SegmentRow, key: string): number | null {
  const entries = Object.entries(row.byClass);
  if (key === "10+") {
    const deep = entries.filter(([k]) => /^\d+$/.test(k) && Number(k) >= 10).map(([, v]) => v);
    const plus = entries.find(([k]) => k === "10+")?.[1];
    return deep.length > 0 || plus != null ? deep.reduce((a, b) => a + b, 0) + (plus ?? 0) : null;
  }
  const hit = entries.find(([k]) => k.toLowerCase() === key.toLowerCase());
  return hit ? hit[1] : null;
}

/** `% Segmented` = URLs of the segment / all internal URLs × 100 (UI-067). */
export function segmented(rows: readonly SegmentRow[]): { total: number; pct: (row: SegmentRow) => number } {
  const total = rows.reduce((s, r) => s + r.urls, 0);
  return { total, pct: row => pct(row.urls, total) };
}

/** “Ohne Segment” always comes last, whatever the order delivered. */
export function orderRows(rows: readonly SegmentRow[]): SegmentRow[] {
  return [...rows.filter(r => r.name != null), ...rows.filter(r => r.name == null)];
}

export const hasSegments = (rows: readonly SegmentRow[]): boolean => rows.some(r => r.name != null);
