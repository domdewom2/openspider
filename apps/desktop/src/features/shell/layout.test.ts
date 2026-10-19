import { describe, expect, it } from "vitest";
import { defaultLayout, finishDrag, measureLayout, resetDivider } from "./layout";
import { restoreBounds } from "./window";
import legacy from "./__fixtures__/ui-state-v1.json";
import { normalizeUiState } from "../../state/layout";

const viewport = { width: 1440, height: 836 };
const fixture = () => normalizeUiState(legacy, viewport.width, viewport.height).layout;

describe("shell layout in logical pixels", () => {
  // spec: UI-001
  it("uses the exact first-start formulas at 1440 × 900 and only collapses narrow first starts", () => {
    const layout = defaultLayout(1440, 836);
    expect(measureLayout(viewport, layout)).toEqual({ sidebar: 360, detail: 292.59999999999997,
      sidebarGrip: 6, detailGrip: 6, tableWidth: 1074, tableHeight: 537.4000000000001 });
    expect(defaultLayout(1279, 576).sidebar.collapsed).toBe(true);
    expect(defaultLayout(1280, 576).sidebar.collapsed).toBe(false);
    expect(measureLayout({ width: 1024, height: 576 }, defaultLayout(1024, 576)).sidebar).toBe(0);
  });
  // spec: UI-001
  it("resizes the fixture, collapses strictly below half minimum and preserves its last open size", () => {
    let layout = finishDrag(fixture(), "sidebar", 500, viewport);
    layout = finishDrag(layout, "detail", 300, viewport);
    expect(measureLayout(viewport, layout).detail).toBeCloseTo(300);
    for (const [pane, low, half, min] of [["sidebar", 99, 100, 200], ["detail", 50, 60, 120]] as const) {
      const collapsed = finishDrag(layout, pane, low, viewport);
      expect(collapsed[pane]).toEqual({ ...layout[pane], visible: true, collapsed: true });
      expect(measureLayout(viewport, collapsed)[pane]).toBe(0);
      const restored = { ...collapsed, [pane]: { ...collapsed[pane], collapsed: false } };
      expect(measureLayout(viewport, restored)[pane]).toBeCloseTo(pane === "sidebar" ? 500 : 300);
      expect(measureLayout(viewport, finishDrag(layout, pane, half, viewport))[pane]).toBeCloseTo(min);
      expect(measureLayout(viewport, finishDrag(layout, pane, min - 1, viewport))[pane]).toBeCloseTo(min);
    }
  });
  // spec: UI-001
  it("hides without a residual grip and resets exactly one divider at the current window size", () => {
    const layout = fixture();
    const reset = resetDivider(layout, "sidebar", { width: 1600, height: 1000 });
    expect(reset.detail).toEqual(layout.detail);
    expect(reset.sidebar.share).toBe(.25);
    const hidden = { ...layout, sidebar: { ...layout.sidebar, visible: false }, detail: { ...layout.detail, visible: false } };
    expect(measureLayout(viewport, hidden)).toMatchObject({ tableWidth: 1440, tableHeight: 836, sidebarGrip: 0, detailGrip: 0 });
  });
  // spec: UI-001
  it("preserves shares across resize and clamps expanded panes to their minimum dimensions", () => {
    const layout = fixture();
    layout.detail.visible = true;
    const before = structuredClone(layout);
    const small = measureLayout({ width: 1024, height: 576 }, layout);
    expect(small.sidebar).toBeCloseTo(1024 * 500 / 1440);
    expect(small.detail).toBeCloseTo(576 * 300 / 836);
    const huge = { sidebar: { share: 1, visible: true, collapsed: false }, detail: { share: 1, visible: true, collapsed: false } };
    const limited = measureLayout({ width: 1024, height: 576 }, huge);
    expect(limited.tableHeight).toBeGreaterThanOrEqual(200);
    expect(limited.detail).toBeGreaterThanOrEqual(120);
    expect(limited.sidebar).toBeGreaterThanOrEqual(200);
    expect(layout).toEqual(before);
    expect(measureLayout(viewport, layout).sidebar).toBeCloseTo(500);
  });
  // spec: UI-125 [P1]
  it("moves a window from a removed monitor to the primary work area and handles mixed DPI", () => {
    const primary = { x: 0, y: 0, width: 2880, height: 1800, scaleFactor: 2 };
    const secondary = { x: -1920, y: 0, width: 1920, height: 1080, scaleFactor: 1 };
    expect(restoreBounds({ x: 8000, y: 100, width: 1440, height: 900, scaleFactor: 2 }, [primary], primary))
      .toEqual({ x: 0, y: 0, width: 1440, height: 900, scaleFactor: 2 });
    expect(restoreBounds({ x: -1800, y: 20, width: 1200, height: 700, scaleFactor: 1 }, [secondary, primary], primary))
      .toMatchObject({ x: -1800, y: 20, width: 1200, height: 700, scaleFactor: 1 });
  });
});
