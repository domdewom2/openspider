// UI-102 layout of the Overview tab: tree above, chart area below, a movable divider between. Divider rules of UI-001:
// below half the minimum the area collapses and leaves a 6-px grip; between half and the minimum it snaps back.
import type { ChartLayout } from "../frame/layout";

export const CHART_MIN = 140;
export const CHART_DEFAULT_MIN = 180;
export const CHART_DEFAULT_SHARE = 0.4;
export const COLLAPSE_BELOW = CHART_MIN / 2;
export const GRIP = 6;
/** The tree keeps one header row visible. */
export const TREE_MIN = 32;

export function defaultChartHeight(content: number): number {
  return Math.max(CHART_DEFAULT_MIN, content * CHART_DEFAULT_SHARE);
}

function maxChart(content: number): number {
  return Math.max(CHART_MIN, content - GRIP - TREE_MIN);
}

/** Height of the chart area in px (0 when collapsed); the stored share applies to the current content height. */
export function chartHeight(content: number, layout: ChartLayout): number {
  if (layout.collapsed) return 0;
  const wanted = layout.share == null ? defaultChartHeight(content) : layout.share * content;
  return Math.min(Math.max(wanted, CHART_MIN), maxChart(content));
}

/** Commit on release: `< 70 px` collapses (the last height stays for restoring), `70 … 140 px` snaps to 140 px. */
export function finishDrag(layout: ChartLayout, pixels: number, content: number): Partial<ChartLayout> {
  if (pixels < COLLAPSE_BELOW) return { collapsed: true };
  const height = Math.min(Math.max(pixels, CHART_MIN), maxChart(content));
  return { collapsed: false, share: content > 0 ? height / content : layout.share };
}
