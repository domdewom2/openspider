import { useEffect, useLayoutEffect, useRef, useState, type PointerEvent, type ReactNode } from "react";
import { useT } from "../../../i18n";
import { useChartLayout } from "../frame/layout";
import { chartHeight, CHART_MIN, finishDrag, GRIP } from "./pane";

/** Content height of the Overview tab: the sidebar tab panel (UI-102 „Inhaltshöhe des Tabs“). */
function panelOf(el: HTMLElement | null): HTMLElement | null {
  return el?.closest<HTMLElement>('[role="tabpanel"]') ?? el?.parentElement ?? null;
}

/**
 * UI-102 layout: tree above, chart area below, a movable divider between (UI-001 rules: under 70 px collapsed with a
 * 6-px grip, a click on the grip restores the last height, double click restores the default). Height share, collapsed
 * state and view survive a restart (UI-125, `useChartLayout`).
 */
export function ChartPane({ tree, chart }: { tree: ReactNode; chart: ReactNode }) {
  const t = useT();
  const root = useRef<HTMLDivElement>(null);
  const layout = useChartLayout(s => s.layout);
  const update = useChartLayout(s => s.update);
  const [content, setContent] = useState(0);
  const [preview, setPreview] = useState<number | null>(null);
  const drag = useRef<{ id: number; y: number; start: number; next: number; moved: boolean } | null>(null);
  useEffect(() => { void useChartLayout.getState().load(); }, []);
  useLayoutEffect(() => {
    const panel = panelOf(root.current);
    if (!panel) return;
    const measure = () => setContent(panel.clientHeight);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(panel);
    return () => ro.disconnect();
  }, []);
  const height = preview ?? (content > 0 ? chartHeight(content, layout) : layout.collapsed ? 0 : CHART_MIN);
  const end = (cancel: boolean) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    setPreview(null);
    if (cancel) return;
    if (!d.moved) { if (layout.collapsed) update({ collapsed: false }); return; }
    update(finishDrag(layout, d.next, content));
  };
  const step = (delta: number) => update(layout.collapsed ? { collapsed: false } : finishDrag(layout, Math.max(CHART_MIN, height + delta), content));
  return <div ref={root} className="os-ovc-split" data-chart-collapsed={layout.collapsed || undefined}>
    {tree}
    <div className="os-ovc-grip" style={{ height: GRIP }} role="separator" tabIndex={0} aria-label={t("ovc-resize")} aria-orientation="horizontal"
      aria-valuemin={0} aria-valuemax={Math.max(CHART_MIN, content - GRIP)} aria-valuenow={Math.round(height)} data-dragging={preview !== null || undefined}
      onPointerDown={(e: PointerEvent<HTMLDivElement>) => {
        if (e.button !== 0) return;
        e.currentTarget.setPointerCapture?.(e.pointerId);
        drag.current = { id: e.pointerId, y: e.clientY, start: height, next: height, moved: false };
        e.preventDefault();
      }}
      onPointerMove={(e: PointerEvent<HTMLDivElement>) => {
        const d = drag.current;
        if (!d || d.id !== e.pointerId) return;
        const delta = d.y - e.clientY;
        if (Math.abs(delta) > 2) d.moved = true;
        if (d.moved) { d.next = Math.max(0, Math.min(content - GRIP, d.start + delta)); setPreview(d.next); }
      }}
      onPointerUp={() => end(false)} onPointerCancel={() => end(true)} onLostPointerCapture={() => end(false)}
      onDoubleClick={() => update({ collapsed: false, share: null })}
      onKeyDown={e => {
        if (e.key === "Enter" || e.key === " ") { if (layout.collapsed) update({ collapsed: false }); }
        else if (e.key === "ArrowUp") step(e.shiftKey ? 40 : 8);
        else if (e.key === "ArrowDown") { if (!layout.collapsed) update(finishDrag(layout, height - (e.shiftKey ? 40 : 8), content)); }
        else if (e.key === "Home") update({ collapsed: true });
        else if (e.key === "End") update(finishDrag(layout, content, content));
        else return;
        e.preventDefault();
      }} />
    <div className="os-ovc-pane" style={{ height }} hidden={height === 0}>{height > 0 && chart}</div>
  </div>;
}
