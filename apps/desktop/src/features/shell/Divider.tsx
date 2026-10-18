import { useRef, useState, type PointerEvent } from "react";
import { GRIP, MIN_DETAIL, MIN_SIDEBAR, MIN_TABLE, TABLE_TABS, type Pane, type Viewport } from "./layout";

export interface DividerProps {
  pane: Pane;
  label: string;
  collapsed: boolean;
  size: number;
  viewport: Viewport;
  onCommit(size: number): void;
  onRestore(): void;
  onReset(): void;
  onPreview(size: number | null): void;
}
/** Pointer capture, keyboard resizing, cancel and double-click all share the same size rules. */
export function Divider({ pane, label, collapsed, size, viewport, onCommit, onRestore, onReset, onPreview }: DividerProps) {
  const drag = useRef<{ id: number; coordinate: number; size: number; next: number; moved: boolean; scale: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const horizontal = pane === "sidebar";
  const total = horizontal ? viewport.width : viewport.height;
  const minTable = MIN_TABLE + (horizontal ? 0 : TABLE_TABS);
  const min = horizontal ? MIN_SIDEBAR : MIN_DETAIL;
  const coordinate = (e: PointerEvent) => horizontal ? e.clientX : e.clientY;
  const end = (cancel: boolean) => {
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    setDragging(false);
    onPreview(null);
    if (cancel) return;
    if (!d.moved && collapsed) onRestore();
    else if (d.moved) onCommit(d.next);
  };
  return <div className={`os-shell-divider os-shell-divider-${pane}`} data-dragging={dragging || undefined}
    role="separator" tabIndex={0} aria-label={label} aria-controls={`shell-${pane}`}
    aria-orientation={horizontal ? "vertical" : "horizontal"} aria-valuemin={0}
    aria-valuemax={Math.max(min, total - GRIP - minTable)} aria-valuenow={Math.round(size)}
    onPointerDown={(e) => {
      if (e.button !== 0) return;
      e.currentTarget.focus();
      e.currentTarget.setPointerCapture(e.pointerId);
      drag.current = { id: e.pointerId, coordinate: coordinate(e), size, next: size, moved: false,
        scale: horizontal ? e.currentTarget.getBoundingClientRect().width / (e.currentTarget.offsetWidth || 1)
          : e.currentTarget.getBoundingClientRect().height / (e.currentTarget.offsetHeight || 1) };
      setDragging(true);
      e.preventDefault();
    }}
    onPointerMove={(e) => {
      const d = drag.current;
      if (!d || d.id !== e.pointerId) return;
      const delta = (d.coordinate - coordinate(e)) / (d.scale || 1);
      if (Math.abs(delta) > 2) d.moved = true;
      if (d.moved) {
        d.next = Math.max(0, Math.min(total - GRIP - minTable, d.size + delta));
        onPreview(d.next);
      }
    }}
    onPointerUp={() => end(false)} onPointerCancel={() => end(true)} onLostPointerCapture={() => end(true)}
    onDoubleClick={onReset}
    onKeyDown={(e) => {
      const grow = horizontal ? "ArrowLeft" : "ArrowUp";
      const shrink = horizontal ? "ArrowRight" : "ArrowDown";
      if (e.key === "Enter" || e.key === " ") { if (collapsed) onRestore(); }
      else if (e.key === "Home") onCommit(0);
      else if (e.key === "End") onCommit(total - GRIP - minTable);
      else if (e.key === grow) { if (collapsed) onRestore(); else onCommit(size + (e.shiftKey ? 40 : 8)); }
      else if (e.key === shrink) onCommit(size - (e.shiftKey ? 40 : 8));
      else if (e.key === "Escape" && drag.current) end(true);
      else return;
      e.preventDefault();
    }} />;
}
