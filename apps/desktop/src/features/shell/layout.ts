/** UI-001: logical pixels; persisted sizes are shares of the complete content area. */
export const GRIP = 6;
export const MIN_TABLE = 200;
export const TABLE_TABS = 28;
export const MIN_DETAIL = 120;
export const MIN_SIDEBAR = 200;
export type Pane = "sidebar" | "detail";
export interface PaneState {
  share: number;
  /** Menu hiding gives back the grip as well; dragging to collapse keeps it. */
  visible: boolean;
  collapsed: boolean;
}
export interface Layout {
  sidebar: PaneState;
  detail: PaneState;
}
export interface Viewport { width: number; height: number }
export interface LayoutPixels {
  sidebar: number;
  detail: number;
  sidebarGrip: number;
  detailGrip: number;
  tableWidth: number;
  tableHeight: number;
}
const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), Math.max(min, max));

export function defaultShare(pane: Pane, total: number): number {
  return pane === "sidebar" ? Math.max(260, total * 0.25) / total : Math.max(120, total * 0.35) / total;
}
export function defaultLayout(width: number, contentHeight: number): Layout {
  return {
    sidebar: { share: defaultShare("sidebar", width), visible: true, collapsed: width < 1280 },
    detail: { share: defaultShare("detail", contentHeight), visible: true, collapsed: false },
  };
}

/** Resize never overwrites shares: expanding a window restores the chosen proportions. */
export function measureLayout(viewport: Viewport, layout: Layout): LayoutPixels {
  const sidebarGrip = layout.sidebar.visible ? GRIP : 0;
  const detailGrip = layout.detail.visible ? GRIP : 0;
  const sidebar = !layout.sidebar.visible || layout.sidebar.collapsed ? 0
    : clamp(viewport.width * layout.sidebar.share, MIN_SIDEBAR, viewport.width - sidebarGrip - MIN_TABLE);
  const detail = !layout.detail.visible || layout.detail.collapsed ? 0
    : clamp(viewport.height * layout.detail.share, MIN_DETAIL, viewport.height - detailGrip - MIN_TABLE - TABLE_TABS);
  return { sidebar, detail, sidebarGrip, detailGrip,
    tableWidth: viewport.width - sidebarGrip - sidebar,
    tableHeight: viewport.height - detailGrip - detail };
}

/** Commit on release: strict '< half' collapses; the half/minimum band snaps back. */
export function finishDrag(layout: Layout, pane: Pane, pixels: number, viewport: Viewport): Layout {
  const min = pane === "sidebar" ? MIN_SIDEBAR : MIN_DETAIL;
  const total = pane === "sidebar" ? viewport.width : viewport.height;
  const before = layout[pane];
  const next: PaneState = pixels < min / 2
    ? { ...before, visible: true, collapsed: true }
    : { visible: true, collapsed: false, share: clamp(pixels, min, total - GRIP - MIN_TABLE - (pane === "detail" ? TABLE_TABS : 0)) / total };
  return { ...layout, [pane]: next };
}
export function resetDivider(layout: Layout, pane: Pane, viewport: Viewport): Layout {
  return { ...layout, [pane]: { visible: true, collapsed: false,
    share: defaultShare(pane, pane === "sidebar" ? viewport.width : viewport.height) } };
}
