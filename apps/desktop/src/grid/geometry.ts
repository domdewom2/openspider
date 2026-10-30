import { gridHeaderHeight } from "./theme";
import type { ColumnState } from "./DataGrid";

// Well below every supported engine's scroll-height limit, including at 200% zoom.
export const MAX_SCROLL_HEIGHT = 1_000_000;
export const clamp = (n: number, max: number) => Math.max(0, Math.min(max, n));
export function rowWindow(total: number, height: number, rowHeight: number) {
  const capacity = Math.max(1, Math.floor(Math.max(0, height - gridHeaderHeight(rowHeight)) / rowHeight));
  return { capacity, maxFirst: Math.max(0, total - capacity) };
}
export function scrollExtent(total: number, height: number, rowHeight: number) {
  return Math.max(height, Math.min(MAX_SCROLL_HEIGHT, total * rowHeight + gridHeaderHeight(rowHeight)));
}
export function rowToScroll(row: number, maxFirst: number, range: number) {
  return maxFirst ? clamp(row, maxFirst) / maxFirst * range : 0;
}
export function scrollToRow(top: number, maxFirst: number, range: number) {
  return range > 0 ? Math.round(clamp(top, range) / range * maxFirst) : 0;
}
export function columnLayout(columns: ColumnState[], left: number, width: number) {
  let offset = 0;
  const pinnedWidth = columns.reduce((n, c) => n + (c.pinned || c.key === "Row" ? c.width : 0), 0);
  return columns.map((c, index) => {
    const pinned = c.pinned || c.key === "Row";
    const x = offset - (pinned ? 0 : left);
    offset += c.width;
    const clipLeft = Math.max(x, pinned ? 0 : pinnedWidth);
    const clipRight = Math.min(x + c.width, width);
    return { ...c, index, x, clipLeft, clipRight };
  }).filter(c => c.clipRight > c.clipLeft);
}
